//! Module that contains the [skills.sh](https://skills.sh) audit client for skills.
//!
//! Reads the public, unauthenticated `GET /api/v1/skills/audit/{owner}/{repo}/{skill}`
//! endpoint through the 24h audit cache. Only `github.com` sources map to a
//! skills.sh key; a skill with no audit yet answers `404`, which is cached as a
//! negative like any other answer.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::source::{parse_repo_url, RepoUrl};

use super::cache::{cached_get, encode_segment};
use super::{AssetAudit, AuditOutcome, RiskLevel, Verdict};

/// Shown with every skill report: skills.sh audits a repo, not the commit the
/// lock pins, so a verdict can describe code newer or older than what's installed.
pub(crate) const REPO_LEVEL_NOTE: &str =
    "skill verdicts are repo-level on skills.sh, not pinned to your locked commit";

const API_BASE: &str = "https://skills.sh/api/v1/skills/audit";
const CACHE_NAMESPACE: &str = "skills-sh";
const LABEL: &str = "skills.sh";

/// One partner's verdict as skills.sh returns it, before normalization.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawVerdict {
    #[serde(default)]
    provider: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    summary: String,
    #[serde(default, rename = "riskLevel")]
    risk_level: Option<String>,
    #[serde(default, rename = "auditedAt")]
    audited_at: Option<String>,
    #[serde(default)]
    categories: Vec<String>,
}

/// The audit payload for one skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawAudit {
    #[serde(default)]
    audits: Vec<RawVerdict>,
}

/// Resolve one installed skill to its audit outcome.
pub(super) fn audit_skill(skill: &str, source: &str, refresh: bool) -> AuditOutcome {
    let Some((owner, repo)) = github_owner_repo(source) else {
        return AuditOutcome::Unaudited("not on skills.sh");
    };
    match fetch(&owner, &repo, skill, refresh) {
        Ok(Some(raw)) if !raw.audits.is_empty() => AuditOutcome::Audited(normalize_all(raw)),
        Ok(_) => AuditOutcome::Unaudited("no audit yet"),
        Err(e) => AuditOutcome::Error(e.to_string()),
    }
}

/// Decompose a source URL into the `(owner, repo)` skills.sh keys on, or `None`
/// for anything not on `github.com` (GitLab, Gitea, GitHub Enterprise, local
/// paths), none of which skills.sh indexes.
fn github_owner_repo(source: &str) -> Option<(String, String)> {
    match parse_repo_url(source).ok()? {
        RepoUrl::GitHub { host, owner, repo } if host == "github.com" => Some((owner, repo)),
        _ => None,
    }
}

/// `Ok(None)` is skills.sh's `404`: the skill is not audited (yet).
fn fetch(owner: &str, repo: &str, skill: &str, refresh: bool) -> Result<Option<RawAudit>> {
    let url = format!(
        "{API_BASE}/{}/{}/{}",
        encode_segment(owner),
        encode_segment(repo),
        encode_segment(skill)
    );
    cached_get(
        LABEL,
        CACHE_NAMESPACE,
        &url,
        refresh,
        std::convert::identity,
    )
}

/// Every partner verdict that can be classified, plus one warning per partner
/// whose answer cannot: that is missing data, not a severity.
fn normalize_all(raw: RawAudit) -> AssetAudit {
    let mut audit = AssetAudit::default();
    for verdict in raw.audits {
        let partner = if verdict.provider.is_empty() {
            "a partner".to_string()
        } else {
            verdict.provider.clone()
        };
        match normalize(verdict) {
            Some(v) => audit.verdicts.push(v),
            None => audit
                .warnings
                .push(format!("{partner} returned an unrecognized status")),
        }
    }
    audit
}

/// Prefer the explicit `riskLevel` over the pass/warn/fail `status`: partners
/// contradict themselves (Snyk has answered `status: fail` with "No issues"),
/// and the level is the field they agree on the meaning of. `None` when
/// neither field is recognized.
fn normalize(raw: RawVerdict) -> Option<Verdict> {
    let level = raw
        .risk_level
        .as_deref()
        .and_then(RiskLevel::from_risk_str)
        .or_else(|| level_from_status(&raw.status))?;
    Some(Verdict {
        provider: raw.provider,
        level,
        summary: raw.summary,
        audited_at: raw.audited_at,
        categories: raw.categories,
    })
}

