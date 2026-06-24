//! `audit` — overlay skills.sh security verdicts onto installed skills.
//!
//! Read-only and advisory: it resolves the scope's locked skills, looks each up
//! on [skills.sh](https://skills.sh), and reports the worst partner verdict per
//! skill (with the full breakdown under `--verbose`). It never changes files or
//! the lock. The verdicts are repo-level — not pinned to the installed commit —
//! and only `github.com` sources are indexed; both caveats are surfaced in the
//! output rather than hidden behind a single trusted score.

use rayon::prelude::*;
use serde::Serialize;

use crate::colors::{ACCENT, ATTENTION, ERROR, INFO, RESET, SECONDARY, SUCCESS};
use crate::error::Result;
use crate::lock::load_lock;
use crate::model::{resolve_scope, RiskLevel, Scope, SkillAudit};
use crate::skills_sh::{audit_skill, AuditOutcome};
use crate::ui::{
    animations_enabled, color_stdout_enabled, print_json, print_section_header,
    print_source_header, print_tip, print_tree_leaf, short_source, with_spinner_transient,
};

/// One installed skill plus the audit outcome for it.
#[derive(Serialize)]
struct AuditRow {
    skill: String,
    source: String,
    /// `audited` | `no_audit` | `not_github` | `error`
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    worst: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audit: Option<SkillAudit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize, Default)]
struct AuditSummary {
    safe: usize,
    flagged: usize,
    unaudited: usize,
    errors: usize,
}

pub(crate) fn run(
    as_json: bool,
    plain: bool,
    quiet: bool,
    refresh: bool,
    verbose: bool,
    scope_override: Option<Scope>,
    names: &[String],
) -> Result<()> {
    if quiet && !as_json {
        return Ok(());
    }

    let scope = resolve_scope(scope_override, None);
    let project_root = std::env::current_dir().unwrap_or_default();
    let lock = load_lock(scope, &project_root)?;

    // (skill, source) for every locked skill, optionally filtered by name.
    let mut targets: Vec<(String, String)> = lock
        .state()
        .skills
        .values()
        .filter(|e| names.is_empty() || names.iter().any(|n| n == &e.skill))
        .map(|e| (e.skill.clone(), e.source.clone()))
        .collect();
    targets.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));

    let color = !plain && color_stdout_enabled();
    let animate = animations_enabled(quiet, as_json, plain);

    let rows = with_spinner_transient(
        animate,
        !color,
        format!("auditing {} skills via skills.sh", targets.len()),
        || Ok(audit_targets(&targets, refresh)),
    )?;

    let summary = summarize(&rows);

    if as_json {
        let out = serde_json::json!({
            "scope": scope_label(scope),
            "skills": rows,
            "summary": summary,
        });
        return print_json(&out);
    }

    render(&rows, &summary, verbose, !color);
    Ok(())
}

/// Resolve every target through skills.sh in parallel, preserving config order.
fn audit_targets(targets: &[(String, String)], refresh: bool) -> Vec<AuditRow> {
    targets
        .par_iter()
        .map(|(skill, source)| {
            let skill = skill.clone();
            let source = source.clone();
            match audit_skill(&skill, &source, refresh) {
                AuditOutcome::Audited(audit) => {
                    let worst = audit.worst().map(|w| w.label().to_string());
                    AuditRow {
                        skill,
                        source,
                        status: "audited",
                        worst,
                        audit: Some(audit),
                        error: None,
                    }
                }
                AuditOutcome::NoAudit => AuditRow {
                    skill,
                    source,
                    status: "no_audit",
                    worst: None,
                    audit: None,
                    error: None,
                },
                AuditOutcome::NotGitHub => AuditRow {
                    skill,
                    source,
                    status: "not_github",
                    worst: None,
                    audit: None,
                    error: None,
                },
                AuditOutcome::Error(e) => AuditRow {
                    skill,
                    source,
                    status: "error",
                    worst: None,
                    audit: None,
                    error: Some(e),
                },
            }
        })
        .collect()
}

fn summarize(rows: &[AuditRow]) -> AuditSummary {
    let mut s = AuditSummary::default();
    for r in rows {
        match r.status {
            "audited" => {
                let worst = r.audit.as_ref().and_then(|a| a.worst());
                if matches!(worst, Some(RiskLevel::Safe) | Some(RiskLevel::Low)) {
                    s.safe += 1;
                } else {
                    s.flagged += 1;
                }
            }
            "no_audit" | "not_github" => s.unaudited += 1,
            _ => s.errors += 1,
        }
    }
    s
}

