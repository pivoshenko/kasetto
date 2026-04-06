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

    let scope_label = match scope {
        Scope::Global => "global".to_string(),
        Scope::Project => "project".to_string(),
    };

    let output = DoctorOutput {
        version,
        lock_file: lock_file_path.to_string_lossy().to_string(),
        scope: scope_label,
        skills,
        installation_path,
        last_sync,
        failures,
        mcps: managed_mcps,
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

    Ok(())
}
