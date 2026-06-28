use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::colors::{ACCENT, ERROR, RESET, SECONDARY};
use crate::error::Result;
use crate::fsops::{resolve_dest, scope_root};
use crate::lock::{load_lock, lock_path};
use crate::model::{resolve_scope, Scope, SyncFailure};
use crate::profile::{format_updated_ago, list_color_enabled};
use crate::state::load_runtime_state;
use crate::ui::{
    print_check, print_dir_row, print_doctor_head, print_doctor_kv, print_group_header, print_json,
    relativize_home,
};

#[derive(serde::Serialize)]
struct DoctorOutput {
    version: String,
    lock_file: String,
    scope: String,
    skills: Vec<String>,
    /// Locked skills whose recorded destination is absent on disk.
    missing_skills: Vec<String>,
    installation_path: String,
    last_sync: Option<String>,
    failures: Vec<SyncFailure>,
    mcps: Vec<String>,
    commands: Vec<String>,
    instructions: Vec<String>,
    command_dirs: Vec<CommandDirCheck>,
    /// Assets found in managed install paths that the lock does not track.
    /// Read-only/advisory — kasetto never deletes files it does not own.
    unmanaged: Vec<UnmanagedEntry>,
    update_check: UpdateCheckOutput,
}

#[derive(serde::Serialize)]
struct CommandDirCheck {
    path: String,
    writable: bool,
}

/// One asset present in a kasetto-managed install path with no matching lock
/// entry. `kind` is `"skill" | "command" | "instruction" | "instruction-block"
/// | "mcp"`.
#[derive(serde::Serialize, Clone)]
struct UnmanagedEntry {
    kind: String,
    name: String,
    /// Absolute on-disk path (the file, dir, or aggregate file for a block).
    path: String,
}

#[derive(serde::Serialize)]
struct UpdateCheckOutput {
    /// "up_to_date" | "update_available" | "unknown" (no cache yet)
    status: String,
    latest_version: Option<String>,
    checked_at: Option<u64>,
    age_seconds: Option<u64>,
}

