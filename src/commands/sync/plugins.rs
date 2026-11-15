//! Module that contains the `sync` leg that declares plugin marketplaces and
//! enables plugins in Claude Code and Copilot CLI settings.
//!
//! Two lock kinds back it. `plugin` records each `enabledPlugins` key Kasetto
//! set. `plugin-marketplace` records an `extraKnownMarketplaces` entry Kasetto
//! declared, with the settings files it declared it in; a marketplace the user
//! had already declared with the same source is used but never recorded, so
//! pruning cannot delete it.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::fsops::{now_unix, relativize_dest, resolve_dest, resolve_plugin_targets};
use crate::lock::{AssetEntry, LockFile};
use crate::model::{Action, PluginSourceSpec, PluginTarget, PluginsField, Summary};
use crate::plugins::{
    declared_source_hash, marketplace_source, plugin_enabled, plugin_id, plugin_value,
    read_marketplace, remove_marketplace, remove_plugin, source_hash, write_plugins,
};
use crate::source::materialize_source;
use crate::ui::with_spinner_transient;

use super::{sync_label_with, update_active_for_source, SyncContext};

const PLUGIN: &str = "plugin";
const MARKETPLACE: &str = "plugin-marketplace";

fn plugin_asset_id(source: &str, name: &str) -> String {
    format!("plugin::{source}::{name}")
}

fn marketplace_asset_id(source: &str) -> String {
    format!("plugin-marketplace::{source}")
}

/// Marketplace entry name, from a locked `plugin@marketplace` id.
fn marketplace_of(id: &str) -> &str {
    id.rsplit_once('@').map_or(id, |(_, m)| m)
}

fn plugin_of(id: &str) -> &str {
    id.rsplit_once('@').map_or(id, |(p, _)| p)
}

/// Declare marketplaces and enable plugins so every plugin target matches the
/// config, and prune the keys Kasetto set for plugins no longer configured.
pub(super) fn sync_plugins(
    ctx: &SyncContext,
    lock: &mut LockFile,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
) -> Result<()> {
    let targets = resolve_plugin_targets(ctx.cfg, ctx.scope, ctx.cfg_dir)?;

    if ctx.cfg.plugins.is_empty() {
        remove_stale(ctx, lock, summary, actions, &HashSet::new());
        return Ok(());
    }
    if targets.is_empty() {
        return Ok(());
    }

    let mut desired_ids = HashSet::new();
    let mut cleanup_dirs: Vec<PathBuf> = Vec::new();
    let mut plans: Vec<Plan> = Vec::new();

    for (i, src) in ctx.cfg.plugins.iter().enumerate() {
        let desired_names = desired_plugin_names(src, lock);
        desired_ids.insert(marketplace_asset_id(&src.source));
        // A source that cannot be read this run keeps the plugins it already has:
        // pruning them over a transient or unrelated upstream error would disable
        // what the user has working
        let protect = |ids: &mut HashSet<String>| {
            for n in &desired_names {
                ids.insert(plugin_asset_id(&src.source, n));
            }
        };

        if ctx.locked {
            if let Err(msg) = locked_satisfiable(src, &desired_names, lock) {
                protect(&mut desired_ids);
                fail(summary, actions, &src.source, "locked_error", msg);
                continue;
            }
        }

        let update_active = update_active_for_source(ctx, &src.source, &desired_names);
        let fetch = update_active || needs_fetch(src, &desired_names, lock);
        if fetch && ctx.locked {
            protect(&mut desired_ids);
            fail(
                summary,
                actions,
                &src.source,
                "locked_error",
                "lock requires a fetch to satisfy this source, but --locked forbids fetching"
                    .into(),
            );
            continue;
        }

        let source = match marketplace_source(src, ctx.cfg_dir) {
            Ok(s) => s,
            Err(e) => {
                protect(&mut desired_ids);
                broken(summary, actions, &src.source, "plugin", e.to_string());
                continue;
            }
        };

        if !fetch {
            // Everything needed is in the lock, so drifted settings are repaired
            // without the network
            let ids: Vec<String> = desired_names
                .iter()
                .filter_map(|n| lock.assets.get(&plugin_asset_id(&src.source, n)))
                .map(|a| a.name.clone())
                .collect();
            let Some(mp_name) = ids.first().map(|id| marketplace_of(id).to_string()) else {
                continue;
            };
            for n in &desired_names {
                desired_ids.insert(plugin_asset_id(&src.source, n));
            }
            plans.push(Plan {
                source: src.source.clone(),
                marketplace: mp_name,
                source_value: source,
                revision: String::new(),
                plugins: ids.into_iter().map(|id| (id, None)).collect(),
            });
            continue;
        }

        let stage = std::env::temp_dir().join(format!("kasetto-plugin-{}-{}", now_unix(), i));
        let materialized = match materialize_source(&src.as_source_spec(), ctx.cfg_dir, &stage) {
            Ok(m) => m,
            Err(e) => {
                protect(&mut desired_ids);
                fail(summary, actions, &src.source, "source_error", e.to_string());
                continue;
            }
        };
        let root = materialized.source_root.clone();
        if let Some(d) = materialized.cleanup_dir {
            cleanup_dirs.push(d);
        }

        let marketplace = match read_marketplace(&root) {
            Ok(m) => m,
            Err(e) => {
                protect(&mut desired_ids);
                broken(summary, actions, &src.source, "plugin", e.to_string());
                continue;
            }
        };

        let selected: Vec<&crate::plugins::MarketplacePlugin> = match &src.plugins {
            PluginsField::Wildcard(s) if s == "*" => marketplace.plugins.iter().collect(),
            PluginsField::Wildcard(s) => {
                protect(&mut desired_ids);
                broken(
                    summary,
                    actions,
                    &src.source,
                    "plugin",
                    format!("invalid plugins value \"{s}\": expected \"*\" or a list"),
                );
                continue;
            }
            PluginsField::List(names) => {
                let mut out = Vec::new();
                for name in names {
                    match marketplace.plugins.iter().find(|p| &p.name == name) {
                        Some(p) => out.push(p),
                        None => {
                            desired_ids.insert(plugin_asset_id(&src.source, name));
                            let msg = if marketplace.invalid.contains(name) {
                                format!(
                                    "plugin name `{name}` in marketplace `{}` is not a valid plugin id",
                                    marketplace.name
                                )
                            } else {
                                let known: Vec<&str> = marketplace
                                    .plugins
                                    .iter()
                                    .map(|p| p.name.as_str())
                                    .collect();
                                format!(
                                    "plugin `{name}` is not in marketplace `{}` (it has: {})",
                                    marketplace.name,
                                    known.join(", ")
                                )
                            };
                            broken(
                                summary,
                                actions,
                                &src.source,
                                &format!("plugin:{name}"),
                                msg,
                            );
                        }
                    }
                }
                out
            }
        };

        let mut plugins = Vec::new();
        for p in selected {
            desired_ids.insert(plugin_asset_id(&src.source, &p.name));
            match p.hash(&root) {
                Ok(h) => plugins.push((plugin_id(&p.name, &marketplace.name), Some(h))),
                Err(e) => broken(
                    summary,
                    actions,
                    &src.source,
                    &format!("plugin:{}", p.name),
                    e.to_string(),
                ),
            }
        }
        plans.push(Plan {
            source: src.source.clone(),
            marketplace: marketplace.name,
            source_value: source,
            revision: materialized.source_revision,
            plugins,
        });
    }

    // Prune before applying, like the other legs: a source that moved retires its
    // old ids while writing the same settings keys
    if summary.failed == 0 {
        remove_stale(ctx, lock, summary, actions, &desired_ids);
    }

    for plan in &plans {
        apply_plan(ctx, lock, summary, actions, &targets, plan)?;
    }
    for d in cleanup_dirs {
        let _ = fs::remove_dir_all(d);
    }
    Ok(())
}

