mod hooks;
mod mcps;
mod skills;

use std::fs;
use std::path::{Path, PathBuf};

use crate::banner::print_banner_or_plain;
use crate::colors::{ACCENT, ATTENTION, ERROR, INFO, RESET, SECONDARY, SUCCESS, WARNING};
use crate::error::Result;
use crate::fsops::{load_config_any, now_iso, now_unix, resolve_destinations};
use crate::lock::{load_lock, save_lock};
use crate::model::{resolve_scope, Config, Hooks, Report, Scope, Summary};
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
    pub no_hooks: bool,
    pub scope_override: Option<Scope>,
    pub show_banner: bool,
}

/// Load hooks from the local `kasetto.yaml` if it exists, falling back to the
/// global config. If both define hooks, local wins.
fn resolve_hooks() -> Option<Hooks> {
    let local_path = crate::DEFAULT_CONFIG_FILENAME;
    if let Ok(text) = std::fs::read_to_string(local_path) {
        if let Ok(cfg) = serde_yaml::from_str::<Config>(&text) {
            if cfg.hooks.is_some() {
                return cfg.hooks;
            }
        }
    }

    if let Ok(global_dir) = crate::fsops::dirs_kasetto_config() {
        let global_path = global_dir.join(crate::DEFAULT_GLOBAL_CONFIG_FILENAME);
        if let Ok(text) = std::fs::read_to_string(&global_path) {
            if let Ok(cfg) = serde_yaml::from_str::<Config>(&text) {
                return cfg.hooks;
            }
        }
    }

    None
}

