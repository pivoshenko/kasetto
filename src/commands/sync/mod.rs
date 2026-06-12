mod commands;
mod mcps;
mod skills;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::colors::{ACCENT, ATTENTION, ERROR, RESET, SECONDARY, SUCCESS};
use crate::error::Result;
use crate::fsops::{load_config_any, now_unix, now_unix_str, resolve_destinations, scope_root};
use crate::lock::{load_lock, save_lock};
use crate::model::{resolve_scope, Action, Config, Report, Scope, State, Summary};
use crate::state::{load_runtime_state, save_runtime_state, RuntimeState};
use crate::ui::{
    action_glyph, animations_enabled, print_json, print_source_header, print_sync_chips,
    print_tree_leaf, short_source, status_tail,
};

pub(super) struct SyncContext<'a> {
    pub(super) cfg: &'a Config,
    pub(super) cfg_dir: &'a Path,
    pub(super) destinations: &'a [PathBuf],
    /// Root that lock `destination` paths are stored relative to.
    pub(super) scope_root: PathBuf,
    pub(super) scope: Scope,
    pub(super) dry_run: bool,
    pub(super) animate: bool,
    pub(super) plain: bool,
    pub(super) as_json: bool,
    pub(super) quiet: bool,
    /// `--update`: re-resolve moving refs and rewrite locked hashes.
    pub(super) update: bool,
    /// `--update <name>...`: when non-empty, only sources providing these skills are re-resolved.
    pub(super) update_only: Vec<String>,
    /// `--locked`/`--frozen`: never fetch; error if the lock cannot satisfy the config.
    pub(super) locked: bool,
}

/// Bundle of the mutable bookkeeping state threaded through the skill sync
/// path. Replaces what used to be four separate `&mut` parameters.
pub(super) struct SyncMut<'a> {
    pub(super) state: &'a mut State,
    pub(super) runtime: &'a mut RuntimeState,
    pub(super) summary: &'a mut Summary,
    pub(super) actions: &'a mut Vec<Action>,
}

/// One stale asset candidate processed by [`remove_stale`].
///
/// `action_source` matches what the original per-kind helper emitted: skills
/// preserve the locked `entry.source`; commands/MCPs emit `None`.
/// `action_skill` is the pre-formatted action label (e.g. `"alpha"`,
/// `"command:foo"`, `"mcp:github.json"`).
pub(super) struct StaleEntry {
    pub(super) id: String,
    pub(super) action_source: Option<String>,
    pub(super) action_skill: String,
}

/// Shared orphan-cleanup pass: bumps `summary.removed`, pushes a `removed` or
/// `would_remove` action, and (when not a dry run) invokes `on_remove` so each
/// caller can drop its lock/state entry plus tear down on-disk artifacts.
pub(super) fn remove_stale<F>(
    dry_run: bool,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
    desired_ids: &HashSet<String>,
    candidates: Vec<StaleEntry>,
    mut on_remove: F,
) where
    F: FnMut(&str),
{
    for entry in candidates {
        if desired_ids.contains(&entry.id) {
            continue;
        }
        let status = if dry_run { "would_remove" } else { "removed" };
        if !dry_run {
            on_remove(&entry.id);
        }
        summary.removed += 1;
        actions.push(Action {
            source: entry.action_source,
            skill: Some(entry.action_skill),
            status: status.into(),
            error: None,
        });
    }
}

/// Options for the `sync` command.
pub(crate) struct SyncOptions<'a> {
    pub config_path: &'a str,
    pub dry_run: bool,
    pub quiet: bool,
    pub as_json: bool,
    pub plain: bool,
    pub verbose: u8,
    pub scope_override: Option<Scope>,
    pub update: bool,
    pub update_only: Vec<String>,
    pub locked: bool,
}