/// One source's resolved marketplace and the plugins to enable from it. A
/// plugin's hash is `None` on the no-fetch path, where the lock already has it.
struct Plan {
    source: String,
    marketplace: String,
    source_value: serde_json::Value,
    revision: String,
    plugins: Vec<(String, Option<String>)>,
}

fn apply_plan(
    ctx: &SyncContext,
    lock: &mut LockFile,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
    targets: &[PluginTarget],
    plan: &Plan,
) -> Result<()> {
    let new_hash = source_hash(&plan.source_value);
    let rel = |p: &Path| relativize_dest(p, &ctx.scope_root);

    // Every target is read before anything is written. An unreadable settings
    // file marks this source broken rather than aborting the run, so the lock
    // still saves what the other legs installed
    let mut declared: Vec<Option<String>> = Vec::with_capacity(targets.len());
    for t in targets {
        match declared_source_hash(&t.path, &plan.marketplace) {
            Ok(h) => declared.push(h),
            Err(e) => {
                broken_all(
                    summary,
                    actions,
                    plan,
                    format!("cannot read {}: {e}", rel(&t.path)),
                );
                return Ok(());
            }
        }
    }

    // Decide per settings file whether Kasetto declares the marketplace there.
    // An entry Kasetto wrote is updated; one the user wrote with the same source
    // is used as is; one with a different source is never overwritten. Ownership
    // also covers an entry this marketplace name left under a source no longer
    // configured, so a source move is not mistaken for a hand-declared entry in a
    // dry run, before the old id has been pruned; another configured source's
    // entry is still never taken over
    let mut declare_in: Vec<&PluginTarget> = Vec::new();
    for (t, declared) in targets.iter().zip(&declared) {
        let ours = declared.as_ref().is_some_and(|h| {
            lock.assets.values().any(|a| {
                a.kind == MARKETPLACE
                    && a.name == plan.marketplace
                    && (a.source == plan.source
                        || !ctx.cfg.plugins.iter().any(|p| p.source == a.source))
                    && &a.hash == h
                    && dests_of(a, &ctx.scope_root).contains(&t.path)
            })
        });
        match declared {
            None => declare_in.push(t),
            _ if ours => declare_in.push(t),
            Some(h) if *h == new_hash => {}
            Some(_) => {
                broken_all(
                    summary,
                    actions,
                    plan,
                    format!(
                        "marketplace `{}` is already declared in {} with a different source; Kasetto does not overwrite it",
                        plan.marketplace,
                        rel(&t.path)
                    ),
                );
                return Ok(());
            }
        }
    }

    // Per plugin, the settings files Kasetto writes its key into. A key the user
    // set before Kasetto owned it is left alone, like a hand-declared marketplace:
    // a `true` is used as is, and a `false` is never flipped
    let mut writes: Vec<(&String, &Option<String>, Vec<&PluginTarget>)> = Vec::new();
    let mut first_in_run = true;
    let mut changed = false;
    'plugins: for (id, hash) in &plan.plugins {
        let asset_id = plugin_asset_id(&plan.source, plugin_of(id));
        let existing = lock.assets.get(&asset_id).filter(|a| a.kind == PLUGIN);
        let owned_paths = existing
            .filter(|a| &a.name == id)
            .map(|a| dests_of(a, &ctx.scope_root))
            .unwrap_or_default();
        let mut write_to = Vec::new();
        for t in targets {
            match plugin_value(&t.path, id) {
                Err(e) => {
                    broken(
                        summary,
                        actions,
                        &plan.source,
                        &format!("plugin:{id}"),
                        format!("cannot read {}: {e}", rel(&t.path)),
                    );
                    continue 'plugins;
                }
                Ok(Some(v)) if !owned_paths.contains(&t.path) => {
                    if v != serde_json::Value::Bool(true) {
                        broken(
                            summary,
                            actions,
                            &plan.source,
                            &format!("plugin:{id}"),
                            format!(
                                "`{id}` is turned off by hand in {}; Kasetto does not override it",
                                rel(&t.path)
                            ),
                        );
                        continue 'plugins;
                    }
                }
                Ok(_) => write_to.push(t),
            }
        }

        let settings_ok = targets.iter().all(|t| plugin_enabled(&t.path, id))
            && declare_in.iter().all(|t| {
                declared_source_hash(&t.path, &plan.marketplace)
                    .ok()
                    .flatten()
                    .as_deref()
                    == Some(new_hash.as_str())
            });
        let hash_ok = match hash {
            Some(h) => existing.is_some_and(|a| &a.hash == h),
            None => true,
        };
        let name_ok = existing.is_some_and(|a| &a.name == id);
        let status = match (
            existing.is_some(),
            settings_ok && hash_ok && name_ok,
            ctx.dry_run,
        ) {
            (true, true, _) => "unchanged",
            (true, false, false) => "updated",
            (true, false, true) => "would_update",
            (false, _, false) => "installed",
            (false, _, true) => "would_install",
        };
        let label = sync_label_with(id, &plan.source, ctx.plain, first_in_run);
        first_in_run = false;
        with_spinner_transient(ctx.animate, ctx.plain, &label, || {
            match status {
                "unchanged" => summary.unchanged += 1,
                "updated" | "would_update" => summary.updated += 1,
                _ => summary.installed += 1,
            }
            actions.push(Action {
                source: Some(plan.source.clone()),
                skill: Some(format!("plugin:{id}")),
                status: status.into(),
                error: None,
            });
            Ok(())
        })?;
        changed |= status != "unchanged";
        writes.push((id, hash, write_to));
    }

    if ctx.dry_run || !changed {
        return Ok(());
    }

    // An upstream rename leaves keys under names this plan no longer writes.
    // They are retired while the lock still records that Kasetto set them
    let owned_marketplace = lock
        .assets
        .get(&marketplace_asset_id(&plan.source))
        .filter(|a| a.kind == MARKETPLACE)
        .cloned();
    let mut retire: Vec<(PathBuf, String, Option<String>)> = Vec::new();
    if let Some(m) = owned_marketplace
        .as_ref()
        .filter(|m| m.name != plan.marketplace)
    {
        for p in dests_of(m, &ctx.scope_root) {
            retire.push((p, m.name.clone(), Some(m.hash.clone())));
        }
    }
    for (id, _, _) in &writes {
        let asset_id = plugin_asset_id(&plan.source, plugin_of(id));
        if let Some(prev) = lock
            .assets
            .get(&asset_id)
            .filter(|a| a.kind == PLUGIN && &&a.name != id)
        {
            for p in dests_of(prev, &ctx.scope_root) {
                retire.push((p, prev.name.clone(), None));
            }
        }
    }
    for (path, name, marketplace_hash) in &retire {
        let done = match marketplace_hash {
            Some(h) => remove_marketplace(path, name, h).map(|_| ()),
            None => remove_plugin(path, name),
        };
        if let Err(e) = done {
            broken_all(
                summary,
                actions,
                plan,
                format!("cannot edit {}: {e}", rel(path)),
            );
            return Ok(());
        }
    }

    for t in targets {
        let declare = declare_in.iter().any(|d| d.path == t.path);
        let ids: Vec<String> = writes
            .iter()
            .filter(|(_, _, to)| to.iter().any(|w| w.path == t.path))
            .map(|(id, _, _)| (*id).clone())
            .collect();
        if !declare && ids.is_empty() {
            continue;
        }
        if let Err(e) = write_plugins(
            &t.path,
            &plan.marketplace,
            &plan.source_value,
            declare,
            &ids,
        ) {
            broken_all(
                summary,
                actions,
                plan,
                format!("cannot edit {}: {e}", rel(&t.path)),
            );
            return Ok(());
        }
    }

    let dests = |ts: &[&PluginTarget]| {
        ts.iter()
            .map(|t| rel(&t.path))
            .collect::<Vec<_>>()
            .join(",")
    };
    for (id, hash, write_to) in &writes {
        let asset_id = plugin_asset_id(&plan.source, plugin_of(id));
        let previous = lock.assets.get(&asset_id).cloned();
        let hash = (*hash)
            .clone()
            .or_else(|| previous.as_ref().map(|a| a.hash.clone()))
            .unwrap_or_default();
        let revision = if plan.revision.is_empty() {
            previous.map(|a| a.source_revision).unwrap_or_default()
        } else {
            plan.revision.clone()
        };
        lock.save_tracked_asset(
            &asset_id,
            AssetEntry {
                kind: PLUGIN.into(),
                name: (*id).clone(),
                hash,
                source: plan.source.clone(),
                destination: dests(write_to),
                source_revision: revision,
                has_secrets: false,
            },
        );
    }
    if declare_in.is_empty() {
        if owned_marketplace.is_some_and(|m| m.name != plan.marketplace) {
            lock.remove_tracked_asset(&marketplace_asset_id(&plan.source));
        }
    } else {
        let revision = if plan.revision.is_empty() {
            owned_marketplace
                .map(|m| m.source_revision)
                .unwrap_or_default()
        } else {
            plan.revision.clone()
        };
        lock.save_tracked_asset(
            &marketplace_asset_id(&plan.source),
            AssetEntry {
                kind: MARKETPLACE.into(),
                name: plan.marketplace.clone(),
                hash: new_hash,
                source: plan.source.clone(),
                destination: dests(&declare_in),
                source_revision: revision,
                has_secrets: false,
            },
        );
    }
    Ok(())
}