pub(crate) fn run(
    as_json: bool,
    plain: bool,
    quiet: bool,
    scope_override: Option<Scope>,
    program_name: &str,
) -> Result<()> {
    if quiet && !as_json {
        return Ok(());
    }

    let scope = resolve_scope(scope_override, None);
    let project_root = std::env::current_dir().unwrap_or_default();
    let lock = load_lock(scope, &project_root)?;
    let runtime = load_runtime_state(scope, &project_root)?;

    let version = env!("CARGO_PKG_VERSION").to_string();
    let lock_file_path = lock_path(scope, &project_root)?;

    let state = lock.state();
    let root = scope_root(scope, &project_root)?;

    let mut install_paths: Vec<String> = state
        .skills
        .values()
        .flat_map(|entry| entry.destination.split(',').filter(|s| !s.is_empty()))
        .map(|d| {
            let p = resolve_dest(d, &root);
            p.parent().unwrap_or(&p).to_string_lossy().to_string()
        })
        .collect();
    install_paths.sort();
    install_paths.dedup();

    // Verify each locked skill destination actually exists on disk. The lock is
    // authoritative for `sync`, so a destination that was never written (or was
    // deleted out-of-band) is invisible unless `doctor` stats the filesystem.
    let mut missing_skills: Vec<String> = state
        .skills
        .values()
        .filter(|entry| {
            entry
                .destination
                .split(',')
                .filter(|s| !s.is_empty())
                .any(|d| !resolve_dest(d, &root).exists())
        })
        .map(|entry| entry.skill.clone())
        .collect();
    missing_skills.sort();
    missing_skills.dedup();
    let installation_path = if install_paths.is_empty() {
        "none".to_string()
    } else if install_paths.len() == 1 {
        install_paths.remove(0)
    } else {
        install_paths.join(", ")
    };

    let mut skills: Vec<String> = state.skills.values().map(|e| e.skill.clone()).collect();
    skills.sort();

    let failures = runtime.load_latest_failures();
    let last_sync = runtime.last_run;

    let managed_mcps = lock.list_installed_mcps();
    let managed_commands = lock.list_installed_commands();
    let managed_instructions = lock.list_installed_instructions();
    let command_dirs = collect_command_dirs(scope, &project_root);
    let unmanaged = collect_unmanaged(scope, &root, &lock);

    let scope_label = match scope {
        Scope::Global => "global".to_string(),
        Scope::Project => "project".to_string(),
    };

    let update_check = build_update_check(&version);

    let output = DoctorOutput {
        version,
        lock_file: lock_file_path.to_string_lossy().to_string(),
        scope: scope_label,
        skills,
        missing_skills,
        installation_path,
        last_sync,
        failures,
        mcps: managed_mcps,
        commands: managed_commands,
        instructions: managed_instructions,
        command_dirs,
        unmanaged,
        update_check,
    };

    if as_json {
        return print_json(&output);
    }

    let color = list_color_enabled() && !plain;
    let update_check_text = format_update_check(&output.update_check);

    let healthy = output.failures.is_empty() && output.missing_skills.is_empty();
    print_doctor_head(&output.version, healthy, !color);

    print_group_header("Environment", color);
    let last_sync_short = match &output.last_sync {
        Some(ts) => format_updated_ago(ts),
        None => "none".to_string(),
    };
    let env_rows: Vec<(&str, String)> = vec![
        ("Scope", output.scope.clone()),
        ("Lock file", relativize_home(&output.lock_file)),
        ("Install path", relativize_home(&output.installation_path)),
        ("Last sync", last_sync_short),
        ("Updates", update_check_text),
    ];
    let env_key_w = env_rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (k, v) in &env_rows {
        print_doctor_kv(k, v, env_key_w, None, !color);
    }

    print_group_header("Inventory", color);
    use crate::colors::ATTENTION;
    let inv_rows: Vec<(&str, String)> = vec![
        ("Skills", output.skills.len().to_string()),
        ("MCP servers", output.mcps.len().to_string()),
        ("Commands", output.commands.len().to_string()),
        ("Instructions", output.instructions.len().to_string()),
    ];
    let inv_key_w = inv_rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (k, v) in &inv_rows {
        print_doctor_kv(k, v, inv_key_w, Some(ATTENTION), !color);
    }
    let _ = program_name;

    print_group_header("Checks", color);
    let lock_ok = std::path::Path::new(&output.lock_file).exists() || !output.lock_file.is_empty();
    print_check(lock_ok, "Lock file readable", !color);
    let install_ok = std::path::Path::new(&output.installation_path).exists()
        || output.installation_path == "none";
    print_check(install_ok, "Install path writable", !color);
    print_check(
        output.failures.is_empty(),
        if output.failures.is_empty() {
            "No failed skills"
        } else {
            "Failed skills present"
        },
        !color,
    );
    let missing_label = if output.missing_skills.is_empty() {
        "All locked skills present on disk".to_string()
    } else {
        format!(
            "{} locked skill(s) missing on disk: {}",
            output.missing_skills.len(),
            output.missing_skills.join(", ")
        )
    };
    print_check(output.missing_skills.is_empty(), &missing_label, !color);
    let n_untracked = output.unmanaged.len();
    let untracked_label = if n_untracked == 0 {
        "No untracked entries in managed paths".to_string()
    } else {
        format!(
            "{n_untracked} untracked entr{} in managed paths",
            if n_untracked == 1 { "y" } else { "ies" }
        )
    };
    print_check(output.unmanaged.is_empty(), &untracked_label, !color);
    let dirs_writable = output.command_dirs.iter().filter(|d| d.writable).count();
    let dirs_total = output.command_dirs.len();
    let dirs_label = format!("{dirs_writable} of {dirs_total} command directories writable");
    print_check(dirs_writable == dirs_total, &dirs_label, !color);

    if !output.command_dirs.is_empty() {
        print_group_header_with_count("Command directories", output.command_dirs.len(), color);
        for d in &output.command_dirs {
            print_dir_row(&d.path, d.writable, !color);
        }
    }

    // Untracked detail (only when present) — advisory; never deleted.
    if !output.unmanaged.is_empty() {
        use crate::colors::ATTENTION;
        print_group_header_with_count("Untracked", output.unmanaged.len(), color);
        for u in &output.unmanaged {
            let tag = short_kind(&u.kind);
            let path = relativize_home(&u.path);
            if color {
                println!(
                    "  {ATTENTION}{ACCENT}!{RESET} {SECONDARY}[{tag}]{RESET} {ACCENT}{}{RESET} {SECONDARY}{path}{RESET}",
                    u.name
                );
            } else {
                println!("  ! [{tag}] {} {path}", u.name);
            }
        }
    }

    // Failures detail (only when present)
    if !output.failures.is_empty() {
        print_group_header("Failures", color);
        for f in &output.failures {
            if color {
                println!(
                    "  {ERROR}{ACCENT}!{RESET} {ACCENT}{}{RESET} {} {SECONDARY}{}{RESET}",
                    f.name, f.reason, f.source
                );
            } else {
                println!("  ! {} {} {}", f.name, f.reason, f.source);
            }
        }
    }

    Ok(())
}

