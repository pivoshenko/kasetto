use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::error::{err, Result};
use crate::fsops::{
    dirs_home, dirs_kasetto_config, hash_file, now_unix, resolve_mcp_settings_targets,
};
use crate::lock::LockFile;
use crate::mcps::{merge_mcp_config, remove_mcp_server, servers_present_in_settings};
use crate::model::{
    all_mcp_project_targets, all_mcp_settings_targets, Action, McpSettingsTarget, McpsField, Scope,
    Summary,
};
use crate::source::{discover_mcps, materialize_source, resolve_mcp_entry};
use crate::ui::with_spinner_transient;

use super::{
    file_name_str, remove_stale as remove_stale_shared, sync_label_with, update_active_for_source,
    StaleEntry, SyncContext,
};

/// An MCP entry ready to be installed or updated.
struct PendingMcp {
    source: String,
    file_name: String,
    mcp_path: PathBuf,
    hash: String,
    server_names: Vec<String>,
    asset_id: String,
    is_new: bool,
}

pub(super) fn sync_mcps(
    ctx: &SyncContext,
    lock: &mut LockFile,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
) -> Result<()> {
    let mut desired_mcp_ids = HashSet::new();
    let mcp_settings_list = resolve_mcp_settings_targets(ctx.cfg, ctx.scope, ctx.cfg_dir)?;

    // No agents configured (e.g. user dropped `agent:`) but lock still has MCP
    // entries — config is source of truth, so scrub the orphans from every
    // known agent's settings file as best-effort, prune the lock, and return.
    if mcp_settings_list.is_empty() {
        let has_orphans = lock.assets.values().any(|a| a.kind == "mcp");
        if has_orphans {
            let fallback_targets: Vec<McpSettingsTarget> = match ctx.scope {
                Scope::Project => all_mcp_project_targets(&ctx.scope_root),
                Scope::Global => match (dirs_home(), dirs_kasetto_config()) {
                    (Ok(home), Ok(cfg_dir)) => all_mcp_settings_targets(&home, &cfg_dir),
                    _ => Vec::new(),
                },
            };
            remove_stale(
                ctx,
                lock,
                summary,
                actions,
                &desired_mcp_ids,
                &fallback_targets,
            );
        }
        return Ok(());
    }

    // Phase 1: discover and classify all MCP entries
    let mut pending: Vec<PendingMcp> = Vec::new();
    let mut cleanup_dirs: Vec<PathBuf> = Vec::new();

    for (i, src) in ctx.cfg.mcps.iter().enumerate() {
        // Desired MCP file names for this source, derived without any network:
        // predicted file names for a list, or the locked set for a wildcard.
        let desired_file_names = desired_mcp_file_names(src, lock);

        // `--locked`/`--frozen`: the lock must be able to satisfy the config.
        if ctx.locked {
            if let Err(e) = ensure_locked_satisfiable_mcps(src, &desired_file_names, lock) {
                summary.failed += 1;
                actions.push(Action {
                    source: Some(src.source.clone()),
                    skill: None,
                    status: "locked_error".into(),
                    error: Some(e.to_string()),
                });
                continue;
            }
        }

        // Selective `--update <name>` accepts either the file name (`github.json`)
        // or the bare stem (`github`), matching how skills/commands use bare names.
        let update_names: Vec<String> = desired_file_names
            .iter()
            .flat_map(|f| {
                std::iter::once(f.clone()).chain(f.strip_suffix(".json").map(str::to_string))
            })
            .collect();
        let update_active = update_active_for_source(ctx, &update_names);
        let fetch = update_active
            || needs_fetch_mcps(ctx, src, &desired_file_names, lock, &mcp_settings_list);

        if fetch && ctx.locked {
            summary.failed += 1;
            actions.push(Action {
                source: Some(src.source.clone()),
                skill: None,
                status: "locked_error".into(),
                error: Some(
                    "lock requires a fetch to satisfy this source, but --locked forbids fetching"
                        .into(),
                ),
            });
            continue;
        }

        if !fetch {
            // Skip path: no network. Honor each desired MCP file from the lock.
            let mut first_in_run = true;
            for file_name in &desired_file_names {
                let asset_id = format!("mcp::{}::{}", src.source, file_name);
                desired_mcp_ids.insert(asset_id);
                let label = sync_label_with(file_name, &src.source, ctx.plain, first_in_run);
                first_in_run = false;
                with_spinner_transient(ctx.animate, ctx.plain, &label, || {
                    summary.unchanged += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(format!("mcp:{file_name}")),
                        status: "unchanged".into(),
                        error: None,
                    });
                    Ok(())
                })?;
            }
            continue;
        }

        let stage = std::env::temp_dir().join(format!("kasetto-mcp-{}-{}", now_unix(), i));
        let materialized = match materialize_source(&src.as_source_spec(), ctx.cfg_dir, &stage) {
            Ok(m) => m,
            Err(e) => {
                summary.failed += 1;
                actions.push(Action {
                    source: Some(src.source.clone()),
                    skill: None,
                    status: "source_error".into(),
                    error: Some(e.to_string()),
                });
                continue;
            }
        };
        let root = materialized
            .cleanup_dir
            .as_deref()
            .unwrap_or_else(|| std::path::Path::new(&src.source));
        let resolve_result: Result<Vec<PathBuf>> = match &src.mcps {
            McpsField::Wildcard(s) if s == "*" => discover_mcps(root),
            McpsField::Wildcard(s) => Err(err(format!(
                "invalid mcps value \"{s}\": expected \"*\" or a list"
            ))),
            McpsField::List(entries) => {
                let mut paths = Vec::new();
                for entry in entries {
                    let name = match entry {
                        crate::model::McpEntry::Name(n) => n.clone(),
                        crate::model::McpEntry::Obj { name, .. } => name.clone(),
                    };
                    match resolve_mcp_entry(root, entry) {
                        Ok(p) => paths.push(p),
                        Err(e) => {
                            summary.broken += 1;
                            actions.push(Action {
                                source: Some(src.source.clone()),
                                skill: Some(name),
                                status: "broken".into(),
                                error: Some(e.to_string()),
                            });
                        }
                    }
                }
                Ok(paths)
            }
        };
        let mcps = match resolve_result {
            Ok(paths) => paths,
            Err(e) => {
                summary.broken += 1;
                actions.push(Action {
                    source: Some(src.source.clone()),
                    skill: Some("mcp".into()),
                    status: "broken".into(),
                    error: Some(e.to_string()),
                });
                if let Some(d) = materialized.cleanup_dir {
                    let _ = fs::remove_dir_all(d);
                }
                continue;
            }
        };
        if mcps.is_empty() {
            summary.broken += 1;
            actions.push(Action {
                source: Some(src.source.clone()),
                skill: Some("mcp".into()),
                status: "broken".into(),
                error: Some(
                    "no MCP JSON files found in source (expected .mcp.json, mcp.json, or mcps/*.json)"
                        .into(),
                ),
            });
            if let Some(d) = materialized.cleanup_dir {
                let _ = fs::remove_dir_all(d);
            }
            continue;
        }
        let mut first_in_run = true;
        for mcp_path in &mcps {
            let file_name = file_name_str(mcp_path);
            let file_name_for_err = file_name.clone();
            let row_first = first_in_run;
            first_in_run = false;
            let r: std::result::Result<(), crate::error::Error> = (|| {
                let hash = hash_file(mcp_path)?;
                let mcp_text = fs::read_to_string(mcp_path)?;
                let mcp_val: serde_json::Value = serde_json::from_str(&mcp_text)?;
                let server_names: Vec<String> = mcp_val
                    .get("mcpServers")
                    .and_then(|v| v.as_object())
                    .map(|m| m.keys().cloned().collect())
                    .unwrap_or_default();

                let asset_id = format!("mcp::{}::{}", src.source, file_name);
                desired_mcp_ids.insert(asset_id.clone());

                let existing = lock.get_tracked_asset("mcp", &asset_id);
                let is_unchanged = existing
                    .as_ref()
                    .map(|(h, _)| {
                        h == &hash
                            && mcp_settings_list
                                .iter()
                                .all(|target| servers_present_in_settings(&server_names, target))
                    })
                    .unwrap_or(false);

                if is_unchanged {
                    let label = sync_label_with(&file_name, &src.source, ctx.plain, row_first);
                    with_spinner_transient(ctx.animate, ctx.plain, &label, || {
                        summary.unchanged += 1;
                        actions.push(Action {
                            source: Some(src.source.clone()),
                            skill: Some(format!("mcp:{file_name}")),
                            status: "unchanged".into(),
                            error: None,
                        });
                        Ok(())
                    })?;
                } else {
                    pending.push(PendingMcp {
                        source: src.source.clone(),
                        file_name,
                        mcp_path: mcp_path.clone(),
                        hash,
                        server_names,
                        asset_id,
                        is_new: existing.is_none(),
                    });
                }
                Ok(())
            })();
            if let Err(e) = r {
                summary.broken += 1;
                actions.push(Action {
                    source: Some(src.source.clone()),
                    skill: Some(format!("mcp:{file_name_for_err}")),
                    status: "broken".into(),
                    error: Some(e.to_string()),
                });
            }
        }
        // Defer cleanup so mcp_path references remain valid
        if let Some(d) = materialized.cleanup_dir {
            cleanup_dirs.push(d);
        }
    }

    // Phase 2: apply all pending installs and updates
    apply_pending(ctx, lock, summary, actions, &mcp_settings_list, &pending)?;
    cleanup_staged(&cleanup_dirs);

    // Remove MCP servers no longer in config
    remove_stale(
        ctx,
        lock,
        summary,
        actions,
        &desired_mcp_ids,
        &mcp_settings_list,
    );

    Ok(())
}