/// The settings files a lock entry records, resolved against the scope root.
fn dests_of(a: &AssetEntry, scope_root: &Path) -> Vec<PathBuf> {
    a.destination
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|p| resolve_dest(p, scope_root))
        .collect()
}

/// Mark every plugin of a plan broken with one reason.
fn broken_all(summary: &mut Summary, actions: &mut Vec<Action>, plan: &Plan, msg: String) {
    for (id, _) in &plan.plugins {
        broken(
            summary,
            actions,
            &plan.source,
            &format!("plugin:{id}"),
            msg.clone(),
        );
    }
}

fn fail(summary: &mut Summary, actions: &mut Vec<Action>, source: &str, status: &str, msg: String) {
    summary.failed += 1;
    actions.push(Action {
        source: Some(source.to_string()),
        skill: None,
        status: status.into(),
        error: Some(msg),
    });
}

fn broken(
    summary: &mut Summary,
    actions: &mut Vec<Action>,
    source: &str,
    skill: &str,
    msg: String,
) {
    summary.broken += 1;
    actions.push(Action {
        source: Some(source.to_string()),
        skill: Some(skill.to_string()),
        status: "broken".into(),
        error: Some(msg),
    });
}

/// Desired plugin entry names for a source, without network: the config list,
/// or the locked plugins for a wildcard.
fn desired_plugin_names(src: &PluginSourceSpec, lock: &LockFile) -> Vec<String> {
    match &src.plugins {
        PluginsField::List(names) => names.clone(),
        PluginsField::Wildcard(s) if s == "*" => lock
            .assets
            .values()
            .filter(|a| a.kind == PLUGIN && a.source == src.source)
            .map(|a| plugin_of(&a.name).to_string())
            .collect(),
        PluginsField::Wildcard(_) => Vec::new(),
    }
}

