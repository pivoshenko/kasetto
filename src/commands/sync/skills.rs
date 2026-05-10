use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::error::Result;
use crate::fsops::{copy_dir, hash_dir, now_iso, now_unix, select_targets, BrokenSkill};
use crate::model::{Action, SkillEntry, SkillTarget, SkillsField, SourceSpec, State, Summary};
use crate::profile::read_skill_profile_from_dir;
use crate::source::{materialize_source, resolve_source_revision, UNKNOWN_REMOTE_REVISION};
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
        // For remote sources, try to skip materialization when the revision hasn't changed
        if src.source.contains("://") {
            let stored_revision = state
                .skills
                .values()
                .find(|e| e.source == src.source)
                .map(|e| &e.source_revision);
            if let Some(prev) = stored_revision {
                if let Some(current) = resolve_source_revision(src) {
                    if current == *prev
                        && record_unchanged_source_selection(
                            src,
                            &current,
                            state,
                            &mut desired_keys,
                            summary,
                            actions,
                        )
                    {
                        continue;
                    }
                }
            }
        }
        let stage = std::env::temp_dir().join(format!("kasetto-{}-{}", now_unix(), i));
        match materialize_source(src, ctx.cfg_dir, &stage) {
            Ok(materialized) => {
                let (targets, broken_skills) = select_targets(
                    &src.skills,
                    &materialized.available,
                    &materialized.source_root,
                )?;

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

fn record_unchanged_source_selection(
    src: &SourceSpec,
    current_revision: &str,
    state: &State,
    desired_keys: &mut HashSet<String>,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
) -> bool {
    let selected_entries = match &src.skills {
        SkillsField::Wildcard(s) if s == "*" => {
            let entries: Vec<_> = state
                .skills
                .values()
                .filter(|entry| entry.source == src.source)
                .collect();
            if entries.is_empty()
                || !entries
                    .iter()
                    .all(|entry| can_skip_entry(entry, current_revision))
            {
                return false;
            }
            entries
        }
        SkillsField::List(items) => {
            let mut names = Vec::new();
            for item in items {
                let name = skill_target_name(item);
                if !names.contains(&name) {
                    names.push(name);
                }
            }

            let mut entries = Vec::new();
            for name in names {
                let key = format!("{}::{}", src.source, name);
                let Some(entry) = state.skills.get(&key) else {
                    return false;
                };
                if entry.source != src.source || !can_skip_entry(entry, current_revision) {
                    return false;
                }
                entries.push(entry);
            }
            entries
        }
        _ => return false,
    };

    for entry in selected_entries {
        desired_keys.insert(format!("{}::{}", entry.source, entry.skill));
        summary.unchanged += 1;
        actions.push(Action {
            source: Some(entry.source.clone()),
            skill: Some(entry.skill.clone()),
            status: "unchanged".into(),
            error: None,
        });
    }
    true
}

fn skill_target_name(target: &SkillTarget) -> &str {
    match target {
        SkillTarget::Name(name) => name,
        SkillTarget::Obj { name, .. } => name,
    }
}

fn can_skip_entry(entry: &SkillEntry, current_revision: &str) -> bool {
    entry.source_revision == current_revision && Path::new(&entry.destination).exists()
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
        let dest = destination.join(skill_name);

        let is_unchanged_by_revision = can_skip_source_revision(source_revision)
            && state
                .skills
                .get(&key)
                .map(|prev| prev.source_revision == source_revision && dest.exists())
                .unwrap_or(false);

        if is_unchanged_by_revision {
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

        let hash = hash_dir(skill_path)?;

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
                scope: Some(ctx.scope),
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

fn can_skip_source_revision(source_revision: &str) -> bool {
    source_revision != "local" && source_revision != UNKNOWN_REMOTE_REVISION
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

#[cfg(test)]
mod tests {
    use super::*;

    fn source_spec(source: &str, skills: SkillsField) -> SourceSpec {
        SourceSpec {
            source: source.to_string(),
            branch: None,
            git_ref: None,
            sub_dir: None,
            skills,
        }
    }

    fn skill_entry(source: &str, skill: &str, revision: &str, destination: &Path) -> SkillEntry {
        SkillEntry {
            destination: destination.to_string_lossy().to_string(),
            hash: String::new(),
            skill: skill.to_string(),
            description: String::new(),
            source: source.to_string(),
            source_revision: revision.to_string(),
            updated_at: String::new(),
            scope: None,
        }
    }

    #[test]
    fn unchanged_source_list_selection_marks_only_requested_skills() {
        let temp = tempfile::tempdir().unwrap();
        let source = "https://example.test/skills";
        let revision = "branch:main@example-skills-abc123";
        let old_dest = temp.path().join("old");
        let new_dest = temp.path().join("new");
        std::fs::create_dir_all(&old_dest).unwrap();
        std::fs::create_dir_all(&new_dest).unwrap();

        let mut state = State::default();
        state.skills.insert(
            format!("{source}::old"),
            skill_entry(source, "old", revision, &old_dest),
        );
        state.skills.insert(
            format!("{source}::new"),
            skill_entry(source, "new", revision, &new_dest),
        );
        let src = source_spec(
            source,
            SkillsField::List(vec![SkillTarget::Name("new".to_string())]),
        );
        let mut desired_keys = HashSet::new();
        let mut summary = Summary::default();
        let mut actions = Vec::new();

        assert!(record_unchanged_source_selection(
            &src,
            revision,
            &state,
            &mut desired_keys,
            &mut summary,
            &mut actions,
        ));

        assert_eq!(summary.unchanged, 1);
        assert!(desired_keys.contains(&format!("{source}::new")));
        assert!(!desired_keys.contains(&format!("{source}::old")));
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].skill.as_deref(), Some("new"));
    }

    #[test]
    fn unchanged_source_list_selection_requires_requested_skill() {
        let temp = tempfile::tempdir().unwrap();
        let source = "https://example.test/skills";
        let revision = "branch:main@example-skills-abc123";
        let old_dest = temp.path().join("old");
        std::fs::create_dir_all(&old_dest).unwrap();

        let mut state = State::default();
        state.skills.insert(
            format!("{source}::old"),
            skill_entry(source, "old", revision, &old_dest),
        );
        let src = source_spec(
            source,
            SkillsField::List(vec![SkillTarget::Name("new".to_string())]),
        );
        let mut desired_keys = HashSet::new();
        let mut summary = Summary::default();
        let mut actions = Vec::new();

        assert!(!record_unchanged_source_selection(
            &src,
            revision,
            &state,
            &mut desired_keys,
            &mut summary,
            &mut actions,
        ));

        assert!(desired_keys.is_empty());
        assert_eq!(summary.unchanged, 0);
        assert!(actions.is_empty());
    }

    #[test]
    fn unknown_remote_revision_does_not_enable_revision_skip() {
        assert!(!can_skip_source_revision(UNKNOWN_REMOTE_REVISION));
        assert!(!can_skip_source_revision("local"));
        assert!(can_skip_source_revision(
            "branch:main@example-skills-abc123"
        ));
    }
}
