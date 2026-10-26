//! Module that contains the [skills.sh](https://skills.sh) audit client for skills.
//!
//! Reads the public, unauthenticated `GET /api/v1/skills/audit/{owner}/{repo}/{skill}`
//! endpoint through the 24h audit cache. Only `github.com` sources map to a
//! skills.sh key; a skill with no audit yet answers `404`, which is cached as a
//! negative like any other answer.

use serde::{Deserialize, Serialize};

use crate::error::{err, Result};
use crate::fsops::http_client;
use crate::source::{parse_repo_url, RepoUrl};

use super::{cache, AssetAudit, AuditOutcome, RiskLevel, Verdict};

const API_BASE: &str = "https://skills.sh/api/v1/skills/audit";
const CACHE_NAMESPACE: &str = "skills-sh";

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
    match fetch(&format!("{owner}/{repo}/{skill}"), refresh) {
        Ok(Some(raw)) if !raw.audits.is_empty() => AuditOutcome::Audited(AssetAudit {
            verdicts: raw.audits.into_iter().map(normalize).collect(),
            warnings: Vec::new(),
        }),
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
fn fetch(key: &str, refresh: bool) -> Result<Option<RawAudit>> {
    if !refresh {
        if let Some(hit) = cache::lookup(CACHE_NAMESPACE, key) {
            return Ok(hit);
        }
    }
    let resp = http_client()?
        .get(format!("{API_BASE}/{key}"))
        .send()
        .map_err(|e| err(format!("skills.sh request failed: {e}")))?;
    let status = resp.status();
    let found = if status.as_u16() == 404 {
        None
    } else if !status.is_success() {
        return Err(err(format!("skills.sh returned HTTP {status}")));
    } else {
        let body = resp
            .text()
            .map_err(|e| err(format!("failed to read skills.sh response: {e}")))?;
        Some(
            serde_json::from_str::<RawAudit>(&body)
                .map_err(|e| err(format!("failed to parse skills.sh response: {e}")))?,
        )
    };
    cache::store(CACHE_NAMESPACE, key, &found);
    Ok(found)
}

/// Prefer the explicit `riskLevel` over the pass/warn/fail `status`: partners
/// contradict themselves (Snyk has answered `status: fail` with "No issues"),
/// and the level is the field they agree on the meaning of.
fn normalize(raw: RawVerdict) -> Verdict {
    let level = raw
        .risk_level
        .as_deref()
        .and_then(RiskLevel::from_risk_str)
        .unwrap_or_else(|| level_from_status(&raw.status));
    Verdict {
        provider: raw.provider,
        level,
        summary: raw.summary,
        audited_at: raw.audited_at,
        categories: raw.categories,
    }
}

fn level_from_status(status: &str) -> RiskLevel {
    match status.trim().to_ascii_lowercase().as_str() {
        "pass" | "ok" | "safe" => RiskLevel::Safe,
        "warn" | "warning" | "review" => RiskLevel::Medium,
        "fail" | "danger" | "blocked" => RiskLevel::High,
        _ => RiskLevel::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(status: &str, risk: Option<&str>) -> RawVerdict {
        RawVerdict {
            provider: "P".into(),
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
        assert_eq!(normalize(raw("fail", Some("HIGH"))).level, RiskLevel::High);
        assert_eq!(normalize(raw("fail", Some("NONE"))).level, RiskLevel::Safe);
    }

    #[test]
    fn status_is_the_fallback_and_unknown_is_fail_safe() {
        assert_eq!(normalize(raw("pass", None)).level, RiskLevel::Safe);
        assert_eq!(normalize(raw("warn", None)).level, RiskLevel::Medium);
        assert_eq!(
            normalize(raw("weird", Some("???"))).level,
            RiskLevel::Unknown
        );
    }

    #[test]
    fn parses_the_published_payload_shape() {
        let body = r#"{"id":"anthropics/skills/pdf","source":"anthropics/skills","slug":"pdf",
            "audits":[{"provider":"Snyk","slug":"snyk","status":"fail","summary":"Risk: HIGH",
            "riskLevel":"HIGH","auditedAt":"2026-06-01T00:00:00Z","categories":["PROMPT_INJECTION"]}]}"#;
        let parsed: RawAudit = serde_json::from_str(body).unwrap();
        let v = normalize(parsed.audits.into_iter().next().unwrap());
        assert_eq!(v.provider, "Snyk");
        assert_eq!(v.level, RiskLevel::High);
        assert_eq!(v.categories, vec!["PROMPT_INJECTION".to_string()]);
    }
}