fn render(rows: &[AuditRow], summary: &AuditSummary, verbose: bool, plain: bool) {
    if rows.is_empty() {
        println!("No skills installed to audit.");
        print_tip("run `kasetto sync` to install skills first", plain);
        return;
    }

    print_section_header("Security", Some((rows.len(), "skills · skills.sh")), plain);

    // Group by source, preserving the already-sorted order.
    let mut groups: Vec<(String, Vec<&AuditRow>)> = Vec::new();
    for r in rows {
        if let Some(g) = groups.iter_mut().find(|(k, _)| k == &r.source) {
            g.1.push(r);
        } else {
            groups.push((r.source.clone(), vec![r]));
        }
    }

    for (source, items) in &groups {
        print_source_header(
            &short_source(source),
            Some(items.len()),
            Some(false),
            Some(62),
            plain,
        );
        for (i, r) in items.iter().enumerate() {
            let is_last = i == items.len() - 1;
            let (badge, tail) = row_badge_and_tail(r, plain);
            print_tree_leaf(
                is_last,
                None,
                &r.skill,
                false,
                &format!("{badge}  {tail}"),
                24,
                plain,
            );
            if verbose {
                if let Some(audit) = &r.audit {
                    for v in &audit.verdicts {
                        print_verdict_line(&v.provider, v.level, &v.summary, plain);
                    }
                }
            }
        }
    }

    print_summary_line(rows.len(), summary, plain);
    println!();
    print_tip(
        "verdicts are repo-level on skills.sh, not pinned to your locked commit",
        plain,
    );
}

/// The headline badge (colored severity) + a dim explanatory tail for one row.
fn row_badge_and_tail(r: &AuditRow, plain: bool) -> (String, String) {
    match r.status {
        "audited" => {
            let audit = r.audit.as_ref();
            let worst = audit.and_then(|a| a.worst()).unwrap_or(RiskLevel::Unknown);
            let n = audit.map(|a| a.verdicts.len()).unwrap_or(0);
            let tail = format!("{n} partners");
            (risk_badge(worst, plain), dim(&tail, plain))
        }
        "no_audit" => (risk_badge_dash(plain), dim("no audit yet", plain)),
        "not_github" => (risk_badge_dash(plain), dim("not on skills.sh", plain)),
        _ => {
            let msg = r.error.as_deref().unwrap_or("lookup failed");
            (risk_badge_dash(plain), err_text(msg, plain))
        }
    }
}

fn print_verdict_line(provider: &str, level: RiskLevel, summary: &str, plain: bool) {
    let badge = risk_badge(level, plain);
    let summary = truncate(summary, 64);
    if plain {
        println!("      {provider:<22} {} {summary}", level.label());
    } else {
        println!("      {SECONDARY}{provider:<22}{RESET} {badge}  {SECONDARY}{summary}{RESET}");
    }
}

fn print_summary_line(total: usize, s: &AuditSummary, plain: bool) {
    println!();
    if plain {
        println!(
            "Audited {total} skills — {} safe, {} flagged, {} unaudited{}",
            s.safe,
            s.flagged,
            s.unaudited,
            if s.errors > 0 {
                format!(", {} errors", s.errors)
            } else {
                String::new()
            }
        );
        return;
    }
    let errors = if s.errors > 0 {
        format!("{SECONDARY}, {RESET}{ERROR}{} errors{RESET}", s.errors)
    } else {
        String::new()
    };
    println!(
        "{ACCENT}{SUCCESS}Audited{RESET} {total} skills {SECONDARY}—{RESET} \
         {SUCCESS}{} safe{RESET}{SECONDARY}, {RESET}{ATTENTION}{} flagged{RESET}\
         {SECONDARY}, {} unaudited{RESET}{errors}",
        s.safe, s.flagged, s.unaudited
    );
}

fn risk_badge(level: RiskLevel, plain: bool) -> String {
    if plain {
        return level.label().to_string();
    }
    let color = match level {
        RiskLevel::Safe => SUCCESS,
        RiskLevel::Low => INFO,
        RiskLevel::Medium | RiskLevel::Unknown => ATTENTION,
        RiskLevel::High | RiskLevel::Critical => ERROR,
    };
    let bold = matches!(level, RiskLevel::High | RiskLevel::Critical);
    if bold {
        format!("{ACCENT}{color}{}{RESET}", level.label())
    } else {
        format!("{color}{}{RESET}", level.label())
    }
}

fn risk_badge_dash(plain: bool) -> String {
    if plain {
        "—".to_string()
    } else {
        format!("{SECONDARY}—{RESET}")
    }
}

fn dim(s: &str, plain: bool) -> String {
    if plain {
        s.to_string()
    } else {
        format!("{SECONDARY}{s}{RESET}")
    }
}

fn err_text(s: &str, plain: bool) -> String {
    if plain {
        s.to_string()
    } else {
        format!("{ERROR}{s}{RESET}")
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

fn scope_label(scope: Scope) -> &'static str {
    match scope {
        Scope::Global => "global",
        Scope::Project => "project",
    }
}