pub(crate) fn run(opts: &SyncOptions) -> Result<()> {
    let animate = animations_enabled(opts.quiet, opts.as_json, opts.plain);
    if opts.show_banner
        && !opts.quiet
        && !opts.as_json
        && std::io::IsTerminal::is_terminal(&std::io::stdout())
    {
        print_banner_or_plain(opts.plain);
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

    let hooks = if opts.no_hooks { None } else { resolve_hooks() };

    if let Some(ref h) = hooks {
        hooks::run_hooks(&ctx, &h.pre_sync, "pre-sync", &[])?;
    }

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

    if let Some(ref h) = hooks {
        let env = hooks::post_sync_env(&report);
        let env_ref: Vec<(&str, &str)> = env.iter().map(|(k, v)| (*k, v.as_str())).collect();
        hooks::run_hooks(&ctx, &h.post_sync, "post-sync", &env_ref)?;
    }

    if opts.as_json {
        print_json(&report)?;
    } else if !opts.quiet {
        print_sync_summary(&report, opts.plain, opts.verbose);
    }

    if report.summary.failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn print_sync_summary(report: &Report, plain: bool, verbose: bool) {
    // Column widths align labels and right-align counts across the two summary rows.
    const L1: usize = 12;
    const L2: usize = 9;
    const L3: usize = 9;
    const NW: usize = 5;

    println!();
    if plain {
        println!(
            "  {:<L1$} {:>NW$}   {:<L2$} {:>NW$}   {:<L3$} {:>NW$}",
            "Installed:",
            report.summary.installed,
            "Updated:",
            report.summary.updated,
            "Removed:",
            report.summary.removed,
        );
        println!(
            "  {:<L1$} {:>NW$}   {:<L2$} {:>NW$}   {:<L3$} {:>NW$}",
            "Unchanged:",
            report.summary.unchanged,
            "Broken:",
            report.summary.broken,
            "Failed:",
            report.summary.failed,
        );
    } else {
        const W1: usize = 10;
        const W2: usize = 7;
        const W3: usize = 7;
        println!(
            "  {}Installed{}{}: {:>NW$}   {}Updated{}{}: {:>NW$}   {}Removed{}{}: {:>NW$}",
            SUCCESS,
            RESET,
            " ".repeat(W1.saturating_sub("Installed".len())),
            report.summary.installed,
            INFO,
            RESET,
            " ".repeat(W2.saturating_sub("Updated".len())),
            report.summary.updated,
            WARNING,
            RESET,
            " ".repeat(W3.saturating_sub("Removed".len())),
            report.summary.removed,
        );
        println!(
            "  {}Unchanged{}{}: {:>NW$}   {}Broken{}{}: {:>NW$}   {}Failed{}{}: {:>NW$}",
            SECONDARY,
            RESET,
            " ".repeat(W1.saturating_sub("Unchanged".len())),
            report.summary.unchanged,
            ATTENTION,
            RESET,
            " ".repeat(W2.saturating_sub("Broken".len())),
            report.summary.broken,
            ERROR,
            RESET,
            " ".repeat(W3.saturating_sub("Failed".len())),
            report.summary.failed,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;

    /// Serialise hook-resolution tests because they mutate the process-wide
    /// current-working directory and `XDG_CONFIG_HOME`.
    static HOOKS_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn temp_dir(prefix: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn resolve_hooks_prefers_local() {
        let _guard = HOOKS_TEST_LOCK.lock().unwrap();

        let dir = temp_dir("kasetto-hooks-local");
        fs::create_dir_all(&dir).unwrap();
        let local_cfg = dir.join("kasetto.yaml");
        fs::write(
            &local_cfg,
            r#"
agent: cursor
skills: []
hooks:
  pre_sync:
    - echo local
"#,
        )
        .unwrap();

        let global_dir = dir.join("global");
        fs::create_dir_all(&global_dir).unwrap();
        let global_cfg = global_dir.join("kasetto").join("kasetto.yaml");
        fs::create_dir_all(global_cfg.parent().unwrap()).unwrap();
        fs::write(
            &global_cfg,
            r#"
agent: cursor
skills: []
hooks:
  pre_sync:
    - echo global
"#,
        )
        .unwrap();

        let _orig_local = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let _orig_xdg = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("XDG_CONFIG_HOME", &global_dir);

        let hooks = resolve_hooks();
        assert!(hooks.is_some());
        assert_eq!(hooks.unwrap().pre_sync, vec!["echo local"]);

        let _ = fs::remove_dir_all(&dir);
        if let Some(v) = _orig_xdg {
            std::env::set_var("XDG_CONFIG_HOME", v);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        std::env::set_current_dir(_orig_local).unwrap();
    }

    #[test]
    fn resolve_hooks_falls_back_to_global() {
        let _guard = HOOKS_TEST_LOCK.lock().unwrap();

        let dir = temp_dir("kasetto-hooks-global");
        fs::create_dir_all(&dir).unwrap();

        let global_dir = dir.join("global");
        fs::create_dir_all(&global_dir).unwrap();
        let global_cfg = global_dir.join("kasetto").join("kasetto.yaml");
        fs::create_dir_all(global_cfg.parent().unwrap()).unwrap();
        fs::write(
            &global_cfg,
            r#"
agent: cursor
skills: []
hooks:
  pre_sync:
    - echo global
"#,
        )
        .unwrap();

        let _orig_local = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let _orig_xdg = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("XDG_CONFIG_HOME", &global_dir);

        let hooks = resolve_hooks();
        assert!(hooks.is_some());
        assert_eq!(hooks.unwrap().pre_sync, vec!["echo global"]);

        let _ = fs::remove_dir_all(&dir);
        if let Some(v) = _orig_xdg {
            std::env::set_var("XDG_CONFIG_HOME", v);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        std::env::set_current_dir(_orig_local).unwrap();
    }

    #[test]
    fn resolve_hooks_returns_none_when_neither_exists() {
        let _guard = HOOKS_TEST_LOCK.lock().unwrap();

        let dir = temp_dir("kasetto-hooks-none");
        fs::create_dir_all(&dir).unwrap();

        let _orig_local = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let _orig_xdg = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("XDG_CONFIG_HOME", &dir);

        let hooks = resolve_hooks();
        assert!(hooks.is_none());

        let _ = fs::remove_dir_all(&dir);
        if let Some(v) = _orig_xdg {
            std::env::set_var("XDG_CONFIG_HOME", v);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        std::env::set_current_dir(_orig_local).unwrap();
    }
}
