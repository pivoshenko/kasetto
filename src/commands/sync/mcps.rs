use std::collections::HashSet;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

use crate::colors::{ACCENT, RESET, SECONDARY, WARNING};
use crate::error::{err, Result};
use crate::fsops::{hash_file, now_unix, resolve_mcp_settings_targets};
use crate::lock::LockFile;
use crate::mcps::{merge_mcp_config, remove_mcp_server, servers_present_in_settings};
use crate::model::{Action, Summary};
use crate::source::{discover_mcps, materialize_source, resolve_mcp_path};
use crate::ui::with_spinner;

use super::{file_name_str, sync_label, SyncContext};

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
    if mcp_settings_list.is_empty() {
        return Ok(());
    }

    // Phase 1: discover and classify all MCP entries
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
            let file_name_for_err = file_name.clone();
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
                    let label = sync_label("MCP", &file_name, &src.source, ctx.plain);
                    with_spinner(ctx.animate, ctx.plain, &label, || {
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

    // Phase 2: prompt for confirmation when new MCP servers will be registered
    let new_servers: Vec<&PendingMcp> = pending.iter().filter(|p| p.is_new).collect();
    if !new_servers.is_empty() && !ctx.dry_run && !ctx.yes {
        let all_names: Vec<&str> = new_servers
            .iter()
            .flat_map(|p| p.server_names.iter().map(|s| s.as_str()))
            .collect();

        if !all_names.is_empty() {
            let approved = confirm_new_mcps(&all_names, ctx.plain)?;
            if !approved {
                // User declined — mark new installs as skipped, still apply updates
                for p in &pending {
                    if p.is_new {
                        actions.push(Action {
                            source: Some(p.source.clone()),
                            skill: Some(format!("mcp:{}", p.file_name)),
                            status: "skipped".into(),
                            error: None,
                        });
                    }
                }
                // Only keep updates
                let pending_updates: Vec<PendingMcp> =
                    pending.into_iter().filter(|p| !p.is_new).collect();
                apply_pending(
                    ctx,
                    lock,
                    summary,
                    actions,
                    &mcp_settings_list,
                    &pending_updates,
                )?;
                cleanup_staged(&cleanup_dirs);
                remove_stale(
                    ctx,
                    lock,
                    summary,
                    actions,
                    &desired_mcp_ids,
                    &mcp_settings_list,
                );
                return Ok(());
            }
        }
    }

    // Phase 3: apply all pending installs and updates
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

fn apply_pending(
    ctx: &SyncContext,
    lock: &mut LockFile,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
    mcp_settings_list: &[crate::model::McpSettingsTarget],
    pending: &[PendingMcp],
) -> Result<()> {
    for p in pending {
        let label = sync_label("MCP", &p.file_name, &p.source, ctx.plain);
        with_spinner(ctx.animate, ctx.plain, &label, || {
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
            for target in mcp_settings_list {
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
}

/// Prompt the user to confirm registration of new MCP servers.
/// Returns `true` if approved, `false` if declined.
/// Errors in non-interactive mode (piped stdin) to prevent unreviewed registration.
fn confirm_new_mcps(server_names: &[&str], plain: bool) -> Result<bool> {
    if !io::stdin().is_terminal() {
        return Err(err(
            "new MCP servers would be registered but stdin is not a terminal; pass --yes to confirm",
        ));
    }

    println!();
    if plain {
        println!("The following MCP servers will be registered:");
    } else {
        println!("{WARNING}The following MCP servers will be registered:{RESET}");
    }
    for name in server_names {
        if plain {
            println!("  - {name}");
        } else {
            println!("  {SECONDARY}-{RESET} {ACCENT}{name}{RESET}");
        }
    }
    println!();

    if plain {
        print!("Proceed? [y/N] ");
    } else {
        print!("{ACCENT}Proceed?{RESET} [y/N] ");
    }
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(matches!(input.trim(), "y" | "Y" | "yes"))
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

        let pending = vec![new_entry, update_entry];
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
        let update_only = vec![PendingMcp {
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
}