/// Desired MCP file names for a source, derived without any network access.
/// - `List`: predicted file name per entry (`"{name}.json"`). If a prediction
///   doesn't match a lock entry, `needs_fetch_mcps` returns true (safe fallback).
/// - `Wildcard("*")`: the file names of lock mcp-assets for this source.
/// - other wildcard values: empty (broken-value handling stays on the fetch path).
fn desired_mcp_file_names(src: &crate::model::McpSourceSpec, lock: &LockFile) -> Vec<String> {
    match &src.mcps {
        McpsField::List(entries) => entries
            .iter()
            .map(|e| {
                let name = match e {
                    crate::model::McpEntry::Name(n) => n.clone(),
                    crate::model::McpEntry::Obj { name, .. } => name.clone(),
                };
                format!("{name}.json")
            })
            .collect(),
        McpsField::Wildcard(s) if s == "*" => lock
            .assets
            .values()
            .filter(|a| a.kind == "mcp" && a.source == src.source)
            .map(|a| a.name.clone())
            .collect(),
        McpsField::Wildcard(_) => Vec::new(),
    }
}

/// Per-source fetch decision (computed before any download). Fetch when a
/// wildcard source has never been resolved, when any desired MCP file lacks a
/// lock entry, or when the locked servers are not all present in every target.
fn needs_fetch_mcps(
    _ctx: &SyncContext,
    src: &crate::model::McpSourceSpec,
    desired_file_names: &[String],
    lock: &LockFile,
    mcp_settings_list: &[McpSettingsTarget],
) -> bool {
    // A wildcard source with no lock mcp-asset has never been resolved.
    if matches!(&src.mcps, McpsField::Wildcard(s) if s == "*")
        && !lock
            .assets
            .values()
            .any(|a| a.kind == "mcp" && a.source == src.source)
    {
        return true;
    }
    for file_name in desired_file_names {
        let asset_id = format!("mcp::{}::{}", src.source, file_name);
        let Some((_, servers_csv)) = lock.get_tracked_asset("mcp", &asset_id) else {
            return true;
        };
        let server_names: Vec<String> = servers_csv
            .split(',')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        let all_present = mcp_settings_list
            .iter()
            .all(|target| servers_present_in_settings(&server_names, target));
        if !all_present {
            return true;
        }
    }
    false
}

