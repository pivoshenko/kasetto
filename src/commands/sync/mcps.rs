//! Module that contains the `sync` leg that merges, updates, and prunes MCP packs
//! in agent-native settings files.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{err, Result};
use crate::fsops::{
    dirs_home, dirs_kasetto_config, hash_file, now_unix, resolve_mcp_settings_targets,
};
use crate::lock::{AssetEntry, LockFile};
use crate::mcps::{merge_mcp_config, remove_mcp_server, servers_present_in_settings};
use crate::model::{
    all_mcp_project_targets, all_mcp_settings_targets, Action, InlineMcpSpec, McpSettingsTarget,
    McpSpec, McpsField, Scope, Summary, INLINE_SOURCE,
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
    /// The pack's `mcpServers` object, already secret-injected in phase 1.
    servers: serde_json::Map<String, serde_json::Value>,
    hash: String,
    server_names: Vec<String>,
    asset_id: String,
    is_new: bool,
    /// Replace an existing same-named server on merge (the `--update` rotation
    /// path for secret-bearing packs).
    overwrite: bool,
    /// Pack carries `${kst...}` placeholders, recorded in the lock so the skip
    /// path can hint that rotation needs `--update`, and used to perms-check the
    /// destination after a plaintext secret is written.
    has_secrets: bool,
    source_revision: String,
}

impl PendingMcp {
    fn new(
        source: &str,
        file_name: String,
        servers: serde_json::Map<String, serde_json::Value>,
        hash: String,
        source_revision: &str,
    ) -> Result<Self> {
        Ok(Self {
            asset_id: format!("mcp::{source}::{file_name}"),
            server_names: servers.keys().cloned().collect(),
            has_secrets: crate::secrets::has_placeholder(&serde_json::to_string(&servers)?),
            source: source.into(),
            file_name,
            servers,
            hash,
            is_new: false,
            overwrite: false,
            source_revision: source_revision.into(),
        })
    }

    fn lock_entry(&self) -> AssetEntry {
        AssetEntry {
            kind: "mcp".into(),
            name: self.file_name.clone(),
            hash: self.hash.clone(),
            source: self.source.clone(),
            destination: self.server_names.join(","),
            source_revision: self.source_revision.clone(),
            has_secrets: self.has_secrets,
        }
    }
}

