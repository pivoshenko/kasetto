use std::fs;
use std::time::{Duration, Instant};

use crate::colors::{ACCENT, ATTENTION, ERROR, RESET, SECONDARY};
use crate::error::Result;
use crate::fsops::{dirs_home, dirs_kasetto_config, resolve_dest, scope_root};
use crate::lock::{load_lock, save_lock, LockFile};
use crate::mcps::remove_mcp_server;
use crate::model::{
    all_mcp_project_targets, all_mcp_settings_targets, resolve_scope, Scope, State,
};
use crate::profile::list_color_enabled;
use crate::state::clear_runtime_state;
use crate::ui::{
    action_glyph, print_json, print_section_header, print_source_header, print_tip,
    print_tree_leaf, short_source, status_tail,
};

#[derive(serde::Serialize)]
struct CleanOutput {
    skills_removed: usize,
    mcps_removed: usize,
    commands_removed: usize,
    instructions_removed: usize,
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
    let command_assets = lock.list_tracked_asset_ids("command");
    // Instructions need source + name (to recompute the managed-block id at teardown),
    // so snapshot them directly rather than via the (id, dest) list helper.
    let instruction_meta: Vec<(String, String, String)> = lock
        .assets
        .iter()
        .filter(|(_, a)| a.kind == "instructions")
        .map(|(_, a)| (a.source.clone(), a.name.clone(), a.destination.clone()))
        .collect();

    let skills_count = state.skills.len();
    let mcps_count = mcp_assets.len();
    let commands_count = command_assets.len();
    let instructions_count = instruction_meta.len();

    if !dry_run
        && !as_json
        && !quiet
        && (skills_count + mcps_count + commands_count + instructions_count) > 0
    {
        let color = list_color_enabled() && !plain;
        if color {
            println!(
                "{ATTENTION}⚠{RESET} Removing {skills_count} skills, {mcps_count} MCP servers, {commands_count} commands, and {instructions_count} instructions."
            );
        } else {
            println!(
                "Removing {skills_count} skills, {mcps_count} MCP servers, {commands_count} commands, and {instructions_count} instructions."
            );
        }
    }

    if !dry_run {
        apply_removals(
            &state,
            &mcp_assets,
            &command_assets,
            &instruction_meta,
            scope,
            &project_root,
        )?;
        lock.clear_all();
        save_lock(&mut lock, scope, &project_root)?;
        clear_runtime_state(scope, &project_root)?;
    }

    let output = CleanOutput {
        skills_removed: skills_count,
        mcps_removed: mcps_count,
        commands_removed: commands_count,
        instructions_removed: instructions_count,
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
            skills_count + mcps_count + commands_count + instructions_count,
            started.elapsed(),
        );
    }

    Ok(())
}

