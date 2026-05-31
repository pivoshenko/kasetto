use std::fs;
use std::time::{Duration, Instant};

use crate::colors::{ATTENTION, ERROR, RESET, SECONDARY};
use crate::error::Result;
use crate::fsops::{dirs_home, dirs_kasetto_config, resolve_dest, scope_root};
use crate::lock::{load_lock, save_lock, LockFile};
use crate::mcps::remove_mcp_server;
use crate::model::{
    all_mcp_project_targets, all_mcp_settings_targets, resolve_scope, Scope, State,
};
use crate::profile::list_color_enabled;
use crate::state::clear_runtime_state;
use crate::ui::{action_glyph, print_group_header, print_json, print_tip, short_source};

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
    let started = Instant::now();
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
            started.elapsed(),
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

fn print_report(
    lock: &LockFile,
    state: &State,
    dry_run: bool,
    plain: bool,
    total: usize,
    elapsed: Duration,
) {
    let color = list_color_enabled() && !plain;
    let timing = format_elapsed(elapsed);

    if total == 0 {
        if color {
            println!("{SECONDARY}Nothing to clean{RESET} {SECONDARY}(in {timing}){RESET}");
        } else {
            println!("Nothing to clean (in {timing})");
        }
        return;
    }

    let (verb, color_code) = if dry_run {
        ("Would remove", ATTENTION)
    } else {
        ("Removed", ERROR)
    };
    if plain {
        println!("{verb} {total} items in {timing}");
    } else {
        println!("{color_code}\x1b[1m{verb}{RESET} {total} items {SECONDARY}in {timing}{RESET}");
    }

    if dry_run {
        print_dry_run_detail(lock, state, color);
        println!();
        print_tip("run without `--dry-run` to apply", plain);
    } else if color {
        println!("{SECONDARY}Reset lock file{RESET}");
    } else {
        println!("Reset lock file");
    }
}

fn format_elapsed(d: Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.2}s", d.as_secs_f64())
    }
}

fn print_dry_run_detail(lock: &LockFile, state: &State, color: bool) {
    let plain = !color;

    if !state.skills.is_empty() {
        print_group_header("Skills", color);
        let mut entries: Vec<_> = state.skills.values().collect();
        entries.sort_by(|a, b| a.source.cmp(&b.source).then_with(|| a.skill.cmp(&b.skill)));
        let mut last_source = String::new();
        for entry in entries {
            let glyph = action_glyph("would_remove", plain);
            let show_source = entry.source != last_source;
            last_source = entry.source.clone();
            let src_cell = if show_source {
                short_source(&entry.source)
            } else {
                String::new()
            };
            if color {
                if src_cell.is_empty() {
                    println!(" {glyph} \x1b[1m{}{RESET}", entry.skill);
                } else {
                    println!(
                        " {glyph} \x1b[1m{}{RESET}  {SECONDARY}{}{RESET}",
                        entry.skill, src_cell
                    );
                }
            } else if src_cell.is_empty() {
                println!(" - {}", entry.skill);
            } else {
                println!(" - {}  {}", entry.skill, src_cell);
            }
        }
    }

    let mcp_packs: Vec<_> = lock
        .assets
        .iter()
        .filter(|(_, a)| a.kind == "mcp")
        .collect();
    if !mcp_packs.is_empty() {
        print_group_header("MCP Servers", color);
        for (_, a) in mcp_packs {
            let servers: String = a
                .destination
                .split(',')
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(", ");
            let glyph = action_glyph("would_remove", plain);
            if color {
                println!(
                    " {glyph} \x1b[1m{}{RESET}  {SECONDARY}pack: {}, {}{RESET}",
                    servers,
                    a.name,
                    short_source(&a.source)
                );
            } else {
                println!(" - {}  pack: {}, {}", servers, a.name, short_source(&a.source));
            }
        }
    }

    let cmd_assets: Vec<_> = lock
        .assets
        .iter()
        .filter(|(_, a)| a.kind == "command")
        .collect();
    if !cmd_assets.is_empty() {
        print_group_header("Commands", color);
        for (_, a) in cmd_assets {
            let glyph = action_glyph("would_remove", plain);
            if color {
                println!(
                    " {glyph} \x1b[1m{}{RESET}  {SECONDARY}{}{RESET}",
                    a.name,
                    short_source(&a.source)
                );
            } else {
                println!(" - {}  {}", a.name, short_source(&a.source));
            }
        }
    }
}
