//! Module that contains the opt-in security gate `sync` runs after installing.
//!
//! The gate is off unless a threshold is in effect: `--audit [LEVEL]` on the
//! command line, else `audit.fail-on` in the config; `--no-audit` turns it off
//! outright. When on, every skill and MCP server recorded in the lock is audited
//! (see [`crate::audit`]) and any at or above the threshold fails the run. An
//! asset that cannot be verified only warns, so a flaky network or an
//! unindexed source can never block a sync.
//!
//! The gate judges what was installed, so a failing run leaves those files in
//! place: the non-zero exit is what lets CI stop before the environment is used.

use crate::audit::{audit_all, collect_targets, source_label, AssetKind, AuditResult, RiskLevel};
use crate::commands::audit::{risk_badge, REPO_LEVEL_NOTE};
use crate::error::Result;
use crate::lock::LockFile;
use crate::ui::{
    color_stderr_enabled, eprint_error, eprint_warn, pluralize, with_spinner_transient,
};

use super::SyncContext;

/// Resolve the gate threshold. `--no-audit` wins, then the `--audit` flag,
/// then the config's `audit.fail-on`.
pub(super) fn effective_threshold(
    cfg_fail_on: Option<RiskLevel>,
    audit_flag: Option<RiskLevel>,
    no_audit: bool,
) -> Option<RiskLevel> {
    if no_audit {
        return None;
    }
    audit_flag.or(cfg_fail_on)
}

/// Audit the installed set and report breaches on stderr (stdout may carry the
/// JSON report). Returns `true` when the run should fail.
pub(super) fn enforce(ctx: &SyncContext, lock: &LockFile, threshold: RiskLevel) -> Result<bool> {
    let targets = collect_targets(lock, ctx.scope, &ctx.scope_root, &[])?;
    if targets.is_empty() {
        return Ok(false);
    }
    let label = format!(
        "auditing {} {}",
        targets.len(),
        pluralize(targets.len(), "asset", "assets")
    );
    let results = with_spinner_transient(ctx.animate, ctx.plain, label, || {
        Ok(audit_all(targets, false))
    })?;
    let (violations, unverified) = classify(&results, threshold);
    // stderr may be piped while stdout is a terminal, so gate color on stderr
    report(
        &violations,
        unverified,
        threshold,
        ctx.plain || !color_stderr_enabled(),
    );
    Ok(!violations.is_empty())
}

/// Split results into breaches (worst first) and a count of assets that could
/// not be fully verified.
fn classify(results: &[AuditResult], threshold: RiskLevel) -> (Vec<&AuditResult>, usize) {
    let mut violations: Vec<&AuditResult> = results
        .iter()
        .filter(|r| r.worst().is_some_and(|w| w >= threshold))
        .collect();
    violations.sort_by(|a, b| {
        b.worst()
            .cmp(&a.worst())
            .then_with(|| a.target.name.cmp(&b.target.name))
    });
    let unverified = results.iter().filter(|r| r.is_unverified()).count();
    (violations, unverified)
}

fn report(violations: &[&AuditResult], unverified: usize, threshold: RiskLevel, plain: bool) {
    if !violations.is_empty() {
        let n = violations.len();
        eprint_error(
            &format!(
                "audit gate (fail-on: {}): {n} {} at or above the threshold",
                threshold.label(),
                pluralize(n, "asset is", "assets are")
            ),
            plain,
        );
        for r in violations {
            let kind = match r.target.kind {
                AssetKind::Skill => "skill",
                AssetKind::Mcp => "mcp",
            };
            let level = r.worst().unwrap_or(RiskLevel::Unknown);
            let provider = match &r.outcome {
                crate::audit::AuditOutcome::Audited(a) => a
                    .worst_provider()
                    .map(|p| format!(" · {p}"))
                    .unwrap_or_default(),
                _ => String::new(),
            };
            eprintln!(
                "  ! {} [{kind}]  {}  {}{provider}",
                r.target.name,
                risk_badge(level, plain),
                source_label(&r.target.source)
            );
        }
        if violations.iter().any(|r| r.target.kind == AssetKind::Skill) {
            eprintln!("  note: {REPO_LEVEL_NOTE}");
        }
        eprintln!("  review with `kst audit -v`, or pass `--no-audit` to skip the gate");
    }
    if unverified > 0 {
        eprint_warn(
            &format!(
                "audit gate (fail-on: {}): {unverified} {} could not be fully verified",
                threshold.label(),
                pluralize(unverified, "asset", "assets")
            ),
            plain,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AssetAudit, AuditOutcome, AuditTarget, Verdict};

    #[test]
    fn no_audit_wins_then_flag_then_config() {
        let (h, c) = (Some(RiskLevel::High), Some(RiskLevel::Critical));
        assert_eq!(effective_threshold(c, h, true), None);
        assert_eq!(effective_threshold(c, h, false), h);
        assert_eq!(effective_threshold(c, None, false), c);
        assert_eq!(effective_threshold(None, None, false), None);
    }

    fn result(name: &str, outcome: AuditOutcome) -> AuditResult {
        AuditResult {
            target: AuditTarget {
                kind: AssetKind::Skill,
                name: name.into(),
                source: "https://github.com/o/r".into(),
                definition: None,
            },
            outcome,
        }
    }

    fn audited(level: RiskLevel) -> AuditOutcome {
        AuditOutcome::Audited(AssetAudit {
            verdicts: vec![Verdict {
                provider: "P".into(),
                level,
                summary: String::new(),
                audited_at: None,
                categories: Vec::new(),
            }],
            warnings: Vec::new(),
        })
    }

    #[test]
    fn breaches_are_at_or_above_and_unverifiable_never_fails() {
        let results = vec![
            result("a-high", audited(RiskLevel::High)),
            result("b-medium", audited(RiskLevel::Medium)),
            result("c-critical", audited(RiskLevel::Critical)),
            result("d-unknown", audited(RiskLevel::Unknown)),
            result("e-offline", AuditOutcome::Error("timeout".into())),
            result("f-unindexed", AuditOutcome::Unaudited("not on skills.sh")),
        ];
        let (violations, unverified) = classify(&results, RiskLevel::High);
        let names: Vec<&str> = violations.iter().map(|r| r.target.name.as_str()).collect();
        assert_eq!(names, vec!["d-unknown", "c-critical", "a-high"]);
        assert_eq!(unverified, 2);
    }
}