/// Fetch when a wildcard was never resolved, a plugin has no lock entry, or the
/// configured ref moved. Settings drift alone never needs the network.
fn needs_fetch(src: &PluginSourceSpec, desired: &[String], lock: &LockFile) -> bool {
    if matches!(&src.plugins, PluginsField::Wildcard(s) if s == "*")
        && !lock
            .assets
            .values()
            .any(|a| a.kind == PLUGIN && a.source == src.source)
    {
        return true;
    }
    let expected = src.as_source_spec().expected_revision();
    desired.iter().any(|name| {
        match lock
            .assets
            .get(&plugin_asset_id(&src.source, name))
            .filter(|a| a.kind == PLUGIN)
        {
            None => true,
            // An empty hash means `kst lock` repinned the ref without fetching
            Some(a) => {
                a.hash.is_empty()
                    || (!a.source_revision.is_empty() && a.source_revision != expected)
            }
        }
    })
}

fn locked_satisfiable(
    src: &PluginSourceSpec,
    desired: &[String],
    lock: &LockFile,
) -> std::result::Result<(), String> {
    let present = |name: &str| {
        lock.assets
            .get(&plugin_asset_id(&src.source, name))
            .is_some_and(|a| a.kind == PLUGIN)
    };
    match &src.plugins {
        PluginsField::List(_) => match desired.iter().find(|n| !present(n)) {
            Some(n) => Err(format!(
                "--locked: plugin `{n}` from `{}` is not in the lock",
                src.source
            )),
            None => Ok(()),
        },
        PluginsField::Wildcard(_) if desired.is_empty() => Err(format!(
            "--locked: source `{}` has no plugin entries in the lock",
            src.source
        )),
        PluginsField::Wildcard(_) => Ok(()),
    }
}