/// `LABEL  N` header — amber uppercase + dim count, blank line above.
fn print_group_header_with_count(title: &str, count: usize, color: bool) {
    println!();
    if color {
        use crate::colors::{ATTENTION, SECONDARY};
        println!(
            "{ACCENT}{ATTENTION}{}{RESET}  {SECONDARY}{count}{RESET}",
            title.to_uppercase()
        );
    } else {
        println!("{}  {count}", title.to_uppercase());
    }
}

fn collect_command_dirs(scope: crate::model::Scope, project_root: &Path) -> Vec<CommandDirCheck> {
    // Scope COMMAND DIRECTORIES to the agents the config actually wires.
    // If no config or no agents configured, fall back to every supported agent
    // (debugging view — "what does kasetto know how to write to?").
    let agents: Vec<crate::model::Agent> =
        match crate::fsops::load_config_any(&crate::default_config_path()) {
            Ok((cfg, _, _)) => cfg.agents(),
            Err(_) => Vec::new(),
        };
    let targets = match scope {
        crate::model::Scope::Project => {
            if agents.is_empty() {
                crate::model::all_command_project_targets(project_root)
            } else {
                crate::model::command_project_targets(project_root, &agents)
            }
        }
        crate::model::Scope::Global => match crate::fsops::dirs_home() {
            Ok(home) => {
                if agents.is_empty() {
                    crate::model::all_command_global_targets(&home)
                } else {
                    crate::model::command_global_targets(&home, &agents)
                }
            }
            Err(_) => return Vec::new(),
        },
    };
    targets
        .into_iter()
        .map(|t| CommandDirCheck {
            writable: is_writable(&t.path),
            path: t.path.to_string_lossy().to_string(),
        })
        .collect()
}

/// Short display tag for an `UnmanagedEntry.kind` in the human-mode tree.
fn short_kind(kind: &str) -> &str {
    match kind {
        "instruction-block" => "block",
        other => other,
    }
}

