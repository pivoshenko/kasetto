//! Module that contains `kasetto audit`, which reports security verdicts for the
//! installed skills and MCP servers read from the lock.
//!
//! Read-only: it never changes files or the lock. Skills are looked up on
//! skills.sh; MCP servers are checked offline and against OSV (see
//! [`crate::audit`]). It exits non-zero only when `--fail-on` is given and an
//! asset meets the threshold, or `--fail-on-unverified` is given and an asset
//! could not be verified, so it doubles as a CI gate.

use serde::Serialize;

use crate::audit::{
    audit_all, collect_targets, source_label, AssetKind, AuditOutcome, AuditResult, RiskLevel,
    MCP_PROVIDERS, REPO_LEVEL_NOTE,
};
use crate::colors::{ACCENT, ATTENTION, ERROR, INFRA, RESET, SECONDARY, SUCCESS};
use crate::commands::Outcome;
use crate::error::Result;
use crate::fsops::scope_root;
use crate::lock::load_lock;
use crate::model::{resolve_scope, Scope};
use crate::ui::{
    animations_enabled, color_stdout_enabled, pluralize, print_json, print_section_header,
    print_source_header, print_tip, print_tree_leaf, risk_badge, with_spinner_transient,
};

pub(crate) struct AuditOptions<'a> {
    pub names: &'a [String],
    pub as_json: bool,
    pub plain: bool,
    pub quiet: bool,
    pub refresh: bool,
    pub verbose: bool,
    pub fail_on: Option<RiskLevel>,
    /// `--fail-on-unverified`: an asset that could not be verified sets the
    /// exit code too, with or without `--fail-on`.
    pub fail_on_unverified: bool,
    pub scope_override: Option<Scope>,
}

#[derive(Serialize)]
struct Row {
    name: String,
    source: String,
    /// `audited` | `unaudited` | `error`
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    worst: Option<RiskLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audit: Option<crate::audit::AssetAudit>,
}

/// Every asset lands in exactly one of `safe`, `flagged`, or `unverified`.
/// `unaudited` and `errors` break `unverified` down by outcome; the rest of it
/// is assets audited with a lookup warning or no usable verdict.
#[derive(Serialize, Default, Debug, PartialEq, Eq)]
struct Summary {
    safe: usize,
    flagged: usize,
    unverified: usize,
    unaudited: usize,
    errors: usize,
}

pub(crate) fn run(opts: &AuditOptions) -> Result<Outcome> {
    let scope = resolve_scope(opts.scope_override, None);
    let project_root = std::env::current_dir().unwrap_or_default();
    let lock = load_lock(scope, &project_root)?;
    let targets = collect_targets(&lock, scope, &scope_root(scope, &project_root)?, opts.names)?;

    let color = !opts.plain && color_stdout_enabled();
    let animate = animations_enabled(opts.quiet, opts.as_json, opts.plain);
    let label = format!(
        "auditing {} {}",
        targets.len(),
        pluralize(targets.len(), "asset", "assets")
    );
    let refresh = opts.refresh;
    let results =
        with_spinner_transient(animate, !color, label, || Ok(audit_all(targets, refresh)))?;

    let breached = breached(&results, opts.fail_on, opts.fail_on_unverified);
    let outcome = Outcome::from_failed(breached);

    if opts.as_json {
        let (skills, mcps): (Vec<_>, Vec<_>) = results
            .iter()
            .partition(|r| r.target.kind == AssetKind::Skill);
        print_json(&serde_json::json!({
            "scope": match scope {
                Scope::Global => "global",
                Scope::Project => "project",
            },
            "skills": skills.into_iter().map(row).collect::<Vec<_>>(),
            "mcps": mcps.into_iter().map(row).collect::<Vec<_>>(),
            "summary": summarize(&results),
            "fail_on": opts.fail_on,
            "fail_on_unverified": opts.fail_on_unverified,
            "failed": breached,
            "note": REPO_LEVEL_NOTE,
        }))?;
        return Ok(outcome);
    }
    if !opts.quiet {
        render(
            &results,
            opts.verbose,
            opts.fail_on,
            opts.fail_on_unverified,
            !color,
        );
    }
    Ok(outcome)
}

