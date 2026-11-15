//! Module that contains `kasetto clean`, which removes every installed asset for
//! the scope and resets the lock.

use std::collections::BTreeMap;
use std::fs;
use std::time::{Duration, Instant};

use crate::audit::clear_cache as clear_audit_cache;
use crate::colors::{ACCENT, ATTENTION, ERROR, RESET, SECONDARY};
use crate::error::Result;
use crate::fsops::{dirs_home, dirs_kasetto_config, resolve_dest, scope_root};
use crate::lock::{load_lock, save_lock, AssetEntry};
use crate::mcps::remove_mcp_server;
use crate::model::{
    all_mcp_project_targets, all_mcp_settings_targets, resolve_scope, Scope, State,
};
use crate::profile::list_color_enabled;
use crate::state::clear_runtime_state;
use crate::ui::{
    pluralize, print_json, print_section_header, print_source_header, print_status_leaf, print_tip,
    short_source,
};

#[derive(serde::Serialize)]
struct CleanOutput {
    skills_removed: usize,
    mcps_removed: usize,
    commands_removed: usize,
    agents_removed: usize,
    plugins_removed: usize,
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
    let agent_assets = lock.list_tracked_asset_ids("agent");
    // Instructions need source + name (to recompute the managed-block id at teardown),
    // so snapshot them directly rather than via the (id, dest) list helper
    let instruction_meta: Vec<(String, String, String)> = lock
        .assets
        .iter()
        .filter(|(_, a)| a.kind == "instructions")
        .map(|(_, a)| (a.source.clone(), a.name.clone(), a.destination.clone()))
        .collect();
    // Plugins before marketplaces, so a plugin is never left enabled against a
    // marketplace that is already gone
    let plugin_assets: Vec<PluginAsset> = ["plugin", "plugin-marketplace"]
        .iter()
        .flat_map(|kind| {
            lock.assets
                .values()
                .filter(move |a| a.kind == *kind)
                .map(|a| PluginAsset {
                    marketplace: *kind == "plugin-marketplace",
                    name: a.name.clone(),
                    hash: a.hash.clone(),
                    destination: a.destination.clone(),
                })
        })
        .collect();

    let skills_count = state.skills.len();
    let mcps_count = mcp_assets.len();
    let commands_count = command_assets.len();
    let agents_count = agent_assets.len();
    let plugins_count = plugin_assets.iter().filter(|p| !p.marketplace).count();
    let instructions_count = instruction_meta.len();

    // `clear_all` empties the lock before the tree renders it
    let tracked_assets = lock.assets.clone();

    let total = skills_count
        + mcps_count
        + commands_count
        + agents_count
        + plugins_count
        + instructions_count;
    // A real clean announces what it is about to remove; the tree that follows
    // must then separate itself from that line
    let preamble = !dry_run && !as_json && !quiet && total > 0;
    if preamble {
        let color = list_color_enabled() && !plain;
        if color {
            println!(
                "{ATTENTION}⚠{RESET} Removing {skills_count} skills, {mcps_count} MCP servers, {commands_count} commands, {agents_count} agents, {plugins_count} plugins, and {instructions_count} instructions."
            );
        } else {
            println!(
                "Removing {skills_count} skills, {mcps_count} MCP servers, {commands_count} commands, {agents_count} agents, {plugins_count} plugins, and {instructions_count} instructions."
            );
        }
    }

    if !dry_run {
        apply_removals(
            &state,
            &mcp_assets,
            &command_assets,
            &agent_assets,
            &plugin_assets,
            &instruction_meta,
            scope,
            &project_root,
        )?;
        lock.clear_all();
        save_lock(&mut lock, scope, &project_root)?;
        clear_runtime_state(scope, &project_root)?;
        // Cached audit verdicts describe assets that no longer exist here;
        // a later sync starts from fresh lookups
        clear_audit_cache()?;
    }

    let output = CleanOutput {
        skills_removed: skills_count,
        mcps_removed: mcps_count,
        commands_removed: commands_count,
        agents_removed: agents_count,
        plugins_removed: plugins_count,
        instructions_removed: instructions_count,
        dry_run,
    };