/// Returns `true` when a secret-bearing pack was left unchanged without
/// `--update`, so the caller can hint (after the summary) that a rotated secret
/// won't propagate on a plain sync.
pub(super) fn sync_mcps(
    ctx: &SyncContext,
    lock: &mut LockFile,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
) -> Result<bool> {
    let mut desired_mcp_ids = HashSet::new();
    // Set when a secret-bearing pack is left unchanged without `--update`, so we
    // can hint that a rotated secret won't propagate on a plain sync
    let mut secrets_need_update = false;
    let mcp_settings_list = resolve_mcp_settings_targets(ctx.cfg, ctx.scope, ctx.cfg_dir)?;

    // No configured agent has a native MCP target (e.g. no `agent:` or Pi only).
    // Config is source of truth, so scrub any previously locked MCPs from every
    // known target as best-effort, prune the lock, and return without fetching
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
        return Ok(false);
    }

    // Phase 1: discover and classify all MCP entries
    let mut pending: Vec<PendingMcp> = Vec::new();
    let mut cleanup_dirs: Vec<PathBuf> = Vec::new();

    for (i, spec) in ctx.cfg.mcps.iter().enumerate() {
        let src = match spec {
            McpSpec::Source(src) => src,
            McpSpec::Inline(inline) => {
                let entry = inline_pending(inline)?;
                desired_mcp_ids.insert(entry.asset_id.clone());
                let (status, error) = if ctx.locked
                    && lock
                        .assets
                        .get(&entry.asset_id)
                        .is_none_or(|a| a.hash != entry.hash)
                {
                    (
                        "locked_error",
                        Some(
                            "inline MCP definition does not match the lock; run sync without --locked"
                                .to_string(),
                        ),
                    )
                } else {
                    match classify_inline_mcp(ctx, lock, &mcp_settings_list, entry)? {
                        McpFileOutcome::Install(p) => {
                            pending.push(*p);
                            continue;
                        }
                        McpFileOutcome::Unchanged { has_secrets } => {
                            secrets_need_update |= has_secrets;
                            ("unchanged", None)
                        }
                        McpFileOutcome::SecretError(e) => ("source_error", Some(e)),
                    }
                };
                if error.is_some() {
                    summary.failed += 1;
                } else {
                    summary.unchanged += 1;
                }
                actions.push(Action {
                    source: Some(INLINE_SOURCE.into()),
                    skill: Some(format!("mcp:{}", inline.name)),
                    status: status.into(),
                    error,
                });
                continue;
            }
        };
        // Desired MCP file names for this source, derived without any network:
        // predicted file names for a list, or the locked set for a wildcard
        let desired_file_names = desired_mcp_file_names(src, lock);

        // `--locked`/`--frozen`: the lock must be able to satisfy the config
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

        // Whether any file in this source is targeted by `--update`. Drives the
        // fetch decision (a moving source must be re-downloaded once)
        let update_names: Vec<String> = desired_file_names
            .iter()
            .flat_map(|f| update_aliases(f))
            .collect();
        let update_active = update_active_for_source(ctx, &src.source, &update_names);
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
            // Skip path: no network. Honor each desired MCP file from the lock
            let mut first_in_run = true;
            for file_name in &desired_file_names {
                let asset_id = format!("mcp::{}::{}", src.source, file_name);
                if lock.assets.get(&asset_id).is_some_and(|a| a.has_secrets) {
                    secrets_need_update = true;
                }
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
        // Resolve MCP files against the materialized source root. `source_root`
        // is correct for every case: a local path, a freshly-staged remote, and
        // a cache-served `ref:` source (which has no stage dir, so `cleanup_dir`
        // is `None`). `cleanup_dir` is only a teardown handle, never the root
        let root = materialized.source_root.as_path();
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
            let row_first = first_in_run;
            first_in_run = false;
            // Scope `--update <name>` to the file actually named. A source can
            // hold several MCP files; rotating one (force-remerge with overwrite)
            // must not clobber hand-edited servers from the source's other files
            let file_update =
                update_active_for_source(ctx, &src.source, &update_aliases(&file_name));
            let classified = classify_mcp_file(
                ctx,
                lock,
                &mcp_settings_list,
                &src.source,
                mcp_path,
                file_update,
                &materialized.source_revision,
            );
            let (asset_id, outcome) = match classified {
                Ok(c) => c,
                Err(e) => {
                    // A malformed/unreadable file is `broken` (exit 0), distinct
                    // from an unresolved secret below
                    summary.broken += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(format!("mcp:{file_name}")),
                        status: "broken".into(),
                        error: Some(e.to_string()),
                    });
                    continue;
                }
            };
            desired_mcp_ids.insert(asset_id);
            match outcome {
                McpFileOutcome::Unchanged { has_secrets } => {
                    if has_secrets {
                        secrets_need_update = true;
                    }
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
                }
                McpFileOutcome::SecretError(e) => {
                    // Missing required secret: hard failure (non-zero exit),
                    // nothing written, never to a destination, cache, or lock
                    summary.failed += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(format!("mcp:{file_name}")),
                        status: "source_error".into(),
                        error: Some(e),
                    });
                }
                McpFileOutcome::Install(pmcp) => pending.push(*pmcp),
            }
        }
        // Defer cleanup so mcp_path references remain valid
        if let Some(d) = materialized.cleanup_dir {
            cleanup_dirs.push(d);
        }
    }

    // Remove MCP servers no longer in config. Skipped when any source failed
    // (locked_error et al.): `desired_mcp_ids` would be missing the failed
    // source's existing entries and they'd be treated as orphans. Every
    // `summary.failed` bump happens in phase 1, so the count is already final
    //
    // Runs before the merge because asset ids carry the source: a pack that
    // moved to a different `source` retires its old id and claims a new one for
    // the same server names. Clearing the retired names first is what lets the
    // new definitions land, since a merge skips a name that is already present
    if summary.failed == 0 {
        remove_stale(
            ctx,
            lock,
            summary,
            actions,
            &desired_mcp_ids,
            &mcp_settings_list,
        );
    }

    // Phase 2: apply all pending installs and updates
    apply_pending(ctx, lock, summary, actions, &mcp_settings_list, &pending)?;
    cleanup_staged(&cleanup_dirs);

    Ok(secrets_need_update)
}

/// Names a `--update <name>` accepts for an MCP file: the file name itself
/// (`github.json`) and its bare stem (`github`), matching how skills/commands
/// match names.
fn update_aliases(file_name: &str) -> Vec<String> {
    std::iter::once(file_name.to_string())
        .chain(file_name.strip_suffix(".json").map(str::to_string))
        .collect()
}