pub(crate) fn run(opts: &SyncOptions) -> Result<()> {
    if opts.locked && opts.update {
        return Err(crate::error::err(
            "`--locked`/`--frozen` and `--update` are contradictory: \
             --update fetches to re-resolve refs, --locked forbids fetching",
        ));
    }
    let animate = animations_enabled(opts.quiet, opts.as_json, opts.plain);
    let started = Instant::now();

    let (cfg, cfg_dir, cfg_label) = load_config_any(opts.config_path)?;
    let scope = resolve_scope(opts.scope_override, Some(&cfg));
    let destinations = resolve_destinations(&cfg_dir, &cfg, scope)?;
    let destination = destinations[0].clone();

    if opts.verbose >= 2 && !opts.quiet && !opts.as_json {
        emit_resolution_diag(&cfg_label, scope, &destinations, opts.plain);
    }

    if !opts.dry_run {
        for d in &destinations {
            fs::create_dir_all(d)?;
        }
    }

    let ctx = SyncContext {
        cfg: &cfg,
        cfg_dir: &cfg_dir,
        destinations: &destinations,
        scope_root: scope_root(scope, &cfg_dir)?,
        scope,
        dry_run: opts.dry_run,
        animate,
        plain: opts.plain,
        as_json: opts.as_json,
        quiet: opts.quiet,
        update: opts.update,
        update_only: opts.update_only.clone(),
        locked: opts.locked,
    };

    let mut lock = load_lock(scope, &cfg_dir)?;
    let mut state = lock.state();
    let mut runtime = load_runtime_state(scope, &cfg_dir)?;
    let mut summary = Summary::default();
    let mut actions = Vec::new();

    {
        let mut sm = SyncMut {
            state: &mut state,
            runtime: &mut runtime,
            summary: &mut summary,
            actions: &mut actions,
        };
        skills::sync_skills(&ctx, &mut sm)?;
    }
    commands::sync_commands(&ctx, &mut lock, &mut summary, &mut actions)?;
    mcps::sync_mcps(&ctx, &mut lock, &mut summary, &mut actions)?;

    if !opts.dry_run {
        lock.apply_state(&state);
    }

    let report = Report {
        run_id: format!("{}", now_unix()),
        config: cfg_label,
        destination: destination.to_string_lossy().to_string(),
        dry_run: opts.dry_run,
        summary,
        actions,
    };

    if !opts.dry_run {
        save_lock(&mut lock, scope, &cfg_dir)?;
        runtime.last_run = Some(now_unix_str());
        runtime.save_report_json(&serde_json::to_string(&report)?);
        save_runtime_state(&runtime, scope, &cfg_dir)?;
    }

    if opts.as_json {
        print_json(&report)?;
    } else if !opts.quiet {
        let elapsed = started.elapsed();
        print_resolution_header(&report, opts.plain);
        print_sync_tree(&report, opts.plain);
        print_sync_summary(&report, opts.plain, opts.verbose, elapsed, opts.locked);
    }

    if report.summary.failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// `-vv` (and higher) diagnostic header printed before the sync proper:
/// resolved config path, scope, and each destination dir.
fn emit_resolution_diag(cfg_label: &str, scope: Scope, destinations: &[PathBuf], plain: bool) {
    let scope_str = match scope {
        Scope::Global => "global",
        Scope::Project => "project",
    };
    if plain {
        println!("config: {cfg_label}");
        println!("scope:  {scope_str}");
        for d in destinations {
            println!("dest:   {}", d.display());
        }
    } else {
        println!("{SECONDARY}config:{RESET} {cfg_label}");
        println!("{SECONDARY}scope: {RESET} {scope_str}");
        for d in destinations {
            println!("{SECONDARY}dest:  {RESET} {}", d.display());
        }
    }
    println!();
}

fn pluralize_item(n: usize) -> &'static str {
    if n == 1 {
        "item"
    } else {
        "items"
    }
}

/// uv-style duration: sub-second → `Nms`, otherwise `N.Ns`.
fn format_elapsed(d: Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.2}s", d.as_secs_f64())
    }
}

fn print_sync_summary(report: &Report, plain: bool, verbose: u8, elapsed: Duration, locked: bool) {
    let s = &report.summary;
    let dry = report.dry_run;
    let timing = format_elapsed(elapsed);

    let only_unchanged = s.installed == 0 && s.updated == 0 && s.removed == 0;
    let lines: Vec<(&str, usize, &str)> =
        if locked && only_unchanged && s.broken == 0 && s.failed == 0 {
            vec![("Audited", s.unchanged, SUCCESS)]
        } else {
            let mut v = vec![
                (
                    if dry { "Would install" } else { "Installed" },
                    s.installed,
                    SUCCESS,
                ),
                (
                    if dry { "Would update" } else { "Updated" },
                    s.updated,
                    ATTENTION,
                ),
                (
                    if dry { "Would remove" } else { "Removed" },
                    s.removed,
                    ERROR,
                ),
            ];
            v.retain(|(_, n, _)| *n > 0);
            if v.is_empty() && s.unchanged > 0 {
                v.push(("Unchanged", s.unchanged, SECONDARY));
            }
            v
        };

    for (i, (verb, count, color)) in lines.iter().enumerate() {
        let suffix = match (i, plain) {
            (0, true) => format!(" in {timing}"),
            (0, false) => format!(" {SECONDARY}in {timing}{RESET}"),
            _ => String::new(),
        };
        let lead = if plain {
            (*verb).to_string()
        } else {
            format!("{color}{ACCENT}{verb}{RESET}")
        };
        println!("{lead} {count} {}{suffix}", pluralize_item(*count));
    }

    if !(lines.is_empty() || locked && only_unchanged) {
        print_sync_chips(s.updated, s.installed, s.removed, s.unchanged, plain);
    }

    if lines.is_empty() && s.broken == 0 && s.failed == 0 {
        let lead = if plain {
            "Nothing to sync".to_string()
        } else {
            format!("{SECONDARY}Nothing to sync{RESET}")
        };
        println!("{lead} (in {timing})");
    }

    if s.broken > 0 {
        crate::ui::eprint_warn(
            &format!("{} {} broken", s.broken, pluralize_item(s.broken)),
            plain,
        );
    }
    if s.failed > 0 {
        crate::ui::eprint_error(
            &format!("{} {} failed", s.failed, pluralize_item(s.failed)),
            plain,
        );
    }

    if verbose >= 1 {
        println!();
        for a in &report.actions {
            let glyph = action_glyph(&a.status, plain);
            let src = a.source.as_deref().unwrap_or("-");
            let skill = a.skill.as_deref().unwrap_or("-");
            if let Some(err) = &a.error {
                println!(" {} {} ({}) — {}", glyph, skill, src, err);
            } else {
                println!(" {} {} ({})", glyph, skill, src);
            }
        }
    }
}

