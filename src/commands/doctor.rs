use std::path::Path;

use crate::banner::print_banner;
use crate::colors::{ACCENT, RESET, SECONDARY};
use crate::error::Result;
use crate::lock::{load_lock, lock_path};
use crate::model::{resolve_scope, Scope, SyncFailure};
use crate::profile::{format_updated_ago, list_color_enabled};
use crate::ui::{animations_enabled, print_field, print_json, print_label};

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

    let version = env!("CARGO_PKG_VERSION").to_string();
    let lock_file_path = lock_path(scope, &project_root)?;

    let state = lock.state();

    let mut install_paths: Vec<String> = state
        .skills
        .values()
        .map(|entry| {
            let p = Path::new(&entry.destination);
            p.parent().unwrap_or(p).to_string_lossy().to_string()
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

    let failures = lock.load_latest_failures();
    let last_sync = state.last_run.clone();

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

    let animate = animations_enabled(false, false, plain);
    let color = list_color_enabled() && !plain;
    if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        if color && animate {
            print_banner();
        } else {
            println!("kasetto | カセット");
        }
        println!();
    }
    let last_sync_text = match &output.last_sync {
        Some(ts) => format!("{} ({})", format_updated_ago(ts), ts),
        None => "none".to_string(),
    };

    print_field("Version", &output.version, color);
    print_field("Lock File", &output.lock_file, color);
    print_field("Scope", &output.scope, color);
    print_field("Installation Path", &output.installation_path, color);
    print_field("Last Sync", &last_sync_text, color);
    print_field(
        "Update Check",
        &format_update_check(&output.update_check),
        color,
    );

    print_label("Failures", color);
    if output.failures.is_empty() {
        println!("  none");
    } else {
        for f in &output.failures {
            if color {
                println!(
                    "  {ACCENT}{}{RESET} {} {SECONDARY}{}{RESET}",
                    f.name, f.reason, f.source
                );
            } else {
                println!("  {} {} {}", f.name, f.reason, f.source);
            }
        }
    }

    print_field(
        "Skills",
        &format!("{} ({program_name} list)", output.skills.len()),
        color,
    );
    print_field(
        "MCP Servers",
        &format!("{} ({program_name} list)", output.mcps.len()),
        color,
    );
    print_field(
        "Commands",
        &format!("{} ({program_name} list)", output.commands.len()),
        color,
    );

    print_label("Command Directories", color);
    if output.command_dirs.is_empty() {
        println!("  none");
    } else {
        for d in &output.command_dirs {
            let state = if d.writable {
                "writable"
            } else {
                "not writable"
            };
            if color {
                println!("  {ACCENT}{}{RESET} {SECONDARY}({state}){RESET}", d.path);
            } else {
                println!("  {} ({state})", d.path);
            }
        }
    }

    Ok(())
}

fn collect_command_dirs(scope: crate::model::Scope, project_root: &Path) -> Vec<CommandDirCheck> {
    let targets = match scope {
        crate::model::Scope::Project => crate::model::all_command_project_targets(project_root),
        crate::model::Scope::Global => match crate::fsops::dirs_home() {
            Ok(home) => crate::model::all_command_global_targets(&home),
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