    if as_json {
        print_json(&output)?;
    } else if !quiet {
        print_report(
            &tracked_assets,
            &state,
            dry_run,
            plain,
            total,
            started.elapsed(),
            preamble,
        );
    }

    Ok(())
}

/// A `plugin` or `plugin-marketplace` lock entry, snapshotted before the lock is cleared.
struct PluginAsset {
    marketplace: bool,
    name: String,
    hash: String,
    destination: String,
}

#[allow(clippy::too_many_arguments)]
fn apply_removals(
    state: &State,
    mcp_assets: &[(&str, &str)],
    command_assets: &[(&str, &str)],
    agent_assets: &[(&str, &str)],
    plugin_assets: &[PluginAsset],
    instruction_assets: &[(String, String, String)],
    scope: Scope,
    project_root: &std::path::Path,
) -> Result<()> {
    let root = scope_root(scope, project_root)?;
    for entry in state.skills.values() {
        for p in entry.destination.split(',').filter(|s| !s.is_empty()) {
            let _ = fs::remove_dir_all(resolve_dest(p, &root));
        }
    }

    for (_id, dest_csv) in command_assets {
        for p in dest_csv.split(',').filter(|s| !s.is_empty()) {
            let path = resolve_dest(p, &root);
            if path.exists() && path.is_file() {
                let _ = fs::remove_file(path);
            }
        }
    }

    for (_id, dest_csv) in agent_assets {
        for p in dest_csv.split(',').filter(|s| !s.is_empty()) {
            let path = resolve_dest(p, &root);
            if path.exists() && path.is_file() {
                let _ = fs::remove_file(path);
            }
        }
    }

    // Only the keys Kasetto wrote come out of each settings file. Claude Code keeps its
    // own clone and cache under ~/.claude/plugins/, which clean leaves alone
    for plugin in plugin_assets {
        for p in plugin.destination.split(',').filter(|s| !s.is_empty()) {
            let path = resolve_dest(p, &root);
            let _ = if plugin.marketplace {
                crate::plugins::remove_marketplace(&path, &plugin.name, &plugin.hash).map(|_| ())
            } else {
                crate::plugins::remove_plugin(&path, &plugin.name)
            };
        }
    }

    // Instructions: strip the managed block from shared aggregate files (never deleting
    // the user-owned file) or delete a standalone per-instruction file
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
    assets: &BTreeMap<String, AssetEntry>,
    state: &State,
    dry_run: bool,
    plain: bool,
    total: usize,
    elapsed: Duration,
    preamble: bool,
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

    print_removal_tree(assets, state, dry_run, !color, preamble);

    let items = pluralize(total, "item", "items");
    if dry_run {
        if color {
            println!(
                "{ATTENTION}{ACCENT}Would remove{RESET} {total} {items} {SECONDARY}in {timing}{RESET}"
            );
        } else {
            println!("Would remove {total} {items} in {timing}");
        }
        print_tip("run without `--dry-run` to apply", !color);
    } else {
        if color {
            println!("{ERROR}{ACCENT}Removed{RESET} {total} {items} {SECONDARY}in {timing}{RESET}");
            println!(
                "  {SECONDARY}lock file reset · run{RESET} {ATTENTION}kst sync{RESET} {SECONDARY}to restore{RESET}"
            );
        } else {
            println!("Removed {total} {items} in {timing}");
            println!("  lock file reset · run kst sync to restore");
        }
    }
}

