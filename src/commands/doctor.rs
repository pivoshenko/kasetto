use std::path::Path;

use crate::colors::{ACCENT, ERROR, RESET, SECONDARY};
use crate::error::Result;
use crate::fsops::{resolve_dest, scope_root};
use crate::lock::{load_lock, lock_path};
use crate::model::{resolve_scope, Scope, SyncFailure};
use crate::profile::{format_updated_ago, list_color_enabled};
use crate::state::load_runtime_state;
use crate::ui::{
    print_check, print_dir_row, print_doctor_head, print_doctor_kv, print_group_header,
    print_json, relativize_home,
};

#[derive(serde::Serialize)]
struct DoctorOutput {
    version: String,
    lock_file: String,
    scope: String,
    skills: Vec<String>,
    installation_path: String,
    last_sync: Option<String>,
    failures: Vec<SyncFailure>,
    mcps: Vec<String>,
    commands: Vec<String>,
    command_dirs: Vec<CommandDirCheck>,
    update_check: UpdateCheckOutput,
}

#[derive(serde::Serialize)]
struct CommandDirCheck {
    path: String,
    writable: bool,
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
        .map(|entry| {
            let p = resolve_dest(&entry.destination, &root);
            p.parent().unwrap_or(&p).to_string_lossy().to_string()
        })
        .collect();
    install_paths.sort();
    install_paths.dedup();
    let installation_path = if install_paths.is_empty() {
        "none".to_string()
    } else if install_paths.len() == 1 {
        install_paths[0].clone()
    } else {
        install_paths.join(", ")
    };

    let mut skills: Vec<String> = state.skills.values().map(|e| e.skill.clone()).collect();
    skills.sort();

    let failures = runtime.load_latest_failures();
    let last_sync = runtime.last_run.clone();

    let managed_mcps = lock.list_installed_mcps();
    let managed_commands = lock.list_installed_commands();
    let command_dirs = collect_command_dirs(scope, &project_root);

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
        installation_path,
        last_sync,
        failures,
        mcps: managed_mcps,
        commands: managed_commands,
        command_dirs,
        update_check,
    };

    if as_json {
        return print_json(&output);
    }

    let color = list_color_enabled() && !plain;
    let update_check_text = format_update_check(&output.update_check);

    print_doctor_head(&output.version, output.failures.is_empty(), !color);

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
        ("Updates", update_check_text.clone()),
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
    ];
    let inv_key_w = inv_rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (k, v) in &inv_rows {
        print_doctor_kv(k, v, inv_key_w, Some(ATTENTION), !color);
    }
    let _ = program_name;

    print_group_header("Checks", color);
    let lock_ok = std::path::Path::new(&output.lock_file).exists()
        || !output.lock_file.is_empty();
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
    let dirs_writable = output
        .command_dirs
        .iter()
        .filter(|d| d.writable)
        .count();
    let dirs_total = output.command_dirs.len();
    let dirs_label = format!(
        "{dirs_writable} of {dirs_total} command directories writable"
    );
    print_check(dirs_writable == dirs_total, &dirs_label, !color);

    if !output.command_dirs.is_empty() {
        print_group_header_with_count("Command directories", output.command_dirs.len(), color);
        for d in &output.command_dirs {
            print_dir_row(&d.path, d.writable, !color);
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