/// Diff every kasetto-managed install path for the active scope against the lock,
/// returning assets present on disk that the lock does not track. Purely
/// read-only — kasetto never deletes files it does not own, so this only
/// surfaces. Mirrors `collect_command_dirs`' active-vs-all-agents fallback; for
/// both scopes `root` is the right base (`scope_root` → project root or `$HOME`,
/// the same bases the `*_project_*`/`*_global_*` target fns take).
///
/// A config-level custom `destination` overrides only the *skill* install
/// location (it short-circuits `resolve_destinations`), so the skill scan
/// follows that single dir instead of the agent dirs — resolved with the same
/// `root` the locked skill destinations resolve under, so tracked vs untracked
/// stays a clean diff. Commands/MCPs/instructions ignore `destination` and stay
/// agent-based.
fn collect_unmanaged(
    scope: crate::model::Scope,
    root: &Path,
    lock: &crate::lock::LockFile,
) -> Vec<UnmanagedEntry> {
    use crate::model::Scope;
    let cfg = crate::fsops::load_config_any(&crate::default_config_path())
        .ok()
        .map(|(cfg, _, _)| cfg);
    let agents: Vec<crate::model::Agent> = cfg.as_ref().map(|c| c.agents()).unwrap_or_default();
    let custom_dest = cfg.as_ref().and_then(|c| c.destination.clone());
    let all = agents.is_empty();

    let (skill_dirs, command_targets, instr_targets, mcp_targets) = match scope {
        Scope::Project => (
            if all {
                crate::model::all_skill_project_targets(root)
            } else {
                crate::model::skill_project_targets(root, &agents)
            },
            if all {
                crate::model::all_command_project_targets(root)
            } else {
                crate::model::command_project_targets(root, &agents)
            },
            if all {
                crate::model::all_instruction_project_targets(root)
            } else {
                crate::model::instruction_project_targets(root, &agents)
            },
            if all {
                crate::model::all_mcp_project_targets(root)
            } else {
                crate::model::mcp_settings_project_targets(root, &agents)
            },
        ),
        Scope::Global => (
            if all {
                crate::model::all_skill_global_targets(root)
            } else {
                crate::model::skill_global_targets(root, &agents)
            },
            if all {
                crate::model::all_command_global_targets(root)
            } else {
                crate::model::command_global_targets(root, &agents)
            },
            if all {
                crate::model::all_instruction_global_targets(root)
            } else {
                crate::model::instruction_global_targets(root, &agents)
            },
            if all {
                crate::model::all_mcp_settings_targets(root, Path::new(""))
            } else {
                crate::model::mcp_settings_targets(root, &agents)
            },
        ),
    };

    // A custom `destination` redirects only skills (see `resolve_destinations`),
    // resolved against the same `root` the locked skill paths use.
    let skill_dirs = match &custom_dest {
        Some(dest) => vec![crate::fsops::resolve_path(root, dest)],
        None => skill_dirs,
    };

    let command_dirs: Vec<PathBuf> = command_targets.into_iter().map(|t| t.path).collect();
    let dir_instr: Vec<PathBuf> = instr_targets
        .iter()
        .filter(|t| !t.format.is_aggregate())
        .map(|t| t.path.clone())
        .collect();
    let agg_instr: Vec<PathBuf> = instr_targets
        .iter()
        .filter(|t| t.format.is_aggregate())
        .map(|t| t.path.clone())
        .collect();

    // Locked destination sets, built under the same `root` as the scan so plain
    // PathBuf equality holds (no canonicalize — it would fail on missing paths
    // and resolve symlinks the scan side does not).
    let mut locked_skill_dirs: HashSet<PathBuf> = HashSet::new();
    for e in lock.skills.values() {
        for d in e.destination.split(',').filter(|s| !s.is_empty()) {
            locked_skill_dirs.insert(resolve_dest(d, root));
        }
    }
    let mut locked_command_files: HashSet<PathBuf> = HashSet::new();
    let mut locked_instr_files: HashSet<PathBuf> = HashSet::new();
    let mut locked_agg_owned: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    for a in lock.assets.values() {
        match a.kind.as_str() {
            "command" => {
                for d in a.destination.split(',').filter(|s| !s.is_empty()) {
                    locked_command_files.insert(resolve_dest(d, root));
                }
            }
            "instructions" => {
                for token in a.destination.split(',').filter(|s| !s.is_empty()) {
                    if let Some(rel) = token.strip_prefix("file:") {
                        locked_instr_files.insert(resolve_dest(rel, root));
                    } else if let Some(rel) = token.strip_prefix("agg:") {
                        let path = resolve_dest(rel, root);
                        locked_agg_owned
                            .entry(path)
                            .or_default()
                            .insert(crate::instructions::block_id(&a.source, &a.name));
                    }
                }
            }
            _ => {}
        }
    }
    let locked_mcp_names: HashSet<String> = lock.list_installed_mcps().into_iter().collect();

    let mut out = Vec::new();
    out.extend(find_unmanaged_skills(&skill_dirs, &locked_skill_dirs));
    out.extend(find_unmanaged_files(
        &command_dirs,
        &locked_command_files,
        "command",
    ));
    out.extend(find_unmanaged_files(
        &dir_instr,
        &locked_instr_files,
        "instruction",
    ));
    out.extend(find_orphan_blocks(&agg_instr, &locked_agg_owned));
    out.extend(find_unmanaged_mcp_servers(&mcp_targets, &locked_mcp_names));
    out.sort_by(|a, b| {
        (a.kind.as_str(), a.path.as_str(), a.name.as_str()).cmp(&(
            b.kind.as_str(),
            b.path.as_str(),
            b.name.as_str(),
        ))
    });
    out
}