/// `✓ Resolved N sources · M items` lead line — printed before the tree.
fn print_resolution_header(report: &Report, plain: bool) {
    let sources: std::collections::BTreeSet<&str> = report
        .actions
        .iter()
        .filter_map(|a| a.source.as_deref())
        .collect();
    let n_sources = sources.len();
    let n_items = report.actions.len();
    if n_sources == 0 {
        return;
    }
    if plain {
        println!("✓ Resolved {n_sources} sources · {n_items} items");
    } else {
        println!(
            "{SUCCESS}✓{RESET} {SUCCESS}{ACCENT}Resolved{RESET} {n_sources} sources {SECONDARY}· {n_items} items{RESET}"
        );
    }
    println!();
}

/// Source-grouped tree per design: each source gets a cyan header with a green
/// ✓ + item count, then `├─/└─` leaves with the action glyph + name + status
/// tail. Order: actions stay in the order the sync emitted them (already
/// grouped by source per the underlying sync flow).
fn print_sync_tree(report: &Report, plain: bool) {
    // Group actions by source, preserving first-seen order.
    let mut groups: Vec<(String, Vec<&crate::model::Action>)> = Vec::new();
    for a in &report.actions {
        let src = a.source.clone().unwrap_or_else(|| "-".to_string());
        if let Some(g) = groups.iter_mut().find(|(k, _)| k == &src) {
            g.1.push(a);
        } else {
            groups.push((src, vec![a]));
        }
    }

    for (source, items) in &groups {
        let repo = short_source(source);
        // Sync source headers have NO count per design (terminal.jsx runSync).
        print_source_header(&repo, None, Some(true), None, plain);
        for (i, a) in items.iter().enumerate() {
            let is_last = i == items.len() - 1;
            let glyph = action_glyph(&a.status, plain);
            let name = a.skill.as_deref().unwrap_or("-");
            let strike = matches!(a.status.as_str(), "removed" | "would_remove");
            let tail = status_tail(&a.status, None, None, plain);
            print_tree_leaf(is_last, Some(&glyph), name, strike, &tail, 24, plain);
        }
    }
}

/// Whether `--update` re-resolves a source. Active when `--update` was passed
/// with no names, or when `--update <name>...` includes one of this source's
/// desired asset names. Shared across the skill, command, and MCP sync paths.
pub(super) fn update_active_for_source(ctx: &SyncContext, desired: &[String]) -> bool {
    if !ctx.update {
        return false;
    }
    if ctx.update_only.is_empty() {
        return true;
    }
    desired.iter().any(|s| ctx.update_only.contains(s))
}

/// Per-row sync label. `show_source = true` appends ` <dim short-source>`
/// after the bold name; `false` shows only the name. Callers run-length-group
/// consecutive rows from the same source so the URL appears once per run.
pub(super) fn sync_label_with(name: &str, source: &str, plain: bool, show_source: bool) -> String {
    match (plain, show_source) {
        (true, true) => format!(" {name}  {source}"),
        (true, false) => format!(" {name}"),
        (false, true) => format!(
            " {ACCENT}{name}{RESET}  {SECONDARY}{}{RESET}",
            crate::ui::short_source(source)
        ),
        (false, false) => format!(" {ACCENT}{name}{RESET}"),
    }
}

pub(super) fn file_name_str(path: &std::path::Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}