/// Outcome of classifying one MCP file against the lock and current settings.
enum McpFileOutcome {
    /// Already installed and identical, so nothing to write.
    Unchanged { has_secrets: bool },
    /// A required secret could not be resolved: hard failure, non-zero exit.
    SecretError(String),
    /// Needs install/update, carrying the prepared, secret-injected entry
    /// (boxed to keep the enum small).
    Install(Box<PendingMcp>),
}

/// Hash, parse, and (on the merge path) secret-inject one MCP file, deciding
/// whether it is unchanged or a pending install. Returns the asset id plus the
/// outcome; `Err` means a malformed/unreadable file (the caller marks it
/// broken). Side effects (summary counts, lock writes, desired-id tracking)
/// stay with the caller so this stays a pure classification step.
fn classify_mcp_file(
    ctx: &SyncContext,
    lock: &LockFile,
    mcp_settings_list: &[McpSettingsTarget],
    source: &str,
    mcp_path: &Path,
    update_active: bool,
    source_revision: &str,
) -> Result<(String, McpFileOutcome)> {
    // `update_active` here is scoped to *this* file (see the caller): true only
    // when a plain `--update` ran or `--update <name>` named this file
    let file_name = file_name_str(mcp_path);
    let hash = hash_file(mcp_path)?;
    let mcp_text = fs::read_to_string(mcp_path)?;
    let mcp_val: serde_json::Value = serde_json::from_str(&mcp_text)?;
    let servers: serde_json::Map<String, serde_json::Value> = mcp_val
        .get("mcpServers")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let entry = PendingMcp::new(source, file_name, servers, hash, source_revision)?;
    let asset_id = entry.asset_id.clone();
    let outcome = classify_mcp_servers(ctx, lock, mcp_settings_list, entry, update_active)?;
    Ok((asset_id, outcome))
}

fn classify_inline_mcp(
    ctx: &SyncContext,
    lock: &LockFile,
    mcp_settings_list: &[McpSettingsTarget],
    entry: PendingMcp,
) -> Result<McpFileOutcome> {
    let update_active =
        ctx.update && (ctx.update_only.is_empty() || ctx.update_only.contains(&entry.file_name));
    classify_mcp_servers(ctx, lock, mcp_settings_list, entry, update_active)
}

fn inline_pending(inline: &InlineMcpSpec) -> Result<PendingMcp> {
    PendingMcp::new(
        INLINE_SOURCE,
        inline.name.clone(),
        inline.servers.clone(),
        inline_hash(&inline.servers)?,
        INLINE_SOURCE,
    )
}

/// The lock entry a sync records for an inline MCP, so `kst lock` can pin it without syncing.
pub(crate) fn inline_mcp_lock_asset(inline: &InlineMcpSpec) -> Result<(String, AssetEntry)> {
    let pending = inline_pending(inline)?;
    Ok((pending.asset_id.clone(), pending.lock_entry()))
}

fn inline_hash(servers: &serde_json::Map<String, serde_json::Value>) -> Result<String> {
    // Canonical because serde_json maps are key-sorted while `preserve_order` stays off
    Ok(crate::fsops::hash_str(&serde_json::to_string(servers)?))
}