/// Base file/dir name of `p` as a display string.
fn base_name(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Files under `dir`, recursing into subdirectories (commands and rules can be
/// namespaced into nested folders). Read-only; missing/unreadable dirs yield none.
fn list_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.is_file() {
                out.push(p);
            }
        }
    }
    out
}

/// Skill dirs (a child holding `SKILL.md`) present in a managed path but absent
/// from the lock.
fn find_unmanaged_skills(scan_dirs: &[PathBuf], locked: &HashSet<PathBuf>) -> Vec<UnmanagedEntry> {
    let mut out = Vec::new();
    for dir in scan_dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("SKILL.md").is_file() && !locked.contains(&path) {
                out.push(UnmanagedEntry {
                    kind: "skill".into(),
                    name: base_name(&path),
                    path: path.to_string_lossy().into(),
                });
            }
        }
    }
    out
}

/// Files in a managed dir (commands or per-instruction rules) absent from the lock.
fn find_unmanaged_files(
    scan_dirs: &[PathBuf],
    locked: &HashSet<PathBuf>,
    kind: &str,
) -> Vec<UnmanagedEntry> {
    let mut out = Vec::new();
    for dir in scan_dirs {
        for path in list_files(dir) {
            if !locked.contains(&path) {
                out.push(UnmanagedEntry {
                    kind: kind.into(),
                    name: base_name(&path),
                    path: path.to_string_lossy().into(),
                });
            }
        }
    }
    out
}

/// Managed `<!-- kasetto:instruction:ID … -->` blocks in shared aggregate files
/// whose id the lock no longer tracks (a stale block kasetto left behind). User
/// prose is never inspected — only kasetto's own markers.
fn find_orphan_blocks(
    agg_files: &[PathBuf],
    owned: &HashMap<PathBuf, HashSet<String>>,
) -> Vec<UnmanagedEntry> {
    let mut out = Vec::new();
    let mut seen_files: HashSet<PathBuf> = HashSet::new();
    let empty = HashSet::new();
    for path in agg_files {
        if !seen_files.insert(path.clone()) || !path.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let owned_ids = owned.get(path).unwrap_or(&empty);
        let mut seen_ids: HashSet<String> = HashSet::new();
        for id in crate::instructions::scan_managed_block_ids(&text) {
            if !owned_ids.contains(&id) && seen_ids.insert(id.clone()) {
                out.push(UnmanagedEntry {
                    kind: "instruction-block".into(),
                    name: id,
                    path: path.to_string_lossy().into(),
                });
            }
        }
    }
    out
}

