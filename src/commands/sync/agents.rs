//! Module that contains the `sync` leg that installs, updates, and prunes
//! subagent definitions.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::error::{err, Result};
use crate::fsops::{hash_file, now_unix, relativize_dest, resolve_agent_targets};
use crate::lock::LockFile;
use crate::model::{Action, AgentSourceSpec, AgentTarget, AgentsField, Scope, Summary};
use crate::source::{discover_agents, materialize_source, resolve_agent_entry};
use crate::subagents::{apply_agent, destination_path, dropped_fields, validate_agent};
use crate::ui::with_spinner_transient;

use super::{
    remove_stale as remove_stale_shared, sync_label_with, update_active_for_source, StaleEntry,
    SyncContext,
};

struct PendingAgent {
    source: String,
    name: String,
    src_path: PathBuf,
    hash: String,
    asset_id: String,
    is_new: bool,
    source_revision: String,
}

/// Install, update, and prune subagents so every agent target matches the config.
///
/// Returns one warning line per (agent, target dir) whose format dropped source
/// fields, for real writes and dry runs alike. The unchanged path never renders,
/// so a repeat sync stays silent.
pub(super) fn sync_agents(
    ctx: &SyncContext,
    lock: &mut LockFile,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
) -> Result<Vec<String>> {
    let targets = resolve_agent_targets(ctx.cfg, ctx.scope, ctx.cfg_dir)?;

    // Same as commands: a removed `agents:` block orphans everything this leg
    // installed, so prune with an empty desired set and skip the install loop
    if ctx.cfg.subagents.is_empty() {
        remove_stale(ctx, lock, summary, actions, &HashSet::new());
        return Ok(Vec::new());
    }

    if targets.is_empty() {
        return Ok(Vec::new());
    }

    let mut desired_ids = HashSet::new();
    let mut pending: Vec<PendingAgent> = Vec::new();
    let mut cleanup_dirs: Vec<PathBuf> = Vec::new();

    for (i, src) in ctx.cfg.subagents.iter().enumerate() {
        let desired_names = desired_agent_names(src, lock);

        if ctx.locked {
            if let Err(e) = ensure_locked_satisfiable_agents(src, &desired_names, lock) {
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

        let update_active = update_active_for_source(ctx, &src.source, &desired_names);
        let fetch = update_active || needs_fetch_agents(src, &desired_names, lock, &targets);

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
                desired_ids.insert(agent_asset_id(&src.source, name));
                let label = sync_label_with(name, &src.source, ctx.plain, first_in_run);
                first_in_run = false;
                with_spinner_transient(ctx.animate, ctx.plain, &label, || {
                    summary.unchanged += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(format!("agent:{name}")),
                        status: "unchanged".into(),
                        error: None,
                    });
                    Ok(())
                })?;
            }
            continue;
        }

        let stage = std::env::temp_dir().join(format!("kasetto-agent-{}-{}", now_unix(), i));
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
        // `source_root` honors `sub-dir`; `cleanup_dir` is the archive root and
        // only a teardown handle
        let root = materialized.source_root.as_path();

        let selected: Vec<(String, PathBuf)> = match &src.agents {
            AgentsField::Wildcard(s) if s == "*" => match discover_agents(root) {
                Ok(map) => {
                    let mut found: Vec<(String, PathBuf)> = map.into_iter().collect();
                    found.sort();
                    found
                }
                Err(e) => {
                    summary.broken += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some("agent".into()),
                        status: "broken".into(),
                        error: Some(e.to_string()),
                    });
                    if let Some(d) = materialized.cleanup_dir {
                        cleanup_dirs.push(d);
                    }
                    continue;
                }
            },
            AgentsField::Wildcard(s) => {
                summary.broken += 1;
                actions.push(Action {
                    source: Some(src.source.clone()),
                    skill: Some("agent".into()),
                    status: "broken".into(),
                    error: Some(format!(
                        "invalid agents value \"{s}\": expected \"*\" or a list"
                    )),
                });
                if let Some(d) = materialized.cleanup_dir {
                    cleanup_dirs.push(d);
                }
                continue;
            }
            AgentsField::List(entries) => {
                let mut out = Vec::new();
                for entry in entries {
                    match resolve_agent_entry(root, entry) {
                        Ok(pair) => out.push(pair),
                        Err(e) => {
                            summary.broken += 1;
                            actions.push(Action {
                                source: Some(src.source.clone()),
                                skill: Some(format!("agent:{}", entry_name(entry))),
                                status: "broken".into(),
                                error: Some(e.to_string()),
                            });
                        }
                    }
                }
                out
            }
        };

        if selected.is_empty() && matches!(&src.agents, AgentsField::Wildcard(s) if s == "*") {
            summary.broken += 1;
            actions.push(Action {
                source: Some(src.source.clone()),
                skill: Some("agent".into()),
                status: "broken".into(),
                error: Some("no agents found in source (expected agents/*.md)".into()),
            });
        }

        let mut first_in_run = true;
        for (name, src_path) in selected {
            let asset_id = agent_asset_id(&src.source, &name);
            // Desired even when broken below, so a source that regressed keeps
            // the last good install instead of having it pruned
            desired_ids.insert(asset_id.clone());
            let row_first = first_in_run;
            first_in_run = false;
            let checked =
                hash_file(&src_path).and_then(|h| validate_agent(&src_path, &name).map(|()| h));
            let hash = match checked {
                Ok(h) => h,
                Err(e) => {
                    summary.broken += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(format!("agent:{name}")),
                        status: "broken".into(),
                        error: Some(e.to_string()),
                    });
                    continue;
                }
            };

            let existing = lock.get_tracked_asset("agent", &asset_id);
            let is_unchanged = existing
                .as_ref()
                .map(|(h, _)| {
                    h == &hash && targets.iter().all(|t| destination_path(t, &name).exists())
                })
                .unwrap_or(false);

            if is_unchanged {
                let label = sync_label_with(&name, &src.source, ctx.plain, row_first);
                with_spinner_transient(ctx.animate, ctx.plain, &label, || {
                    summary.unchanged += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(format!("agent:{name}")),
                        status: "unchanged".into(),
                        error: None,
                    });
                    Ok(())
                })?;
            } else {
                pending.push(PendingAgent {
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

    // Pruned before the install for the same reason as commands: a pack moved to
    // another `source` retires its old id but keeps the destination path. Skipped
    // after any `failed` source, whose ids never reached `desired_ids`
    if summary.failed == 0 {
        remove_stale(ctx, lock, summary, actions, &desired_ids);
    }

    let warnings = apply_pending(ctx, lock, summary, actions, &targets, &pending)?;
    for d in cleanup_dirs {
        let _ = fs::remove_dir_all(d);
    }
    Ok(warnings)
}

fn agent_asset_id(source: &str, name: &str) -> String {
    format!("agent::{source}::{name}")
}

/// The config name as `resolve_agent_entry` keys it, so `reviewer.md` and
/// `reviewer` land on the same asset id.
fn entry_name(entry: &crate::model::AgentEntry) -> &str {
    entry.name().trim_end_matches(".md")
}

/// Desired agent names for a source, derived without any network access: the
/// config names for a list, the locked set for `"*"`, nothing for any other
/// wildcard (the fetch path reports that value as broken).
fn desired_agent_names(src: &AgentSourceSpec, lock: &LockFile) -> Vec<String> {
    match &src.agents {
        AgentsField::List(entries) => entries.iter().map(|e| entry_name(e).to_string()).collect(),
        AgentsField::Wildcard(s) if s == "*" => lock
            .assets
            .values()
            .filter(|a| a.kind == "agent" && a.source == src.source)
            .map(|a| a.name.clone())
            .collect(),
        AgentsField::Wildcard(_) => Vec::new(),
    }
}

/// Per-source fetch decision, made before any download. Fetch when a wildcard
/// source was never resolved, a desired agent has no lock entry or a retargeted
/// ref, or any expected destination file is missing.
fn needs_fetch_agents(
    src: &AgentSourceSpec,
    desired: &[String],
    lock: &LockFile,
    targets: &[AgentTarget],
) -> bool {
    if matches!(&src.agents, AgentsField::Wildcard(s) if s == "*")
        && !lock
            .assets
            .values()
            .any(|a| a.kind == "agent" && a.source == src.source)
    {
        return true;
    }
    let expected_revision = src.as_source_spec().expected_revision();
    for name in desired {
        let asset_id = agent_asset_id(&src.source, name);
        let Some(asset) = lock.assets.get(&asset_id).filter(|a| a.kind == "agent") else {
            return true;
        };
        if !asset.source_revision.is_empty() && asset.source_revision != expected_revision {
            return true;
        }
        if targets.iter().any(|t| !destination_path(t, name).exists()) {
            return true;
        }
    }
    false
}

/// `--locked` validation: every config-named agent must have a lock entry, and
/// a wildcard source must contribute at least one locked agent.
fn ensure_locked_satisfiable_agents(
    src: &AgentSourceSpec,
    desired: &[String],
    lock: &LockFile,
) -> Result<()> {
    match &src.agents {
        AgentsField::List(_) => {
            for name in desired {
                let asset_id = agent_asset_id(&src.source, name);
                if lock.get_tracked_asset("agent", &asset_id).is_none() {
                    return Err(err(format!(
                        "--locked: agent `{name}` from `{}` is not in the lock",
                        src.source
                    )));
                }
            }
            Ok(())
        }
        AgentsField::Wildcard(_) => {
            let present = lock
                .assets
                .values()
                .any(|a| a.kind == "agent" && a.source == src.source);
            if present {
                Ok(())
            } else {
                Err(err(format!(
                    "--locked: source `{}` has no agent entries in the lock",
                    src.source
                )))
            }
        }
    }
}

/// A target dir as the user would type it: relative to the project root, or
/// under `~` for global scope (whose scope root is the home directory).
fn display_target(ctx: &SyncContext, target: &AgentTarget) -> String {
    let rel = relativize_dest(&target.path, &ctx.scope_root);
    if ctx.scope == Scope::Global && !std::path::Path::new(&rel).is_absolute() {
        format!("~/{rel}")
    } else {
        rel
    }
}

fn apply_pending(
    ctx: &SyncContext,
    lock: &mut LockFile,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
    targets: &[AgentTarget],
    pending: &[PendingAgent],
) -> Result<Vec<String>> {
    let mut warnings = Vec::new();
    let mut last_source = String::new();
    for p in pending {
        let first_in_run = p.source != last_source;
        last_source = p.source.clone();
        let label = sync_label_with(&p.name, &p.source, ctx.plain, first_in_run);
        with_spinner_transient(ctx.animate, ctx.plain, &label, || {
            let status = match (p.is_new, ctx.dry_run) {
                (true, true) => "would_install",
                (true, false) => "installed",
                (false, true) => "would_update",
                (false, false) => "updated",
            };

            let mut written: Vec<String> = Vec::new();
            for target in targets {
                let applied = if ctx.dry_run {
                    dropped_fields(&p.src_path, target, &p.name).map(|d| (None, d))
                } else {
                    apply_agent(&p.src_path, target, &p.name).map(|(dest, d)| (Some(dest), d))
                };
                let (dest, dropped) = applied.map_err(|e| {
                    err(format!(
                        "failed to apply agent `{}` to {}: {e}",
                        p.name,
                        target.path.display()
                    ))
                })?;
                if !dropped.is_empty() {
                    warnings.push(format!(
                        "agent `{}` for {}: dropped {}",
                        p.name,
                        display_target(ctx, target),
                        dropped.join(", ")
                    ));
                }
                if let Some(dest) = dest {
                    written.push(relativize_dest(&dest, &ctx.scope_root));
                }
            }
            if !ctx.dry_run {
                lock.save_tracked_asset(
                    &p.asset_id,
                    crate::lock::AssetEntry {
                        kind: "agent".into(),
                        name: p.name.clone(),
                        hash: p.hash.clone(),
                        source: p.source.clone(),
                        destination: written.join(","),
                        source_revision: p.source_revision.clone(),
                        has_secrets: false,
                    },
                );
            }

            if p.is_new {
                summary.installed += 1;
            } else {
                summary.updated += 1;
            }
            actions.push(Action {
                source: Some(p.source.clone()),
                skill: Some(format!("agent:{}", p.name)),
                status: status.into(),
                error: None,
            });
            Ok(())
        })?;
    }
    Ok(warnings)
}

/// Lock-scoped orphan pass: only files the lock recorded are deleted, so a
/// user-authored agent in the same directory is never touched.
fn remove_stale(
    ctx: &SyncContext,
    lock: &mut LockFile,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
    desired_ids: &HashSet<String>,
) {
    let existing: Vec<(String, String)> = lock
        .list_tracked_asset_ids("agent")
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
                action_skill: format!("agent:{name}"),
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
                    if path.is_file() {
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
    use crate::model::{Agent, AgentEntry, AgentField, Config};
    use std::path::Path;

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn spec(src: &Path, sub_dir: Option<&str>, agents: AgentsField) -> AgentSourceSpec {
        AgentSourceSpec {
            source: src.to_string_lossy().to_string(),
            branch: None,
            git_ref: None,
            sub_dir: sub_dir.map(str::to_string),
            agents,
        }
    }

    fn cfg_with(tools: Vec<Agent>, subagents: Vec<AgentSourceSpec>) -> Config {
        Config {
            destination: None,
            scope: Some(Scope::Project),
            agent: Some(AgentField::Many(tools)),
            skills: Vec::new(),
            mcps: Vec::new(),
            commands: Vec::new(),
            instructions: Vec::new(),
            subagents,
            plugins: Vec::new(),
            secrets: None,
            audit: None,
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
            update: false,
            update_only: Vec::new(),
            update_local: false,
            locked,
            secrets: crate::secrets::SecretContext::empty(),
        }
    }

    struct Run {
        summary: Summary,
        actions: Vec<Action>,
        warnings: Vec<String>,
    }

    fn run(cfg: &Config, project: &PathBuf, lock: &mut LockFile, locked: bool) -> Run {
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        let warnings = sync_agents(
            &make_ctx(cfg, project, locked),
            lock,
            &mut summary,
            &mut actions,
        )
        .unwrap();
        Run {
            summary,
            actions,
            warnings,
        }
    }

    fn names(entries: &[&str]) -> AgentsField {
        AgentsField::List(
            entries
                .iter()
                .map(|n| AgentEntry::Name((*n).to_string()))
                .collect(),
        )
    }

    const REVIEWER: &str =
        "---\nname: reviewer\ndescription: Reviews diffs\ncolor: blue\n---\nReview it.\n";
    const PLANNER: &str = "---\nname: planner\ndescription: Plans work\n---\nPlan it.\n";

    #[test]
    fn installs_to_every_target_then_stays_silent_and_unchanged() {
        let src = temp_dir("kasetto-agent-src");
        write(&src.join("agents/reviewer.md"), REVIEWER);
        write(&src.join("agents/planner.md"), PLANNER);
        let project = temp_dir("kasetto-agent-proj");
        fs::create_dir_all(&project).unwrap();

        let cfg = cfg_with(
            vec![Agent::ClaudeCode, Agent::GeminiCli],
            vec![spec(&src, None, names(&["reviewer", "planner"]))],
        );
        let mut lock = LockFile::default();

        // A dry run previews the same warning and writes nothing
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        let ctx = SyncContext {
            dry_run: true,
            ..make_ctx(&cfg, &project, false)
        };
        let preview = sync_agents(&ctx, &mut lock, &mut summary, &mut actions).unwrap();
        assert_eq!(
            preview,
            vec!["agent `reviewer` for .gemini/agents: dropped color".to_string()]
        );
        assert!(!project.join(".gemini/agents/reviewer.md").exists());
        assert!(lock.assets.is_empty());

        let first = run(&cfg, &project, &mut lock, false);
        assert_eq!(first.summary.installed, 2);
        for name in ["reviewer", "planner"] {
            assert!(project.join(format!(".claude/agents/{name}.md")).is_file());
            assert!(project.join(format!(".gemini/agents/{name}.md")).is_file());
        }
        // Gemini cannot carry `color`; Claude Code takes the source verbatim
        assert_eq!(
            first.warnings,
            vec!["agent `reviewer` for .gemini/agents: dropped color".to_string()]
        );
        let entry = &lock.assets[&agent_asset_id(&src.to_string_lossy(), "reviewer")];
        assert_eq!(entry.kind, "agent");
        assert_eq!(
            entry.destination,
            ".claude/agents/reviewer.md,.gemini/agents/reviewer.md"
        );

        let second = run(&cfg, &project, &mut lock, false);
        assert_eq!(second.summary.unchanged, 2);
        assert_eq!(second.summary.installed + second.summary.updated, 0);
        assert!(second.warnings.is_empty(), "a repeat sync must be silent");

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn pruning_removes_only_kasetto_files() {
        let src = temp_dir("kasetto-agent-prune-src");
        write(&src.join("agents/reviewer.md"), REVIEWER);
        write(&src.join("agents/planner.md"), PLANNER);
        let project = temp_dir("kasetto-agent-prune-proj");
        fs::create_dir_all(&project).unwrap();
        let user_file = project.join(".claude/agents/mine.md");
        write(&user_file, "user authored\n");

        let tools = vec![Agent::ClaudeCode, Agent::GeminiCli];
        let mut lock = LockFile::default();
        let both = cfg_with(
            tools.clone(),
            vec![spec(&src, None, names(&["reviewer", "planner"]))],
        );
        assert_eq!(run(&both, &project, &mut lock, false).summary.installed, 2);

        // Dropping one agent from the list deletes just its files
        let one = cfg_with(tools.clone(), vec![spec(&src, None, names(&["planner"]))]);
        let r = run(&one, &project, &mut lock, false);
        assert_eq!(r.summary.removed, 1);
        assert_eq!(r.summary.unchanged, 1);
        assert!(!project.join(".claude/agents/reviewer.md").exists());
        assert!(!project.join(".gemini/agents/reviewer.md").exists());
        assert!(project.join(".claude/agents/planner.md").is_file());
        assert!(user_file.is_file());

        // Dropping the whole `agents:` block prunes everything Kasetto installed
        let none = cfg_with(tools, Vec::new());
        let r = run(&none, &project, &mut lock, false);
        assert_eq!(r.summary.removed, 1);
        assert!(!project.join(".claude/agents/planner.md").exists());
        assert!(!project.join(".gemini/agents/planner.md").exists());
        assert!(user_file.is_file());
        assert_eq!(
            lock.assets.values().filter(|a| a.kind == "agent").count(),
            0
        );

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn wildcard_discovers_flat_agents_and_reports_an_empty_source() {
        let src = temp_dir("kasetto-agent-wild-src");
        write(&src.join("agents/reviewer.md"), REVIEWER);
        write(&src.join("agents/planner.md"), PLANNER);
        write(&src.join("agents/README.md"), "# Agents\n");
        let project = temp_dir("kasetto-agent-wild-proj");
        fs::create_dir_all(&project).unwrap();

        let cfg = cfg_with(
            vec![Agent::ClaudeCode],
            vec![spec(&src, None, AgentsField::Wildcard("*".into()))],
        );
        let mut lock = LockFile::default();
        let r = run(&cfg, &project, &mut lock, false);
        assert_eq!(r.summary.installed, 2);
        assert!(!project.join(".claude/agents/README.md").exists());
        assert_eq!(lock.list_installed_agents(), ["planner", "reviewer"]);

        let empty = temp_dir("kasetto-agent-wild-empty");
        fs::create_dir_all(&empty).unwrap();
        let cfg = cfg_with(
            vec![Agent::ClaudeCode],
            vec![spec(&empty, None, AgentsField::Wildcard("*".into()))],
        );
        let r = run(&cfg, &project, &mut LockFile::default(), false);
        assert_eq!(r.summary.broken, 1);
        assert_eq!(
            r.actions[0].error.as_deref(),
            Some("no agents found in source (expected agents/*.md)")
        );

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&empty);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn invalid_source_agent_is_broken_not_a_run_failure() {
        let src = temp_dir("kasetto-agent-broken-src");
        write(&src.join("agents/reviewer.md"), REVIEWER);
        write(
            &src.join("agents/nodesc.md"),
            "---\nname: nodesc\n---\nBody\n",
        );
        let project = temp_dir("kasetto-agent-broken-proj");
        fs::create_dir_all(&project).unwrap();

        let cfg = cfg_with(
            vec![Agent::ClaudeCode],
            vec![spec(&src, None, names(&["reviewer", "nodesc"]))],
        );
        for dry_run in [true, false] {
            let mut lock = LockFile::default();
            let mut summary = Summary::default();
            let mut actions = Vec::new();
            let ctx = SyncContext {
                dry_run,
                ..make_ctx(&cfg, &project, false)
            };
            sync_agents(&ctx, &mut lock, &mut summary, &mut actions).unwrap();
            assert_eq!(summary.broken, 1, "dry_run={dry_run}");
            assert_eq!(summary.installed, 1, "dry_run={dry_run}");
            assert_eq!(summary.failed, 0);
            let broken = actions.iter().find(|a| a.status == "broken").unwrap();
            assert_eq!(broken.skill.as_deref(), Some("agent:nodesc"));
            assert!(broken.error.as_deref().unwrap().contains("description"));
            assert!(!project.join(".claude/agents/nodesc.md").exists());
            assert_eq!(
                project.join(".claude/agents/reviewer.md").is_file(),
                !dry_run
            );
        }

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn update_by_agent_name_refetches_only_that_source() {
        let a = temp_dir("kasetto-agent-upd-a");
        let b = temp_dir("kasetto-agent-upd-b");
        write(&a.join("agents/reviewer.md"), REVIEWER);
        write(&b.join("agents/planner.md"), PLANNER);
        let project = temp_dir("kasetto-agent-upd-proj");
        fs::create_dir_all(&project).unwrap();

        let cfg = cfg_with(
            vec![Agent::ClaudeCode],
            vec![
                spec(&a, None, names(&["reviewer"])),
                spec(&b, None, names(&["planner"])),
            ],
        );
        let mut lock = LockFile::default();
        assert_eq!(run(&cfg, &project, &mut lock, false).summary.installed, 2);

        write(
            &b.join("agents/planner.md"),
            "---\nname: planner\ndescription: Plans work\n---\nPlan it again.\n",
        );
        let mut summary = Summary::default();
        let mut actions = Vec::new();
        let ctx = SyncContext {
            update: true,
            update_only: vec!["planner".into()],
            ..make_ctx(&cfg, &project, false)
        };
        sync_agents(&ctx, &mut lock, &mut summary, &mut actions).unwrap();
        assert_eq!(summary.updated, 1);
        assert_eq!(summary.unchanged, 1);
        let planner = fs::read_to_string(project.join(".claude/agents/planner.md")).unwrap();
        assert!(planner.contains("Plan it again."));

        let _ = fs::remove_dir_all(&a);
        let _ = fs::remove_dir_all(&b);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn locked_errors_when_agent_absent_from_lock() {
        let src = temp_dir("kasetto-agent-locked-src");
        write(&src.join("agents/reviewer.md"), REVIEWER);
        let project = temp_dir("kasetto-agent-locked-proj");
        fs::create_dir_all(&project).unwrap();

        let cfg = cfg_with(
            vec![Agent::ClaudeCode],
            vec![spec(&src, None, names(&["reviewer"]))],
        );
        let r = run(&cfg, &project, &mut LockFile::default(), true);
        assert_eq!(r.summary.failed, 1);
        assert_eq!(r.summary.installed, 0);
        assert_eq!(r.actions[0].status, "locked_error");
        assert!(!project.join(".claude/agents/reviewer.md").exists());

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn sub_dir_is_honored_for_a_pstack_shaped_source() {
        let src = temp_dir("kasetto-agent-pstack-src");
        let pack = src.join("harness/pstack/claude-code");
        write(
            &pack.join("agents/poteto-agent.md"),
            "---\nname: poteto-agent\ndescription: Delegate\nis_background: true\n---\nbody\n",
        );
        write(
            &pack.join("agents/comment-sicko.md"),
            "---\nname: comment-sicko\ndescription: Comments\n---\nbody\n",
        );
        let project = temp_dir("kasetto-agent-pstack-proj");
        fs::create_dir_all(&project).unwrap();

        let cfg = cfg_with(
            vec![Agent::Cursor, Agent::GeminiCli],
            vec![spec(
                &src,
                Some("harness/pstack/claude-code"),
                AgentsField::Wildcard("*".into()),
            )],
        );
        let r = run(&cfg, &project, &mut LockFile::default(), false);
        assert_eq!(r.summary.installed, 2);
        let cursor = fs::read_to_string(project.join(".cursor/agents/poteto-agent.md")).unwrap();
        assert!(cursor.contains("is_background: true"));
        assert!(project.join(".gemini/agents/comment-sicko.md").is_file());
        assert_eq!(
            r.warnings,
            vec!["agent `poteto-agent` for .gemini/agents: dropped is_background".to_string()]
        );

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&project);
    }
}