fn classify_mcp_servers(
    ctx: &SyncContext,
    lock: &LockFile,
    mcp_settings_list: &[McpSettingsTarget],
    mut pending: PendingMcp,
    update_active: bool,
) -> Result<McpFileOutcome> {
    let existing = lock.get_tracked_asset("mcp", &pending.asset_id);
    // A secret-bearing entry under `--update` is re-merged even when unchanged,
    // so a secret rotated only in env/credentials.yaml propagates
    pending.overwrite = update_active && pending.has_secrets;
    let is_unchanged = !pending.overwrite
        && existing.as_ref().is_some_and(|(hash, _)| {
            hash == &pending.hash
                && mcp_settings_list
                    .iter()
                    .all(|target| servers_present_in_settings(&pending.server_names, target))
        });
    if is_unchanged {
        return Ok(McpFileOutcome::Unchanged {
            has_secrets: pending.has_secrets,
        });
    }
    if pending.has_secrets {
        let mut wrap = serde_json::Value::Object(std::mem::take(&mut pending.servers));
        if let Err(e) = ctx.secrets.inject_value(&mut wrap) {
            return Ok(McpFileOutcome::SecretError(e.to_string()));
        }
        if let serde_json::Value::Object(servers) = wrap {
            pending.servers = servers;
        }
    }
    pending.is_new = existing.is_none();
    Ok(McpFileOutcome::Install(Box::new(pending)))
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
    // A wildcard source with no lock mcp-asset has never been resolved
    if matches!(&src.mcps, McpsField::Wildcard(s) if s == "*")
        && !lock
            .assets
            .values()
            .any(|a| a.kind == "mcp" && a.source == src.source)
    {
        return true;
    }
    let expected_revision = src.as_source_spec().expected_revision();
    for file_name in desired_file_names {
        let asset_id = format!("mcp::{}::{}", src.source, file_name);
        let Some(asset) = lock.assets.get(&asset_id).filter(|a| a.kind == "mcp") else {
            return true;
        };
        // Retargeted source (ref/branch changed since the lock was written)
        if !asset.source_revision.is_empty() && asset.source_revision != expected_revision {
            return true;
        }
        let server_names: Vec<String> = asset
            .destination
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
                    merge_mcp_config(&p.servers, target, p.overwrite)?;
                    // A resolved plaintext secret now lives in this file; warn if
                    // it is group/world-readable (symmetric to credentials.yaml)
                    if p.has_secrets {
                        crate::secrets::warn_if_world_readable(&target.path, ctx.plain);
                    }
                }
                lock.save_tracked_asset(&p.asset_id, p.lock_entry());
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
        .into_iter()
        .map(|(id, dest)| (id.to_owned(), dest.to_owned()))
        .collect();
    let servers_by_id: std::collections::HashMap<&str, &str> = existing_mcps
        .iter()
        .map(|(id, dest)| (id.as_str(), dest.as_str()))
        .collect();
    let candidates: Vec<StaleEntry> = existing_mcps
        .iter()
        .map(|(id, _)| {
            let mcp_name = id.rsplit("::").next().unwrap_or(id);
            StaleEntry {
                id: id.clone(),
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
    use crate::fsops::temp_dir;
    use crate::model::{Agent, AgentField, Config, McpSourceSpec};

    fn mcp_cfg(src_root: &Path, agents: Vec<Agent>) -> Config {
        Config {
            destination: None,
            scope: Some(Scope::Project),
            agent: Some(AgentField::Many(agents)),
            skills: Vec::new(),
            mcps: vec![McpSpec::Source(McpSourceSpec {
                source: src_root.to_string_lossy().to_string(),
                branch: None,
                git_ref: None,
                mcps: McpsField::Wildcard("*".into()),
            })],
            commands: Vec::new(),
            instructions: Vec::new(),
            secrets: None,
        }
    }

    fn project_ctx<'a>(cfg: &'a Config, project: &'a PathBuf) -> SyncContext<'a> {
        SyncContext {
            cfg,
            cfg_dir: project,
            destinations: std::slice::from_ref(project),
            scope_root: project.clone(),
            scope: Scope::Project,
            dry_run: false,
            animate: false,
            plain: true,
            update: false,
            update_only: Vec::new(),
            update_local: false,
            locked: false,
            secrets: crate::secrets::SecretContext::empty(),
        }
    }

    fn project_mcp_settings(project: &Path) -> serde_json::Value {
        serde_json::from_str(&fs::read_to_string(project.join(".mcp.json")).unwrap()).unwrap()
    }

    fn inline_cfg() -> Config {
        serde_yaml::from_str("scope: project\nagent: claude-code\nmcps:\n  - name: laptop\n    servers: {gitea: {type: http, url: 'https://example.com/mcp'}}\n").unwrap()
    }

    fn run_sync(ctx: &SyncContext, lock: &mut LockFile) -> Summary {
        let mut summary = Summary::default();
        sync_mcps(ctx, lock, &mut summary, &mut Vec::new()).unwrap();
        summary
    }

    #[test]
    fn inline_targeted_update_rotates_secrets_without_changing_lock_hash() {
        let project = temp_dir("kasetto-inline-secrets");
        fs::create_dir_all(&project).unwrap();
        let cfg: Config = serde_yaml::from_str(
            "scope: project\nagent: claude-code\nsecrets:\n  files: [fixture.yaml]\nmcps:\n  - name: laptop\n    servers: {gitea: {url: 'https://example.com/mcp', headers: {Authorization: 'Bearer ${kst_inline_test_token}'}}}\n  - name: sibling\n    servers: {other: {command: original}}\n",
        ).unwrap();
        let creds = project.join("fixture.yaml");
        let mut lock = LockFile::default();
        let mut ctx = project_ctx(&cfg, &project);
        let mut sync_with_token = |ctx: &mut SyncContext, token: &str| {
            fs::write(&creds, format!("inline_test_token: {token}\n")).unwrap();
            ctx.secrets = crate::secrets::SecretContext::from_config(
                cfg.secrets.as_ref(),
                &project,
                false,
                true,
            )
            .unwrap();
            let mut summary = Summary::default();
            let needs_update = sync_mcps(ctx, &mut lock, &mut summary, &mut Vec::new()).unwrap();
            (needs_update, summary)
        };
        sync_with_token(&mut ctx, "fixture-first");
        let mut settings = project_mcp_settings(&project);
        settings["mcpServers"]["other"]["command"] = "hand-edited".into();
        fs::write(
            project.join(".mcp.json"),
            serde_json::to_string(&settings).unwrap(),
        )
        .unwrap();

        let (needs_update, summary) = sync_with_token(&mut ctx, "fixture-second");
        assert!(needs_update);
        assert_eq!(summary.unchanged, 2);
        let auth = |project: &Path| {
            project_mcp_settings(project)["mcpServers"]["gitea"]["headers"]["Authorization"].clone()
        };
        assert_eq!(auth(&project), "Bearer fixture-first");

        ctx.update = true;
        ctx.update_only = vec!["laptop".into()];
        let (_, summary) = sync_with_token(&mut ctx, "fixture-second");
        assert_eq!((summary.updated, summary.unchanged), (1, 1));
        assert_eq!(auth(&project), "Bearer fixture-second");
        assert_eq!(
            project_mcp_settings(&project)["mcpServers"]["other"]["command"],
            "hand-edited"
        );
        let McpSpec::Inline(inline) = &cfg.mcps[0] else {
            unreachable!()
        };
        let asset = &lock.assets["mcp::<inline>::laptop"];
        assert_eq!(asset.hash, inline_hash(&inline.servers).unwrap());
        assert!(asset.has_secrets);
        let lock_json = serde_json::to_string(&lock).unwrap();
        assert!(!lock_json.contains("fixture-first") && !lock_json.contains("fixture-second"));
        fs::remove_dir_all(project).unwrap();
    }

    #[test]
    fn inline_missing_secret_blocks_install_and_orphan_cleanup() {
        let project = temp_dir("kasetto-inline-missing-secret");
        let mut cfg = inline_cfg();
        let mut lock = LockFile::default();
        run_sync(&project_ctx(&cfg, &project), &mut lock);
        let original_settings = fs::read(project.join(".mcp.json")).unwrap();
        let McpSpec::Inline(inline) = &mut cfg.mcps[0] else {
            unreachable!()
        };
        inline.name = "replacement".into();
        inline.servers = serde_json::from_str(r#"{"new":{"command":"${kst_missing}"}}"#).unwrap();
        assert_eq!(run_sync(&project_ctx(&cfg, &project), &mut lock).failed, 1);
        assert!(lock.assets.contains_key("mcp::<inline>::laptop"));
        assert!(!lock.assets.contains_key("mcp::<inline>::replacement"));
        assert_eq!(
            fs::read(project.join(".mcp.json")).unwrap(),
            original_settings
        );
        fs::remove_dir_all(project).unwrap();
    }

    #[test]
    fn inline_hash_sorts_nested_objects_and_preserves_arrays() {
        let hash = |json: &str| inline_hash(&serde_json::from_str(json).unwrap()).unwrap();
        let a = hash(r#"{"z":{"b":2,"a":1},"a":[{"y":0,"x":"${kst_token}"},2]}"#);
        assert_eq!(
            a,
            hash(r#"{"a":[{"x":"${kst_token}","y":0},2],"z":{"a":1,"b":2}}"#)
        );
        assert_ne!(
            a,
            hash(r#"{"a":[2,{"x":"${kst_token}","y":0}],"z":{"a":1,"b":2}}"#)
        );
    }

    #[test]
    fn inline_install_resync_locked_restore_rename_and_prune() {
        let project = temp_dir("kasetto-inline-lifecycle");
        let mut cfg = inline_cfg();
        let mut lock = LockFile::default();
        let has_gitea = |project: &Path| {
            project_mcp_settings(project)["mcpServers"]
                .get("gitea")
                .is_some()
        };
        assert_eq!(
            run_sync(&project_ctx(&cfg, &project), &mut lock).installed,
            1
        );
        assert!(has_gitea(&project));
        assert_eq!(
            lock.assets["mcp::<inline>::laptop"].source_revision,
            INLINE_SOURCE
        );

        let mut ctx = project_ctx(&cfg, &project);
        ctx.locked = true;
        assert_eq!(run_sync(&ctx, &mut lock).unchanged, 1);
        fs::remove_file(project.join(".mcp.json")).unwrap();
        assert_eq!(run_sync(&ctx, &mut lock).updated, 1);

        let McpSpec::Inline(inline) = &mut cfg.mcps[0] else {
            unreachable!()
        };
        inline.name = "renamed".into();
        assert_eq!(run_sync(&project_ctx(&cfg, &project), &mut lock).removed, 1);
        assert!(lock.assets.contains_key("mcp::<inline>::renamed"));
        assert!(has_gitea(&project));

        cfg.mcps.clear();
        assert_eq!(run_sync(&project_ctx(&cfg, &project), &mut lock).removed, 1);
        assert!(lock.assets.is_empty());
        assert!(!has_gitea(&project));
        fs::remove_dir_all(project).unwrap();
    }

    #[test]
    fn inline_edits_keep_an_installed_server_like_packs() {
        let project = temp_dir("kasetto-inline-edit");
        let mut cfg = inline_cfg();
        let mut lock = LockFile::default();
        run_sync(&project_ctx(&cfg, &project), &mut lock);
        let McpSpec::Inline(inline) = &mut cfg.mcps[0] else {
            unreachable!()
        };
        inline.servers["gitea"]["url"] = "https://example.com/v2".into();
        assert_eq!(run_sync(&project_ctx(&cfg, &project), &mut lock).updated, 1);
        assert_eq!(
            project_mcp_settings(&project)["mcpServers"]["gitea"]["url"],
            "https://example.com/mcp"
        );
        fs::remove_dir_all(project).unwrap();
    }

    #[test]
    fn inline_lock_asset_matches_sync_and_satisfies_locked_without_a_prior_sync() {
        let project = temp_dir("kasetto-inline-lock-asset");
        let cfg = inline_cfg();
        let McpSpec::Inline(inline) = &cfg.mcps[0] else {
            unreachable!()
        };
        let (id, asset) = inline_mcp_lock_asset(inline).unwrap();
        let mut lock = LockFile::default();
        lock.save_tracked_asset(&id, asset);
        let pinned = serde_json::to_value(&lock).unwrap();

        let mut ctx = project_ctx(&cfg, &project);
        ctx.locked = true;
        let summary = run_sync(&ctx, &mut lock);
        assert_eq!((summary.updated, summary.failed), (1, 0));
        assert!(project_mcp_settings(&project)["mcpServers"]
            .get("gitea")
            .is_some());
        assert_eq!(serde_json::to_value(&lock).unwrap(), pinned);
        fs::remove_dir_all(project).unwrap();
    }

    #[test]
    fn inline_locked_rejects_missing_or_changed_hash_without_pruning() {
        let project = temp_dir("kasetto-inline-locked");
        let cfg = inline_cfg();
        let mut lock = LockFile::default();
        let mut ctx = project_ctx(&cfg, &project);
        ctx.locked = true;
        assert_eq!(run_sync(&ctx, &mut lock).failed, 1);
        assert!(!project.join(".mcp.json").exists());
        ctx.locked = false;
        run_sync(&ctx, &mut lock);
        lock.assets.get_mut("mcp::<inline>::laptop").unwrap().hash = "different".into();
        ctx.locked = true;
        assert_eq!(run_sync(&ctx, &mut lock).failed, 1);
        assert!(lock.assets.contains_key("mcp::<inline>::laptop"));
        fs::remove_dir_all(project).unwrap();
    }

    #[test]
    fn pending_mcp_classification_new_vs_update() {
        let new_entry = PendingMcp {
            source: "https://github.com/org/pack".into(),
            file_name: "mcp.json".into(),
            servers: serde_json::Map::new(),
            hash: "abc123".into(),
            server_names: vec!["server-a".into(), "server-b".into()],
            asset_id: "mcp::source::mcp.json".into(),
            is_new: true,
            overwrite: false,
            has_secrets: false,
            source_revision: "branch:main".into(),
        };
        let update_entry = PendingMcp {
            source: "https://github.com/org/pack".into(),
            file_name: "other.json".into(),
            servers: serde_json::Map::new(),
            hash: "def456".into(),
            server_names: vec!["server-c".into()],
            asset_id: "mcp::source::other.json".into(),
            is_new: false,
            overwrite: false,
            has_secrets: false,
            source_revision: "branch:main".into(),
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
            servers: serde_json::Map::new(),
            hash: "abc123".into(),
            server_names: vec!["existing-server".into()],
            asset_id: "mcp::source::mcp.json".into(),
            is_new: false,
            overwrite: false,
            has_secrets: false,
            source_revision: "branch:main".into(),
        }];

        assert!(
            !update_only.iter().any(|p| p.is_new),
            "updates should not trigger the gate"
        );
    }

    #[test]
    fn reconfig_from_mixed_pi_to_pi_only_prunes_installed_mcp() {
        let src_root = temp_dir("kasetto-mcp-pi-src");
        fs::create_dir_all(&src_root).unwrap();
        fs::write(
            src_root.join("mcp.json"),
            r#"{"mcpServers":{"managed":{"command":"echo"}}}"#,
        )
        .unwrap();
        let project = temp_dir("kasetto-mcp-pi-project");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join(".mcp.json"),
            r#"{"mcpServers":{"user-owned":{"command":"keep"}}}"#,
        )
        .unwrap();

        let mixed_cfg = mcp_cfg(&src_root, vec![Agent::Pi, Agent::ClaudeCode]);
        let mut lock = LockFile::default();
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        sync_mcps(
            &project_ctx(&mixed_cfg, &project),
            &mut lock,
            &mut summary,
            &mut actions,
        )
        .unwrap();

        assert_eq!(summary.installed, 1);
        let settings = project_mcp_settings(&project);
        assert!(settings["mcpServers"]["managed"].is_object());
        assert!(settings["mcpServers"]["user-owned"].is_object());
        assert_eq!(lock.assets.values().filter(|a| a.kind == "mcp").count(), 1);

        // Pi has no MCP target, so this transition must prune the prior managed
        // server without reading the still-configured source or touching user data
        fs::remove_dir_all(&src_root).unwrap();
        let pi_cfg = mcp_cfg(&src_root, vec![Agent::Pi]);
        let mut summary2 = Summary::default();
        let mut actions2 = Vec::new();
        sync_mcps(
            &project_ctx(&pi_cfg, &project),
            &mut lock,
            &mut summary2,
            &mut actions2,
        )
        .unwrap();

        assert_eq!(summary2.removed, 1);
        let settings = project_mcp_settings(&project);
        assert!(settings["mcpServers"].get("managed").is_none());
        assert!(settings["mcpServers"]["user-owned"].is_object());
        assert_eq!(lock.assets.values().filter(|a| a.kind == "mcp").count(), 0);

        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn moving_a_pack_to_another_source_keeps_its_servers_installed() {
        let old_src = temp_dir("kasetto-mcp-move-old");
        fs::create_dir_all(old_src.join("mcps")).unwrap();
        fs::write(
            old_src.join("mcps/pack.json"),
            r#"{"mcpServers":{"alpha":{"command":"a"},"bravo":{"command":"b"}}}"#,
        )
        .unwrap();
        let project = temp_dir("kasetto-mcp-move-project");
        fs::create_dir_all(&project).unwrap();

        let mut lock = LockFile::default();
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        sync_mcps(
            &project_ctx(&mcp_cfg(&old_src, vec![Agent::ClaudeCode]), &project),
            &mut lock,
            &mut summary,
            &mut actions,
        )
        .unwrap();
        assert_eq!(summary.installed, 1);

        // Same file name and server names under a different `source`, plus a new
        // server. The old source's asset id is retired, but its servers are not
        let new_src = temp_dir("kasetto-mcp-move-new");
        fs::create_dir_all(new_src.join("mcps")).unwrap();
        fs::write(
            new_src.join("mcps/pack.json"),
            r#"{"mcpServers":{"alpha":{"command":"NEW"},"bravo":{"command":"b"},"echo":{"command":"e"}}}"#,
        )
        .unwrap();

        let mut summary2 = Summary::default();
        let mut actions2 = Vec::new();
        sync_mcps(
            &project_ctx(&mcp_cfg(&new_src, vec![Agent::ClaudeCode]), &project),
            &mut lock,
            &mut summary2,
            &mut actions2,
        )
        .unwrap();

        assert_eq!(summary2.removed, 1, "the old source's lock entry is pruned");
        let settings = project_mcp_settings(&project);
        for name in ["alpha", "bravo", "echo"] {
            assert!(
                settings["mcpServers"][name].is_object(),
                "{name} must survive the source move"
            );
        }
        // The new source redefines `alpha`, so the move must also carry the new
        // definition over, not leave the retired source's copy in place
        assert_eq!(settings["mcpServers"]["alpha"]["command"], "NEW");
        let mcp_assets: Vec<&str> = lock
            .assets
            .iter()
            .filter(|(_, a)| a.kind == "mcp")
            .map(|(id, _)| id.as_str())
            .collect();
        assert_eq!(mcp_assets.len(), 1);
        assert!(mcp_assets[0].contains(&new_src.to_string_lossy().to_string()));

        let _ = fs::remove_dir_all(&old_src);
        let _ = fs::remove_dir_all(&new_src);
        let _ = fs::remove_dir_all(&project);
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
        // Desired file name predicted from the entry name
        let desired = vec!["foo.json".to_string()];

        // No lock entry -> must fetch
        let lock = LockFile::default();
        let no_targets: Vec<McpSettingsTarget> = Vec::new();
        // SyncContext is not needed by needs_fetch_mcps (ignored arg); build a minimal one
        let cfg = crate::model::Config {
            destination: None,
            scope: Some(crate::model::Scope::Project),
            agent: None,
            skills: Vec::new(),
            mcps: Vec::new(),
            commands: Vec::new(),
            instructions: Vec::new(),
            secrets: None,
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
            update: false,
            update_only: Vec::new(),
            update_local: false,
            locked: false,
            secrets: crate::secrets::SecretContext::empty(),
        };
        assert!(
            needs_fetch_mcps(&ctx, &src, &desired, &lock, &no_targets),
            "absent lock asset forces a fetch"
        );

        // With a lock entry and no targets to satisfy, nothing is unsatisfied
        let mut lock2 = LockFile::default();
        lock2.save_tracked_asset(
            "mcp::https://github.com/org/pack::foo.json",
            crate::lock::AssetEntry {
                kind: "mcp".into(),
                name: "foo.json".into(),
                hash: "h1".into(),
                source: "https://github.com/org/pack".into(),
                destination: "server-a".into(),
                source_revision: "branch:main".into(),
                has_secrets: false,
            },
        );
        assert!(
            !needs_fetch_mcps(&ctx, &src, &desired, &lock2, &no_targets),
            "present lock asset with no targets needs no fetch"
        );
    }

    #[test]
    fn selective_update_is_scoped_per_file_not_per_source() {
        // `--update vercel` against a source holding both vercel.json and
        // notion.json must force-remerge only vercel.json; remerging notion.json
        // (overwrite) would clobber hand-edited servers from a sibling file
        let cfg = crate::model::Config {
            destination: None,
            scope: Some(crate::model::Scope::Project),
            agent: None,
            skills: Vec::new(),
            mcps: Vec::new(),
            commands: Vec::new(),
            instructions: Vec::new(),
            secrets: None,
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
            update: true,
            update_only: vec!["vercel".into()],
            update_local: false,
            locked: false,
            secrets: crate::secrets::SecretContext::empty(),
        };

        // Source-level: the source is targeted (vercel matches), so a fetch happens
        let src = "https://github.com/org/pack";
        let source_names: Vec<String> = ["vercel.json", "notion.json"]
            .iter()
            .flat_map(|f| update_aliases(f))
            .collect();
        assert!(update_active_for_source(&ctx, src, &source_names));

        // File-level: only vercel.json is active; notion.json is not
        assert!(update_active_for_source(
            &ctx,
            src,
            &update_aliases("vercel.json")
        ));
        assert!(!update_active_for_source(
            &ctx,
            src,
            &update_aliases("notion.json")
        ));
    }

    #[test]
    fn update_local_activates_local_sources_only() {
        let cfg = crate::model::Config {
            destination: None,
            scope: Some(crate::model::Scope::Project),
            agent: None,
            skills: Vec::new(),
            mcps: Vec::new(),
            commands: Vec::new(),
            instructions: Vec::new(),
            secrets: None,
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
            update: false,
            update_only: Vec::new(),
            update_local: true,
            locked: false,
            secrets: crate::secrets::SecretContext::empty(),
        };

        // Local paths (relative or absolute) are active, names never needed.
        assert!(update_active_for_source(&ctx, "./skills", &[]));
        assert!(update_active_for_source(&ctx, "/abs/pack", &["x".into()]));
        // Remote sources stay honored from the lock.
        assert!(!update_active_for_source(
            &ctx,
            "https://github.com/org/pack",
            &["x".into()]
        ));
    }
}