/// `--locked` validation: every config-listed MCP file must have a lock entry,
/// and a wildcard source must contribute at least one locked mcp-asset.
fn ensure_locked_satisfiable_mcps(
    src: &crate::model::McpSourceSpec,
    desired_file_names: &[String],
    lock: &LockFile,
) -> Result<()> {
    match &src.mcps {
        McpsField::List(_) => {
            for file_name in desired_file_names {
                let asset_id = format!("mcp::{}::{}", src.source, file_name);
                if lock.get_tracked_asset("mcp", &asset_id).is_none() {
                    return Err(err(format!(
                        "--locked: MCP `{file_name}` from `{}` is not in the lock",
                        src.source
                    )));
                }
            }
            Ok(())
        }
        McpsField::Wildcard(_) => {
            let present = lock
                .assets
                .values()
                .any(|a| a.kind == "mcp" && a.source == src.source);
            if present {
                Ok(())
            } else {
                Err(err(format!(
                    "--locked: source `{}` has no MCP entries in the lock",
                    src.source
                )))
            }
        }
    }
}

fn apply_pending(
    ctx: &SyncContext,
    lock: &mut LockFile,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
    mcp_settings_list: &[crate::model::McpSettingsTarget],
    pending: &[PendingMcp],
) -> Result<()> {
    let mut last_source = String::new();
    for p in pending {
        let first_in_run = p.source != last_source;
        last_source = p.source.clone();
        let label = sync_label_with(&p.file_name, &p.source, ctx.plain, first_in_run);
        with_spinner_transient(ctx.animate, ctx.plain, &label, || {
            let status = if !p.is_new {
                if ctx.dry_run {
                    "would_update"
                } else {
                    "updated"
                }
            } else if ctx.dry_run {
                "would_install"
            } else {
                "installed"
            };

            if !ctx.dry_run {
                for target in mcp_settings_list {
                    merge_mcp_config(&p.mcp_path, target)?;
                }
                let servers_csv = p.server_names.join(",");
                lock.save_tracked_asset(
                    "mcp",
                    &p.asset_id,
                    &p.file_name,
                    &p.hash,
                    &p.source,
                    &servers_csv,
                );
            }

            if status.contains("install") {
                summary.installed += 1;
            } else {
                summary.updated += 1;
            }
            actions.push(Action {
                source: Some(p.source.clone()),
                skill: Some(format!("mcp:{}", p.file_name)),
                status: status.into(),
                error: None,
            });
            Ok(())
        })?;
    }
    Ok(())
}

