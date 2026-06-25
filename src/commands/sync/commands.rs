use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::error::{err, Result};
use crate::fsops::{hash_file, now_unix, relativize_dest, resolve_command_targets};
use crate::lock::LockFile;
use crate::model::{Action, CommandTarget, CommandsField, Summary};
use crate::prompts::{apply_command, destination_path};
use crate::source::{discover_commands, materialize_source, resolve_command_entry};
use crate::ui::with_spinner_transient;

use super::{
    remove_stale as remove_stale_shared, sync_label_with, update_active_for_source, StaleEntry,
    SyncContext,
};

struct PendingCommand {
    source: String,
    name: String,
    src_path: PathBuf,
    hash: String,
    asset_id: String,
    is_new: bool,
    source_revision: String,
}

pub(super) fn sync_commands(
    ctx: &SyncContext,
    lock: &mut LockFile,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
) -> Result<()> {
    let targets = resolve_command_targets(ctx.cfg, ctx.scope, ctx.cfg_dir)?;

    // When the user removes the `commands:` block from config, all previously
    // installed commands are orphans — skip the install loop but still run
    // remove_stale with an empty desired-set so the lock and on-disk files
    // both get cleaned up.
    if ctx.cfg.commands.is_empty() {
        remove_stale(ctx, lock, summary, actions, &HashSet::new());
        return Ok(());
    }

    if targets.is_empty() {
        return Ok(());
    }

    let mut desired_ids = HashSet::new();
    let mut pending: Vec<PendingCommand> = Vec::new();
    let mut cleanup_dirs: Vec<PathBuf> = Vec::new();

    for (i, src) in ctx.cfg.commands.iter().enumerate() {
        // Desired command names for this source, derived without any network:
        // explicit config names for a list, or the locked set for a wildcard.
        let desired_names = desired_command_names(src, lock);

        // `--locked`/`--frozen`: the lock must be able to satisfy the config.
        if ctx.locked {
            if let Err(e) = ensure_locked_satisfiable_commands(src, &desired_names, lock) {
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

        let update_active = update_active_for_source(ctx, &desired_names);
        let fetch = update_active || needs_fetch_commands(src, &desired_names, lock, &targets);

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
            // Skip path: no network. Honor each desired command from the lock.
            let mut first_in_run = true;
            for name in &desired_names {
                let asset_id = format!("command::{}::{}", src.source, name);
                desired_ids.insert(asset_id);
                let label = sync_label_with(name, &src.source, ctx.plain, first_in_run);
                first_in_run = false;
                with_spinner_transient(ctx.animate, ctx.plain, &label, || {
                    summary.unchanged += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(format!("command:{name}")),
                        status: "unchanged".into(),
                        error: None,
                    });
                    Ok(())
                })?;
            }
            continue;
        }

        let stage = std::env::temp_dir().join(format!("kasetto-cmd-{}-{}", now_unix(), i));
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
        // Resolve commands against `source_root`, which honors `sub-dir` for
        // local, staged-remote, and cache-served sources alike. `cleanup_dir` is
        // the archive root (no sub-dir applied) and a teardown-only handle — using
        // it as the root would miss commands under a `sub-dir`.
        let root = materialized.source_root.as_path();

        let selected: Vec<(String, PathBuf)> = match &src.commands {
            CommandsField::Wildcard(s) if s == "*" => match discover_commands(root) {
                Ok(map) => map.into_iter().collect(),
                Err(e) => {
                    summary.broken += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some("command".into()),
                        status: "broken".into(),
                        error: Some(e.to_string()),
                    });
                    if let Some(d) = materialized.cleanup_dir {
                        cleanup_dirs.push(d);
                    }
                    continue;
                }
            },
            CommandsField::Wildcard(s) => {
                summary.broken += 1;
                actions.push(Action {
                    source: Some(src.source.clone()),
                    skill: Some("command".into()),
                    status: "broken".into(),
                    error: Some(format!(
                        "invalid commands value \"{s}\": expected \"*\" or a list"
                    )),
                });
                if let Some(d) = materialized.cleanup_dir {
                    cleanup_dirs.push(d);
                }
                continue;
            }
            CommandsField::List(entries) => {
                let mut out = Vec::new();
                for entry in entries {
                    let entry_name = match entry {
                        crate::model::CommandEntry::Name(n) => n.clone(),
                        crate::model::CommandEntry::Obj { name, .. } => name.clone(),
                    };
                    match resolve_command_entry(root, entry) {
                        Ok(pair) => out.push(pair),
                        Err(e) => {
                            summary.broken += 1;
                            actions.push(Action {
                                source: Some(src.source.clone()),
                                skill: Some(entry_name),
                                status: "broken".into(),
                                error: Some(e.to_string()),
                            });
                        }
                    }
                }
                out
            }
        };

        if selected.is_empty() && matches!(&src.commands, CommandsField::Wildcard(s) if s == "*") {
            summary.broken += 1;
            actions.push(Action {
                source: Some(src.source.clone()),
                skill: Some("command".into()),
                status: "broken".into(),
                error: Some("no commands found in source (expected commands/*.md)".into()),
            });
        }

        let mut first_in_run = true;
        for (name, src_path) in selected {
            let asset_id = format!("command::{}::{}", src.source, name);
            desired_ids.insert(asset_id.clone());
            let row_first = first_in_run;
            first_in_run = false;
            let hash = match hash_file(&src_path) {
                Ok(h) => h,
                Err(e) => {
                    summary.broken += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(format!("command:{name}")),
                        status: "broken".into(),
                        error: Some(e.to_string()),
                    });
                    continue;
                }
            };

            // Decide unchanged vs pending. Unchanged requires: stored hash matches
            // AND every expected destination file exists.
            let expected_paths: Vec<PathBuf> =
                targets.iter().map(|t| destination_path(t, &name)).collect();
            let existing = lock.get_tracked_asset("command", &asset_id);
            let is_unchanged = existing
                .as_ref()
                .map(|(h, _)| h == &hash && expected_paths.iter().all(|p| p.exists()))
                .unwrap_or(false);

            if is_unchanged {
                let label = sync_label_with(&name, &src.source, ctx.plain, row_first);
                with_spinner_transient(ctx.animate, ctx.plain, &label, || {
                    summary.unchanged += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(format!("command:{name}")),
                        status: "unchanged".into(),
                        error: None,
                    });
                    Ok(())
                })?;
            } else {
                pending.push(PendingCommand {
                    source: src.source.clone(),
                    name,
                    src_path,
                    hash,
                    asset_id,
                    is_new: existing.is_none(),
                    source_revision: materialized.source_revision.clone(),
                });
            }
        }

        if let Some(d) = materialized.cleanup_dir {
            cleanup_dirs.push(d);
        }
    }

    apply_pending(ctx, lock, summary, actions, &targets, &pending)?;
    for d in cleanup_dirs {
        let _ = fs::remove_dir_all(d);
    }

    // Same hazard as `sync_skills`: a partial failure (e.g. `locked_error`)
    // would have skipped extending `desired_ids` for the failed source. Defer
    // stale removal until the next clean run.
    if summary.failed == 0 {
        remove_stale(ctx, lock, summary, actions, &desired_ids);
    }
    Ok(())
}