/// Whether the run exits non-zero: an asset at or above `fail_on`, or, with
/// `fail_on_unverified`, one that could not be fully verified.
fn breached(results: &[AuditResult], fail_on: Option<RiskLevel>, fail_on_unverified: bool) -> bool {
    at_or_above(results, fail_on) > 0 || fail_on_unverified && unverified(results) > 0
}

fn at_or_above(results: &[AuditResult], fail_on: Option<RiskLevel>) -> usize {
    let Some(t) = fail_on else {
        return 0;
    };
    results
        .iter()
        .filter(|r| r.worst().is_some_and(|w| w >= t))
        .count()
}

fn unverified(results: &[AuditResult]) -> usize {
    results.iter().filter(|r| r.is_unverified()).count()
}

fn row(r: &AuditResult) -> Row {
    let (status, reason, audit) = match &r.outcome {
        AuditOutcome::Audited(a) => ("audited", None, Some(a.clone())),
        AuditOutcome::Unaudited(why) => ("unaudited", Some((*why).to_string()), None),
        AuditOutcome::Error(e) => ("error", Some(e.clone()), None),
    };
    Row {
        name: r.target.name.clone(),
        source: r.target.source.clone(),
        status,
        worst: r.worst(),
        reason,
        audit,
    }
}

/// `low` counts as safe: it is the level of hygiene findings (an unpinned
/// package) that the default `high` gate lets through. Anything not fully
/// verified is `unverified` whatever its verdicts say, so a lookup that failed
/// can never read as safe.
fn summarize(results: &[AuditResult]) -> Summary {
    let mut s = Summary::default();
    for r in results {
        // A known problem outranks a missing lookup: the asset is flagged even
        // when another provider could not be reached
        if r.worst() > Some(RiskLevel::Low) {
            s.flagged += 1;
            continue;
        }
        if r.is_unverified() {
            s.unverified += 1;
            match &r.outcome {
                AuditOutcome::Unaudited(_) => s.unaudited += 1,
                AuditOutcome::Error(_) => s.errors += 1,
                AuditOutcome::Audited(_) => {}
            }
            continue;
        }
        s.safe += 1;
    }
    s
}

fn render(
    results: &[AuditResult],
    verbose: bool,
    fail_on: Option<RiskLevel>,
    fail_on_unverified: bool,
    plain: bool,
) {
    if results.is_empty() {
        println!("No skills or MCP servers installed to audit.");
        print_tip("run `kst sync` to install assets first", plain);
        return;
    }

    let mut printed = false;
    let mcp_via = format!("via {}", MCP_PROVIDERS.join(" + "));
    for (kind, label, via) in [
        (AssetKind::Skill, "Skills", "via skills.sh"),
        (AssetKind::Mcp, "MCP servers", mcp_via.as_str()),
    ] {
        let rows: Vec<&AuditResult> = results.iter().filter(|r| r.target.kind == kind).collect();
        if rows.is_empty() {
            continue;
        }
        print_section_header(label, Some((rows.len(), via)), printed, plain);
        printed = true;
        render_group(&rows, verbose, plain);
    }

    let summary = summarize(results);
    println!();
    print_summary_line(results.len(), &summary, plain);
    let mut errors = Vec::new();
    if let Some(t) = fail_on {
        let n = at_or_above(results, fail_on);
        if n > 0 {
            errors.push(format!(
                "{n} {} at or above the threshold (fail-on: {})",
                pluralize(n, "asset is", "assets are"),
                t.label()
            ));
        }
    }
    if fail_on_unverified {
        let n = unverified(results);
        if n > 0 {
            errors.push(format!(
                "{n} {} could not be verified (fail-on-unverified)",
                pluralize(n, "asset", "assets")
            ));
        }
    }
    for msg in errors {
        if plain {
            println!("error: {msg}");
        } else {
            println!("{ACCENT}{ERROR}error:{RESET} {msg}");
        }
    }
    if results.iter().any(|r| r.target.kind == AssetKind::Skill) {
        print_tip(REPO_LEVEL_NOTE, plain);
    }
}

