use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::error::{err, Result};
use crate::fsops::{hash_file, now_unix, resolve_path, select_command_targets, BrokenSkill};
use crate::lock::LockFile;
use crate::model::{Action, Summary};
use crate::source::{discover_commands, materialize_source};
use crate::ui::{eprint_fail, with_spinner};

use super::{sync_label, SyncContext};

pub(super) fn sync_commands(
    ctx: &SyncContext,
    lock: &mut LockFile,
    destinations: &[PathBuf],
    summary: &mut Summary,
    actions: &mut Vec<Action>,
) -> Result<()> {
    if ctx.cfg.commands.is_empty() {
        return Ok(());
    }
    if destinations.is_empty() {
        return Err(err(
            "commands are configured but none of the selected agents support commands",
        ));
    }

    let mut desired_ids = HashSet::new();

    for (i, src) in ctx.cfg.commands.iter().enumerate() {
        let stage = std::env::temp_dir().join(format!("kasetto-cmd-{}-{}", now_unix(), i));
        let source_spec = src.as_source_spec();
        match materialize_source(&source_spec, ctx.cfg_dir, &stage) {
            Ok(materialized) => {
                let local_root = resolve_path(ctx.cfg_dir, &src.source);
                let discovery_root = if src.source.contains("://") {
                    stage.as_path()
                } else {
                    local_root.as_path()
                };
                let (targets, broken_commands) =
                    select_command_targets(&src.commands, &discover_commands(discovery_root)?)?;
                record_broken_commands(ctx, &src.source, broken_commands, summary, actions);

                for (command_name, command_path) in targets {
                    let label = sync_label("command", &command_name, &src.source, ctx.plain);
                    with_spinner(ctx.animate, ctx.plain, &label, || {
                        let id = format!("command::{}::{command_name}", src.source);
                        desired_ids.insert(id.clone());

                        let hash = hash_file(&command_path)?;
                        let destination_paths: Vec<PathBuf> = destinations
                            .iter()
                            .map(|d| {
                                let file_name = command_path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| format!("{command_name}.md"));
                                d.join(file_name)
                            })
                            .collect();

                        let unchanged = lock
                            .get_tracked_asset("command", &id)
                            .map(|(prev_hash, _)| {
                                prev_hash == hash && destination_paths.iter().all(|p| p.exists())
                            })
                            .unwrap_or(false);

                        if unchanged {
                            summary.unchanged += 1;
                            actions.push(Action {
                                source: Some(src.source.clone()),
                                skill: Some(format!("command:{command_name}")),
                                status: "unchanged".into(),
                                error: None,
                            });
                            return Ok(());
                        }

                        if ctx.dry_run {
                            let status = if lock.get_tracked_asset("command", &id).is_some() {
                                summary.updated += 1;
                                "would_update"
                            } else {
                                summary.installed += 1;
                                "would_install"
                            };
                            actions.push(Action {
                                source: Some(src.source.clone()),
                                skill: Some(format!("command:{command_name}")),
                                status: status.into(),
                                error: None,
                            });
                            return Ok(());
                        }

                        for destination in &destination_paths {
                            if let Some(parent) = destination.parent() {
                                fs::create_dir_all(parent)?;
                            }
                            fs::copy(&command_path, destination)?;
                        }

                        let status = if lock.get_tracked_asset("command", &id).is_some() {
                            summary.updated += 1;
                            "updated"
                        } else {
                            summary.installed += 1;
                            "installed"
                        };

                        let destination_csv = destination_paths
                            .iter()
                            .map(|p| p.to_string_lossy().to_string())
                            .collect::<Vec<_>>()
                            .join(",");

                        lock.save_tracked_asset(
                            "command",
                            &id,
                            &command_path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| format!("{command_name}.md")),
                            &hash,
                            &src.source,
                            &destination_csv,
                        );

                        actions.push(Action {
                            source: Some(src.source.clone()),
                            skill: Some(format!("command:{command_name}")),
                            status: status.into(),
                            error: None,
                        });
                        Ok(())
                    })?;
                }
                if let Some(cleanup_dir) = materialized.cleanup_dir {
                    let _ = fs::remove_dir_all(cleanup_dir);
                }
            }
            Err(e) => {
                summary.failed += 1;
                actions.push(Action {
                    source: Some(src.source.clone()),
                    skill: None,
                    status: "source_error".into(),
                    error: Some(e.to_string()),
                });
            }
        }
    }

    remove_stale_commands(ctx, lock, &desired_ids, summary, actions);
    Ok(())
}

