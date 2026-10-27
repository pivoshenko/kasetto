//! Module that contains the opt-in security gate `sync` runs after installing.
//!
//! The gate is off unless a threshold is in effect: `--audit [LEVEL]` on the
//! command line, else `audit.fail-on` in the config; `--no-audit` turns it off
//! outright. When on, every skill and MCP server recorded in the lock is audited
//! (see [`crate::audit`]) and any at or above the threshold fails the run. An
//! asset that cannot be verified only warns, so a flaky network or an
//! unindexed source never blocks a sync by default; `--fail-on-unverified` (or
//! `audit.fail-on-unverified`) makes it fail closed instead.
//!
//! The gate judges what was installed, so a failing run leaves those files in
//! place: the non-zero exit is what lets CI stop before the environment is used.
//! Its verdict travels in the sync report (`audit` in `--json`, and the runtime
//! state `doctor` reads), while the human rendering goes to stderr.

use crate::audit::{
    audit_all, collect_targets, source_label, AssetKind, AuditOutcome, AuditResult, RiskLevel,
    REPO_LEVEL_NOTE,
};
use crate::error::Result;
use crate::lock::LockFile;
use crate::model::{AuditGate, AuditViolation};
use crate::ui::{eprint_error, eprint_warn, pluralize, risk_badge, with_spinner_transient};

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

/// Audit the installed set and decide. Nothing is printed here: the caller
/// renders [`report`] after its own output so the verdicts land where they did
/// before the result joined the JSON report.
pub(super) fn enforce(
    ctx: &SyncContext,
    lock: &LockFile,
    threshold: RiskLevel,
    fail_on_unverified: bool,
) -> Result<AuditGate> {
    let targets = collect_targets(lock, ctx.scope, &ctx.scope_root, &[])?;
    let results = if targets.is_empty() {
        Vec::new()
    } else {
        let label = format!(
            "auditing {} {}",
            targets.len(),
            pluralize(targets.len(), "asset", "assets")
        );
        with_spinner_transient(ctx.animate, ctx.plain, label, || {
            Ok(audit_all(targets, false))
        })?
    };
    Ok(decide(&results, threshold, fail_on_unverified))
}

/// Split results into breaches (worst first) and a count of assets that could
/// not be fully verified, then apply the policy.
fn decide(results: &[AuditResult], threshold: RiskLevel, fail_on_unverified: bool) -> AuditGate {
    let mut breaches: Vec<(&AuditResult, RiskLevel)> = results
        .iter()
        .filter_map(|r| r.worst().filter(|w| *w >= threshold).map(|w| (r, w)))
        .collect();
    breaches.sort_by(|(a, la), (b, lb)| lb.cmp(la).then_with(|| a.target.name.cmp(&b.target.name)));
    let violations: Vec<AuditViolation> = breaches
        .into_iter()
        .map(|(r, level)| AuditViolation {
            kind: r.target.kind,
            name: r.target.name.clone(),
            source: r.target.source.clone(),
            level,
            provider: match &r.outcome {
                AuditOutcome::Audited(a) => a.worst_provider().map(str::to_string),
                _ => None,
            },
        })
        .collect();
    let unverified = results.iter().filter(|r| r.is_unverified()).count();
    let passed = violations.is_empty() && !(fail_on_unverified && unverified > 0);
    AuditGate {
        threshold,
        fail_on_unverified,
        violations,
        unverified,
        passed,
    }
}

