//! Opt-in skills.sh security gate for `sync`.
//!
//! Advisory by default — the gate only runs when a threshold is in effect:
//! either `audit.fail-on` in the config, or the `--audit` flag (which defaults
//! to `high`). `--no-audit` disables it outright. When active, every synced
//! skill is checked against skills.sh; any whose worst partner verdict meets or
//! exceeds the threshold fails the run (exit 1). A skill that can't be verified
//! (skills.sh unreachable, or not a github.com source) only warns — the gate
//! never fails on missing data, so a flaky network can't block a sync.

use rayon::prelude::*;

use crate::colors::{ACCENT, ERROR, INFO, RESET, SECONDARY};
use crate::model::{RiskLevel, State};
use crate::skills_sh::{audit_skill, AuditOutcome};
use crate::ui::{eprint_warn, short_source, with_spinner_transient};

use super::SyncContext;

/// Resolve the effective gate threshold from config + CLI flags.
///
/// `--no-audit` wins (gate off). Otherwise the config `audit.fail-on` sets the
/// threshold; a bare `--audit` with no config value defaults to `high`.
pub(super) fn effective_threshold(
    cfg_fail_on: Option<RiskLevel>,
    audit_flag: bool,
    no_audit_flag: bool,
) -> Option<RiskLevel> {
    if no_audit_flag {
        return None;
    }
    cfg_fail_on.or(if audit_flag {
        Some(RiskLevel::High)
    } else {
        None
    })
}

/// One skill that met or exceeded the gate threshold.
struct Violation {
    skill: String,
    source: String,
    level: RiskLevel,
    worst_provider: Option<String>,
}

/// Run the gate over the synced skill set. Returns `true` when the run should
/// fail (one or more skills breached the threshold). Unverifiable skills are
/// reported as a single warning and never cause a failure.
pub(super) fn enforce(ctx: &SyncContext, state: &State, threshold: RiskLevel) -> bool {
    let targets: Vec<(String, String)> = state
        .skills
        .values()
        .map(|e| (e.skill.clone(), e.source.clone()))
        .collect();
    if targets.is_empty() {
        return false;
    }

    let outcomes: Vec<(String, String, AuditOutcome)> = with_spinner_transient(
        ctx.animate,
        ctx.plain,
        format!("checking {} skills against skills.sh", targets.len()),
        || {
            Ok(targets
                .par_iter()
                .map(|(skill, source)| {
                    (
                        skill.clone(),
                        source.clone(),
                        audit_skill(skill, source, false),
                    )
                })
                .collect())
        },
    )
    .unwrap_or_default();

    let mut violations: Vec<Violation> = Vec::new();
    let mut unverified = 0usize;
    for (skill, source, outcome) in outcomes {
        match outcome {
            AuditOutcome::Audited(audit) => {
                if let Some(level) = audit.worst() {
                    if level >= threshold {
                        let worst_provider = audit
                            .verdicts
                            .iter()
                            .find(|v| v.level == level)
                            .map(|v| v.provider.clone());
                        violations.push(Violation {
                            skill,
                            source,
                            level,
                            worst_provider,
                        });
                    }
                }
            }
            // No audit, a non-github source, or a failed lookup are all "couldn't
            // confirm safe" — never a breach (the gate can't fail on missing data),
            // but unverified coverage a gate-user deserves to know about. Listed
            // exhaustively (not `_`) so a new variant forces a decision here.
            AuditOutcome::NoAudit | AuditOutcome::NotGitHub | AuditOutcome::Error(_) => {
                unverified += 1
            }
        }
    }

    violations.sort_by(|a, b| b.level.cmp(&a.level).then(a.skill.cmp(&b.skill)));
    report(ctx, &violations, unverified, threshold);
    !violations.is_empty()
}

fn report(ctx: &SyncContext, violations: &[Violation], unverified: usize, threshold: RiskLevel) {
    if violations.is_empty() {
        if unverified > 0 {
            eprint_warn(
                &format!(
                    "audit gate (fail-on: {}): {unverified} skill(s) could not be verified against skills.sh",
                    threshold.label()
                ),
                ctx.plain,
            );
        }
        return;
    }

    // In JSON mode stdout is reserved for the report — send the gate verdict to
    // stderr so the exit code is still meaningful for scripts/CI.
    if ctx.as_json {
        eprintln!(
            "error: audit gate (fail-on: {}) failed — {} skill(s) breached",
            threshold.label(),
            violations.len()
        );
        for v in violations {
            eprintln!(
                "  {} {} {}",
                v.skill,
                v.level.label(),
                short_source(&v.source)
            );
        }
        return;
    }

    println!();
    let n = violations.len();
    if ctx.plain {
        println!(
            "error: {n} skill(s) meet or exceed the audit threshold (fail-on: {})",
            threshold.label()
        );
        for v in violations {
            let p = v
                .worst_provider
                .as_deref()
                .map(|p| format!(" ({p})"))
                .unwrap_or_default();
            println!(
                "  ! {}  {}  {}{p}",
                v.skill,
                v.level.label(),
                short_source(&v.source)
            );
        }
    } else {
        println!(
            "{ACCENT}{ERROR}error:{RESET} {n} skill(s) meet or exceed the audit threshold \
             {SECONDARY}(fail-on: {}){RESET}",
            threshold.label()
        );
        for v in violations {
            let p = v
                .worst_provider
                .as_deref()
                .map(|p| format!(" {SECONDARY}({p}){RESET}"))
                .unwrap_or_default();
            println!(
                "  {ERROR}!{RESET} {ACCENT}{}{RESET}  {ERROR}{}{RESET}  {SECONDARY}{}{RESET}{p}",
                v.skill,
                v.level.label(),
                short_source(&v.source)
            );
        }
    }

    if unverified > 0 {
        eprint_warn(
            &format!("{unverified} additional skill(s) could not be verified against skills.sh"),
            ctx.plain,
        );
    }

    let note = "verdicts are repo-level on skills.sh, not your installed commit — review, then `--no-audit` to override";
    if ctx.plain {
        println!("note: {note}");
    } else {
        println!("{ACCENT}{INFO}note:{RESET} {SECONDARY}{note}{RESET}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_off_when_no_audit_flag_wins() {
        assert_eq!(effective_threshold(Some(RiskLevel::High), true, true), None);
        assert_eq!(effective_threshold(None, true, true), None);
    }

    #[test]
    fn config_threshold_used_when_present() {
        assert_eq!(
            effective_threshold(Some(RiskLevel::Medium), false, false),
            Some(RiskLevel::Medium)
        );
    }

    #[test]
    fn audit_flag_defaults_to_high() {
        assert_eq!(
            effective_threshold(None, true, false),
            Some(RiskLevel::High)
        );
    }

    #[test]
    fn config_threshold_beats_flag_default() {
        assert_eq!(
            effective_threshold(Some(RiskLevel::Critical), true, false),
            Some(RiskLevel::Critical)
        );
    }

    #[test]
    fn off_when_nothing_requested() {
        assert_eq!(effective_threshold(None, false, false), None);
    }
}
