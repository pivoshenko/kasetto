use std::collections::HashSet;
use std::fs;

use crate::banner::print_banner;
use crate::error::Result;
use crate::fsops::{
    copy_dir, hash_dir, load_config_any, load_state, materialize_source, now_iso, now_unix,
    resolve_destination, save_report, save_state, select_targets,
};
use crate::model::{Action, Report, SkillEntry, Summary};
use crate::profile::read_skill_profile_from_dir;
use crate::ui::{animations_enabled, status_chip, with_spinner};

pub fn run(
    config_path: &str,
    dry_run: bool,
    quiet: bool,
    as_json: bool,
    plain: bool,
    verbose: bool,
) -> Result<()> {
    let animate = animations_enabled(quiet, as_json, plain);
    if !quiet && !as_json {
        if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
            if plain {
                println!("kasetto | カセット");
            } else {
                print_banner();
            }
        }
    }

    let (cfg, cfg_dir, cfg_label) = load_config_any(config_path)?;
    let destination = resolve_destination(&cfg_dir, &cfg)?;
    if !dry_run {
        fs::create_dir_all(&destination)?;
    }

    let mut state = load_state()?;
    let mut desired_keys = HashSet::new();
    let mut summary = Summary::default();
    let mut actions = Vec::new();

    for (i, src) in cfg.skills.iter().enumerate() {
        let stage = std::env::temp_dir().join(format!("kasetto-{}-{}", now_unix(), i));
        match materialize_source(src, &cfg_dir, &stage) {
            Ok(materialized) => {
                let (targets, broken_skills) =
                    select_targets(&src.skills, &materialized.available)?;
                for broken in broken_skills {
                    let broken_name = broken.name.clone();
                    let broken_reason = broken.reason.clone();
                    summary.broken += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(broken_name.clone()),
                        status: "broken".into(),
                        error: Some(broken_reason.clone()),
                    });
                    if !as_json && !quiet {
                        if plain {
                            eprintln!("x Failed {} {}", broken_name, src.source);
                        } else {
                            eprintln!(
                                "\x1b[31mx\x1b[0m Failed \x1b[1;35m{}\x1b[0m \x1b[90m{}\x1b[0m",
                                broken_name, src.source
                            );
                        }
                    }
                }
                for (skill_name, skill_path) in targets {
                    let (_, profile_description) =
                        read_skill_profile_from_dir(&skill_path, &skill_name);
                    let sync_step = if plain {
                        format!("Syncing {} {}", skill_name, src.source)
                    } else {
                        format!(
                            "Syncing \x1b[1;35m{}\x1b[0m \x1b[90m{}\x1b[0m",
                            skill_name, src.source
                        )
                    };
                    with_spinner(animate, plain, &sync_step, || {
                        let key = format!("{}::{}", src.source, skill_name);
                        desired_keys.insert(key.clone());
                        let hash = hash_dir(&skill_path)?;
                        let dest = destination.join(&skill_name);
                        let is_unchanged = state
                            .skills
                            .get(&key)
                            .map(|prev| prev.hash == hash && dest.exists())
                            .unwrap_or(false);
                        if is_unchanged {
                            if !dry_run {
                                if let Some(entry) = state.skills.get_mut(&key) {
                                    entry.description = profile_description.clone();
                                }
                            }
                            summary.unchanged += 1;
                            actions.push(Action {
                                source: Some(src.source.clone()),
                                skill: Some(skill_name.clone()),
                                status: "unchanged".into(),
                                error: None,
                            });
                            return Ok(());
                        }

                        if dry_run {
                            let status = if state.skills.contains_key(&key) {
                                "would_update"
                            } else {
                                "would_install"
                            };
                            if status == "would_install" {
                                summary.installed += 1
                            } else {
                                summary.updated += 1
                            }
                            actions.push(Action {
                                source: Some(src.source.clone()),
                                skill: Some(skill_name.clone()),
                                status: status.into(),
                                error: None,
                            });
                            return Ok(());
                        }

                        copy_dir(&skill_path, &dest)?;
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
                                skill: skill_name.clone(),
                                description: profile_description.clone(),
                                source: src.source.clone(),
                                source_revision: materialized.source_revision.clone(),
                                updated_at: now_iso(),
                            },
                        );
                        actions.push(Action {
                            source: Some(src.source.clone()),
                            skill: Some(skill_name.clone()),
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

    let existing_keys: Vec<String> = state.skills.keys().cloned().collect();
    for k in existing_keys {
        if desired_keys.contains(&k) {
            continue;
        }
        if let Some(entry) = state.skills.get(&k).cloned() {
            if dry_run {
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

    if !dry_run {
        state.last_run = Some(now_iso());
        save_state(&state)?;
    }

    let report = Report {
        run_id: format!("{}", now_unix()),
        config: cfg_label,
        destination: destination.to_string_lossy().to_string(),
        dry_run,
        summary,
        actions,
    };
    let _manifest_path = save_report(&report)?;

    if as_json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if !quiet {
        if plain {
            println!();
            println!("  Installed: {}", report.summary.installed);
            println!("  Updated:   {}", report.summary.updated);
            println!("  Removed:   {}", report.summary.removed);
            println!("  Unchanged: {}", report.summary.unchanged);
            println!("  Broken:    {}", report.summary.broken);
            println!("  Failed:    {}", report.summary.failed);
        } else {
            println!();
            println!(
                "  \x1b[32mInstalled\x1b[0m: {}   \x1b[36mUpdated\x1b[0m: {}   \x1b[33mRemoved\x1b[0m: {}",
                report.summary.installed, report.summary.updated, report.summary.removed
            );
            println!(
                "  \x1b[90mUnchanged\x1b[0m: {}   \x1b[35mBroken\x1b[0m: {}   \x1b[31mFailed\x1b[0m: {}",
                report.summary.unchanged, report.summary.broken, report.summary.failed
            );
        }

        if verbose {
            println!("\nActions:");
            for a in &report.actions {
                let status = status_chip(&a.status, plain);
                let src = a.source.as_deref().unwrap_or("-");
                let skill = a.skill.as_deref().unwrap_or("-");
                if let Some(err) = &a.error {
                    println!("  {} {} :: {} -> {}", status, src, skill, err);
                } else {
                    println!("  {} {} :: {}", status, src, skill);
                }
            }
        }
    }

    if report.summary.failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}