/// MCP servers present in a managed settings file that the lock does not track.
/// kasetto writes no ownership marker into settings files and deliberately
/// preserves user servers, so this necessarily also lists the user's own
/// hand-added servers — it is an inventory, not an orphan report.
fn find_unmanaged_mcp_servers(
    targets: &[crate::model::McpSettingsTarget],
    locked: &HashSet<String>,
) -> Vec<UnmanagedEntry> {
    let mut out = Vec::new();
    for t in targets {
        if !t.path.is_file() {
            continue;
        }
        for name in crate::mcps::list_server_names(t) {
            if !locked.contains(&name) {
                out.push(UnmanagedEntry {
                    kind: "mcp".into(),
                    name,
                    path: t.path.to_string_lossy().into(),
                });
            }
        }
    }
    out
}

fn is_writable(path: &Path) -> bool {
    // Walk up to the first ancestor that exists, then probe write permissions there.
    let mut probe = path.to_path_buf();
    loop {
        if probe.exists() {
            break;
        }
        let Some(parent) = probe.parent().map(Path::to_path_buf) else {
            return false;
        };
        if parent == probe {
            return false;
        }
        probe = parent;
    }
    match std::fs::metadata(&probe) {
        Ok(meta) => !meta.permissions().readonly(),
        Err(_) => false,
    }
}

fn build_update_check(current_version: &str) -> UpdateCheckOutput {
    let Some(entry) = crate::update_notifier::read_cached_entry() else {
        return UpdateCheckOutput {
            status: "unknown".to_string(),
            latest_version: None,
            checked_at: None,
            age_seconds: None,
        };
    };
    let age = crate::update_notifier::now_unix_secs().saturating_sub(entry.checked_at);
    let status = if crate::commands::self_update::is_newer(current_version, &entry.latest_version) {
        "update_available"
    } else {
        "up_to_date"
    };
    UpdateCheckOutput {
        status: status.to_string(),
        latest_version: Some(entry.latest_version),
        checked_at: Some(entry.checked_at),
        age_seconds: Some(age),
    }
}

fn format_update_check(uc: &UpdateCheckOutput) -> String {
    let age_label = uc.age_seconds.map(format_age).unwrap_or_default();
    match uc.status.as_str() {
        "update_available" => format!(
            "{} available (checked {age_label})",
            uc.latest_version.as_deref().unwrap_or("?"),
        ),
        "up_to_date" => format!("up-to-date (checked {age_label})"),
        _ => "not yet checked".to_string(),
    }
}

