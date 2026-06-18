use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::error::{err, Result};
use crate::fsops::{hash_file, now_unix, relativize_dest, resolve_rule_targets};
use crate::lock::LockFile;
use crate::model::{Action, RuleTarget, RulesField, Summary};
use crate::rules::{apply_rule, dest_present, dest_token, teardown_dest};
use crate::source::{discover_rules, materialize_source, resolve_rule_entry};
use crate::ui::with_spinner_transient;

use super::{
    remove_stale as remove_stale_shared, sync_label_with, update_active_for_source, StaleEntry,
    SyncContext,
};

struct PendingRule {
    source: String,
    name: String,
    src_path: PathBuf,
    hash: String,
    asset_id: String,
    is_new: bool,
    source_revision: String,
}

pub(super) fn sync_rules(
    ctx: &SyncContext,
    lock: &mut LockFile,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
) -> Result<()> {
    let targets = resolve_rule_targets(ctx.cfg, ctx.scope, ctx.cfg_dir)?;

    // Dropping the `rules:` block orphans every installed rule — skip the
    // install loop but still run remove_stale with an empty desired-set so the
    // lock and on-disk blocks/files both get cleaned up.
    if ctx.cfg.rules.is_empty() {
        remove_stale(ctx, lock, summary, actions, &HashSet::new());
        return Ok(());
    }

    if targets.is_empty() {
        return Ok(());
    }

    let mut desired_ids = HashSet::new();
    let mut pending: Vec<PendingRule> = Vec::new();
    let mut cleanup_dirs: Vec<PathBuf> = Vec::new();

    for (i, src) in ctx.cfg.rules.iter().enumerate() {
        let desired_names = desired_rule_names(src, lock);

        if ctx.locked {
            if let Err(e) = ensure_locked_satisfiable_rules(src, &desired_names, lock) {
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
        let fetch = update_active || needs_fetch_rules(src, &desired_names, lock, &targets);

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
                let asset_id = format!("rules::{}::{}", src.source, name);
                desired_ids.insert(asset_id);
                let label = sync_label_with(name, &src.source, ctx.plain, first_in_run);
                first_in_run = false;
                with_spinner_transient(ctx.animate, ctx.plain, &label, || {
                    summary.unchanged += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(format!("rule:{name}")),
                        status: "unchanged".into(),
                        error: None,
                    });
                    Ok(())
                })?;
            }
            continue;
        }

        let stage = std::env::temp_dir().join(format!("kasetto-rule-{}-{}", now_unix(), i));
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
            .unwrap_or(&materialized.source_root);

        let selected: Vec<(String, PathBuf)> = match &src.rules {
            RulesField::Wildcard(s) if s == "*" => match discover_rules(root) {
                Ok(map) => map.into_iter().collect(),
                Err(e) => {
                    summary.broken += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some("rule".into()),
                        status: "broken".into(),
                        error: Some(e.to_string()),
                    });
                    if let Some(d) = materialized.cleanup_dir {
                        cleanup_dirs.push(d);
                    }
                    continue;
                }
            },
            RulesField::Wildcard(s) => {
                summary.broken += 1;
                actions.push(Action {
                    source: Some(src.source.clone()),
                    skill: Some("rule".into()),
                    status: "broken".into(),
                    error: Some(format!(
                        "invalid rules value \"{s}\": expected \"*\" or a list"
                    )),
                });
                if let Some(d) = materialized.cleanup_dir {
                    cleanup_dirs.push(d);
                }
                continue;
            }
            RulesField::List(entries) => {
                let mut out = Vec::new();
                for entry in entries {
                    let entry_name = match entry {
                        crate::model::RuleEntry::Name(n) => n.clone(),
                        crate::model::RuleEntry::Obj { name, .. } => name.clone(),
                    };
                    match resolve_rule_entry(root, entry) {
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

        if selected.is_empty() && matches!(&src.rules, RulesField::Wildcard(s) if s == "*") {
            summary.broken += 1;
            actions.push(Action {
                source: Some(src.source.clone()),
                skill: Some("rule".into()),
                status: "broken".into(),
                error: Some("no rules found in source (expected rules/*.md)".into()),
            });
        }

        let mut first_in_run = true;
        for (name, src_path) in selected {
            let asset_id = format!("rules::{}::{}", src.source, name);
            desired_ids.insert(asset_id.clone());
            let row_first = first_in_run;
            first_in_run = false;
            let hash = match hash_file(&src_path) {
                Ok(h) => h,
                Err(e) => {
                    summary.broken += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(format!("rule:{name}")),
                        status: "broken".into(),
                        error: Some(e.to_string()),
                    });
                    continue;
                }
            };

            // Unchanged requires: stored hash matches AND every expected
            // destination is present (for aggregate files, the managed block).
            let existing = lock.get_tracked_asset("rules", &asset_id);
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
                        skill: Some(format!("rule:{name}")),
                        status: "unchanged".into(),
                        error: None,
                    });
                    Ok(())
                })?;
            } else {
                pending.push(PendingRule {
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

/// Desired rule names for a source, derived without any network access.
fn desired_rule_names(src: &crate::model::RuleSourceSpec, lock: &LockFile) -> Vec<String> {
    match &src.rules {
        RulesField::List(entries) => entries
            .iter()
            .map(|e| match e {
                crate::model::RuleEntry::Name(n) => n.clone(),
                crate::model::RuleEntry::Obj { name, .. } => name.clone(),
            })
            .collect(),
        RulesField::Wildcard(s) if s == "*" => lock
            .assets
            .values()
            .filter(|a| a.kind == "rules" && a.source == src.source)
            .map(|a| a.name.clone())
            .collect(),
        RulesField::Wildcard(_) => Vec::new(),
    }
}

/// Per-source fetch decision (computed before any download). Fetch when a
/// wildcard source has never been resolved, when any desired rule lacks a lock
/// entry, when the source was retargeted, or when any expected destination is
/// missing (a transform has no local repair path).
fn needs_fetch_rules(
    src: &crate::model::RuleSourceSpec,
    desired: &[String],
    lock: &LockFile,
    targets: &[RuleTarget],
) -> bool {
    if matches!(&src.rules, RulesField::Wildcard(s) if s == "*")
        && !lock
            .assets
            .values()
            .any(|a| a.kind == "rules" && a.source == src.source)
    {
        return true;
    }
    let expected_revision = src.as_source_spec().expected_revision();
    for name in desired {
        let asset_id = format!("rules::{}::{}", src.source, name);
        let Some(asset) = lock.assets.get(&asset_id).filter(|a| a.kind == "rules") else {
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

/// `--locked` validation: every config-named rule must have a lock entry, and a
/// wildcard source must contribute at least one locked rule-asset.
fn ensure_locked_satisfiable_rules(
    src: &crate::model::RuleSourceSpec,
    desired: &[String],
    lock: &LockFile,
) -> Result<()> {
    match &src.rules {
        RulesField::List(_) => {
            for name in desired {
                let asset_id = format!("rules::{}::{}", src.source, name);
                if lock.get_tracked_asset("rules", &asset_id).is_none() {
                    return Err(err(format!(
                        "--locked: rule `{name}` from `{}` is not in the lock",
                        src.source
                    )));
                }
            }
            Ok(())
        }
        RulesField::Wildcard(_) => {
            let present = lock
                .assets
                .values()
                .any(|a| a.kind == "rules" && a.source == src.source);
            if present {
                Ok(())
            } else {
                Err(err(format!(
                    "--locked: source `{}` has no rule entries in the lock",
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
    targets: &[RuleTarget],
    pending: &[PendingRule],
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
                    let dest =
                        apply_rule(&p.src_path, target, &p.source, &p.name).map_err(|e| {
                            err(format!(
                                "failed to apply rule `{}` to {}: {e}",
                                p.name,
                                target.path.display()
                            ))
                        })?;
                    let rel = relativize_dest(&dest, &ctx.scope_root);
                    written.push(dest_token(target, &rel));
                }
                lock.save_tracked_asset(
                    &p.asset_id,
                    crate::lock::AssetEntry {
                        kind: "rules".into(),
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
                skill: Some(format!("rule:{}", p.name)),
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
        .filter(|(_, a)| a.kind == "rules")
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
            action_skill: format!("rule:{name}"),
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
    use crate::model::{Agent, AgentField, Config, RuleSourceSpec, RulesField, Scope};

    fn write(path: &std::path::Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn base_cfg(src_root: &std::path::Path, agents: Vec<Agent>, rules: RulesField) -> Config {
        Config {
            destination: None,
            scope: Some(Scope::Project),
            agent: Some(AgentField::Many(agents)),
            skills: Vec::new(),
            mcps: Vec::new(),
            commands: Vec::new(),
            rules: vec![RuleSourceSpec {
                source: src_root.to_string_lossy().to_string(),
                branch: None,
                git_ref: None,
                sub_dir: None,
                rules,
            }],
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
        }
    }

    #[test]
    fn sync_writes_aggregate_and_dir_formats_then_prunes() {
        let src_root = temp_dir("kasetto-rule-src");
        write(
            &src_root.join("rules/style.mdc"),
            "---\ndescription: house style\nglobs: \"*.rs\"\n---\nUse tabs.\n",
        );

        let project = temp_dir("kasetto-rule-proj");
        fs::create_dir_all(&project).unwrap();
        // Pre-existing user CLAUDE.md content that must survive.
        write(&project.join("CLAUDE.md"), "# Project\n\nUser notes.\n");

        let cfg = base_cfg(
            &src_root,
            vec![Agent::ClaudeCode, Agent::Cursor, Agent::Windsurf],
            RulesField::Wildcard("*".into()),
        );
        let mut lock = LockFile::default();
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        let ctx = make_ctx(&cfg, &project, false);
        sync_rules(&ctx, &mut lock, &mut summary, &mut actions).unwrap();
        assert_eq!(summary.installed, 1);

        // Claude aggregate: managed block added, user content preserved.
        let claude = fs::read_to_string(project.join("CLAUDE.md")).unwrap();
        assert!(claude.contains("User notes."));
        assert!(claude.contains("Use tabs."));
        assert!(claude.contains("kasetto:rule:style-"));
        // Cursor mdc: per-rule file with reconstructed frontmatter.
        let cursor = fs::read_to_string(project.join(".cursor/rules/style.mdc")).unwrap();
        assert!(cursor.contains("globs: *.rs"));
        // Windsurf plain dir: body only.
        let windsurf = fs::read_to_string(project.join(".windsurf/rules/style.md")).unwrap();
        assert!(!windsurf.contains("description:"));
        assert!(windsurf.contains("Use tabs."));

        // One rules asset tracked.
        assert_eq!(
            lock.assets.values().filter(|a| a.kind == "rules").count(),
            1
        );

        // Drop the rules: remove_stale strips the block + deletes the dir files.
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
        let src_root = temp_dir("kasetto-rule-src2");
        write(&src_root.join("rules/style.md"), "---\n---\nbody\n");
        let project = temp_dir("kasetto-rule-proj2");
        fs::create_dir_all(&project).unwrap();

        let cfg = base_cfg(
            &src_root,
            vec![Agent::ClaudeCode],
            RulesField::Wildcard("*".into()),
        );
        let mut lock = LockFile::default();
        let ctx = make_ctx(&cfg, &project, false);
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        sync_rules(&ctx, &mut lock, &mut summary, &mut actions).unwrap();
        assert_eq!(summary.installed, 1);

        fs::remove_dir_all(&src_root).unwrap();
        let mut summary2 = Summary::default();
        let mut actions2 = Vec::new();
        sync_rules(&ctx, &mut lock, &mut summary2, &mut actions2).unwrap();
        assert_eq!(summary2.unchanged, 1);
        assert_eq!(summary2.failed, 0);
        assert_eq!(summary2.removed, 0);

        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn locked_errors_when_rule_absent_from_lock() {
        let src_root = temp_dir("kasetto-rule-src3");
        write(&src_root.join("rules/style.md"), "---\n---\nbody\n");
        let project = temp_dir("kasetto-rule-proj3");
        fs::create_dir_all(&project).unwrap();

        let cfg = base_cfg(
            &src_root,
            vec![Agent::ClaudeCode],
            RulesField::List(vec![crate::model::RuleEntry::Name("style".into())]),
        );
        let mut lock = LockFile::default();
        let ctx = make_ctx(&cfg, &project, true);
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        sync_rules(&ctx, &mut lock, &mut summary, &mut actions).unwrap();
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.installed, 0);

        let _ = fs::remove_dir_all(&src_root);
        let _ = fs::remove_dir_all(&project);
    }
}