/// One section: rows grouped by source (already sorted that way), names padded
/// so the severity column lines up within the section.
fn render_group(rows: &[&AuditResult], verbose: bool, plain: bool) {
    let name_w = rows
        .iter()
        .map(|r| r.target.name.chars().count())
        .max()
        .unwrap_or(0);
    let mut i = 0;
    while i < rows.len() {
        let source = &rows[i].target.source;
        let group: Vec<&AuditResult> = rows[i..]
            .iter()
            .take_while(|r| &r.target.source == source)
            .copied()
            .collect();
        i += group.len();
        print_source_header(&source_label(source), Some(group.len()), None, plain);
        for (j, r) in group.iter().enumerate() {
            let is_last = j + 1 == group.len();
            let pad = " ".repeat(name_w - r.target.name.chars().count());
            let (badge, badge_w, tail) = headline(r, plain);
            let badge_pad = " ".repeat(8usize.saturating_sub(badge_w));
            print_tree_leaf(
                is_last,
                &format!("{}{pad}  {badge}{badge_pad}  {tail}", r.target.name),
                plain,
            );
            if verbose {
                print_details(r, is_last, plain);
            }
        }
    }
}

/// Severity badge (and its printed width) plus a dim explanation for one row.
fn headline(r: &AuditResult, plain: bool) -> (String, usize, String) {
    match &r.outcome {
        AuditOutcome::Audited(a) => {
            let Some(worst) = a.worst() else {
                return (dim("-", plain), 1, dim("no usable verdict", plain));
            };
            let mut tail = match r.target.kind {
                AssetKind::Skill => {
                    let n = a.verdicts.len();
                    format!("{n} {}", pluralize(n, "partner", "partners"))
                }
                AssetKind::Mcp => {
                    let top = a.verdicts.iter().find(|v| v.level == worst);
                    truncate(top.map(|v| v.summary.as_str()).unwrap_or(""), 64)
                }
            };
            if !a.warnings.is_empty() {
                tail.push_str(" · lookup unavailable");
            }
            (
                risk_badge(worst, plain),
                worst.label().len(),
                dim(&tail, plain),
            )
        }
        AuditOutcome::Unaudited(why) => (dim("-", plain), 1, dim(why, plain)),
        AuditOutcome::Error(e) => {
            let msg = truncate(e, 64);
            let msg = if plain {
                msg
            } else {
                format!("{ERROR}{msg}{RESET}")
            };
            (dim("-", plain), 1, msg)
        }
    }
}

fn print_details(r: &AuditResult, is_last: bool, plain: bool) {
    let AuditOutcome::Audited(a) = &r.outcome else {
        return;
    };
    let rail = if is_last { "   " } else { "│  " };
    let rail = if plain {
        rail.to_string()
    } else {
        format!("{INFRA}{rail}{RESET}")
    };
    for v in &a.verdicts {
        let summary = truncate(&v.summary, 72);
        if plain {
            println!(
                "{rail}  {:<20} {:<8} {summary}",
                v.provider,
                v.level.label()
            );
        } else {
            let pad = " ".repeat(8usize.saturating_sub(v.level.label().len()));
            println!(
                "{rail}  {SECONDARY}{:<20}{RESET} {}{pad} {SECONDARY}{summary}{RESET}",
                v.provider,
                risk_badge(v.level, plain)
            );
        }
    }
    for w in &a.warnings {
        println!("{rail}  {}", dim(&format!("warning: {w}"), plain));
    }
}

fn print_summary_line(total: usize, s: &Summary, plain: bool) {
    let errors = if s.errors > 0 {
        format!(", {} {}", s.errors, pluralize(s.errors, "error", "errors"))
    } else {
        String::new()
    };
    let noun = pluralize(total, "asset", "assets");
    if plain {
        println!(
            "Audited {total} {noun}: {} safe, {} flagged, {} unverified{errors}",
            s.safe, s.flagged, s.unverified
        );
        return;
    }
    let errors = if errors.is_empty() {
        errors
    } else {
        format!("{ERROR}{errors}{RESET}")
    };
    println!(
        "{ACCENT}{SUCCESS}Audited{RESET} {total} {noun}{SECONDARY}:{RESET} \
         {SUCCESS}{} safe{RESET}{SECONDARY},{RESET} {ATTENTION}{} flagged{RESET}\
         {SECONDARY}, {} unverified{RESET}{errors}",
        s.safe, s.flagged, s.unverified
    );
}

