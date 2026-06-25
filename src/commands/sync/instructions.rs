use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::error::{err, Result};
use crate::fsops::{hash_file, now_unix, relativize_dest, resolve_instruction_targets};
use crate::instructions::{apply_instruction, dest_present, dest_token, teardown_dest};
use crate::lock::LockFile;
use crate::model::{Action, InstructionTarget, InstructionsField, Summary};
use crate::source::{discover_instructions, materialize_source, resolve_instruction_entry};
use crate::ui::with_spinner_transient;

use super::{
    remove_stale as remove_stale_shared, sync_label_with, update_active_for_source, StaleEntry,
    SyncContext,
};

struct PendingInstruction {
    source: String,
    name: String,
    src_path: PathBuf,
    hash: String,
    asset_id: String,
    is_new: bool,
    source_revision: String,
}

pub(super) fn sync_instructions(
    ctx: &SyncContext,
    lock: &mut LockFile,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
) -> Result<()> {
    let targets = resolve_instruction_targets(ctx.cfg, ctx.scope, ctx.cfg_dir)?;

    // Dropping the `instructions:` block orphans every installed instruction — skip the
    // install loop but still run remove_stale with an empty desired-set so the
    // lock and on-disk blocks/files both get cleaned up.
    if ctx.cfg.instructions.is_empty() {
        remove_stale(ctx, lock, summary, actions, &HashSet::new());
        return Ok(());
    }

    // No agent in the resolved scope has an instruction target (e.g. only `warp`
    // globally or `openclaw` per-project), yet `instructions:` is still set. Nothing
    // can be installed, so treat previously-installed instructions as orphaned and
    // prune them — same as the empty-config branch above.
    if targets.is_empty() {
        remove_stale(ctx, lock, summary, actions, &HashSet::new());
        return Ok(());
    }

    let mut desired_ids = HashSet::new();
    let mut pending: Vec<PendingInstruction> = Vec::new();
    let mut cleanup_dirs: Vec<PathBuf> = Vec::new();

    for (i, src) in ctx.cfg.instructions.iter().enumerate() {
        let desired_names = desired_instruction_names(src, lock);

        if ctx.locked {
            if let Err(e) = ensure_locked_satisfiable_instructions(src, &desired_names, lock) {
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
        let fetch = update_active || needs_fetch_instructions(src, &desired_names, lock, &targets);

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
            let mut first_in_run = true;
            for name in &desired_names {
                let asset_id = format!("instructions::{}::{}", src.source, name);
                desired_ids.insert(asset_id);
                let label = sync_label_with(name, &src.source, ctx.plain, first_in_run);
                first_in_run = false;
                with_spinner_transient(ctx.animate, ctx.plain, &label, || {
                    summary.unchanged += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(format!("instruction:{name}")),
                        status: "unchanged".into(),
                        error: None,
                    });
                    Ok(())
                })?;
            }
            continue;
        }

        let stage = std::env::temp_dir().join(format!("kasetto-instruction-{}-{}", now_unix(), i));
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
        // `source_root` already honors `sub-dir` for both local and remote sources;
        // `cleanup_dir` is the archive root (remote) and must not be used as the
        // discovery root or a configured `sub-dir` would be ignored.
        let root = materialized.source_root.as_path();

        let selected: Vec<(String, PathBuf)> = match &src.instructions {
            InstructionsField::Wildcard(s) if s == "*" => match discover_instructions(root) {
                Ok(map) => map.into_iter().collect(),
                Err(e) => {
                    summary.broken += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some("instruction".into()),
                        status: "broken".into(),
                        error: Some(e.to_string()),
                    });
                    if let Some(d) = materialized.cleanup_dir {
                        cleanup_dirs.push(d);
                    }
                    continue;
                }
            },
            InstructionsField::Wildcard(s) => {
                summary.broken += 1;
                actions.push(Action {
                    source: Some(src.source.clone()),
                    skill: Some("instruction".into()),
                    status: "broken".into(),
                    error: Some(format!(
                        "invalid instructions value \"{s}\": expected \"*\" or a list"
                    )),
                });
                if let Some(d) = materialized.cleanup_dir {
                    cleanup_dirs.push(d);
                }
                continue;
            }
            InstructionsField::List(entries) => {
                let mut out = Vec::new();
                for entry in entries {
                    let entry_name = match entry {
                        crate::model::InstructionEntry::Name(n) => n.clone(),
                        crate::model::InstructionEntry::Obj { name, .. } => name.clone(),
                    };
                    match resolve_instruction_entry(root, entry) {
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

        if selected.is_empty()
            && matches!(&src.instructions, InstructionsField::Wildcard(s) if s == "*")
        {
            summary.broken += 1;
            actions.push(Action {
                source: Some(src.source.clone()),
                skill: Some("instruction".into()),
                status: "broken".into(),
                error: Some("no instructions found in source (expected instructions/*.md)".into()),
            });
        }

        let mut first_in_run = true;
        for (name, src_path) in selected {
            let asset_id = format!("instructions::{}::{}", src.source, name);
            desired_ids.insert(asset_id.clone());
            let row_first = first_in_run;
            first_in_run = false;
            let hash = match hash_file(&src_path) {
                Ok(h) => h,
                Err(e) => {
                    summary.broken += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(format!("instruction:{name}")),
                        status: "broken".into(),
                        error: Some(e.to_string()),
                    });
                    continue;
                }
            };

            // Unchanged requires: stored hash matches AND every expected
            // destination is present (for aggregate files, the managed block).
            let existing = lock.get_tracked_asset("instructions", &asset_id);
            let is_unchanged = existing
                .as_ref()
                .map(|(h, _)| {
                    h == &hash && targets.iter().all(|t| dest_present(t, &name, &src.source))
                })
                .unwrap_or(false);

            if is_unchanged {
                let label = sync_label_with(&name, &src.source, ctx.plain, row_first);
                with_spinner_transient(ctx.animate, ctx.plain, &label, || {
                    summary.unchanged += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(format!("instruction:{name}")),
                        status: "unchanged".into(),
                        error: None,
                    });
                    Ok(())
                })?;
            } else {
                pending.push(PendingInstruction {
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

    if summary.failed == 0 {
        remove_stale(ctx, lock, summary, actions, &desired_ids);
    }
    Ok(())
}

/// Desired instruction names for a source, derived without any network access.
fn desired_instruction_names(
    src: &crate::model::InstructionSourceSpec,
    lock: &LockFile,
) -> Vec<String> {
    match &src.instructions {
        InstructionsField::List(entries) => entries
            .iter()
            .map(|e| match e {
                crate::model::InstructionEntry::Name(n) => n.clone(),
                // `resolve_instruction_entry` strips an explicit `.md`/`.mdc` extension
                // when deriving the asset name; mirror that here so the lock lookup
                // (and `--locked` validation) keys on the same stored name.
                crate::model::InstructionEntry::Obj { name, .. } => name
                    .trim_end_matches(".mdc")
                    .trim_end_matches(".md")
                    .to_string(),
            })
            .collect(),
        InstructionsField::Wildcard(s) if s == "*" => lock
            .assets
            .values()
            .filter(|a| a.kind == "instructions" && a.source == src.source)
            .map(|a| a.name.clone())
            .collect(),
        InstructionsField::Wildcard(_) => Vec::new(),
    }
}

/// Per-source fetch decision (computed before any download). Fetch when a
/// wildcard source has never been resolved, when any desired instruction lacks a lock
/// entry, when the source was retargeted, or when any expected destination is
/// missing (a transform has no local repair path).
fn needs_fetch_instructions(
    src: &crate::model::InstructionSourceSpec,
    desired: &[String],
    lock: &LockFile,
    targets: &[InstructionTarget],
) -> bool {
    if matches!(&src.instructions, InstructionsField::Wildcard(s) if s == "*")
        && !lock
            .assets
            .values()
            .any(|a| a.kind == "instructions" && a.source == src.source)
    {
        return true;
    }
    let expected_revision = src.as_source_spec().expected_revision();
    for name in desired {
        let asset_id = format!("instructions::{}::{}", src.source, name);
        let Some(asset) = lock
            .assets
            .get(&asset_id)
            .filter(|a| a.kind == "instructions")
        else {
            return true;
        };
        if !asset.source_revision.is_empty() && asset.source_revision != expected_revision {
            return true;
        }
        let any_missing = targets.iter().any(|t| !dest_present(t, name, &src.source));
        if any_missing {
            return true;
        }
    }
    false
}

/// `--locked` validation: every config-named instruction must have a lock entry, and a
/// wildcard source must contribute at least one locked instruction-asset.
fn ensure_locked_satisfiable_instructions(
    src: &crate::model::InstructionSourceSpec,
    desired: &[String],
    lock: &LockFile,
) -> Result<()> {
    match &src.instructions {
        InstructionsField::List(_) => {
            for name in desired {
                let asset_id = format!("instructions::{}::{}", src.source, name);
                if lock.get_tracked_asset("instructions", &asset_id).is_none() {
                    return Err(err(format!(
                        "--locked: instruction `{name}` from `{}` is not in the lock",
                        src.source
                    )));
                }
            }
            Ok(())
        }
        InstructionsField::Wildcard(_) => {
            let present = lock
                .assets
                .values()
                .any(|a| a.kind == "instructions" && a.source == src.source);
            if present {
                Ok(())
            } else {
                Err(err(format!(
                    "--locked: source `{}` has no instruction entries in the lock",
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
    targets: &[InstructionTarget],
    pending: &[PendingInstruction],
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
                    let dest = apply_instruction(&p.src_path, target, &p.source, &p.name).map_err(
                        |e| {
                            err(format!(
                                "failed to apply instruction `{}` to {}: {e}",
                                p.name,
                                target.path.display()
                            ))
                        },
                    )?;
                    let rel = relativize_dest(&dest, &ctx.scope_root);
                    written.push(dest_token(target, &rel));
                }
                lock.save_tracked_asset(
                    &p.asset_id,
                    crate::lock::AssetEntry {
                        kind: "instructions".into(),
                        name: p.name.clone(),
                        hash: p.hash.clone(),
                        source: p.source.clone(),
                        destination: written.join(","),
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
                skill: Some(format!("instruction:{}", p.name)),
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
    // Snapshot (id, source, name, dest_csv) so the teardown closure can strip the
    // right managed block / delete the right file without re-borrowing the lock.
    let existing: Vec<(String, String, String, String)> = lock
        .assets
        .iter()
        .filter(|(_, a)| a.kind == "instructions")
        .map(|(id, a)| {
            (
                id.clone(),
                a.source.clone(),
                a.name.clone(),
                a.destination.clone(),
            )
        })
        .collect();

    let meta_by_id: std::collections::HashMap<String, (String, String, String)> = existing
        .iter()
        .map(|(id, source, name, dest)| (id.clone(), (source.clone(), name.clone(), dest.clone())))
        .collect();

    let candidates: Vec<StaleEntry> = existing
        .iter()
        .map(|(id, _, name, _)| StaleEntry {
            id: id.clone(),
            action_source: None,
            action_skill: format!("instruction:{name}"),
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
            if let Some((source, name, dest_csv)) = meta_by_id.get(id) {
                for token in dest_csv.split(',').filter(|s| !s.is_empty()) {
                    teardown_dest(token, source, name, &scope_root);
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
    use crate::model::{
        Agent, AgentField, Config, InstructionSourceSpec, InstructionsField, Scope,
    };

    fn write(path: &std::path::Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn base_cfg(
        src_root: &std::path::Path,
        agents: Vec<Agent>,
        instructions: InstructionsField,
    ) -> Config {
        Config {
            destination: None,
            scope: Some(Scope::Project),
            agent: Some(AgentField::Many(agents)),
            skills: Vec::new(),
            mcps: Vec::new(),
            commands: Vec::new(),
            instructions: vec![InstructionSourceSpec {
                source: src_root.to_string_lossy().to_string(),
                branch: None,
                git_ref: None,
                sub_dir: None,
                instructions,
            }],
            secrets: None,
        }
    }

    fn make_ctx<'a>(cfg: &'a Config, project: &'a PathBuf, locked: bool) -> SyncContext<'a> {
        SyncContext {
            cfg,
            cfg_dir: project,
            destinations: std::slice::from_ref(project),
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

    #[test]
    fn sync_writes_aggregate_and_dir_formats_then_prunes() {
        let src_root = temp_dir("kasetto-instruction-src");
        write(
            &src_root.join("instructions/style.mdc"),
            "---\ndescription: house style\nglobs: \"*.rs\"\n---\nUse tabs.\n",
        );

        let project = temp_dir("kasetto-instruction-proj");
        fs::create_dir_all(&project).unwrap();
        // Pre-existing user CLAUDE.md content that must survive.
        write(&project.join("CLAUDE.md"), "# Project\n\nUser notes.\n");

        let cfg = base_cfg(
            &src_root,
            vec![Agent::ClaudeCode, Agent::Cursor, Agent::Windsurf],
            InstructionsField::Wildcard("*".into()),
        );
        let mut lock = LockFile::default();
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        let ctx = make_ctx(&cfg, &project, false);
        sync_instructions(&ctx, &mut lock, &mut summary, &mut actions).unwrap();
        assert_eq!(summary.installed, 1);

        // Claude aggregate: managed block added, user content preserved.
        let claude = fs::read_to_string(project.join("CLAUDE.md")).unwrap();
        assert!(claude.contains("User notes."));
        assert!(claude.contains("Use tabs."));
        assert!(claude.contains("kasetto:instruction:style-"));
        // Cursor mdc: per-instruction file with reconstructed frontmatter.
        let cursor = fs::read_to_string(project.join(".cursor/rules/style.mdc")).unwrap();
        assert!(cursor.contains("globs: *.rs"));
        // Windsurf plain dir: body only.
        let windsurf = fs::read_to_string(project.join(".windsurf/rules/style.md")).unwrap();
        assert!(!windsurf.contains("description:"));
        assert!(windsurf.contains("Use tabs."));

        // One instructions asset tracked.
        assert_eq!(
            lock.assets
                .values()
                .filter(|a| a.kind == "instructions")
                .count(),
            1
        );

        // Drop the instructions: remove_stale strips the block + deletes the dir files.
        let mut summary2 = Summary::default();
        let mut actions2 = Vec::new();
        remove_stale(
            &ctx,
            &mut lock,
            &mut summary2,
            &mut actions2,
            &HashSet::new(),
        );
        let claude2 = fs::read_to_string(project.join("CLAUDE.md")).unwrap();
        assert!(claude2.contains("User notes."));
        assert!(!claude2.contains("Use tabs."));
        assert!(!project.join(".cursor/rules/style.mdc").exists());
        assert!(!project.join(".windsurf/rules/style.md").exists());

        let _ = fs::remove_dir_all(&src_root);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn second_run_unchanged_without_source_no_fetch() {
        let src_root = temp_dir("kasetto-instruction-src2");
        write(&src_root.join("instructions/style.md"), "---\n---\nbody\n");
        let project = temp_dir("kasetto-instruction-proj2");
        fs::create_dir_all(&project).unwrap();

        let cfg = base_cfg(
            &src_root,
            vec![Agent::ClaudeCode],
            InstructionsField::Wildcard("*".into()),
        );
        let mut lock = LockFile::default();
        let ctx = make_ctx(&cfg, &project, false);
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        sync_instructions(&ctx, &mut lock, &mut summary, &mut actions).unwrap();
        assert_eq!(summary.installed, 1);

        fs::remove_dir_all(&src_root).unwrap();
        let mut summary2 = Summary::default();
        let mut actions2 = Vec::new();
        sync_instructions(&ctx, &mut lock, &mut summary2, &mut actions2).unwrap();
        assert_eq!(summary2.unchanged, 1);
        assert_eq!(summary2.failed, 0);
        assert_eq!(summary2.removed, 0);

        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn reconfig_to_targetless_agent_prunes_installed_instructions() {
        let src_root = temp_dir("kasetto-instruction-src4");
        write(&src_root.join("instructions/style.md"), "---\n---\nbody\n");
        let project = temp_dir("kasetto-instruction-proj4");
        fs::create_dir_all(&project).unwrap();
        write(&project.join("CLAUDE.md"), "# Project\n\nUser notes.\n");

        // First sync installs into Claude's aggregate CLAUDE.md.
        let cfg = base_cfg(
            &src_root,
            vec![Agent::ClaudeCode],
            InstructionsField::Wildcard("*".into()),
        );
        let mut lock = LockFile::default();
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        let ctx = make_ctx(&cfg, &project, false);
        sync_instructions(&ctx, &mut lock, &mut summary, &mut actions).unwrap();
        assert_eq!(summary.installed, 1);
        assert!(fs::read_to_string(project.join("CLAUDE.md"))
            .unwrap()
            .contains("body"));

        // Reconfigure to OpenClaw (no project instruction target) while keeping
        // `instructions:` set: the orphaned managed block + lock entry must be pruned.
        let cfg2 = base_cfg(
            &src_root,
            vec![Agent::OpenClaw],
            InstructionsField::Wildcard("*".into()),
        );
        let ctx2 = make_ctx(&cfg2, &project, false);
        let mut summary2 = Summary::default();
        let mut actions2 = Vec::new();
        sync_instructions(&ctx2, &mut lock, &mut summary2, &mut actions2).unwrap();
        assert_eq!(summary2.removed, 1);
        let claude = fs::read_to_string(project.join("CLAUDE.md")).unwrap();
        assert!(claude.contains("User notes."));
        assert!(!claude.contains("body"));
        assert_eq!(
            lock.assets
                .values()
                .filter(|a| a.kind == "instructions")
                .count(),
            0
        );

        let _ = fs::remove_dir_all(&src_root);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn explicit_extension_object_entry_is_unchanged_on_second_sync() {
        let src_root = temp_dir("kasetto-instruction-src5");
        write(
            &src_root.join("house/style.mdc"),
            "---\ndescription: x\n---\nbody\n",
        );
        let project = temp_dir("kasetto-instruction-proj5");
        fs::create_dir_all(&project).unwrap();

        // Object entry carries an explicit `.mdc` extension; the resolver stores the
        // asset under the stripped name, so desired-name derivation must strip too —
        // otherwise the second sync re-fetches/re-installs instead of being unchanged.
        let cfg = {
            let mut c = base_cfg(
                &src_root,
                vec![Agent::ClaudeCode],
                InstructionsField::Wildcard("*".into()),
            );
            c.instructions[0].instructions =
                InstructionsField::List(vec![crate::model::InstructionEntry::Obj {
                    name: "style.mdc".into(),
                    path: Some("house".into()),
                }]);
            c
        };
        let mut lock = LockFile::default();
        let ctx = make_ctx(&cfg, &project, false);

        let mut summary = Summary::default();
        let mut actions = Vec::new();
        sync_instructions(&ctx, &mut lock, &mut summary, &mut actions).unwrap();
        assert_eq!(summary.installed, 1);
        assert!(lock
            .get_tracked_asset(
                "instructions",
                &format!("instructions::{}::style", src_root.to_string_lossy())
            )
            .is_some());

        let mut summary2 = Summary::default();
        let mut actions2 = Vec::new();
        sync_instructions(&ctx, &mut lock, &mut summary2, &mut actions2).unwrap();
        assert_eq!(summary2.unchanged, 1);
        assert_eq!(summary2.installed, 0);
        assert_eq!(summary2.updated, 0);

        let _ = fs::remove_dir_all(&src_root);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn locked_errors_when_instruction_absent_from_lock() {
        let src_root = temp_dir("kasetto-instruction-src3");
        write(&src_root.join("instructions/style.md"), "---\n---\nbody\n");
        let project = temp_dir("kasetto-instruction-proj3");
        fs::create_dir_all(&project).unwrap();

        let cfg = base_cfg(
            &src_root,
            vec![Agent::ClaudeCode],
            InstructionsField::List(vec![crate::model::InstructionEntry::Name("style".into())]),
        );
        let mut lock = LockFile::default();
        let ctx = make_ctx(&cfg, &project, true);
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        sync_instructions(&ctx, &mut lock, &mut summary, &mut actions).unwrap();
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.installed, 0);

        let _ = fs::remove_dir_all(&src_root);
        let _ = fs::remove_dir_all(&project);
    }
}
