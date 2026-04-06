use std::fs;

use crate::banner::print_banner;
use crate::colors::{ACCENT, ERROR, RESET, SUCCESS, WARNING};
use crate::error::Result;
use crate::fsops::{dirs_home, dirs_kasetto_config};
use crate::lock::{load_lock, save_lock};
use crate::mcps::remove_mcp_server;
use crate::model::{all_mcp_project_targets, all_mcp_settings_targets, resolve_scope, Scope};
use crate::profile::list_color_enabled;
use crate::ui::{animations_enabled, print_json, SYM_OK};

#[derive(serde::Serialize)]
struct CleanOutput {
    skills_removed: usize,
    mcps_removed: usize,
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
        if plain || !animate {
            println!("kasetto | カセット");
        } else {
            print_banner();
        }
    }

    let scope = resolve_scope(scope_override, None);
    let project_root = std::env::current_dir().unwrap_or_default();
    let mut lock = load_lock(scope, &project_root)?;

    let state = lock.state();
    let mcp_assets = lock.list_tracked_asset_ids("mcp");

    let skills_count = state.skills.len();
    let mcps_count = mcp_assets.len();

    if !dry_run {
        for entry in state.skills.values() {
            let _ = fs::remove_dir_all(&entry.destination);
        }

        let mcp_targets = match scope {
            Scope::Project => all_mcp_project_targets(&project_root),
            Scope::Global => {
                let home = dirs_home()?;
                let kasetto_config = dirs_kasetto_config()?;
                all_mcp_settings_targets(&home, &kasetto_config)
            }
        };
        for (_id, servers_csv) in &mcp_assets {
            for server_name in servers_csv.split(',').filter(|s| !s.is_empty()) {
                for target in &mcp_targets {
                    if target.path.exists() {
                        let _ = remove_mcp_server(server_name, target);
                    }
                }
            }
        }

        lock.clear_all();
        save_lock(&lock, scope, &project_root)?;
    }

    let output = CleanOutput {
        skills_removed: skills_count,
        mcps_removed: mcps_count,
        dry_run,
    };

    if as_json {
        print_json(&output)?;
    } else if !quiet {
        let color = list_color_enabled() && !plain;
        let (label_color, prefix) = if dry_run {
            (WARNING, "Would remove")
        } else {
            (ERROR, "Removed")
        };
        println!();
        println!(
            "  {label_color}{prefix}{RESET}: {}",
            skills_count + mcps_count
        );

        if dry_run {
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
            if !mcp_packs.is_empty() {
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
        }

        if !dry_run {
            println!();
            println!("{SUCCESS}{SYM_OK}{RESET} Lock file reset.");
        } else {
            println!();
            println!("Run without {ACCENT}--dry-run{RESET} to apply.");
        }
    }

    Ok(())
}