/// Render the verdicts on stderr (stdout may carry the JSON report).
pub(super) fn report(gate: &AuditGate, plain: bool) {
    let threshold = gate.threshold.label();
    if !gate.violations.is_empty() {
        let n = gate.violations.len();
        eprint_error(
            &format!(
                "audit gate (fail-on: {threshold}): {n} {} at or above the threshold",
                pluralize(n, "asset is", "assets are")
            ),
            plain,
        );
        for v in &gate.violations {
            let kind = match v.kind {
                AssetKind::Skill => "skill",
                AssetKind::Mcp => "mcp",
            };
            let provider = v
                .provider
                .as_deref()
                .map(|p| format!(" · {p}"))
                .unwrap_or_default();
            eprintln!(
                "  ! {} [{kind}]  {}  {}{provider}",
                v.name,
                risk_badge(v.level, plain),
                source_label(&v.source)
            );
        }
        if gate.violations.iter().any(|v| v.kind == AssetKind::Skill) {
            eprintln!("  note: {REPO_LEVEL_NOTE}");
        }
        eprintln!("  review with `kst audit -v`, or pass `--no-audit` to skip the gate");
    }
    if gate.unverified > 0 {
        let n = gate.unverified;
        let msg = format!(
            "audit gate (fail-on: {threshold}): {n} {} could not be fully verified",
            pluralize(n, "asset", "assets")
        );
        if gate.fail_on_unverified {
            eprint_error(&format!("{msg} (fail-on-unverified)"), plain);
            eprintln!(
                "  review with `kst audit -v`, or drop `--fail-on-unverified` to warn instead"
            );
        } else {
            eprint_warn(&msg, plain);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AssetAudit, AuditTarget, Verdict};

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
            target: AuditTarget::new(AssetKind::Skill, name, "https://github.com/o/r"),
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

    fn fixture() -> Vec<AuditResult> {
        vec![
            result("a-high", audited(RiskLevel::High)),
            result("b-medium", audited(RiskLevel::Medium)),
            result("c-critical", audited(RiskLevel::Critical)),
            result("e-offline", AuditOutcome::Error("timeout".into())),
            result("f-unindexed", AuditOutcome::Unaudited("not on skills.sh")),
        ]
    }

    #[test]
    fn breaches_are_at_or_above_and_unverifiable_only_warns_by_default() {
        let gate = decide(&fixture(), RiskLevel::High, false);
        let names: Vec<&str> = gate.violations.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, vec!["c-critical", "a-high"]);
        assert_eq!(gate.violations[0].provider.as_deref(), Some("P"));
        assert_eq!(gate.unverified, 2);
        assert!(!gate.passed);

        let gate = decide(&fixture(), RiskLevel::Critical, false);
        assert_eq!(gate.violations.len(), 1);
        let clean = decide(&fixture()[3..], RiskLevel::High, false);
        assert!(clean.violations.is_empty());
        assert_eq!(clean.unverified, 2);
        assert!(clean.passed, "unverified assets never fail by default");
    }

    #[test]
    fn fail_on_unverified_fails_the_gate_without_a_breach() {
        let gate = decide(&fixture()[3..], RiskLevel::High, true);
        assert!(gate.violations.is_empty());
        assert_eq!(gate.unverified, 2);
        assert!(!gate.passed);

        let gate = decide(&fixture()[..2], RiskLevel::Critical, true);
        assert_eq!(gate.unverified, 0);
        assert!(
            gate.passed,
            "fully verified assets below the threshold pass"
        );

        let empty = decide(&[], RiskLevel::Low, true);
        assert!(empty.passed);
    }

    #[test]
    fn json_block_carries_the_policy_and_sanitized_rows_only() {
        let gate = decide(&fixture(), RiskLevel::High, true);
        let v = serde_json::to_value(&gate).unwrap();
        assert_eq!(v["threshold"], "high");
        assert_eq!(v["fail_on_unverified"], true);
        assert_eq!(v["unverified"], 2);
        assert_eq!(v["passed"], false);
        let first = &v["violations"][0];
        assert_eq!(first["kind"], "skill");
        assert_eq!(first["name"], "c-critical");
        assert_eq!(first["source"], "https://github.com/o/r");
        assert_eq!(first["level"], "critical");
        assert_eq!(first["provider"], "P");
        let mut keys: Vec<&str> = first
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, ["kind", "level", "name", "provider", "source"]);
    }
}
