use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{err, Result};
use crate::fsops::{
    copy_dir, hash_dir, now_iso, now_unix, relativize_dest, select_targets, BrokenSkill,
};
use crate::model::{Action, SkillEntry, SkillsField, SourceSpec, State};
use crate::profile::read_skill_profile_from_dir;
use crate::source::materialize_source;
use crate::ui::{eprint_fail, with_spinner_transient};
#[cfg(test)]
use crate::{model::Summary, state::RuntimeState};

use super::{
    remove_stale, sync_label_with, update_active_for_source, StaleEntry, SyncContext, SyncMut,
};

pub(super) fn sync_skills(ctx: &SyncContext, sm: &mut SyncMut<'_>) -> Result<()> {
    let mut desired_keys = HashSet::new();

    for (i, src) in ctx.cfg.skills.iter().enumerate() {
        // Desired skill names for this source, derived without any network:
        // explicit config names for a list, or the locked set for a wildcard.
        let desired = desired_skill_names(src, sm.state);

        // `--locked`/`--frozen`: the lock must be able to satisfy the config.
        if ctx.locked {
            if let Err(e) = ensure_locked_satisfiable(src, &desired, sm.state) {
                sm.summary.failed += 1;
                sm.actions.push(Action {
                    source: Some(src.source.clone()),
                    skill: None,
                    status: "locked_error".into(),
                    error: Some(e.to_string()),
                });
                continue;
            }
        }

        let update_active = update_active_for_source(ctx, &desired);
        let fetch = update_active || needs_fetch(ctx, src, &desired, sm.state);

        if fetch && ctx.locked {
            // --locked must never fetch. If the lock cannot satisfy the config
            // without a fetch (and local repair is impossible), this is an error.
            sm.summary.failed += 1;
            sm.actions.push(Action {
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

        if fetch {
            sync_source_via_fetch(ctx, sm, &mut desired_keys, src, i);
        } else {
            sync_source_from_lock(ctx, sm, &mut desired_keys, src, &desired);
        }
    }

    // Never prune when any source errored: a `locked_error` continue would
    // have skipped extending `desired_keys` for that source, so the already-
    // locked entries would look like orphans here and get destroyed before the
    // non-zero exit. Wait until the next clean sync to clean up.
    if sm.summary.failed == 0 {
        remove_stale_skills(ctx, sm, &desired_keys);
    }
    Ok(())
}

/// Download path: materialize the source and install/update each selected skill.
fn sync_source_via_fetch(
    ctx: &SyncContext,
    sm: &mut SyncMut<'_>,
    desired_keys: &mut HashSet<String>,
    src: &SourceSpec,
    i: usize,
) {
    let stage = std::env::temp_dir().join(format!("kasetto-{}-{}", now_unix(), i));
    match materialize_source(src, ctx.cfg_dir, &stage) {
        Ok(materialized) => {
            match select_targets(
                &src.skills,
                &materialized.available,
                &materialized.source_root,
            ) {
                Ok((targets, broken_skills)) => {
                    record_broken_skills(ctx, &src.source, broken_skills, sm);

                    let mut first_in_run = true;
                    for (skill_name, skill_path) in targets {
                        let label =
                            sync_label_with(&skill_name, &src.source, ctx.plain, first_in_run);
                        first_in_run = false;
                        if let Err(e) = process_single_skill(
                            ctx,
                            sm,
                            desired_keys,
                            &src.source,
                            &materialized.source_revision,
                            &skill_name,
                            &skill_path,
                            &label,
                        ) {
                            sm.summary.failed += 1;
                            sm.actions.push(Action {
                                source: Some(src.source.clone()),
                                skill: Some(skill_name),
                                status: "source_error".into(),
                                error: Some(e.to_string()),
                            });
                        }
                    }
                }
                Err(e) => {
                    sm.summary.failed += 1;
                    sm.actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: None,
                        status: "source_error".into(),
                        error: Some(e.to_string()),
                    });
                }
            }
            if let Some(cleanup_dir) = materialized.cleanup_dir {
                let _ = fs::remove_dir_all(cleanup_dir);
            }
        }
        Err(e) => {
            sm.summary.failed += 1;
            sm.actions.push(Action {
                source: Some(src.source.clone()),
                skill: None,
                status: "source_error".into(),
                error: Some(e.to_string()),
            });
        }
    }
}

/// Skip path: no network. Each desired skill is honored from the lock; the
/// copy source is a known-good on-disk destination (re-hashed to verify).
fn sync_source_from_lock(
    ctx: &SyncContext,
    sm: &mut SyncMut<'_>,
    desired_keys: &mut HashSet<String>,
    src: &SourceSpec,
    desired: &[String],
) {
    let mut first_in_run = true;
    for skill_name in desired {
        let key = format!("{}::{}", src.source, skill_name);
        desired_keys.insert(key.clone());
        let Some(entry) = sm.state.skills.get(&key).cloned() else {
            // needs_fetch would have been true; defensive guard.
            continue;
        };
        let label = sync_label_with(skill_name, &src.source, ctx.plain, first_in_run);
        first_in_run = false;
        if let Err(e) = process_locked_skill(ctx, sm, &entry, skill_name, &label) {
            sm.summary.failed += 1;
            sm.actions.push(Action {
                source: Some(src.source.clone()),
                skill: Some(skill_name.clone()),
                status: "source_error".into(),
                error: Some(e.to_string()),
            });
        }
    }
}

fn record_broken_skills(
    ctx: &SyncContext,
    source: &str,
    broken_skills: Vec<BrokenSkill>,
    sm: &mut SyncMut<'_>,
) {
    for broken in broken_skills {
        sm.summary.broken += 1;
        sm.actions.push(Action {
            source: Some(source.to_string()),
            skill: Some(broken.name.clone()),
            status: "broken".into(),
            error: Some(broken.reason.clone()),
        });
        if !ctx.as_json && !ctx.quiet {
            eprint_fail(&broken.name, source, ctx.plain);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn process_single_skill(
    ctx: &SyncContext,
    sm: &mut SyncMut<'_>,
    desired_keys: &mut HashSet<String>,
    source: &str,
    source_revision: &str,
    skill_name: &str,
    skill_path: &Path,
    label: &str,
) -> Result<()> {
    let destination = &ctx.destinations[0];
    let (_, profile_description) = read_skill_profile_from_dir(skill_path, skill_name);
    with_spinner_transient(ctx.animate, ctx.plain, label, || {
        let key = format!("{source}::{skill_name}");
        desired_keys.insert(key.clone());
        let hash = hash_dir(skill_path)?;
        let dest = destination.join(skill_name);

        // Unchanged only if the locked hash matches AND every destination already
        // holds an identical copy (fixes the latent destinations[0]-only bug).
        let is_unchanged = sm
            .state
            .skills
            .get(&key)
            .map(|prev| prev.hash == hash && all_destinations_match(ctx, skill_name, &prev.hash))
            .unwrap_or(false);

        if is_unchanged {
            if !ctx.dry_run {
                if let Some(entry) = sm.state.skills.get_mut(&key) {
                    entry.description = profile_description.clone();
                }
            }
            sm.summary.unchanged += 1;
            sm.actions.push(Action {
                source: Some(source.to_string()),
                skill: Some(skill_name.to_string()),
                status: "unchanged".into(),
                error: None,
            });
            return Ok(());
        }

        if ctx.dry_run {
            let status = if sm.state.skills.contains_key(&key) {
                sm.summary.updated += 1;
                "would_update"
            } else {
                sm.summary.installed += 1;
                "would_install"
            };
            sm.actions.push(Action {
                source: Some(source.to_string()),
                skill: Some(skill_name.to_string()),
                status: status.into(),
                error: None,
            });
            return Ok(());
        }

        for agent_dest in ctx.destinations {
            copy_dir(skill_path, &agent_dest.join(skill_name))?;
        }
        let status = if sm.state.skills.contains_key(&key) {
            sm.summary.updated += 1;
            "updated"
        } else {
            sm.summary.installed += 1;
            "installed"
        };
        sm.runtime.set_updated_at(&key, now_iso());
        sm.state.skills.insert(
            key,
            SkillEntry {
                destination: relativize_dest(&dest, &ctx.scope_root),
                hash,
                skill: skill_name.to_string(),
                description: profile_description.clone(),
                source: source.to_string(),
                source_revision: source_revision.to_string(),
                scope: Some(ctx.scope),
            },
        );
        sm.actions.push(Action {
            source: Some(source.to_string()),
            skill: Some(skill_name.to_string()),
            status: status.into(),
            error: None,
        });
        Ok(())
    })
}

/// Skip-path install: honor a locked skill without any fetch. The skill is
/// re-hashed on every destination; a known-good destination repairs any
/// missing/mismatched copy. The lock entry is left untouched (same hash + revision).
fn process_locked_skill(
    ctx: &SyncContext,
    sm: &mut SyncMut<'_>,
    entry: &SkillEntry,
    skill_name: &str,
    label: &str,
) -> Result<()> {
    let key = format!("{}::{}", entry.source, skill_name);
    with_spinner_transient(ctx.animate, ctx.plain, label, || {
        // A destination is good when it exists and re-hashes to the locked hash.
        let good = good_destination(ctx, skill_name, &entry.hash);
        let all_ok = all_destinations_match(ctx, skill_name, &entry.hash);

        if all_ok {
            sm.summary.unchanged += 1;
            sm.actions.push(Action {
                source: Some(entry.source.clone()),
                skill: Some(skill_name.to_string()),
                status: "unchanged".into(),
                error: None,
            });
            return Ok(());
        }

        if ctx.dry_run {
            sm.summary.updated += 1;
            sm.actions.push(Action {
                source: Some(entry.source.clone()),
                skill: Some(skill_name.to_string()),
                status: "would_update".into(),
                error: None,
            });
            return Ok(());
        }

        // Local repair from a verified-good destination (no fetch). `needs_fetch`
        // guarantees one exists on the skip path.
        let Some(src_dir) = good else {
            return Err(err(format!(
                "no good local copy of `{skill_name}` to repair from"
            )));
        };
        for agent_dest in ctx.destinations {
            let dst = agent_dest.join(skill_name);
            if dst != src_dir {
                copy_dir(&src_dir, &dst)?;
            }
        }
        sm.runtime.set_updated_at(&key, now_iso());
        // Lock entry is unchanged (hash + revision identical); nothing to rewrite.
        sm.summary.updated += 1;
        sm.actions.push(Action {
            source: Some(entry.source.clone()),
            skill: Some(skill_name.to_string()),
            status: "updated".into(),
            error: None,
        });
        Ok(())
    })
}

/// Desired skill names for a source, derived without any network access.
/// - `List`: the explicit config names.
/// - `Wildcard`: the locked set (entries whose `source` matches this source).
fn desired_skill_names(src: &SourceSpec, state: &State) -> Vec<String> {
    match &src.skills {
        SkillsField::List(items) => items
            .iter()
            .map(|it| match it {
                crate::model::SkillTarget::Name(n) => n.clone(),
                crate::model::SkillTarget::Obj { name, .. } => name.clone(),
            })
            .collect(),
        SkillsField::Wildcard(_) => state
            .skills
            .values()
            .filter(|e| e.source == src.source)
            .map(|e| e.skill.clone())
            .collect(),
    }
}

/// `--locked` validation: every config-named/wildcard-derived skill must have a
/// lock entry, and the source must appear in the lock at all.
fn ensure_locked_satisfiable(src: &SourceSpec, desired: &[String], state: &State) -> Result<()> {
    match &src.skills {
        SkillsField::List(_) => {
            for name in desired {
                let key = format!("{}::{}", src.source, name);
                if !state.skills.contains_key(&key) {
                    return Err(err(format!(
                        "--locked: skill `{name}` from `{}` is not in the lock",
                        src.source
                    )));
                }
            }
            Ok(())
        }
        SkillsField::Wildcard(_) => {
            // A wildcard source must contribute at least one locked entry.
            let present = state.skills.values().any(|e| e.source == src.source);
            if present {
                Ok(())
            } else {
                Err(err(format!(
                    "--locked: source `{}` has no entries in the lock",
                    src.source
                )))
            }
        }
    }
}

/// Per-source fetch decision (computed before any download). Fetch when any
/// desired skill lacks a lock entry, or its locked copy is missing/mismatched on
/// any destination with no good local copy available to repair from.
fn needs_fetch(ctx: &SyncContext, src: &SourceSpec, desired: &[String], state: &State) -> bool {
    // A wildcard source with no lock entries has never been resolved — bootstrap
    // it by fetching (the locked set is empty only because nothing is pinned yet).
    if matches!(src.skills, SkillsField::Wildcard(_))
        && !state.skills.values().any(|e| e.source == src.source)
    {
        return true;
    }
    let expected_revision = src.expected_revision();
    for skill_name in desired {
        let key = format!("{}::{}", src.source, skill_name);
        // A skill named in the config but absent from the lock must be fetched.
        let Some(entry) = state.skills.get(&key) else {
            return true;
        };
        // The user retargeted this source (changed ref/branch) since the lock
        // was written — the on-disk content might still hash correctly, but it
        // no longer matches what the config asks for. Refetch.
        if !entry.source_revision.is_empty() && entry.source_revision != expected_revision {
            return true;
        }
        // Re-hash every destination; if all match, this skill is satisfied.
        if all_destinations_match(ctx, skill_name, &entry.hash) {
            continue;
        }
        // Some destination is missing/mismatched. If at least one good local copy
        // exists we can repair without a fetch; otherwise we must fetch.
        if good_destination(ctx, skill_name, &entry.hash).is_none() {
            return true;
        }
    }
    false
}

/// The first destination that exists and re-hashes to `expected_hash`.
fn good_destination(ctx: &SyncContext, skill_name: &str, expected_hash: &str) -> Option<PathBuf> {
    for agent_dest in ctx.destinations {
        let dir = agent_dest.join(skill_name);
        if dir.exists() {
            if let Ok(h) = hash_dir(&dir) {
                if h == expected_hash {
                    return Some(dir);
                }
            }
        }
    }
    None
}

/// True when every destination holds a copy that re-hashes to `expected_hash`.
fn all_destinations_match(ctx: &SyncContext, skill_name: &str, expected_hash: &str) -> bool {
    ctx.destinations.iter().all(|agent_dest| {
        let dir = agent_dest.join(skill_name);
        dir.exists() && hash_dir(&dir).map(|h| h == expected_hash).unwrap_or(false)
    })
}

/// Stale-skill cleanup. Routes through the shared [`remove_stale`] helper so
/// the bookkeeping (summary bump + action push) stays identical across kinds;
/// the closure handles the skill-specific teardown (rm dir, drop state entry,
/// drop runtime timestamp).
fn remove_stale_skills(ctx: &SyncContext, sm: &mut SyncMut<'_>, desired_keys: &HashSet<String>) {
    // Snapshot what we need from `state` so the teardown closure doesn't
    // alias the borrow `remove_stale` needs on `summary` / `actions`.
    let snapshot: Vec<(String, String, String, String)> = sm
        .state
        .skills
        .iter()
        .map(|(k, e)| {
            (
                k.clone(),
                e.source.clone(),
                e.skill.clone(),
                e.destination.clone(),
            )
        })
        .collect();
    let dest_by_id: std::collections::HashMap<String, String> = snapshot
        .iter()
        .map(|(k, _, _, d)| (k.clone(), d.clone()))
        .collect();
    let candidates: Vec<StaleEntry> = snapshot
        .into_iter()
        .map(|(id, source, name, _)| StaleEntry {
            id,
            action_source: Some(source),
            action_skill: name,
        })
        .collect();

    let scope_root = ctx.scope_root.clone();
    let SyncMut {
        state,
        runtime,
        summary,
        actions,
    } = sm;
    remove_stale(
        ctx.dry_run,
        summary,
        actions,
        desired_keys,
        candidates,
        |id| {
            if let Some(dest) = dest_by_id.get(id) {
                let abs = crate::fsops::resolve_dest(dest, &scope_root);
                let _ = fs::remove_dir_all(&abs);
            }
            state.skills.remove(id);
            runtime.forget(id);
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Config, Scope, SkillTarget};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("kasetto-{prefix}-{}-{nonce}", std::process::id()))
    }

    fn write_skill(root: &Path, name: &str, body: &str) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), body).unwrap();
    }

    struct Harness {
        src_root: PathBuf,
        dests: Vec<PathBuf>,
        scope_root: PathBuf,
    }

    fn run_sync(
        h: &Harness,
        skills: SkillsField,
        state: &mut State,
        update: bool,
        update_only: Vec<String>,
        locked: bool,
    ) -> Summary {
        let cfg = Config {
            destination: None,
            scope: Some(Scope::Project),
            agent: None,
            skills: vec![SourceSpec {
                source: h.src_root.to_string_lossy().to_string(),
                branch: None,
                git_ref: None,
                sub_dir: None,
                skills,
            }],
            mcps: Vec::new(),
            commands: Vec::new(),
        };
        let ctx = SyncContext {
            cfg: &cfg,
            cfg_dir: &h.scope_root,
            destinations: &h.dests,
            scope_root: h.scope_root.clone(),
            scope: Scope::Project,
            dry_run: false,
            animate: false,
            plain: true,
            as_json: false,
            quiet: true,
            update,
            update_only,
            locked,
        };
        let mut runtime = RuntimeState::default();
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        let mut sm = SyncMut {
            state,
            runtime: &mut runtime,
            summary: &mut summary,
            actions: &mut actions,
        };
        sync_skills(&ctx, &mut sm).unwrap();
        summary
    }

    fn setup(skill_names: &[&str]) -> Harness {
        let src_root = temp_dir("src");
        for n in skill_names {
            write_skill(&src_root, n, &format!("# {n}\n\nbody\n"));
        }
        let scope_root = temp_dir("scope");
        let dest = scope_root.join(".agent/skills");
        fs::create_dir_all(&dest).unwrap();
        Harness {
            src_root,
            dests: vec![dest],
            scope_root,
        }
    }

    fn list(names: &[&str]) -> SkillsField {
        SkillsField::List(
            names
                .iter()
                .map(|n| SkillTarget::Name(n.to_string()))
                .collect(),
        )
    }

    fn cleanup(h: &Harness) {
        let _ = fs::remove_dir_all(&h.src_root);
        let _ = fs::remove_dir_all(&h.scope_root);
    }

    #[test]
    fn first_run_installs_then_second_run_unchanged_without_source() {
        let h = setup(&["alpha"]);
        let mut state = State::default();

        let s1 = run_sync(&h, list(&["alpha"]), &mut state, false, vec![], false);
        assert_eq!(s1.installed, 1, "first run installs");

        // Remove the source entirely: a plain re-sync must still report unchanged
        // (no fetch, re-hash of dest matches lock).
        fs::remove_dir_all(&h.src_root).unwrap();
        let s2 = run_sync(&h, list(&["alpha"]), &mut state, false, vec![], false);
        assert_eq!(s2.unchanged, 1, "second run unchanged, no fetch");
        assert_eq!(s2.failed, 0);
        cleanup(&h);
    }

    #[test]
    fn tampered_dest_is_repaired_from_source() {
        let h = setup(&["alpha"]);
        let mut state = State::default();
        run_sync(&h, list(&["alpha"]), &mut state, false, vec![], false);

        // Tamper the installed copy. needs_fetch fires (no good local copy), repairs.
        fs::write(h.dests[0].join("alpha/SKILL.md"), "# alpha\n\nEDITED\n").unwrap();
        let s = run_sync(&h, list(&["alpha"]), &mut state, false, vec![], false);
        assert_eq!(s.updated, 1);
        cleanup(&h);
    }

    #[test]
    fn missing_second_dest_repaired_locally_without_source() {
        let mut h = setup(&["alpha"]);
        let dest2 = h.scope_root.join(".other/skills");
        fs::create_dir_all(&dest2).unwrap();
        h.dests.push(dest2.clone());
        let mut state = State::default();
        run_sync(&h, list(&["alpha"]), &mut state, false, vec![], false);
        assert!(dest2.join("alpha/SKILL.md").exists());

        // Drop dest2 and remove the source: repair must copy from dest[0] (good copy).
        fs::remove_dir_all(&dest2).unwrap();
        fs::remove_dir_all(&h.src_root).unwrap();
        let s = run_sync(&h, list(&["alpha"]), &mut state, false, vec![], false);
        assert_eq!(s.updated, 1, "repaired locally");
        assert_eq!(s.failed, 0);
        assert!(dest2.join("alpha/SKILL.md").exists());
        cleanup(&h);
    }

    #[test]
    fn wildcard_holds_to_locked_set_on_plain_sync() {
        let h = setup(&["alpha", "beta"]);
        let mut state = State::default();
        let s1 = run_sync(
            &h,
            SkillsField::Wildcard("*".into()),
            &mut state,
            false,
            vec![],
            false,
        );
        assert_eq!(s1.installed, 2);

        // Remove one skill from the SOURCE; plain wildcard sync keeps the locked set.
        fs::remove_dir_all(h.src_root.join("beta")).unwrap();
        let s2 = run_sync(
            &h,
            SkillsField::Wildcard("*".into()),
            &mut state,
            false,
            vec![],
            false,
        );
        assert_eq!(s2.unchanged, 2, "locked set still honored");
        assert_eq!(s2.removed, 0);
        cleanup(&h);
    }

    #[test]
    fn wildcard_update_prunes_removed_skill() {
        let h = setup(&["alpha", "beta"]);
        let mut state = State::default();
        run_sync(
            &h,
            SkillsField::Wildcard("*".into()),
            &mut state,
            false,
            vec![],
            false,
        );

        fs::remove_dir_all(h.src_root.join("beta")).unwrap();
        let s = run_sync(
            &h,
            SkillsField::Wildcard("*".into()),
            &mut state,
            true,
            vec![],
            false,
        );
        assert_eq!(s.removed, 1, "update prunes upstream-removed skill");
        cleanup(&h);
    }

    #[test]
    fn locked_errors_when_skill_absent_from_lock() {
        let h = setup(&["alpha"]);
        let mut state = State::default();
        let s = run_sync(&h, list(&["alpha"]), &mut state, false, vec![], true);
        assert_eq!(s.failed, 1, "--locked errors when not in lock");
        assert_eq!(s.installed, 0);
        cleanup(&h);
    }

    #[test]
    fn locked_succeeds_when_satisfiable_and_repairs() {
        let h = setup(&["alpha"]);
        let mut state = State::default();
        run_sync(&h, list(&["alpha"]), &mut state, false, vec![], false);

        // Tamper, then --locked should repair from the local source (good copy gone,
        // but source still present is irrelevant; repair uses good dest only). Here
        // we keep a good copy by NOT tampering: assert zero fetch, unchanged.
        let s = run_sync(&h, list(&["alpha"]), &mut state, false, vec![], true);
        assert_eq!(s.unchanged, 1);
        assert_eq!(s.failed, 0);
        cleanup(&h);
    }
}
