use std::collections::HashSet;
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use crate::error::Result;
use crate::fsops::{hash_file, now_unix, resolve_mcp_settings_targets};
use crate::lock::LockFile;
use crate::mcps::{merge_mcp_config, remove_mcp_server, servers_present_in_settings};
use crate::model::{Action, Summary};
use crate::source::{discover_mcps, materialize_source, resolve_mcp_path};
use crate::ui::with_spinner;

use super::{file_name_str, sync_label, SyncContext};

struct PendingMcp {
    source: String,
    mcp_path: PathBuf,
    file_name: String,
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
    if mcp_settings_list.is_empty() {
        return Ok(());
    }

    // Phase 1: materialise all sources and collect pending operations.
    // Temp dirs are held in `cleanup_dirs` until after the apply phase,
    // because `PendingMcp::mcp_path` points into them.
    let mut pending: Vec<PendingMcp> = Vec::new();
    let mut cleanup_dirs: Vec<PathBuf> = Vec::new();

    for (i, src) in ctx.cfg.mcps.iter().enumerate() {
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
        let mcps = match if let Some(ref p) = src.path {
            resolve_mcp_path(root, p)
        } else {
            discover_mcps(root)
        } {
            Ok(paths) => paths,
            Err(e) => {
                summary.broken += 1;
                let skill = src
                    .path
                    .as_ref()
                    .map(|p| format!("mcp:{p}"))
                    .unwrap_or_else(|| "mcp".into());
                actions.push(Action {
                    source: Some(src.source.clone()),
                    skill: Some(skill),
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
                    "no MCP JSON files found in source (expected .mcp.json, mcp.json, or mcp/*.json)"
                        .into(),
                ),
            });
            if let Some(d) = materialized.cleanup_dir {
                let _ = fs::remove_dir_all(d);
            }
            continue;
        }

        for mcp_path in &mcps {
            let file_name = file_name_str(mcp_path);
            let file_name_err = file_name.clone();
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
                    summary.unchanged += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(format!("mcp:{file_name}")),
                        status: "unchanged".into(),
                        error: None,
                    });
                } else {
                    pending.push(PendingMcp {
                        source: src.source.clone(),
                        mcp_path: mcp_path.clone(),
                        file_name,
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
                    skill: Some(format!("mcp:{file_name_err}")),
                    status: "broken".into(),
                    error: Some(e.to_string()),
                });
            }
        }

        if let Some(d) = materialized.cleanup_dir {
            cleanup_dirs.push(d);
        }
    }

    // Phase 2: prompt before registering new MCP servers (unless --no-confirm or dry-run).
    if !ctx.dry_run && !ctx.no_confirm {
        let new_servers: Vec<(&str, &str)> = pending
            .iter()
            .filter(|p| p.is_new)
            .flat_map(|p| {
                p.server_names
                    .iter()
                    .map(move |s| (s.as_str(), p.source.as_str()))
            })
            .collect();

        if !new_servers.is_empty() && std::io::stdin().is_terminal() {
            println!();
            println!("New MCP servers to be registered:");
            println!();
            for (server, source) in &new_servers {
                println!("  • {server}  (from {source})");
            }
            println!();
            print!("Proceed? [y/N] ");
            std::io::stdout().flush()?;

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if input.trim().to_lowercase() != "y" {
                for d in cleanup_dirs {
                    let _ = fs::remove_dir_all(d);
                }
                return Ok(());
            }
        }
    }

    // Phase 3: apply pending operations.
    for p in &pending {
        let label = sync_label("MCP", &p.file_name, &p.source, ctx.plain);
        let r: std::result::Result<(), crate::error::Error> =
            with_spinner(ctx.animate, ctx.plain, &label, || {
                let status = if p.is_new {
                    if ctx.dry_run { "would_install" } else { "installed" }
                } else if ctx.dry_run {
                    "would_update"
                } else {
                    "updated"
                };

                if !ctx.dry_run {
                    for target in &mcp_settings_list {
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
            });
        if let Err(e) = r {
            summary.broken += 1;
            actions.push(Action {
                source: Some(p.source.clone()),
                skill: Some(format!("mcp:{}", p.file_name)),
                status: "broken".into(),
                error: Some(e.to_string()),
            });
        }
    }

    for d in cleanup_dirs {
        let _ = fs::remove_dir_all(d);
    }

    // Remove MCP servers no longer in config.
    let existing_mcps: Vec<(String, String)> = lock
        .list_tracked_asset_ids("mcp")
        .iter()
        .map(|(id, dest)| (id.to_string(), dest.to_string()))
        .collect();
    for (old_id, old_servers_csv) in &existing_mcps {
        if desired_mcp_ids.contains(old_id) {
            continue;
        }
        let mcp_name = old_id.rsplit("::").next().unwrap_or(old_id).to_string();
        if ctx.dry_run {
            summary.removed += 1;
            actions.push(Action {
                source: None,
                skill: Some(format!("mcp:{mcp_name}")),
                status: "would_remove".into(),
                error: None,
            });
        } else {
            for target in &mcp_settings_list {
                for server_name in old_servers_csv.split(',').filter(|s| !s.is_empty()) {
                    let _ = remove_mcp_server(server_name, target);
                }
            }
            lock.remove_tracked_asset(old_id);
            summary.removed += 1;
            actions.push(Action {
                source: None,
                skill: Some(format!("mcp:{mcp_name}")),
                status: "removed".into(),
                error: None,
            });
        }
    }

    Ok(())
}
