//! Module that contains `kasetto audit`, which reports security verdicts for the
//! installed skills and MCP servers read from the lock.
//!
//! Read-only: it never changes files or the lock. Skills are looked up on
//! skills.sh; MCP servers are checked offline and against OSV (see
//! [`crate::audit`]). It exits non-zero only when `--fail-on` is given and an
//! asset meets the threshold, so it doubles as a CI gate.

use serde::Serialize;

use crate::audit::{
    audit_all, collect_targets, source_label, AssetKind, AuditOutcome, AuditResult, RiskLevel,
};
use crate::colors::{ACCENT, ATTENTION, ERROR, INFO, INFRA, RESET, SECONDARY, SUCCESS};
use crate::commands::Outcome;
use crate::error::Result;
use crate::fsops::scope_root;
use crate::lock::load_lock;
use crate::model::{resolve_scope, Scope};
use crate::ui::{
    animations_enabled, color_stdout_enabled, pluralize, print_json, print_section_header,
    print_source_header, print_tip, print_tree_leaf, with_spinner_transient,
};

/// Shown with every skill report: skills.sh audits a repo, not the commit the
/// lock pins, so a verdict can describe code newer or older than what's installed.
pub(crate) const REPO_LEVEL_NOTE: &str =
    "skill verdicts are repo-level on skills.sh, not pinned to your locked commit";

pub(crate) struct AuditOptions<'a> {
    pub names: &'a [String],
    pub as_json: bool,
    pub plain: bool,
    pub quiet: bool,
    pub refresh: bool,
    pub verbose: bool,
    pub fail_on: Option<RiskLevel>,
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

#[derive(Serialize, Default, Debug, PartialEq, Eq)]
struct Summary {
    safe: usize,
    flagged: usize,
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

    let breached = opts
        .fail_on
        .is_some_and(|t| results.iter().any(|r| r.worst().is_some_and(|w| w >= t)));
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
            "failed": breached,
            "note": REPO_LEVEL_NOTE,
        }))?;
        return Ok(outcome);
    }
    if !opts.quiet {
        render(&results, opts.verbose, opts.fail_on, !color);
    }
    Ok(outcome)
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
/// package) that the default `high` gate lets through.
fn summarize(results: &[AuditResult]) -> Summary {
    let mut s = Summary::default();
    for r in results {
        match &r.outcome {
            AuditOutcome::Audited(a) => match a.worst() {
                Some(RiskLevel::Safe | RiskLevel::Low) => s.safe += 1,
                _ => s.flagged += 1,
            },
            AuditOutcome::Unaudited(_) => s.unaudited += 1,
            AuditOutcome::Error(_) => s.errors += 1,
        }
    }
    s
}

fn render(results: &[AuditResult], verbose: bool, fail_on: Option<RiskLevel>, plain: bool) {
    if results.is_empty() {
        println!("No skills or MCP servers installed to audit.");
        print_tip("run `kst sync` to install assets first", plain);
        return;
    }

    let mut printed = false;
    for (kind, label, via) in [
        (AssetKind::Skill, "Skills", "via skills.sh"),
        (
            AssetKind::Mcp,
            "MCP servers",
            "via kasetto + OSV + deps.dev",
        ),
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
    if let Some(t) = fail_on {
        let n = results
            .iter()
            .filter(|r| r.worst().is_some_and(|w| w >= t))
            .count();
        if n > 0 {
            let msg = format!(
                "{n} {} at or above the threshold (fail-on: {})",
                pluralize(n, "asset is", "assets are"),
                t.label()
            );
            if plain {
                println!("error: {msg}");
            } else {
                println!("{ACCENT}{ERROR}error:{RESET} {msg}");
            }
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
            let worst = a.worst().unwrap_or(RiskLevel::Unknown);
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
            "Audited {total} {noun}: {} safe, {} flagged, {} unaudited{errors}",
            s.safe, s.flagged, s.unaudited
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
         {SECONDARY}, {} unaudited{RESET}{errors}",
        s.safe, s.flagged, s.unaudited
    );
}

pub(crate) fn risk_badge(level: RiskLevel, plain: bool) -> String {
    if plain {
        return level.label().to_string();
    }
    let color = match level {
        RiskLevel::Safe => SUCCESS,
        RiskLevel::Low => INFO,
        RiskLevel::Medium | RiskLevel::Unknown => ATTENTION,
        RiskLevel::High | RiskLevel::Critical => ERROR,
    };
    if matches!(level, RiskLevel::High | RiskLevel::Critical) {
        format!("{ACCENT}{color}{}{RESET}", level.label())
    } else {
        format!("{color}{}{RESET}", level.label())
    }
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
            target: AuditTarget {
                kind,
                name: "x".into(),
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
    fn summary_counts_low_as_safe_and_keeps_failures_apart() {
        let results = vec![
            result(AssetKind::Skill, audited(RiskLevel::Safe)),
            result(AssetKind::Mcp, audited(RiskLevel::Low)),
            result(AssetKind::Skill, audited(RiskLevel::High)),
            result(AssetKind::Mcp, audited(RiskLevel::Unknown)),
            result(AssetKind::Skill, AuditOutcome::Unaudited("no audit yet")),
            result(AssetKind::Skill, AuditOutcome::Error("timeout".into())),
        ];
        assert_eq!(
            summarize(&results),
            Summary {
                safe: 2,
                flagged: 2,
                unaudited: 1,
                errors: 1,
            }
        );
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
