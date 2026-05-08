use std::process::Command;

use crate::error::{err, Result};
use crate::model::Report;
use crate::ui::with_spinner;

use super::SyncContext;

/// Run a list of shell commands as hooks.
///
/// - `hooks`: command strings to execute via `sh -c`
/// - `label`: display label like "pre-sync" or "post-sync"
/// - `env_vars`: extra environment variables to inject
///
/// Hooks run in order. The first failure aborts the remaining hooks and
/// returns an error.
pub(super) fn run_hooks(
    ctx: &SyncContext,
    hooks: &[String],
    label: &str,
    env_vars: &[(&str, &str)],
) -> Result<()> {
    if hooks.is_empty() {
        return Ok(());
    }

    for (idx, cmd_str) in hooks.iter().enumerate() {
        let spinner_label = format!("Running {label} hook {}/{}", idx + 1, hooks.len());

        with_spinner(ctx.animate, ctx.plain, &spinner_label, || {
            let status = Command::new("sh")
                .arg("-c")
                .arg(cmd_str)
                .envs(env_vars.iter().map(|(k, v)| (*k, *v)))
                .status()
                .map_err(|e| {
                    err(format!(
                        "{label} hook {}/{hooks_len} failed to start: {e}",
                        idx + 1,
                        hooks_len = hooks.len()
                    ))
                })?;

            if !status.success() {
                return Err(err(format!(
                    "{label} hook {}/{hooks_len} exited with status: {status}",
                    idx + 1,
                    hooks_len = hooks.len(),
                    status = status
                )));
            }

            Ok(())
        })?;
    }

    Ok(())
}

/// Build environment variables for post_sync hooks based on the sync report.
pub(super) fn post_sync_env(report: &Report) -> Vec<(&'static str, String)> {
    vec![
        ("KASETTO_RUN_ID", report.run_id.clone()),
        ("KASETTO_CONFIG", report.config.clone()),
        ("KASETTO_DESTINATION", report.destination.clone()),
        (
            "KASETTO_DRY_RUN",
            if report.dry_run {
                "1".into()
            } else {
                "0".into()
            },
        ),
        ("KASETTO_INSTALLED", report.summary.installed.to_string()),
        ("KASETTO_UPDATED", report.summary.updated.to_string()),
        ("KASETTO_REMOVED", report.summary.removed.to_string()),
        ("KASETTO_UNCHANGED", report.summary.unchanged.to_string()),
        ("KASETTO_BROKEN", report.summary.broken.to_string()),
        ("KASETTO_FAILED", report.summary.failed.to_string()),
    ]
}
