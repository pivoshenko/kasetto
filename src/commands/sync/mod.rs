mod commands;
mod mcps;
mod skills;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::colors::{ACCENT, ATTENTION, ERROR, RESET, SECONDARY, SUCCESS};
use crate::error::Result;
use crate::fsops::{load_config_any, now_iso, now_unix, resolve_destinations, scope_root};
use crate::lock::{load_lock, save_lock};
use crate::model::{resolve_scope, Config, Report, Scope, Summary};
use crate::state::{load_runtime_state, save_runtime_state};
use crate::ui::{action_glyph, animations_enabled, print_json};

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

/// Options for the `sync` command.
pub(crate) struct SyncOptions<'a> {
    pub config_path: &'a str,
    pub dry_run: bool,
    pub quiet: bool,
    pub as_json: bool,
    pub plain: bool,
    pub verbose: bool,
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

    skills::sync_skills(&ctx, &mut state, &mut runtime, &mut summary, &mut actions)?;
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
        runtime.last_run = Some(now_iso());
        runtime.save_report_json(&serde_json::to_string(&report)?);
        save_runtime_state(&runtime, scope, &cfg_dir)?;
    }

    if opts.as_json {
        print_json(&report)?;
    } else if !opts.quiet {
        print_sync_summary(
            &report,
            opts.plain,
            opts.verbose,
            started.elapsed(),
            opts.locked,
        );
    }

    if report.summary.failed > 0 {
        std::process::exit(1);
    }
    Ok(())
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

fn print_sync_summary(
    report: &Report,
    plain: bool,
    verbose: bool,
    elapsed: Duration,
    locked: bool,
) {
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
        let suffix = if i == 0 {
            format!(" in {timing}")
        } else {
            String::new()
        };
        let lead = if plain {
            (*verb).to_string()
        } else {
            format!("{color}\x1b[1m{verb}{RESET}")
        };
        println!("{lead} {count} {}{suffix}", pluralize_item(*count));
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
        let prefix = if plain {
            "warning:".to_string()
        } else {
            format!("{ATTENTION}\x1b[1mwarning:{RESET}")
        };
        eprintln!("{prefix} {} {} broken", s.broken, pluralize_item(s.broken));
    }
    if s.failed > 0 {
        let prefix = if plain {
            "error:".to_string()
        } else {
            format!("{ERROR}\x1b[1merror:{RESET}")
        };
        eprintln!("{prefix} {} {} failed", s.failed, pluralize_item(s.failed));
    }

    if verbose {
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

pub(super) fn sync_label(kind: &str, name: &str, source: &str, plain: bool) -> String {
    if plain {
        format!("Syncing {kind} {name}")
    } else {
        format!(
            "Syncing {kind} {}{}{} {}{}{}",
            ACCENT, name, RESET, SECONDARY, source, RESET
        )
    }
}

pub(super) fn file_name_str(path: &std::path::Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}