/// Desired command names for a source, derived without any network access.
/// - `List`: the explicit config names.
/// - `Wildcard("*")`: the names of lock command-assets for this source.
/// - other wildcard values: empty (broken-value handling stays on the fetch path).
fn desired_command_names(src: &crate::model::CommandSourceSpec, lock: &LockFile) -> Vec<String> {
    match &src.commands {
        CommandsField::List(entries) => entries
            .iter()
            .map(|e| match e {
                crate::model::CommandEntry::Name(n) => n.clone(),
                crate::model::CommandEntry::Obj { name, .. } => name.clone(),
            })
            .collect(),
        CommandsField::Wildcard(s) if s == "*" => lock
            .assets
            .values()
            .filter(|a| a.kind == "command" && a.source == src.source)
            .map(|a| a.name.clone())
            .collect(),
        CommandsField::Wildcard(_) => Vec::new(),
    }
}

/// Per-source fetch decision (computed before any download). Fetch when a
/// wildcard source has never been resolved, when any desired command lacks a
/// lock entry, or when any expected destination file is missing (no local
/// repair exists for commands — the installed file is a transform of the source).
fn needs_fetch_commands(
    src: &crate::model::CommandSourceSpec,
    desired: &[String],
    lock: &LockFile,
    targets: &[CommandTarget],
) -> bool {
    // A wildcard source with no lock command-asset has never been resolved.
    if matches!(&src.commands, CommandsField::Wildcard(s) if s == "*")
        && !lock
            .assets
            .values()
            .any(|a| a.kind == "command" && a.source == src.source)
    {
        return true;
    }
    let expected_revision = src.as_source_spec().expected_revision();
    for name in desired {
        let asset_id = format!("command::{}::{}", src.source, name);
        let Some(asset) = lock.assets.get(&asset_id).filter(|a| a.kind == "command") else {
            return true;
        };
        // Retargeted source (ref/branch changed since the lock was written).
        if !asset.source_revision.is_empty() && asset.source_revision != expected_revision {
            return true;
        }
        let any_missing = targets.iter().any(|t| !destination_path(t, name).exists());
        if any_missing {
            return true;
        }
    }
    false
}