fn apply_removals(
    state: &State,
    mcp_assets: &[(&str, &str)],
    command_assets: &[(&str, &str)],
    instruction_assets: &[(String, String, String)],
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

    // Instructions: strip the managed block from shared aggregate files (never deleting
    // the user-owned file) or delete a standalone per-instruction file.
    for (source, name, dest_csv) in instruction_assets {
        for token in dest_csv.split(',').filter(|s| !s.is_empty()) {
            crate::instructions::teardown_dest(token, source, name, &root);
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

    print_removal_tree(lock, state, dry_run, !color);

    if dry_run {
        if color {
            println!(
                "{ATTENTION}{ACCENT}Would remove{RESET} {total} items {SECONDARY}in {timing}{RESET}"
            );
        } else {
            println!("Would remove {total} items in {timing}");
        }
        println!();
        print_tip("run without `--dry-run` to apply", plain);
    } else {
        if color {
            println!("{ERROR}{ACCENT}Removed{RESET} {total} items {SECONDARY}in {timing}{RESET}");
            println!(
                "  {SECONDARY}lock file reset · run{RESET} {ATTENTION}kasetto sync{RESET} {SECONDARY}to restore{RESET}"
            );
        } else {
            println!("Removed {total} items in {timing}");
            println!("  lock file reset · run kasetto sync to restore");
        }
    }
}

/// Print a source-grouped red teardown tree for skills, MCP packs, and
/// commands captured in the lock state. Used by both `--dry-run` (with
/// `would_remove` glyphs) and the real run (with `removed` glyphs).
fn print_removal_tree(lock: &LockFile, state: &State, dry_run: bool, plain: bool) {
    let status = if dry_run { "would_remove" } else { "removed" };

    if !state.skills.is_empty() {
        let mut by_source: Vec<(String, Vec<(&str, &str)>)> = Vec::new();
        let mut entries: Vec<_> = state.skills.values().collect();
        entries.sort_by(|a, b| a.source.cmp(&b.source).then_with(|| a.skill.cmp(&b.skill)));
        for e in entries {
            let key = e.source.clone();
            if let Some(g) = by_source.iter_mut().find(|(k, _)| k == &key) {
                g.1.push((e.skill.as_str(), e.skill.as_str()));
            } else {
                by_source.push((key, vec![(e.skill.as_str(), e.skill.as_str())]));
            }
        }
        print_section_header("Skills", Some((state.skills.len(), "to remove")), plain);
        for (source, items) in &by_source {
            let repo = short_source(source);
            print_source_header(&repo, None, Some(true), None, plain);
            for (i, (name, _)) in items.iter().enumerate() {
                let is_last = i == items.len() - 1;
                let glyph = action_glyph(status, plain);
                let tail = status_tail(status, None, None, plain);
                print_tree_leaf(is_last, Some(&glyph), name, true, &tail, 30, plain);
            }
        }
    }

    let mcp_packs: Vec<_> = lock
        .assets
        .iter()
        .filter(|(_, a)| a.kind == "mcp")
        .collect();
    if !mcp_packs.is_empty() {
        let total_servers: usize = mcp_packs
            .iter()
            .map(|(_, a)| a.destination.split(',').filter(|s| !s.is_empty()).count())
            .sum();
        print_section_header("Mcp Servers", Some((total_servers, "to remove")), plain);
        let mut by_source: Vec<(String, Vec<&str>)> = Vec::new();
        for (_, a) in &mcp_packs {
            for server in a.destination.split(',').filter(|s| !s.is_empty()) {
                let key = a.source.clone();
                if let Some(g) = by_source.iter_mut().find(|(k, _)| k == &key) {
                    g.1.push(server);
                } else {
                    by_source.push((key, vec![server]));
                }
            }
        }
        for (source, servers) in &by_source {
            let repo = short_source(source);
            print_source_header(&repo, None, Some(true), None, plain);
            for (i, name) in servers.iter().enumerate() {
                let is_last = i == servers.len() - 1;
                let glyph = action_glyph(status, plain);
                let tail = status_tail(status, None, None, plain);
                print_tree_leaf(is_last, Some(&glyph), name, true, &tail, 30, plain);
            }
        }
    }

    let cmd_assets: Vec<_> = lock
        .assets
        .iter()
        .filter(|(_, a)| a.kind == "command")
        .collect();
    if !cmd_assets.is_empty() {
        print_section_header("Commands", Some((cmd_assets.len(), "to remove")), plain);
        let mut by_source: Vec<(String, Vec<&str>)> = Vec::new();
        for (_, a) in &cmd_assets {
            let key = a.source.clone();
            if let Some(g) = by_source.iter_mut().find(|(k, _)| k == &key) {
                g.1.push(a.name.as_str());
            } else {
                by_source.push((key, vec![a.name.as_str()]));
            }
        }
        for (source, items) in &by_source {
            let repo = short_source(source);
            print_source_header(&repo, None, Some(true), None, plain);
            for (i, name) in items.iter().enumerate() {
                let is_last = i == items.len() - 1;
                let glyph = action_glyph(status, plain);
                let tail = status_tail(status, None, None, plain);
                print_tree_leaf(is_last, Some(&glyph), name, true, &tail, 30, plain);
            }
        }
    }

    let instruction_assets: Vec<_> = lock
        .assets
        .iter()
        .filter(|(_, a)| a.kind == "instructions")
        .collect();
    if !instruction_assets.is_empty() {
        print_section_header(
            "Instructions",
            Some((instruction_assets.len(), "to remove")),
            plain,
        );
        let mut by_source: Vec<(String, Vec<&str>)> = Vec::new();
        for (_, a) in &instruction_assets {
            let key = a.source.clone();
            if let Some(g) = by_source.iter_mut().find(|(k, _)| k == &key) {
                g.1.push(a.name.as_str());
            } else {
                by_source.push((key, vec![a.name.as_str()]));
            }
        }
        for (source, items) in &by_source {
            let repo = short_source(source);
            print_source_header(&repo, None, Some(true), None, plain);
            for (i, name) in items.iter().enumerate() {
                let is_last = i == items.len() - 1;
                let glyph = action_glyph(status, plain);
                let tail = status_tail(status, None, None, plain);
                print_tree_leaf(is_last, Some(&glyph), name, true, &tail, 30, plain);
            }
        }
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