fn cleanup_staged(dirs: &[PathBuf]) {
    for d in dirs {
        let _ = fs::remove_dir_all(d);
    }
}

fn remove_stale(
    ctx: &SyncContext,
    lock: &mut LockFile,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
    desired_mcp_ids: &HashSet<String>,
    mcp_settings_list: &[crate::model::McpSettingsTarget],
) {
    let existing_mcps: Vec<(String, String)> = lock
        .list_tracked_asset_ids("mcp")
        .iter()
        .map(|(id, dest)| (id.to_string(), dest.to_string()))
        .collect();
    let servers_by_id: std::collections::HashMap<String, String> =
        existing_mcps.iter().cloned().collect();
    let candidates: Vec<StaleEntry> = existing_mcps
        .into_iter()
        .map(|(id, _)| {
            let mcp_name = id.rsplit("::").next().unwrap_or(&id).to_string();
            StaleEntry {
                id,
                action_source: None,
                action_skill: format!("mcp:{mcp_name}"),
            }
        })
        .collect();

    remove_stale_shared(
        ctx.dry_run,
        summary,
        actions,
        desired_mcp_ids,
        candidates,
        |id| {
            if let Some(servers_csv) = servers_by_id.get(id) {
                for target in mcp_settings_list {
                    for server_name in servers_csv.split(',').filter(|s| !s.is_empty()) {
                        let _ = remove_mcp_server(server_name, target);
                    }
                }
            }
            lock.remove_tracked_asset(id);
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_mcp_classification_new_vs_update() {
        let new_entry = PendingMcp {
            source: "https://github.com/org/pack".into(),
            file_name: "mcp.json".into(),
            mcp_path: PathBuf::from("/tmp/mcp.json"),
            hash: "abc123".into(),
            server_names: vec!["server-a".into(), "server-b".into()],
            asset_id: "mcp::source::mcp.json".into(),
            is_new: true,
        };
        let update_entry = PendingMcp {
            source: "https://github.com/org/pack".into(),
            file_name: "other.json".into(),
            mcp_path: PathBuf::from("/tmp/other.json"),
            hash: "def456".into(),
            server_names: vec!["server-c".into()],
            asset_id: "mcp::source::other.json".into(),
            is_new: false,
        };

        let pending = [new_entry, update_entry];
        let new_servers: Vec<&PendingMcp> = pending.iter().filter(|p| p.is_new).collect();

        assert_eq!(new_servers.len(), 1);
        assert_eq!(new_servers[0].server_names, vec!["server-a", "server-b"]);

        let all_names: Vec<&str> = new_servers
            .iter()
            .flat_map(|p| p.server_names.iter().map(|s| s.as_str()))
            .collect();
        assert_eq!(all_names, vec!["server-a", "server-b"]);
    }

    #[test]
    fn pending_mcp_no_new_servers_skips_gate() {
        let update_only = [PendingMcp {
            source: "https://github.com/org/pack".into(),
            file_name: "mcp.json".into(),
            mcp_path: PathBuf::from("/tmp/mcp.json"),
            hash: "abc123".into(),
            server_names: vec!["existing-server".into()],
            asset_id: "mcp::source::mcp.json".into(),
            is_new: false,
        }];

        let new_servers: Vec<&PendingMcp> = update_only.iter().filter(|p| p.is_new).collect();
        assert!(
            new_servers.is_empty(),
            "updates should not trigger the gate"
        );
    }

    #[test]
    fn needs_fetch_mcps_true_when_asset_absent_false_when_satisfied() {
        use crate::model::{McpEntry, McpSourceSpec, McpsField};

        let src = McpSourceSpec {
            source: "https://github.com/org/pack".into(),
            branch: None,
            git_ref: None,
            mcps: McpsField::List(vec![McpEntry::Name("foo".into())]),
        };
        // Desired file name predicted from the entry name.
        let desired = vec!["foo.json".to_string()];

        // No lock entry -> must fetch.
        let lock = LockFile::default();
        let no_targets: Vec<McpSettingsTarget> = Vec::new();
        // SyncContext is not needed by needs_fetch_mcps (ignored arg); build a minimal one.
        let cfg = crate::model::Config {
            destination: None,
            scope: Some(crate::model::Scope::Project),
            agent: None,
            skills: Vec::new(),
            mcps: Vec::new(),
            commands: Vec::new(),
        };
        let root = PathBuf::from("/tmp");
        let ctx = SyncContext {
            cfg: &cfg,
            cfg_dir: &root,
            destinations: &[],
            scope_root: root.clone(),
            scope: crate::model::Scope::Project,
            dry_run: false,
            animate: false,
            plain: true,
            as_json: false,
            quiet: true,
            update: false,
            update_only: Vec::new(),
            locked: false,
        };
        assert!(
            needs_fetch_mcps(&ctx, &src, &desired, &lock, &no_targets),
            "absent lock asset forces a fetch"
        );

        // With a lock entry and no targets to satisfy, nothing is unsatisfied.
        let mut lock2 = LockFile::default();
        lock2.save_tracked_asset(
            "mcp",
            "mcp::https://github.com/org/pack::foo.json",
            "foo.json",
            "h1",
            "https://github.com/org/pack",
            "server-a",
        );
        assert!(
            !needs_fetch_mcps(&ctx, &src, &desired, &lock2, &no_targets),
            "present lock asset with no targets needs no fetch"
        );
    }
}
