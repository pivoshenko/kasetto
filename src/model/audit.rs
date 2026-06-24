//! Security-audit types for the `audit` command.
//!
//! Kasetto overlays the per-skill security verdicts published by
//! [skills.sh](https://skills.sh) onto the skills recorded in the lock. The
//! audit endpoint aggregates several independent partners (Gen Agent Trust Hub,
//! Socket, Snyk, Runlayer, ZeroLeaks); each returns its own `status` +
//! `riskLevel`, and **they routinely disagree** (Anthropic's own `pdf` skill is
//! flagged `HIGH` by one partner and `pass` by the rest). So the headline a
//! skill gets is the *worst* partner verdict, always shown alongside the full
//! per-partner breakdown — kasetto never collapses the disagreement into a
//! single trusted score.
//!
//! Two further honesty caveats the command surfaces, not these types:
//! - the audit is **repo-level**, not pinned to the commit kasetto installed;
//! - only GitHub-hosted sources indexed by skills.sh return data (others 404).

use serde::{Deserialize, Serialize};

/// Normalized severity, ordered least → most severe. `Unknown` sorts *above*
/// `Critical` so a verdict we cannot classify is treated as the worst case by
/// [`SkillAudit::worst`] and by the opt-in sync gate — fail safe, never silent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RiskLevel {
    Safe,
    Low,
    Medium,
    High,
    Critical,
    Unknown,
}

impl RiskLevel {
    /// Map a skills.sh `riskLevel` string (case-insensitive). `NONE`/`SAFE`
    /// collapse to [`RiskLevel::Safe`]. Returns `None` for absent/unrecognized
    /// values so the caller can fall back to the verdict's `status`.
    pub(crate) fn from_risk_str(s: &str) -> Option<RiskLevel> {
        match s.trim().to_ascii_uppercase().as_str() {
            "NONE" | "SAFE" => Some(RiskLevel::Safe),
            "LOW" => Some(RiskLevel::Low),
            "MED" | "MEDIUM" | "MODERATE" => Some(RiskLevel::Medium),
            "HIGH" => Some(RiskLevel::High),
            "CRITICAL" => Some(RiskLevel::Critical),
            _ => None,
        }
    }

    /// Fallback when no usable `riskLevel`: derive severity from the pass/warn/
    /// fail verdict `status`. An unrecognized status yields [`RiskLevel::Unknown`].
    fn from_status(status: &str) -> RiskLevel {
        match status.trim().to_ascii_lowercase().as_str() {
            "pass" | "ok" | "safe" => RiskLevel::Safe,
            "warn" | "warning" | "review" => RiskLevel::Medium,
            "fail" | "danger" | "blocked" => RiskLevel::High,
            _ => RiskLevel::Unknown,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            RiskLevel::Safe => "safe",
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
            RiskLevel::Critical => "critical",
            RiskLevel::Unknown => "unknown",
        }
    }
}

/// One partner's verdict as returned by skills.sh, before normalization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RawVerdict {
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default, rename = "riskLevel")]
    pub risk_level: Option<String>,
    #[serde(default, rename = "auditedAt")]
    pub audited_at: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
}

/// The full audit payload for one skill (`GET /skills/audit/{owner}/{repo}/{skill}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RawAudit {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub audits: Vec<RawVerdict>,
}

/// A partner verdict with severity normalized to a [`RiskLevel`].
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Verdict {
    pub provider: String,
    pub status: String,
    pub level: RiskLevel,
    pub summary: String,
    pub audited_at: Option<String>,
    pub categories: Vec<String>,
}

impl Verdict {
    fn from_raw(raw: RawVerdict) -> Verdict {
        let level = raw
            .risk_level
            .as_deref()
            .and_then(RiskLevel::from_risk_str)
            .unwrap_or_else(|| RiskLevel::from_status(&raw.status));
        Verdict {
            provider: raw.provider,
            status: raw.status,
            level,
            summary: raw.summary,
            audited_at: raw.audited_at,
            categories: raw.categories,
        }
    }
}

/// Normalized audit for a single installed skill.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SkillAudit {
    /// The skill's directory name (as installed / locked).
    pub skill: String,
    /// The configured source URL the skill came from.
    pub source: String,
    /// The `owner/repo/skill` key on skills.sh.
    pub id: String,
    pub verdicts: Vec<Verdict>,
}

impl SkillAudit {
    pub(crate) fn from_raw(skill: String, source: String, raw: RawAudit) -> SkillAudit {
        SkillAudit {
            skill,
            source,
            id: raw.id,
            verdicts: raw.audits.into_iter().map(Verdict::from_raw).collect(),
        }
    }

