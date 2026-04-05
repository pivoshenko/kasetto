mod mcps;
mod skills;

use std::fs;
use std::path::{Path, PathBuf};

use crate::banner::print_banner;
use crate::colors::{ACCENT, ATTENTION, ERROR, INFO, RESET, SECONDARY, SUCCESS, WARNING};
use crate::error::Result;
use crate::fsops::{load_config_any, now_iso, now_unix, resolve_destinations};
use crate::lock::{load_lock, save_lock};
use crate::model::{resolve_scope, Config, Report, Scope, Summary};
use crate::ui::{animations_enabled, print_json, status_chip};

pub(super) struct SyncContext<'a> {
    pub(super) cfg: &'a Config,
    pub(super) cfg_dir: &'a Path,
    pub(super) destinations: &'a [PathBuf],
    pub(super) scope: Scope,
    pub(super) dry_run: bool,
    pub(super) animate: bool,
    pub(super) plain: bool,
    pub(super) as_json: bool,
    pub(super) quiet: bool,
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
    pub show_banner: bool,
}

pub(crate) fn run(opts: &SyncOptions) -> Result<()> {
    let animate = animations_enabled(opts.quiet, opts.as_json, opts.plain);
    if opts.show_banner
        && !opts.quiet
        && !opts.as_json
        && std::io::IsTerminal::is_terminal(&std::io::stdout())
    {
        if opts.plain {
            println!("kasetto | カセット");
        } else {
            print_banner();
        }
    }

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
        scope,
        dry_run: opts.dry_run,
        animate,
        plain: opts.plain,
        as_json: opts.as_json,
        quiet: opts.quiet,
    };

    let mut lock = load_lock(scope, &cfg_dir)?;
    let mut state = lock.state();
    let mut summary = Summary::default();
    let mut actions = Vec::new();

    skills::sync_skills(&ctx, &mut state, &mut summary, &mut actions)?;
    mcps::sync_mcps(&ctx, &mut lock, &mut summary, &mut actions)?;

    if !opts.dry_run {
        state.last_run = Some(now_iso());
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
        lock.save_report_json(&serde_json::to_string(&report)?);
        save_lock(&lock, scope, &cfg_dir)?;
    }

    if opts.as_json {
        print_json(&report)?;
    } else if !opts.quiet {
        if opts.plain {
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
                "  {}Installed{}: {}   {}Updated{}: {}   {}Removed{}: {}",
                SUCCESS,
                RESET,
                report.summary.installed,
                INFO,
                RESET,
                report.summary.updated,
                WARNING,
                RESET,
                report.summary.removed
            );
            println!(
                "  {}Unchanged{}: {}   {}Broken{}: {}   {}Failed{}: {}",
                SECONDARY,
                RESET,
                report.summary.unchanged,
                ATTENTION,
                RESET,
                report.summary.broken,
                ERROR,
                RESET,
                report.summary.failed
            );
        }

        if opts.verbose {
            println!("\nActions:");
            for a in &report.actions {
                let status = status_chip(&a.status, opts.plain);
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