/// Print a source-grouped red teardown tree for skills, MCP packs, commands,
/// agents, plugins, and instructions captured in the lock state. Used by both `--dry-run` (with
/// `would_remove` glyphs) and the real run (with `removed` glyphs).
fn print_removal_tree(
    assets: &BTreeMap<String, AssetEntry>,
    state: &State,
    dry_run: bool,
    plain: bool,
    preamble: bool,
) {
    let status = if dry_run { "would_remove" } else { "removed" };
    // Blank line between sections, but not above the first one unless the
    // caller already printed the `Removing ...` preamble above this tree
    let mut printed = preamble;

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
        print_section_header(
            "Skills",
            Some((state.skills.len(), "to remove")),
            printed,
            plain,
        );
        printed = true;
        for (source, items) in &by_source {
            let repo = short_source(source);
            print_source_header(&repo, None, Some(true), plain);
            for (i, (name, _)) in items.iter().enumerate() {
                let is_last = i == items.len() - 1;
                print_status_leaf(is_last, status, name, "", plain);
            }
        }
    }

    let mcp_packs: Vec<_> = assets.iter().filter(|(_, a)| a.kind == "mcp").collect();
    if !mcp_packs.is_empty() {
        let total_servers: usize = mcp_packs
            .iter()
            .map(|(_, a)| a.destination.split(',').filter(|s| !s.is_empty()).count())
            .sum();
        print_section_header(
            "MCP Servers",
            Some((total_servers, "to remove")),
            printed,
            plain,
        );
        printed = true;
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
            print_source_header(&repo, None, Some(true), plain);
            for (i, name) in servers.iter().enumerate() {
                let is_last = i == servers.len() - 1;
                print_status_leaf(is_last, status, name, "", plain);
            }
        }
    }

    let cmd_assets: Vec<_> = assets.iter().filter(|(_, a)| a.kind == "command").collect();
    if !cmd_assets.is_empty() {
        print_section_header(
            "Commands",
            Some((cmd_assets.len(), "to remove")),
            printed,
            plain,
        );
        printed = true;
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
            print_source_header(&repo, None, Some(true), plain);
            for (i, name) in items.iter().enumerate() {
                let is_last = i == items.len() - 1;
                print_status_leaf(is_last, status, name, "", plain);
            }
        }
    }

    let agent_assets: Vec<_> = assets.iter().filter(|(_, a)| a.kind == "agent").collect();
    if !agent_assets.is_empty() {
        print_section_header(
            "Agents",
            Some((agent_assets.len(), "to remove")),
            printed,
            plain,
        );
        printed = true;
        let mut by_source: Vec<(String, Vec<&str>)> = Vec::new();
        for (_, a) in &agent_assets {
            let key = a.source.clone();
            if let Some(g) = by_source.iter_mut().find(|(k, _)| k == &key) {
                g.1.push(a.name.as_str());
            } else {
                by_source.push((key, vec![a.name.as_str()]));
            }
        }
        for (source, items) in &by_source {
            let repo = short_source(source);
            print_source_header(&repo, None, Some(true), plain);
            for (i, name) in items.iter().enumerate() {
                let is_last = i == items.len() - 1;
                print_status_leaf(is_last, status, name, "", plain);
            }
        }
    }

    let plugin_assets: Vec<_> = assets.iter().filter(|(_, a)| a.kind == "plugin").collect();
    if !plugin_assets.is_empty() {
        print_section_header(
            "Plugins",
            Some((plugin_assets.len(), "to remove")),
            printed,
            plain,
        );
        printed = true;
        let mut by_source: Vec<(String, Vec<&str>)> = Vec::new();
        for (_, a) in &plugin_assets {
            let key = a.source.clone();
            if let Some(g) = by_source.iter_mut().find(|(k, _)| k == &key) {
                g.1.push(a.name.as_str());
            } else {
                by_source.push((key, vec![a.name.as_str()]));
            }
        }
        for (source, items) in &by_source {
            let repo = short_source(source);
            print_source_header(&repo, None, Some(true), plain);
            for (i, name) in items.iter().enumerate() {
                let is_last = i == items.len() - 1;
                print_status_leaf(is_last, status, name, "", plain);
            }
        }
    }

    let instruction_assets: Vec<_> = assets
        .iter()
        .filter(|(_, a)| a.kind == "instructions")
        .collect();
    if !instruction_assets.is_empty() {
        print_section_header(
            "Instructions",
            Some((instruction_assets.len(), "to remove")),
            printed,
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
            print_source_header(&repo, None, Some(true), plain);
            for (i, name) in items.iter().enumerate() {
                let is_last = i == items.len() - 1;
                print_status_leaf(is_last, status, name, "", plain);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsops::temp_dir;

    #[test]
    fn apply_removals_deletes_only_recorded_agent_files() {
        let root = temp_dir("kasetto-clean-agents");
        let agents = root.join(".claude/agents");
        fs::create_dir_all(&agents).unwrap();
        fs::write(agents.join("reviewer.md"), "x").unwrap();
        fs::write(agents.join("planner.md"), "x").unwrap();
        fs::write(agents.join("mine.md"), "user authored").unwrap();

        let recorded = [(
            "agent::src::reviewer",
            ".claude/agents/reviewer.md,.claude/agents/planner.md",
        )];
        apply_removals(
            &State::default(),
            &[],
            &[],
            &recorded,
            &[],
            &[],
            Scope::Project,
            &root,
        )
        .unwrap();

        assert!(!agents.join("reviewer.md").exists());
        assert!(!agents.join("planner.md").exists());
        assert!(agents.join("mine.md").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_removals_leaves_every_other_settings_key() {
        let root = temp_dir("kasetto-clean-plugins");
        let settings = root.join(".claude/settings.json");
        fs::create_dir_all(settings.parent().unwrap()).unwrap();
        let source = serde_json::json!({"source": "github", "repo": "o/r"});
        let hash = crate::plugins::source_hash(&source);
        fs::write(
            &settings,
            serde_json::json!({
                "model": "opus",
                "enabledPlugins": {"mine@x": true, "kit@market": true},
                "extraKnownMarketplaces": {
                    "market": {"source": source, "autoUpdate": true},
                    "other": {"source": {"source": "github", "repo": "a/b"}}
                }
            })
            .to_string(),
        )
        .unwrap();

        let assets = [
            PluginAsset {
                marketplace: true,
                name: "market".into(),
                hash,
                destination: ".claude/settings.json".into(),
            },
            PluginAsset {
                marketplace: false,
                name: "kit@market".into(),
                hash: String::new(),
                destination: ".claude/settings.json".into(),
            },
        ];
        apply_removals(
            &State::default(),
            &[],
            &[],
            &[],
            &assets,
            &[],
            Scope::Project,
            &root,
        )
        .unwrap();

        let after: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(
            after,
            serde_json::json!({
                "model": "opus",
                "enabledPlugins": {"mine@x": true},
                "extraKnownMarketplaces": {
                    "other": {"source": {"source": "github", "repo": "a/b"}}
                }
            })
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_removals_keeps_a_marketplace_the_user_repointed() {
        let root = temp_dir("kasetto-clean-plugins-repointed");
        let settings = root.join(".claude/settings.json");
        fs::create_dir_all(settings.parent().unwrap()).unwrap();
        let ours = serde_json::json!({"source": "github", "repo": "o/r"});
        let theirs = serde_json::json!({"source": "github", "repo": "me/fork"});
        fs::write(
            &settings,
            serde_json::json!({
                "enabledPlugins": {"kit@market": true},
                "extraKnownMarketplaces": {"market": {"source": theirs}}
            })
            .to_string(),
        )
        .unwrap();

        let assets = [
            PluginAsset {
                marketplace: true,
                name: "market".into(),
                hash: crate::plugins::source_hash(&ours),
                destination: ".claude/settings.json".into(),
            },
            PluginAsset {
                marketplace: false,
                name: "kit@market".into(),
                hash: String::new(),
                destination: ".claude/settings.json".into(),
            },
        ];
        apply_removals(
            &State::default(),
            &[],
            &[],
            &[],
            &assets,
            &[],
            Scope::Project,
            &root,
        )
        .unwrap();

        let after: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(
            after,
            serde_json::json!({"extraKnownMarketplaces": {"market": {"source": theirs}}})
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn removal_tree_lists_agents_without_panicking() {
        let mut assets = BTreeMap::new();
        assets.insert(
            "agent::src::reviewer".to_string(),
            AssetEntry {
                kind: "agent".into(),
                name: "reviewer".into(),
                source: "https://github.com/o/r".into(),
                destination: ".claude/agents/reviewer.md".into(),
                ..AssetEntry::default()
            },
        );
        print_removal_tree(&assets, &State::default(), true, true, false);
    }
}