/// Remove the `enabledPlugins` keys and marketplace entries Kasetto recorded but
/// the config no longer wants. A marketplace entry whose source the user changed
/// since is left in place.
fn remove_stale(
    ctx: &SyncContext,
    lock: &mut LockFile,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
    desired_ids: &HashSet<String>,
) {
    let mut stale: Vec<(String, AssetEntry)> = lock
        .assets
        .iter()
        .filter(|(id, a)| (a.kind == PLUGIN || a.kind == MARKETPLACE) && !desired_ids.contains(*id))
        .map(|(id, a)| (id.clone(), a.clone()))
        .collect();
    // Plugins first, so a marketplace is removed after the plugins that used it
    stale.sort_by_key(|(id, a)| (a.kind == MARKETPLACE, id.clone()));

    for (id, a) in stale {
        let skill = if a.kind == PLUGIN {
            format!("plugin:{}", a.name)
        } else {
            format!("marketplace:{}", a.name)
        };
        if ctx.dry_run {
            summary.removed += 1;
            actions.push(Action {
                source: None,
                skill: Some(skill),
                status: "would_remove".into(),
                error: None,
            });
            continue;
        }
        // The lock entry is the only record that Kasetto wrote these keys, so it
        // outlives a settings file that could not be edited, for the next run to retry
        let failure = dests_of(&a, &ctx.scope_root).into_iter().find_map(|path| {
            let done = if a.kind == PLUGIN {
                remove_plugin(&path, &a.name)
            } else {
                remove_marketplace(&path, &a.name, &a.hash).map(|_| ())
            };
            done.err().map(|e| {
                format!(
                    "cannot edit {}: {e}",
                    relativize_dest(&path, &ctx.scope_root)
                )
            })
        });
        match failure {
            Some(msg) => broken(summary, actions, &a.source, &skill, msg),
            None => {
                lock.remove_tracked_asset(&id);
                summary.removed += 1;
                actions.push(Action {
                    source: None,
                    skill: Some(skill),
                    status: "removed".into(),
                    error: None,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsops::temp_dir;
    use crate::model::{Agent, AgentField, Config, Scope};
    use serde_json::Value;
    use std::path::Path;

    const MARKET: &str = r#"{
  "name": "cursor-plugins",
  "owner": { "name": "Cursor" },
  "plugins": [
    { "name": "pstack", "source": "./harness/pstack/claude-code" },
    { "name": "origin-apps", "source": "./origin-apps" }
  ]
}"#;

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn market(dir: &str) -> PathBuf {
        let src = temp_dir(dir);
        write(&src.join(".claude-plugin/marketplace.json"), MARKET);
        write(
            &src.join("harness/pstack/claude-code/skills/a/SKILL.md"),
            "a",
        );
        write(&src.join("origin-apps/skills/b/SKILL.md"), "b");
        src
    }

    fn spec(src: &Path, plugins: PluginsField) -> PluginSourceSpec {
        PluginSourceSpec {
            source: src.to_string_lossy().to_string(),
            branch: None,
            git_ref: None,
            sub_dir: None,
            plugins,
        }
    }

    fn names(n: &[&str]) -> PluginsField {
        PluginsField::List(n.iter().map(|s| s.to_string()).collect())
    }

    fn cfg_with(tools: Vec<Agent>, plugins: Vec<PluginSourceSpec>) -> Config {
        Config {
            destination: None,
            scope: Some(Scope::Project),
            agent: Some(AgentField::Many(tools)),
            skills: Vec::new(),
            mcps: Vec::new(),
            commands: Vec::new(),
            instructions: Vec::new(),
            subagents: Vec::new(),
            plugins,
            secrets: None,
            audit: None,
        }
    }

    fn ctx<'a>(cfg: &'a Config, project: &'a Path, dry_run: bool, locked: bool) -> SyncContext<'a> {
        SyncContext {
            cfg,
            cfg_dir: project,
            destinations: &[],
            scope_root: project.to_path_buf(),
            scope: Scope::Project,
            dry_run,
            animate: false,
            plain: true,
            update: false,
            update_only: Vec::new(),
            update_local: false,
            locked,
            secrets: crate::secrets::SecretContext::empty(),
        }
    }

    fn run(cfg: &Config, project: &Path, lock: &mut LockFile) -> (Summary, Vec<Action>) {
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        sync_plugins(
            &ctx(cfg, project, false, false),
            lock,
            &mut summary,
            &mut actions,
        )
        .unwrap();
        (summary, actions)
    }

    /// Run with `--dry-run` and/or a forced refetch of local sources.
    fn run_with(
        cfg: &Config,
        project: &Path,
        lock: &mut LockFile,
        dry_run: bool,
        refetch: bool,
    ) -> (Summary, Vec<Action>) {
        let mut c = ctx(cfg, project, dry_run, false);
        c.update_local = refetch;
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        sync_plugins(&c, lock, &mut summary, &mut actions).unwrap();
        (summary, actions)
    }

    fn settings(path: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn enables_in_claude_and_copilot_then_stays_unchanged() {
        let src = market("kasetto-plugin-src");
        let project = temp_dir("kasetto-plugin-proj");
        write(
            &project.join(".claude/settings.json"),
            r#"{"model": "sonnet"}"#,
        );
        let cfg = cfg_with(
            vec![Agent::ClaudeCode, Agent::GithubCopilot, Agent::Cursor],
            vec![spec(&src, names(&["pstack"]))],
        );
        let mut lock = LockFile::default();

        let (s, _) = run(&cfg, &project, &mut lock);
        assert_eq!(s.installed, 1);
        for rel in [".claude/settings.json", ".github/copilot/settings.json"] {
            let v = settings(&project.join(rel));
            assert_eq!(v["enabledPlugins"]["pstack@cursor-plugins"], true, "{rel}");
            assert_eq!(
                v["extraKnownMarketplaces"]["cursor-plugins"]["source"]["source"],
                "directory"
            );
        }
        assert_eq!(
            settings(&project.join(".claude/settings.json"))["model"],
            "sonnet"
        );
        let entry = &lock.assets[&plugin_asset_id(&src.to_string_lossy(), "pstack")];
        assert_eq!(entry.name, "pstack@cursor-plugins");
        assert_eq!(
            entry.destination,
            ".claude/settings.json,.github/copilot/settings.json"
        );
        assert!(lock
            .assets
            .contains_key(&marketplace_asset_id(&src.to_string_lossy())));

        let (s, _) = run(&cfg, &project, &mut lock);
        assert_eq!((s.unchanged, s.installed, s.updated), (1, 0, 0));

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn unknown_plugin_is_broken_and_names_the_marketplace() {
        let src = market("kasetto-plugin-missing");
        let project = temp_dir("kasetto-plugin-missing-proj");
        fs::create_dir_all(&project).unwrap();
        let cfg = cfg_with(vec![Agent::ClaudeCode], vec![spec(&src, names(&["nope"]))]);
        let mut lock = LockFile::default();
        let (s, actions) = run(&cfg, &project, &mut lock);
        assert_eq!(s.broken, 1);
        let err = actions[0].error.as_deref().unwrap();
        assert!(err.contains("not in marketplace `cursor-plugins`"), "{err}");
        assert!(err.contains("pstack, origin-apps"), "{err}");
        assert!(!project.join(".claude/settings.json").exists());
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn a_hand_declared_marketplace_with_another_source_is_left_alone() {
        let src = market("kasetto-plugin-foreign");
        let project = temp_dir("kasetto-plugin-foreign-proj");
        let original = r#"{"extraKnownMarketplaces": {"cursor-plugins": {"source": {"source": "github", "repo": "fork/plugins"}}}}"#;
        write(&project.join(".claude/settings.json"), original);
        let cfg = cfg_with(
            vec![Agent::ClaudeCode],
            vec![spec(&src, names(&["pstack"]))],
        );
        let mut lock = LockFile::default();
        let (s, actions) = run(&cfg, &project, &mut lock);
        assert_eq!(s.broken, 1);
        assert!(actions[0]
            .error
            .as_deref()
            .unwrap()
            .contains("different source"));
        assert_eq!(
            fs::read_to_string(project.join(".claude/settings.json")).unwrap(),
            original
        );
        assert!(lock.assets.is_empty());
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn a_hand_declared_marketplace_with_the_same_source_is_used_not_owned() {
        let src = market("kasetto-plugin-adopt");
        let project = temp_dir("kasetto-plugin-adopt-proj");
        fs::create_dir_all(&project).unwrap();
        let cfg = cfg_with(
            vec![Agent::ClaudeCode],
            vec![spec(&src, names(&["pstack"]))],
        );
        let source = marketplace_source(&cfg.plugins[0], &project).unwrap();
        let path = project.join(".claude/settings.json");
        write_plugins(&path, "cursor-plugins", &source, true, &[]).unwrap();

        let mut lock = LockFile::default();
        assert_eq!(run(&cfg, &project, &mut lock).0.installed, 1);
        assert!(!lock
            .assets
            .contains_key(&marketplace_asset_id(&src.to_string_lossy())));

        // Dropping the plugin removes Kasetto's enable key but not the user's marketplace
        let empty = cfg_with(vec![Agent::ClaudeCode], Vec::new());
        assert_eq!(run(&empty, &project, &mut lock).0.removed, 1);
        let v = settings(&path);
        assert!(v.get("enabledPlugins").is_none());
        assert!(v["extraKnownMarketplaces"].get("cursor-plugins").is_some());
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn pruning_removes_only_kasetto_keys_and_repairs_drift_offline() {
        let src = market("kasetto-plugin-prune");
        let project = temp_dir("kasetto-plugin-prune-proj");
        let path = project.join(".claude/settings.json");
        write(
            &path,
            r#"{"model": "sonnet", "enabledPlugins": {"mine@x": true}}"#,
        );
        let both = cfg_with(
            vec![Agent::ClaudeCode],
            vec![spec(&src, names(&["pstack", "origin-apps"]))],
        );
        let mut lock = LockFile::default();
        assert_eq!(run(&both, &project, &mut lock).0.installed, 2);

        // A hand edit that drops a key is restored from the lock alone
        let mut v = settings(&path);
        v["enabledPlugins"]
            .as_object_mut()
            .unwrap()
            .remove("pstack@cursor-plugins");
        fs::write(&path, v.to_string()).unwrap();
        fs::remove_dir_all(&src).unwrap();
        let (s, _) = run(&both, &project, &mut lock);
        assert_eq!((s.updated, s.unchanged), (1, 1));
        assert_eq!(
            settings(&path)["enabledPlugins"]["pstack@cursor-plugins"],
            true
        );

        // Removing the whole block prunes both plugins and the marketplace
        let none = cfg_with(vec![Agent::ClaudeCode], Vec::new());
        assert_eq!(run(&none, &project, &mut lock).0.removed, 3);
        assert_eq!(
            settings(&path),
            serde_json::json!({"model": "sonnet", "enabledPlugins": {"mine@x": true}})
        );
        assert!(lock.assets.is_empty());
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn wildcard_enables_every_marketplace_plugin() {
        let src = market("kasetto-plugin-wild");
        let project = temp_dir("kasetto-plugin-wild-proj");
        fs::create_dir_all(&project).unwrap();
        let cfg = cfg_with(
            vec![Agent::ClaudeCode],
            vec![spec(&src, PluginsField::Wildcard("*".into()))],
        );
        let mut lock = LockFile::default();
        assert_eq!(run(&cfg, &project, &mut lock).0.installed, 2);
        let v = settings(&project.join(".claude/settings.json"));
        assert_eq!(v["enabledPlugins"]["origin-apps@cursor-plugins"], true);
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn dry_run_and_locked_write_nothing() {
        let src = market("kasetto-plugin-dry");
        let project = temp_dir("kasetto-plugin-dry-proj");
        fs::create_dir_all(&project).unwrap();
        let cfg = cfg_with(
            vec![Agent::ClaudeCode],
            vec![spec(&src, names(&["pstack"]))],
        );
        let mut lock = LockFile::default();
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        sync_plugins(
            &ctx(&cfg, &project, true, false),
            &mut lock,
            &mut summary,
            &mut actions,
        )
        .unwrap();
        assert_eq!(actions[0].status, "would_install");
        assert!(!project.join(".claude/settings.json").exists());

        let mut summary = Summary::default();
        let mut actions = Vec::new();
        sync_plugins(
            &ctx(&cfg, &project, false, true),
            &mut lock,
            &mut summary,
            &mut actions,
        )
        .unwrap();
        assert_eq!(summary.failed, 1);
        assert_eq!(actions[0].status, "locked_error");
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn tools_without_a_plugin_target_get_nothing() {
        let src = market("kasetto-plugin-none");
        let project = temp_dir("kasetto-plugin-none-proj");
        fs::create_dir_all(&project).unwrap();
        let cfg = cfg_with(vec![Agent::Cursor], vec![spec(&src, names(&["pstack"]))]);
        let mut lock = LockFile::default();
        let (s, actions) = run(&cfg, &project, &mut lock);
        assert_eq!(s.installed, 0);
        assert!(actions.is_empty());
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn an_upstream_marketplace_rename_retires_the_old_keys() {
        let src = market("kasetto-plugin-rename");
        let project = temp_dir("kasetto-plugin-rename-proj");
        let path = project.join(".claude/settings.json");
        fs::create_dir_all(&project).unwrap();
        let cfg = cfg_with(
            vec![Agent::ClaudeCode],
            vec![spec(&src, names(&["pstack"]))],
        );
        let mut lock = LockFile::default();
        run(&cfg, &project, &mut lock);

        write(
            &src.join(".claude-plugin/marketplace.json"),
            &MARKET.replace("cursor-plugins", "renamed"),
        );
        let (s, _) = run_with(&cfg, &project, &mut lock, false, true);
        assert_eq!(s.updated, 1);
        let v = settings(&path);
        assert_eq!(
            v["enabledPlugins"],
            serde_json::json!({"pstack@renamed": true})
        );
        assert!(v["extraKnownMarketplaces"].get("cursor-plugins").is_none());

        let none = cfg_with(vec![Agent::ClaudeCode], Vec::new());
        run(&none, &project, &mut lock);
        assert_eq!(settings(&path), serde_json::json!({}));
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn an_unreadable_marketplace_keeps_the_plugins_it_already_enabled() {
        let src = market("kasetto-plugin-unreadable");
        let project = temp_dir("kasetto-plugin-unreadable-proj");
        let path = project.join(".claude/settings.json");
        fs::create_dir_all(&project).unwrap();
        let cfg = cfg_with(
            vec![Agent::ClaudeCode],
            vec![spec(&src, names(&["pstack"]))],
        );
        let mut lock = LockFile::default();
        run(&cfg, &project, &mut lock);

        write(&src.join(".claude-plugin/marketplace.json"), "{ not json");
        let (s, _) = run_with(&cfg, &project, &mut lock, false, true);
        assert_eq!((s.broken, s.removed), (1, 0));
        assert_eq!(
            settings(&path)["enabledPlugins"]["pstack@cursor-plugins"],
            true
        );
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn a_malformed_settings_file_is_broken_not_fatal() {
        let src = market("kasetto-plugin-badjson");
        let project = temp_dir("kasetto-plugin-badjson-proj");
        let path = project.join(".claude/settings.json");
        write(&path, r#"{"model": "sonnet",}"#);
        let cfg = cfg_with(
            vec![Agent::ClaudeCode],
            vec![spec(&src, names(&["pstack"]))],
        );
        let mut lock = LockFile::default();
        let (s, actions) = run(&cfg, &project, &mut lock);
        assert_eq!((s.broken, s.installed), (1, 0));
        assert!(actions[0].error.as_deref().unwrap().contains("cannot read"));
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            r#"{"model": "sonnet",}"#
        );
        assert!(lock.assets.is_empty());
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn a_dry_run_source_move_is_not_reported_as_hand_declared() {
        let src = market("kasetto-plugin-move-a");
        let fork = market("kasetto-plugin-move-b");
        let project = temp_dir("kasetto-plugin-move-proj");
        fs::create_dir_all(&project).unwrap();
        let mut lock = LockFile::default();
        let first = cfg_with(
            vec![Agent::ClaudeCode],
            vec![spec(&src, names(&["pstack"]))],
        );
        run(&first, &project, &mut lock);

        let moved = cfg_with(
            vec![Agent::ClaudeCode],
            vec![spec(&fork, names(&["pstack"]))],
        );
        let (s, _) = run_with(&moved, &project, &mut lock, true, false);
        assert_eq!(s.broken, 0);
        let (s, _) = run(&moved, &project, &mut lock);
        assert_eq!((s.broken, s.installed), (0, 1));
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&fork);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn two_configured_sources_never_take_over_one_marketplace_name() {
        let a = market("kasetto-plugin-twin-a");
        let b = market("kasetto-plugin-twin-b");
        let project = temp_dir("kasetto-plugin-twin-proj");
        fs::create_dir_all(&project).unwrap();
        let cfg = cfg_with(
            vec![Agent::ClaudeCode],
            vec![spec(&a, names(&["pstack"])), spec(&b, names(&["pstack"]))],
        );
        let mut lock = LockFile::default();
        let (s, _) = run(&cfg, &project, &mut lock);
        assert_eq!((s.installed, s.broken), (1, 1));
        let declared = &settings(&project.join(".claude/settings.json"))["extraKnownMarketplaces"]
            ["cursor-plugins"]["source"]["path"];
        assert!(declared.as_str().unwrap().contains("kasetto-plugin-twin-a"));
        let (s, _) = run(&cfg, &project, &mut lock);
        assert_eq!((s.unchanged, s.broken), (1, 1));
        let _ = fs::remove_dir_all(&a);
        let _ = fs::remove_dir_all(&b);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn a_plugin_key_the_user_set_is_never_flipped_or_deleted() {
        let src = market("kasetto-plugin-handkey");
        let project = temp_dir("kasetto-plugin-handkey-proj");
        let path = project.join(".claude/settings.json");
        write(
            &path,
            r#"{"enabledPlugins": {"pstack@cursor-plugins": false, "origin-apps@cursor-plugins": true}}"#,
        );
        let cfg = cfg_with(
            vec![Agent::ClaudeCode],
            vec![spec(&src, names(&["pstack", "origin-apps"]))],
        );
        let mut lock = LockFile::default();
        let (s, _) = run(&cfg, &project, &mut lock);
        assert_eq!((s.broken, s.installed), (1, 1));
        assert_eq!(
            settings(&path)["enabledPlugins"]["pstack@cursor-plugins"],
            false
        );

        let none = cfg_with(vec![Agent::ClaudeCode], Vec::new());
        run(&none, &project, &mut lock);
        assert_eq!(
            settings(&path)["enabledPlugins"],
            serde_json::json!({"pstack@cursor-plugins": false, "origin-apps@cursor-plugins": true})
        );
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn a_prune_that_cannot_edit_settings_keeps_the_lock_entry() {
        let src = market("kasetto-plugin-prunefail");
        let project = temp_dir("kasetto-plugin-prunefail-proj");
        let path = project.join(".claude/settings.json");
        fs::create_dir_all(&project).unwrap();
        let cfg = cfg_with(
            vec![Agent::ClaudeCode],
            vec![spec(&src, names(&["pstack"]))],
        );
        let mut lock = LockFile::default();
        run(&cfg, &project, &mut lock);
        fs::write(&path, "{ broken").unwrap();

        let none = cfg_with(vec![Agent::ClaudeCode], Vec::new());
        let (s, _) = run(&none, &project, &mut lock);
        assert_eq!((s.removed, s.broken), (0, 2));
        assert!(lock
            .assets
            .contains_key(&plugin_asset_id(&src.to_string_lossy(), "pstack")));
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn an_empty_lock_hash_forces_a_fetch() {
        let s = spec(Path::new("/m"), names(&["pstack"]));
        let mut lock = LockFile::default();
        let mut entry = AssetEntry {
            kind: PLUGIN.into(),
            name: "pstack@m".into(),
            hash: "h".into(),
            source: s.source.clone(),
            destination: String::new(),
            source_revision: String::new(),
            has_secrets: false,
        };
        lock.save_tracked_asset(&plugin_asset_id(&s.source, "pstack"), entry.clone());
        assert!(!needs_fetch(&s, &["pstack".into()], &lock));
        entry.hash.clear();
        lock.save_tracked_asset(&plugin_asset_id(&s.source, "pstack"), entry);
        assert!(needs_fetch(&s, &["pstack".into()], &lock));
    }

    #[test]
    fn offline_repair_keeps_the_marketplace_revision() {
        let src = market("kasetto-plugin-rev");
        let project = temp_dir("kasetto-plugin-rev-proj");
        let path = project.join(".claude/settings.json");
        fs::create_dir_all(&project).unwrap();
        let cfg = cfg_with(
            vec![Agent::ClaudeCode],
            vec![spec(&src, names(&["pstack"]))],
        );
        let mut lock = LockFile::default();
        run(&cfg, &project, &mut lock);
        let mp_id = marketplace_asset_id(&src.to_string_lossy());
        lock.assets.get_mut(&mp_id).unwrap().source_revision = "abc123".into();

        fs::write(&path, "{}").unwrap();
        let (s, _) = run(&cfg, &project, &mut lock);
        assert_eq!(s.updated, 1);
        assert_eq!(lock.assets[&mp_id].source_revision, "abc123");
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&project);
    }
}