/// `--locked` validation: every config-named command must have a lock entry, and
/// a wildcard source must contribute at least one locked command-asset.
fn ensure_locked_satisfiable_commands(
    src: &crate::model::CommandSourceSpec,
    desired: &[String],
    lock: &LockFile,
) -> Result<()> {
    match &src.commands {
        CommandsField::List(_) => {
            for name in desired {
                let asset_id = format!("command::{}::{}", src.source, name);
                if lock.get_tracked_asset("command", &asset_id).is_none() {
                    return Err(err(format!(
                        "--locked: command `{name}` from `{}` is not in the lock",
                        src.source
                    )));
                }
            }
            Ok(())
        }
        CommandsField::Wildcard(_) => {
            let present = lock
                .assets
                .values()
                .any(|a| a.kind == "command" && a.source == src.source);
            if present {
                Ok(())
            } else {
                Err(err(format!(
                    "--locked: source `{}` has no command entries in the lock",
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
    targets: &[crate::model::CommandTarget],
    pending: &[PendingCommand],
) -> Result<()> {
    let mut last_source = String::new();
    for p in pending {
        let first_in_run = p.source != last_source;
        last_source = p.source.clone();
        let label = sync_label_with(&p.name, &p.source, ctx.plain, first_in_run);
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
                let mut written: Vec<String> = Vec::new();
                for target in targets {
                    let dest = apply_command(&p.src_path, target, &p.name).map_err(|e| {
                        err(format!(
                            "failed to apply command `{}` to {}: {e}",
                            p.name,
                            target.path.display()
                        ))
                    })?;
                    written.push(relativize_dest(&dest, &ctx.scope_root));
                }
                let dest_csv = written.join(",");
                lock.save_tracked_asset(
                    &p.asset_id,
                    crate::lock::AssetEntry {
                        kind: "command".into(),
                        name: p.name.clone(),
                        hash: p.hash.clone(),
                        source: p.source.clone(),
                        destination: dest_csv,
                        source_revision: p.source_revision.clone(),
                    },
                );
            }

            if status.contains("install") {
                summary.installed += 1;
            } else {
                summary.updated += 1;
            }
            actions.push(Action {
                source: Some(p.source.clone()),
                skill: Some(format!("command:{}", p.name)),
                status: status.into(),
                error: None,
            });
            Ok(())
        })?;
    }
    Ok(())
}

fn remove_stale(
    ctx: &SyncContext,
    lock: &mut LockFile,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
    desired_ids: &HashSet<String>,
) {
    let existing: Vec<(String, String)> = lock
        .list_tracked_asset_ids("command")
        .iter()
        .map(|(id, dest)| (id.to_string(), dest.to_string()))
        .collect();
    let dest_by_id: std::collections::HashMap<String, String> = existing.iter().cloned().collect();
    let candidates: Vec<StaleEntry> = existing
        .into_iter()
        .map(|(id, _)| {
            let name = lock
                .assets
                .get(&id)
                .map(|a| a.name.clone())
                .unwrap_or_else(|| id.rsplit("::").next().unwrap_or(&id).to_string());
            StaleEntry {
                id,
                action_source: None,
                action_skill: format!("command:{name}"),
            }
        })
        .collect();

    let scope_root = ctx.scope_root.clone();
    remove_stale_shared(
        ctx.dry_run,
        summary,
        actions,
        desired_ids,
        candidates,
        |id| {
            if let Some(dest_csv) = dest_by_id.get(id) {
                for p in dest_csv.split(',').filter(|s| !s.is_empty()) {
                    let path = crate::fsops::resolve_dest(p, &scope_root);
                    if path.exists() && path.is_file() {
                        let _ = fs::remove_file(path);
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
    use crate::model::{Agent, AgentField, CommandSourceSpec, CommandsField, Config, Scope};

    fn write(path: &std::path::Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn sync_writes_to_supported_agents_and_skips_unsupported() {
        // Source: a local path with one command file.
        let src_root = temp_dir("kasetto-src");
        write(
            &src_root.join("commands/git/commit.md"),
            "---\ndescription: commit\n---\nBody $ARGUMENTS\n",
        );

        // Project root that doubles as the project scope target for the agents.
        let project = temp_dir("kasetto-proj");
        fs::create_dir_all(&project).unwrap();

        // Pre-existing user file under .claude/commands that must be preserved.
        let user_file = project.join(".claude/commands/user-own.md");
        write(&user_file, "user authored\n");

        let cfg = Config {
            destination: None,
            scope: Some(Scope::Project),
            agent: Some(AgentField::Many(vec![
                Agent::ClaudeCode,
                Agent::GeminiCli,
                Agent::Cursor,
                // No matching enum for "aider" — using Codex which maps to None for project commands.
                Agent::Codex,
            ])),
            skills: Vec::new(),
            mcps: Vec::new(),
            commands: vec![CommandSourceSpec {
                source: src_root.to_string_lossy().to_string(),
                branch: None,
                git_ref: None,
                sub_dir: None,
                commands: CommandsField::Wildcard("*".to_string()),
            }],
            instructions: Vec::new(),
            secrets: None,
        };

        let mut lock = LockFile::default();
        let mut summary = Summary::default();
        let mut actions: Vec<Action> = Vec::new();

        let ctx = SyncContext {
            cfg: &cfg,
            cfg_dir: &project,
            destinations: std::slice::from_ref(&project),
            scope_root: project.clone(),
            scope: Scope::Project,
            dry_run: false,
            animate: false,
            plain: true,
            as_json: false,
            quiet: true,
            update: false,
            update_only: Vec::new(),
            locked: false,
            secrets: crate::secrets::SecretContext::empty(),
        };

        sync_commands(&ctx, &mut lock, &mut summary, &mut actions).unwrap();

        // Claude (frontmatter, nested namespacing).
        assert!(project.join(".claude/commands/git/commit.md").is_file());
        // Gemini (TOML, flattened namespacing).
        assert!(project.join(".gemini/commands/git-commit.toml").is_file());
        // Cursor (plain Markdown).
        assert!(project.join(".cursor/commands/git-commit.md").is_file());
        // Codex has no project commands path — directory should not exist.
        assert!(!project.join(".codex/prompts").exists());

        // User-authored file untouched.
        assert!(user_file.is_file());

        // Lock contains 1 command asset (one source × one command name).
        let lock_assets = lock.assets.values().filter(|a| a.kind == "command").count();
        assert_eq!(lock_assets, 1);

        // Second sync that removes the command (empty commands).
        let cfg2 = Config {
            commands: Vec::new(),
            ..Config {
                destination: cfg.destination.clone(),
                scope: cfg.scope,
                agent: cfg.agent.clone(),
                skills: Vec::new(),
                mcps: Vec::new(),
                commands: Vec::new(),
                instructions: Vec::new(),
                secrets: None,
            }
        };
        let mut summary2 = Summary::default();
        let mut actions2: Vec<Action> = Vec::new();
        let ctx2 = SyncContext {
            cfg: &cfg2,
            cfg_dir: &project,
            destinations: std::slice::from_ref(&project),
            scope_root: project.clone(),
            scope: Scope::Project,
            dry_run: false,
            animate: false,
            plain: true,
            as_json: false,
            quiet: true,
            update: false,
            update_only: Vec::new(),
            locked: false,
            secrets: crate::secrets::SecretContext::empty(),
        };
        // `sync_commands` now invokes `remove_stale` itself when `cfg.commands` is
        // empty — call it directly here to keep this a focused unit test of the
        // cleanup pass.
        let desired = HashSet::new();
        remove_stale(&ctx2, &mut lock, &mut summary2, &mut actions2, &desired);

        // Managed file is gone, user file still there.
        assert!(!project.join(".claude/commands/git/commit.md").exists());
        assert!(user_file.is_file());
        assert!(!project.join(".gemini/commands/git-commit.toml").exists());
        assert!(!project.join(".cursor/commands/git-commit.md").exists());

        // Cleanup
        let _ = fs::remove_dir_all(&src_root);
        let _ = fs::remove_dir_all(&project);
    }

    fn make_ctx<'a>(
        cfg: &'a Config,
        project: &'a PathBuf,
        dests: &'a [PathBuf],
        locked: bool,
    ) -> SyncContext<'a> {
        SyncContext {
            cfg,
            cfg_dir: project,
            destinations: dests,
            scope_root: project.clone(),
            scope: Scope::Project,
            dry_run: false,
            animate: false,
            plain: true,
            as_json: false,
            quiet: true,
            update: false,
            update_only: Vec::new(),
            locked,
            secrets: crate::secrets::SecretContext::empty(),
        }
    }

    fn list_cfg(src_root: &std::path::Path, commands: CommandsField) -> Config {
        Config {
            destination: None,
            scope: Some(Scope::Project),
            agent: Some(AgentField::One(Agent::ClaudeCode)),
            skills: Vec::new(),
            mcps: Vec::new(),
            commands: vec![CommandSourceSpec {
                source: src_root.to_string_lossy().to_string(),
                branch: None,
                git_ref: None,
                sub_dir: None,
                commands,
            }],
            instructions: Vec::new(),
            secrets: None,
        }
    }

    #[test]
    fn second_run_unchanged_without_source_no_fetch() {
        let src_root = temp_dir("kasetto-src");
        write(
            &src_root.join("commands/foo.md"),
            "---\ndescription: foo\n---\nBody\n",
        );
        let project = temp_dir("kasetto-proj");
        fs::create_dir_all(&project).unwrap();
        let dests = vec![project.clone()];

        let cfg = list_cfg(&src_root, CommandsField::Wildcard("*".into()));
        let mut lock = LockFile::default();

        let ctx = make_ctx(&cfg, &project, &dests, false);
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        sync_commands(&ctx, &mut lock, &mut summary, &mut actions).unwrap();
        assert_eq!(summary.installed, 1, "first run installs");

        // Remove the source entirely: plain re-sync must still report unchanged.
        fs::remove_dir_all(&src_root).unwrap();
        let ctx2 = make_ctx(&cfg, &project, &dests, false);
        let mut summary2 = Summary::default();
        let mut actions2 = Vec::new();
        sync_commands(&ctx2, &mut lock, &mut summary2, &mut actions2).unwrap();
        assert_eq!(summary2.unchanged, 1, "second run unchanged, no fetch");
        assert_eq!(summary2.failed, 0);
        assert_eq!(summary2.removed, 0, "lock entry retained, not pruned");

        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn locked_errors_when_command_absent_from_lock() {
        let src_root = temp_dir("kasetto-src");
        write(
            &src_root.join("commands/foo.md"),
            "---\ndescription: foo\n---\nBody\n",
        );
        let project = temp_dir("kasetto-proj");
        fs::create_dir_all(&project).unwrap();
        let dests = vec![project.clone()];

        let cfg = list_cfg(
            &src_root,
            CommandsField::List(vec![crate::model::CommandEntry::Name("foo".into())]),
        );
        let mut lock = LockFile::default();

        let ctx = make_ctx(&cfg, &project, &dests, true);
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        sync_commands(&ctx, &mut lock, &mut summary, &mut actions).unwrap();
        assert_eq!(summary.failed, 1, "--locked errors when not in lock");
        assert_eq!(summary.installed, 0);

        let _ = fs::remove_dir_all(&src_root);
        let _ = fs::remove_dir_all(&project);
    }
}