    /// The most severe partner verdict — the skill's headline severity. `None`
    /// only when no partner reported (an empty `audits` array).
    pub(crate) fn worst(&self) -> Option<RiskLevel> {
        self.verdicts.iter().map(|v| v.level).max()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_str_normalizes_aliases_and_case() {
        assert_eq!(RiskLevel::from_risk_str("none"), Some(RiskLevel::Safe));
        assert_eq!(RiskLevel::from_risk_str("SAFE"), Some(RiskLevel::Safe));
        assert_eq!(RiskLevel::from_risk_str(" med "), Some(RiskLevel::Medium));
        assert_eq!(
            RiskLevel::from_risk_str("Critical"),
            Some(RiskLevel::Critical)
        );
        assert_eq!(RiskLevel::from_risk_str("bogus"), None);
    }

    #[test]
    fn ordering_puts_unknown_above_critical() {
        assert!(RiskLevel::Safe < RiskLevel::Low);
        assert!(RiskLevel::High < RiskLevel::Critical);
        assert!(RiskLevel::Critical < RiskLevel::Unknown);
    }

    #[test]
    fn verdict_prefers_risk_level_over_status() {
        // The real Snyk shape: status=fail but the summary says "No issues" —
        // we key off the explicit riskLevel, not the contradictory status.
        let v = Verdict::from_raw(RawVerdict {
            provider: "Snyk".into(),
            slug: "snyk".into(),
            status: "fail".into(),
            summary: "Risk: HIGH · No issues".into(),
            risk_level: Some("HIGH".into()),
            audited_at: None,
            categories: vec![],
        });
        assert_eq!(v.level, RiskLevel::High);
    }

    #[test]
    fn verdict_falls_back_to_status_when_no_risk_level() {
        let v = Verdict::from_raw(RawVerdict {
            provider: "Socket".into(),
            slug: "socket".into(),
            status: "pass".into(),
            summary: "No alerts".into(),
            risk_level: None,
            audited_at: None,
            categories: vec![],
        });
        assert_eq!(v.level, RiskLevel::Safe);
    }

    #[test]
    fn unparseable_verdict_is_unknown() {
        let v = Verdict::from_raw(RawVerdict {
            provider: "Mystery".into(),
            slug: String::new(),
            status: "weird".into(),
            summary: String::new(),
            risk_level: Some("???".into()),
            audited_at: None,
            categories: vec![],
        });
        assert_eq!(v.level, RiskLevel::Unknown);
    }

    #[test]
    fn worst_is_the_max_partner_severity() {
        // Mirrors anthropics/skills/pdf: four safe-ish partners, one HIGH.
        let audit = SkillAudit::from_raw(
            "pdf".into(),
            "https://github.com/anthropics/skills".into(),
            RawAudit {
                id: "anthropics/skills/pdf".into(),
                source: "anthropics/skills".into(),
                slug: "pdf".into(),
                audits: vec![
                    RawVerdict {
                        provider: "Gen Agent Trust Hub".into(),
                        slug: "agent-trust-hub".into(),
                        status: "pass".into(),
                        summary: "generally safe".into(),
                        risk_level: Some("SAFE".into()),
                        audited_at: None,
                        categories: vec!["PROMPT_INJECTION".into()],
                    },
                    RawVerdict {
                        provider: "Snyk".into(),
                        slug: "snyk".into(),
                        status: "fail".into(),
                        summary: "Risk: HIGH · No issues".into(),
                        risk_level: Some("HIGH".into()),
                        audited_at: None,
                        categories: vec![],
                    },
                    RawVerdict {
                        provider: "ZeroLeaks".into(),
                        slug: "zeroleaks".into(),
                        status: "pass".into(),
                        summary: "Score: 93/100".into(),
                        risk_level: Some("NONE".into()),
                        audited_at: None,
                        categories: vec![],
                    },
                ],
            },
        );
        assert_eq!(audit.worst(), Some(RiskLevel::High));
        assert_eq!(audit.id, "anthropics/skills/pdf");
        assert_eq!(audit.verdicts.len(), 3);
    }

    #[test]
    fn worst_is_none_for_empty_audits() {
        let audit = SkillAudit::from_raw(
            "x".into(),
            "https://github.com/o/r".into(),
            RawAudit {
                id: "o/r/x".into(),
                source: "o/r".into(),
                slug: "x".into(),
                audits: vec![],
            },
        );
        assert_eq!(audit.worst(), None);
    }
}
