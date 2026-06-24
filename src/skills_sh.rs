//! [skills.sh](https://skills.sh) security-audit client.
//!
//! Reads the public, unauthenticated audit endpoint
//! `GET /api/v1/skills/audit/{owner}/{repo}/{skill}` and caches each response
//! on disk with a TTL — the same offline-friendly discipline as the source
//! cache, but time-bounded because audits are regenerated upstream over time.
//!
//! Coverage is best-effort: only `github.com`-hosted sources map to a skills.sh
//! key, and a skill with no audit yet returns `404` (modeled as
//! [`AuditLookup::None`], cached as a negative so we don't re-probe every run).
//! Opt out of caching with `KASETTO_NO_CACHE`.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{err, Result};
use crate::fsops::{dirs_kasetto_cache, hash_str, http_client};
use crate::model::{RawAudit, SkillAudit};
use crate::source::{parse_repo_url, RepoUrl};

const API_BASE: &str = "https://skills.sh/api/v1/skills/audit";
const CACHE_SUBDIR: &str = "audits";
/// Refresh a cached audit (positive or negative) at most once per 24h.
const TTL_SECS: u64 = 24 * 60 * 60;

/// Outcome of an audit lookup. `None` is a real, cacheable answer: the skill is
/// known to skills.sh but has no audit yet (or isn't indexed) — distinct from a
/// transport error, which surfaces as `Err`.
#[derive(Debug, Clone)]
pub(crate) enum AuditLookup {
    Found(RawAudit),
    None,
}

/// The audit outcome for one installed skill — the resolved, caller-agnostic
/// view shared by the `audit` command and the `sync` gate.
#[derive(Debug, Clone)]
pub(crate) enum AuditOutcome {
    /// skills.sh returned one or more partner verdicts.
    Audited(SkillAudit),
    /// The source is `github.com` but skills.sh has no (or empty) audit yet.
    NoAudit,
    /// The source isn't a `github.com` repo, so skills.sh can't index it.
    NotGitHub,
    /// The lookup failed (network/transport) — severity is unverifiable.
    Error(String),
}

/// Resolve a single installed skill (by name + configured source) to its audit
/// outcome, going through the on-disk cache unless `refresh` is set.
pub(crate) fn audit_skill(skill: &str, source: &str, refresh: bool) -> AuditOutcome {
    let Some((owner, repo)) = github_owner_repo(source) else {
        return AuditOutcome::NotGitHub;
    };
    match fetch_skill_audit(&owner, &repo, skill, refresh) {
        Ok(AuditLookup::Found(raw)) => {
            let audit = SkillAudit::from_raw(skill.to_string(), source.to_string(), raw);
            if audit.verdicts.is_empty() {
                AuditOutcome::NoAudit
            } else {
                AuditOutcome::Audited(audit)
            }
        }
        Ok(AuditLookup::None) => AuditOutcome::NoAudit,
        Err(e) => AuditOutcome::Error(e.to_string()),
    }
}

/// Decompose a configured source URL into the `(owner, repo)` skills.sh keys on,
/// or `None` for anything not hosted on `github.com` (GitLab, Gitea, GitHub
/// Enterprise, local paths) — none of which skills.sh indexes.
pub(crate) fn github_owner_repo(source: &str) -> Option<(String, String)> {
    match parse_repo_url(source).ok()? {
        RepoUrl::GitHub { host, owner, repo } if host == "github.com" => Some((owner, repo)),
        _ => None,
    }
}

/// Fetch (or read from cache) the audit for `owner/repo/skill`. Set `refresh`
/// to bypass a fresh cache entry and force a network read.
pub(crate) fn fetch_skill_audit(
    owner: &str,
    repo: &str,
    skill: &str,
    refresh: bool,
) -> Result<AuditLookup> {
    let key = format!("{owner}/{repo}/{skill}");

    if !refresh {
        if let Some(hit) = cache_lookup(&key) {
            return Ok(hit);
        }
    }

    let url = format!("{API_BASE}/{key}");
    let lookup = fetch_remote(&url)?;
    cache_store(&key, &lookup);
    Ok(lookup)
}

fn fetch_remote(url: &str) -> Result<AuditLookup> {
    let resp = http_client()?
        .get(url)
        .send()
        .map_err(|e| err(format!("skills.sh request failed: {e}")))?;

    let status = resp.status();
    if status.as_u16() == 404 {
        return Ok(AuditLookup::None);
    }
    if !status.is_success() {
        return Err(err(format!("skills.sh returned HTTP {status}")));
    }

    let body = resp
        .text()
        .map_err(|e| err(format!("failed to read skills.sh response: {e}")))?;
    let audit: RawAudit = serde_json::from_str(&body)
        .map_err(|e| err(format!("failed to parse skills.sh response: {e}")))?;
    Ok(AuditLookup::Found(audit))
}

// --- on-disk TTL cache (mirrors update_notifier's negative-cacheable shape) ---

#[derive(Serialize, Deserialize)]
struct CachedAudit {
    checked_at: u64,
    /// `None` is a cached negative (skills.sh 404 — no audit yet).
    audit: Option<RawAudit>,
}

fn disabled() -> bool {
    std::env::var_os("KASETTO_NO_CACHE").is_some_and(|v| !v.is_empty())
}

fn cache_path(key: &str) -> Option<PathBuf> {
    Some(
        dirs_kasetto_cache()
            .ok()?
            .join(CACHE_SUBDIR)
            .join(format!("{}.json", hash_str(key))),
    )
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn cache_lookup(key: &str) -> Option<AuditLookup> {
    if disabled() {
        return None;
    }
    let path = cache_path(key)?;
    let text = fs::read_to_string(&path).ok()?;
    let entry: CachedAudit = serde_json::from_str(&text).ok()?;
    if now_secs().saturating_sub(entry.checked_at) >= TTL_SECS {
        return None;
    }
    Some(match entry.audit {
        Some(a) => AuditLookup::Found(a),
        None => AuditLookup::None,
    })
}

fn cache_store(key: &str, lookup: &AuditLookup) {
    if disabled() {
        return;
    }
    let Some(path) = cache_path(key) else {
        return;
    };
    let entry = CachedAudit {
        checked_at: now_secs(),
        audit: match lookup {
            AuditLookup::Found(a) => Some(a.clone()),
            AuditLookup::None => None,
        },
    };
    // Best-effort: a write failure just means the next run re-fetches.
    if let Some(parent) = path.parent() {
        if fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    if let Ok(text) = serde_json::to_string(&entry) {
        let tmp = path.with_extension("json.tmp");
        if fs::write(&tmp, text).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_owner_repo_accepts_github_com() {
        assert_eq!(
            github_owner_repo("https://github.com/anthropics/skills"),
            Some(("anthropics".into(), "skills".into()))
        );
    }

    #[test]
    fn github_owner_repo_rejects_non_github_hosts() {
        assert_eq!(github_owner_repo("https://gitlab.com/group/proj"), None);
        assert_eq!(github_owner_repo("https://codeberg.org/o/r"), None);
        // GitHub Enterprise is a GitHub-shaped host skills.sh doesn't index.
        assert_eq!(github_owner_repo("https://ghe.example.com/acme/pack"), None);
        assert_eq!(github_owner_repo("./local/pack"), None);
    }
}