fn record_broken_commands(
    ctx: &SyncContext,
    source: &str,
    broken_commands: Vec<BrokenSkill>,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
) {
    for broken in broken_commands {
        summary.broken += 1;
        actions.push(Action {
            source: Some(source.to_string()),
            skill: Some(format!("command:{}", broken.name)),
            status: "broken".into(),
            error: Some(broken.reason.clone()),
        });
        if !ctx.as_json && !ctx.quiet {
            eprint_fail(&format!("command:{}", broken.name), source, ctx.plain);
        }
    }
}

fn remove_stale_commands(
    ctx: &SyncContext,
    lock: &mut LockFile,
    desired_ids: &HashSet<String>,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
) {
    let existing: Vec<(String, String)> = lock
        .list_tracked_asset_ids("command")
        .iter()
        .map(|(id, dest)| ((*id).to_string(), (*dest).to_string()))
        .collect();

    for (id, destinations_csv) in existing {
        if desired_ids.contains(&id) {
            continue;
        }

        let command_name = id
            .split("::")
            .last()
            .map(|n| format!("command:{n}"))
            .unwrap_or_else(|| "command:-".to_string());

        if ctx.dry_run {
            summary.removed += 1;
            actions.push(Action {
                source: None,
                skill: Some(command_name),
                status: "would_remove".into(),
                error: None,
            });
            continue;
        }

        for destination in destinations_csv.split(',').filter(|s| !s.is_empty()) {
            let _ = fs::remove_file(destination);
        }
        lock.remove_tracked_asset(&id);
        summary.removed += 1;
        actions.push(Action {
            source: None,
            skill: Some(command_name),
            status: "removed".into(),
            error: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsops::temp_dir;
    use crate::model::{Config, Scope};

    #[test]
    fn sync_commands_installs_and_tracks_markdown_command() {
        let root = temp_dir("kasetto-sync-command-md");
        let source_root = root.join("source");
        let command_dir = source_root.join("commands");
        let destination = root.join("dest/.opencode/commands");
        fs::create_dir_all(&command_dir).expect("create command dir");
        fs::write(command_dir.join("review.md"), "---\n---\nReview\n").expect("write command");

        let cfg: Config = serde_yaml::from_str(
            "skills: []\ncommands:\n  - source: source\n    commands:\n      - review\n",
        )
        .expect("parse config");

        let mut lock = LockFile::default();
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        let destinations = vec![destination.clone()];
        let ctx = SyncContext {
            cfg: &cfg,
            cfg_dir: &root,
            destinations: &[],
            scope: Scope::Project,
            dry_run: false,
            yes: true,
            animate: false,
            plain: true,
            as_json: false,
            quiet: true,
        };

        sync_commands(&ctx, &mut lock, &destinations, &mut summary, &mut actions).expect("sync");

        assert!(destination.join("review.md").exists());
        assert_eq!(summary.installed, 1);
        assert!(lock
            .get_tracked_asset("command", "command::source::review")
            .is_some());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn sync_commands_preserves_toml_extension_for_gemini_style_commands() {
        let root = temp_dir("kasetto-sync-command-toml");
        let source_root = root.join("source");
        let command_dir = source_root.join(".gemini/commands");
        let destination = root.join("dest/.gemini/commands");
        fs::create_dir_all(&command_dir).expect("create command dir");
        fs::write(command_dir.join("triage.toml"), "name='triage'\n").expect("write command");

        let cfg: Config = serde_yaml::from_str(
            "skills: []\ncommands:\n  - source: source\n    commands:\n      - triage\n",
        )
        .expect("parse config");

        let mut lock = LockFile::default();
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        let destinations = vec![destination.clone()];
        let ctx = SyncContext {
            cfg: &cfg,
            cfg_dir: &root,
            destinations: &[],
            scope: Scope::Project,
            dry_run: false,
            yes: true,
            animate: false,
            plain: true,
            as_json: false,
            quiet: true,
        };

        sync_commands(&ctx, &mut lock, &destinations, &mut summary, &mut actions).expect("sync");

        assert!(destination.join("triage.toml").exists());
        assert!(!destination.join("triage.md").exists());
        assert_eq!(summary.installed, 1);

        let _ = fs::remove_dir_all(&root);
    }
}