fn level_from_status(status: &str) -> Option<RiskLevel> {
    match status.trim().to_ascii_lowercase().as_str() {
        "pass" | "ok" | "safe" => Some(RiskLevel::Safe),
        "warn" | "warning" | "review" => Some(RiskLevel::Medium),
        "fail" | "danger" | "blocked" => Some(RiskLevel::High),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(provider: &str, status: &str, risk: Option<&str>) -> RawVerdict {
        RawVerdict {
            provider: provider.into(),
            status: status.into(),
            summary: String::new(),
            risk_level: risk.map(Into::into),
            audited_at: None,
            categories: Vec::new(),
        }
    }

    #[test]
    fn github_owner_repo_accepts_only_github_com() {
        assert_eq!(
            github_owner_repo("https://github.com/anthropics/skills"),
            Some(("anthropics".into(), "skills".into()))
        );
        assert_eq!(github_owner_repo("https://gitlab.com/group/proj"), None);
        assert_eq!(github_owner_repo("https://codeberg.org/o/r"), None);
        assert_eq!(github_owner_repo("https://ghe.example.com/acme/pack"), None);
        assert_eq!(github_owner_repo("./local/pack"), None);
    }

    #[test]
    fn non_github_source_is_unaudited_without_a_request() {
        assert!(matches!(
            audit_skill("x", "https://gitlab.com/g/p", false),
            AuditOutcome::Unaudited("not on skills.sh")
        ));
    }

    #[test]
    fn risk_level_wins_over_a_contradictory_status() {
        assert_eq!(
            normalize(raw("P", "fail", Some("HIGH"))).unwrap().level,
            RiskLevel::High
        );
        assert_eq!(
            normalize(raw("P", "fail", Some("NONE"))).unwrap().level,
            RiskLevel::Safe
        );
    }

    #[test]
    fn status_is_the_fallback_and_an_unrecognized_answer_is_dropped() {
        assert_eq!(
            normalize(raw("P", "pass", None)).unwrap().level,
            RiskLevel::Safe
        );
        assert_eq!(
            normalize(raw("P", "warn", None)).unwrap().level,
            RiskLevel::Medium
        );
        assert!(normalize(raw("P", "weird", Some("???"))).is_none());
        assert!(normalize(raw("P", "pending", None)).is_none());
    }

    #[test]
    fn unrecognized_partners_become_warnings_naming_the_partner() {
        let audit = normalize_all(RawAudit {
            audits: vec![
                raw("Snyk", "pass", None),
                raw("Socket", "pending", None),
                raw("", "skipped", None),
            ],
        });
        assert_eq!(audit.verdicts.len(), 1);
        assert_eq!(audit.verdicts[0].provider, "Snyk");
        assert_eq!(
            audit.warnings,
            vec![
                "Socket returned an unrecognized status".to_string(),
                "a partner returned an unrecognized status".to_string(),
            ]
        );

        let none = normalize_all(RawAudit {
            audits: vec![raw("Socket", "pending", None)],
        });
        assert!(none.verdicts.is_empty());
        assert_eq!(none.warnings.len(), 1);
    }

    #[test]
    fn parses_the_published_payload_shape() {
        let body = r#"{"id":"anthropics/skills/pdf","source":"anthropics/skills","slug":"pdf",
            "audits":[{"provider":"Snyk","slug":"snyk","status":"fail","summary":"Risk: HIGH",
            "riskLevel":"HIGH","auditedAt":"2026-06-01T00:00:00Z","categories":["PROMPT_INJECTION"]}]}"#;
        let parsed: RawAudit = serde_json::from_str(body).unwrap();
        let v = normalize(parsed.audits.into_iter().next().unwrap()).unwrap();
        assert_eq!(v.provider, "Snyk");
        assert_eq!(v.level, RiskLevel::High);
        assert_eq!(v.categories, vec!["PROMPT_INJECTION".to_string()]);
    }
}
