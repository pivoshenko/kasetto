use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::error::Result;
use crate::fsops::{copy_dir, hash_dir, now_iso, now_unix, select_targets, BrokenSkill};
use crate::model::{Action, SkillEntry, State, Summary};
use crate::profile::read_skill_profile_from_dir;
use crate::source::materialize_source;
use crate::ui::{eprint_fail, with_spinner};

use super::{sync_label, SyncContext};

pub(super) fn sync_skills(
    ctx: &SyncContext,
    state: &mut State,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
) -> Result<()> {
    let mut desired_keys = HashSet::new();
    let destination = &ctx.destinations[0];

    for (i, src) in ctx.cfg.skills.iter().enumerate() {
        let stage = std::env::temp_dir().join(format!("kasetto-{}-{}", now_unix(), i));
        match materialize_source(src, ctx.cfg_dir, &stage) {
            Ok(materialized) => {
                let (targets, broken_skills) =
                    select_targets(&src.skills, &materialized.available)?;

                record_broken_skills(ctx, &src.source, broken_skills, summary, actions);

                for (skill_name, skill_path) in targets {
                    let label = sync_label("skill", &skill_name, &src.source, ctx.plain);
                    process_single_skill(
                        ctx,
                        state,
                        summary,
                        actions,
                        &mut desired_keys,
                        destination,
                        &src.source,
                        &materialized.source_revision,
                        &skill_name,
                        &skill_path,
                        &label,
                    )?;
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

    remove_stale_skills(ctx, state, &desired_keys, summary, actions);
    Ok(())
}

fn record_broken_skills(
    ctx: &SyncContext,
    source: &str,
    broken_skills: Vec<BrokenSkill>,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
) {
    for broken in broken_skills {
        summary.broken += 1;
        actions.push(Action {
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
    state: &mut State,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
    desired_keys: &mut HashSet<String>,
    destination: &Path,
    source: &str,
    source_revision: &str,
    skill_name: &str,
    skill_path: &Path,
    label: &str,
) -> Result<()> {
    let (_, profile_description) = read_skill_profile_from_dir(skill_path, skill_name);
    with_spinner(ctx.animate, ctx.plain, label, || {
        let key = format!("{source}::{skill_name}");
        desired_keys.insert(key.clone());
        let hash = hash_dir(skill_path)?;
        let dest = destination.join(skill_name);

        let is_unchanged = state
            .skills
            .get(&key)
            .map(|prev| prev.hash == hash && dest.exists())
            .unwrap_or(false);

        if is_unchanged {
            if !ctx.dry_run {
                if let Some(entry) = state.skills.get_mut(&key) {
                    entry.description = profile_description.clone();
                }
            }
            summary.unchanged += 1;
            actions.push(Action {
                source: Some(source.to_string()),
                skill: Some(skill_name.to_string()),
                status: "unchanged".into(),
                error: None,
            });
            return Ok(());
        }

        if ctx.dry_run {
            let status = if state.skills.contains_key(&key) {
                summary.updated += 1;
                "would_update"
            } else {
                summary.installed += 1;
                "would_install"
            };
            actions.push(Action {
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
        let status = if state.skills.contains_key(&key) {
            summary.updated += 1;
            "updated"
        } else {
            summary.installed += 1;
            "installed"
        };
        state.skills.insert(
            key,
            SkillEntry {
                destination: dest.to_string_lossy().to_string(),
                hash,
                skill: skill_name.to_string(),
                description: profile_description.clone(),
                source: source.to_string(),
                source_revision: source_revision.to_string(),
                updated_at: now_iso(),
            },
        );
        actions.push(Action {
            source: Some(source.to_string()),
            skill: Some(skill_name.to_string()),
            status: status.into(),
            error: None,
        });
        Ok(())
    })
}

fn remove_stale_skills(
    ctx: &SyncContext,
    state: &mut State,
    desired_keys: &HashSet<String>,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
) {
    let existing_keys: Vec<String> = state.skills.keys().cloned().collect();
    for k in existing_keys {
        if desired_keys.contains(&k) {
            continue;
        }
        if let Some(entry) = state.skills.get(&k).cloned() {
            if ctx.dry_run {
                summary.removed += 1;
                actions.push(Action {
                    source: Some(entry.source),
                    skill: Some(entry.skill),
                    status: "would_remove".into(),
                    error: None,
                });
            } else {
                let _ = fs::remove_dir_all(&entry.destination);
                state.skills.remove(&k);
                summary.removed += 1;
                actions.push(Action {
                    source: Some(entry.source),
                    skill: Some(entry.skill),
                    status: "removed".into(),
                    error: None,
                });
            }
        }
    }
}
