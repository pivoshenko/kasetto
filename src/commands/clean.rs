use std::fs;

use crate::banner::print_banner_or_plain;
use crate::colors::{ACCENT, ERROR, RESET, SUCCESS, WARNING};
use crate::error::Result;
use crate::fsops::{dirs_home, dirs_kasetto_config, resolve_dest, scope_root};
use crate::lock::{load_lock, save_lock, LockFile};
use crate::mcps::remove_mcp_server;
use crate::model::{
    all_mcp_project_targets, all_mcp_settings_targets, resolve_scope, Scope, State,
};
use crate::profile::list_color_enabled;
use crate::state::clear_runtime_state;
use crate::ui::{animations_enabled, print_json, SYM_OK};

#[derive(serde::Serialize)]
struct CleanOutput {
    skills_removed: usize,
    mcps_removed: usize,
    commands_removed: usize,
    dry_run: bool,
}

pub(crate) fn run(
    dry_run: bool,
    as_json: bool,
    quiet: bool,
    plain: bool,
    scope_override: Option<Scope>,
) -> Result<()> {
    let animate = animations_enabled(quiet, as_json, plain);
    if !as_json && !quiet {
        print_banner_or_plain(plain || !animate);
    }

    let scope = resolve_scope(scope_override, None);
    let project_root = std::env::current_dir().unwrap_or_default();
    let mut lock = load_lock(scope, &project_root)?;

    let state = lock.state();
    let mcp_assets = lock.list_tracked_asset_ids("mcp");
    let command_assets: Vec<(String, String)> = lock
        .list_tracked_asset_ids("command")
        .iter()
        .map(|(id, dest)| (id.to_string(), dest.to_string()))
        .collect();

    let skills_count = state.skills.len();
    let mcps_count = mcp_assets.len();
    let commands_count = command_assets.len();

    if !dry_run {
        apply_removals(&state, &mcp_assets, &command_assets, scope, &project_root)?;
        lock.clear_all();
        save_lock(&mut lock, scope, &project_root)?;
        clear_runtime_state(scope, &project_root)?;
    }

    let output = CleanOutput {
        skills_removed: skills_count,
        mcps_removed: mcps_count,
        commands_removed: commands_count,
        dry_run,
    };

    if as_json {
        print_json(&output)?;
    } else if !quiet {
        print_report(
            &lock,
            &state,
            dry_run,
            plain,
            skills_count + mcps_count + commands_count,
        );
    }

    Ok(())
}

fn apply_removals(
    state: &State,
    mcp_assets: &[(&str, &str)],
    command_assets: &[(String, String)],
    scope: Scope,
    project_root: &std::path::Path,
) -> Result<()> {
    let root = scope_root(scope, project_root)?;
    for entry in state.skills.values() {
        let _ = fs::remove_dir_all(resolve_dest(&entry.destination, &root));
    }

    for (_id, dest_csv) in command_assets {
        for p in dest_csv.split(',').filter(|s| !s.is_empty()) {
            let path = resolve_dest(p, &root);
            if path.exists() && path.is_file() {
                let _ = fs::remove_file(path);
            }
        }
    }

    let mcp_targets = match scope {
        Scope::Project => all_mcp_project_targets(project_root),
        Scope::Global => {
            let home = dirs_home()?;
            let kasetto_config = dirs_kasetto_config()?;
            all_mcp_settings_targets(&home, &kasetto_config)
        }
    };
    for (_id, servers_csv) in mcp_assets {
        for server_name in servers_csv.split(',').filter(|s| !s.is_empty()) {
            for target in &mcp_targets {
                if target.path.exists() {
                    let _ = remove_mcp_server(server_name, target);
                }
            }
        }
    }
    Ok(())
}

fn print_report(lock: &LockFile, state: &State, dry_run: bool, plain: bool, total: usize) {
    let color = list_color_enabled() && !plain;
    let (label_color, prefix) = if dry_run {
        (WARNING, "Would remove")
    } else {
        (ERROR, "Removed")
    };
    println!();
    println!("  {label_color}{prefix}{RESET}: {total}");

    if dry_run {
        print_dry_run_detail(lock, state, color);
        println!();
        println!("Run without {ACCENT}--dry-run{RESET} to apply.");
    } else {
        println!();
        println!("{SUCCESS}{SYM_OK}{RESET} Lock file reset.");
    }
}

fn print_dry_run_detail(lock: &LockFile, state: &State, color: bool) {
    println!();
    if !state.skills.is_empty() {
        println!("  Skills:");
        for entry in state.skills.values() {
            if color {
                println!(
                    "    {ACCENT}skill{RESET}  {}  ({})",
                    entry.destination, entry.skill
                );
            } else {
                println!("    skill  {}  ({})", entry.destination, entry.skill);
            }
        }
    }
    let mcp_packs: Vec<_> = lock
        .assets
        .iter()
        .filter(|(_, a)| a.kind == "mcp")
        .collect();
    if mcp_packs.is_empty() {
        return;
    }
    println!("  MCP packs (server names merged from kasetto):");
    for (_, a) in mcp_packs {
        let servers: String = a
            .destination
            .split(',')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
        if color {
            println!(
                "    {ACCENT}mcp{RESET}    {}  (pack: {}, source: {})",
                servers, a.name, a.source
            );
        } else {
            println!(
                "    mcp    {}  (pack: {}, source: {})",
                servers, a.name, a.source
            );
        }
    }
}