fn format_age(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86_400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsops::temp_dir;
    use crate::model::{McpSettingsFormat, McpSettingsTarget};
    use std::fs;

    fn write_skill(root: &Path, name: &str) -> PathBuf {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), "# skill\n").unwrap();
        dir
    }

    #[test]
    fn skills_reports_only_unlocked_dirs() {
        let scan = temp_dir("kasetto-doctor-skills");
        fs::create_dir_all(&scan).unwrap();
        let kept = write_skill(&scan, "kept");
        let stray = write_skill(&scan, "stray");
        // A directory without SKILL.md and a loose file must be ignored.
        fs::create_dir_all(scan.join("not-a-skill")).unwrap();
        fs::write(scan.join("loose.md"), "x").unwrap();

        let locked: HashSet<PathBuf> = [kept].into_iter().collect();
        let found = find_unmanaged_skills(std::slice::from_ref(&scan), &locked);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, "skill");
        assert_eq!(found[0].name, "stray");
        assert_eq!(found[0].path, stray.to_string_lossy());

        let _ = fs::remove_dir_all(&scan);
    }

    #[test]
    fn files_reports_unlocked_including_nested() {
        let scan = temp_dir("kasetto-doctor-cmds");
        fs::create_dir_all(scan.join("git")).unwrap();
        let tracked = scan.join("tracked.md");
        fs::write(&tracked, "x").unwrap();
        fs::write(scan.join("stray.md"), "x").unwrap();
        fs::write(scan.join("git/commit.md"), "x").unwrap(); // namespaced

        let locked: HashSet<PathBuf> = [tracked].into_iter().collect();
        let mut found = find_unmanaged_files(std::slice::from_ref(&scan), &locked, "command");
        found.sort_by(|a, b| a.name.cmp(&b.name));

        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "commit.md");
        assert_eq!(found[0].kind, "command");
        assert_eq!(found[1].name, "stray.md");

        let _ = fs::remove_dir_all(&scan);
    }

    #[test]
    fn orphan_blocks_reports_unowned_markers_only() {
        let proj = temp_dir("kasetto-doctor-agg");
        fs::create_dir_all(&proj).unwrap();
        let claude = proj.join("CLAUDE.md");
        let owned_id = crate::instructions::block_id("https://x/a", "style");
        let orphan_id = crate::instructions::block_id("https://x/gone", "ghost");
        let text = format!(
            "# CLAUDE.md\n\nMy own notes (must be ignored).\n\n\
             <!-- kasetto:instruction:{owned_id} START -->\nkept\n<!-- kasetto:instruction:{owned_id} END -->\n\n\
             <!-- kasetto:instruction:{orphan_id} START -->\nstale\n<!-- kasetto:instruction:{orphan_id} END -->\n",
        );
        fs::write(&claude, text).unwrap();

        let mut owned: HashMap<PathBuf, HashSet<String>> = HashMap::new();
        owned.insert(claude.clone(), [owned_id].into_iter().collect());

        let found = find_orphan_blocks(std::slice::from_ref(&claude), &owned);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, "instruction-block");
        assert_eq!(found[0].name, orphan_id);
        assert_eq!(found[0].path, claude.to_string_lossy());

        let _ = fs::remove_dir_all(&proj);
    }

    #[test]
    fn orphan_blocks_dedupes_shared_aggregate_file() {
        // Two agents (e.g. Codex + OpenCode) point at the same AGENTS.md — the
        // file appears twice in the target list but each orphan is reported once.
        let proj = temp_dir("kasetto-doctor-agg-dup");
        fs::create_dir_all(&proj).unwrap();
        let agents = proj.join("AGENTS.md");
        let orphan_id = crate::instructions::block_id("https://x/gone", "ghost");
        fs::write(
            &agents,
            format!(
                "<!-- kasetto:instruction:{orphan_id} START -->\nstale\n<!-- kasetto:instruction:{orphan_id} END -->\n"
            ),
        )
        .unwrap();

        let owned: HashMap<PathBuf, HashSet<String>> = HashMap::new();
        let found = find_orphan_blocks(&[agents.clone(), agents.clone()], &owned);
        assert_eq!(found.len(), 1);

        let _ = fs::remove_dir_all(&proj);
    }

    #[test]
    fn mcp_reports_servers_absent_from_lock() {
        let dir = temp_dir("kasetto-doctor-mcp");
        fs::create_dir_all(&dir).unwrap();
        let settings = dir.join("mcp.json");
        fs::write(
            &settings,
            r#"{"mcpServers":{"tracked":{"command":"x"},"user-own":{"command":"y"}}}"#,
        )
        .unwrap();
        let target = McpSettingsTarget {
            path: settings.clone(),
            format: McpSettingsFormat::McpServers,
        };
        let locked: HashSet<String> = ["tracked".to_string()].into_iter().collect();

        let found = find_unmanaged_mcp_servers(&[target], &locked);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, "mcp");
        assert_eq!(found[0].name, "user-own");
        assert_eq!(found[0].path, settings.to_string_lossy());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn mcp_missing_settings_file_is_noop() {
        let target = McpSettingsTarget {
            path: PathBuf::from("/nonexistent/mcp.json"),
            format: McpSettingsFormat::McpServers,
        };
        assert!(find_unmanaged_mcp_servers(&[target], &HashSet::new()).is_empty());
    }
}