fn dim(s: &str, plain: bool) -> String {
    if plain {
        s.to_string()
    } else {
        format!("{SECONDARY}{s}{RESET}")
    }
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AssetAudit, AuditTarget, Verdict};

    fn result(kind: AssetKind, outcome: AuditOutcome) -> AuditResult {
        AuditResult {
            target: AuditTarget::new(kind, "x", "https://github.com/o/r"),
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

    /// An audited result whose lookup partly failed, or that kept no verdict.
    fn partial(level: Option<RiskLevel>) -> AuditOutcome {
        let AuditOutcome::Audited(mut a) = audited(level.unwrap_or(RiskLevel::Safe)) else {
            unreachable!()
        };
        if level.is_none() {
            a.verdicts.clear();
        }
        a.warnings.push("OSV lookup failed: timeout".into());
        AuditOutcome::Audited(a)
    }

    #[test]
    fn summary_counts_low_as_safe_and_keeps_failures_apart() {
        let results = vec![
            result(AssetKind::Skill, audited(RiskLevel::Safe)),
            result(AssetKind::Mcp, audited(RiskLevel::Low)),
            result(AssetKind::Skill, audited(RiskLevel::High)),
            result(AssetKind::Mcp, audited(RiskLevel::Medium)),
            result(AssetKind::Skill, AuditOutcome::Unaudited("no audit yet")),
            result(AssetKind::Skill, AuditOutcome::Error("timeout".into())),
        ];
        assert_eq!(
            summarize(&results),
            Summary {
                safe: 2,
                flagged: 2,
                unverified: 2,
                unaudited: 1,
                errors: 1,
            }
        );
    }

    #[test]
    fn summary_never_counts_a_partial_audit_as_safe_but_flags_known_problems() {
        let results = vec![
            result(AssetKind::Mcp, partial(Some(RiskLevel::Safe))),
            result(AssetKind::Mcp, partial(Some(RiskLevel::High))),
            result(AssetKind::Skill, partial(None)),
            result(AssetKind::Mcp, audited(RiskLevel::Safe)),
        ];
        let s = summarize(&results);
        assert_eq!(
            s,
            Summary {
                safe: 1,
                flagged: 1,
                unverified: 2,
                unaudited: 0,
                errors: 0,
            }
        );
        assert_eq!(s.safe + s.flagged + s.unverified, results.len());
    }

    #[test]
    fn fail_on_unverified_sets_the_exit_code_with_or_without_fail_on() {
        let results = vec![
            result(AssetKind::Mcp, partial(Some(RiskLevel::High))),
            result(AssetKind::Skill, AuditOutcome::Unaudited("no audit yet")),
            result(AssetKind::Mcp, audited(RiskLevel::Safe)),
        ];
        assert!(!breached(&results, None, false));
        assert!(breached(&results, None, true));
        assert!(breached(&results, Some(RiskLevel::High), false));
        assert!(!breached(&results, Some(RiskLevel::Critical), false));
        assert!(breached(&results, Some(RiskLevel::Critical), true));
        assert_eq!(unverified(&results), 2);

        let verified = vec![result(AssetKind::Mcp, audited(RiskLevel::Low))];
        assert!(!breached(&verified, Some(RiskLevel::High), true));
    }

    #[test]
    fn json_row_carries_status_reason_and_worst() {
        let r = row(&result(
            AssetKind::Skill,
            AuditOutcome::Unaudited("not on skills.sh"),
        ));
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["status"], "unaudited");
        assert_eq!(v["reason"], "not on skills.sh");
        assert!(v.get("worst").is_none());

        let r = row(&result(AssetKind::Mcp, audited(RiskLevel::Critical)));
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["status"], "audited");
        assert_eq!(v["worst"], "critical");
        assert_eq!(v["audit"]["verdicts"][0]["provider"], "P");
    }

    #[test]
    fn truncate_keeps_short_text_and_marks_cuts() {
        assert_eq!(truncate("  short ", 10), "short");
        assert_eq!(truncate("abcdefghij", 5), "abcd…");
    }
}
