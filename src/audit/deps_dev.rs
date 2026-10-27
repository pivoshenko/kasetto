//! Module that contains the [deps.dev](https://deps.dev) lookup for the package
//! an MCP server launches.
//!
//! deps.dev adds the health signals OSV does not carry: whether the publisher
//! deprecated the version that runs, and the OpenSSF Scorecard of the source
//! repository the package names. Its advisories mirror OSV and are ignored here
//! so nothing is counted twice. Every rule only ever adds risk: a missing
//! scorecard or an unknown package produces no verdict at all.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::{err, Result};
use crate::fsops::http_client;

use super::mcp::{Ecosystem, Package};
use super::{cache, RiskLevel, Verdict};

const API_BASE: &str = "https://api.deps.dev/v3";
const CACHE_NAMESPACE: &str = "deps.dev";
const PROVIDER: &str = "deps.dev";

/// Scorecard overall scores (0-10) below which the source repo adds a verdict.
const SCORE_MEDIUM_BELOW: f64 = 3.0;
const SCORE_LOW_BELOW: f64 = 5.0;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageInfo {
    #[serde(default)]
    versions: Vec<VersionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VersionSummary {
    version_key: VersionKey,
    #[serde(default)]
    is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VersionKey {
    version: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VersionInfo {
    #[serde(default)]
    is_deprecated: bool,
    #[serde(default)]
    related_projects: Vec<RelatedProject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RelatedProject {
    project_key: ProjectKey,
    #[serde(default)]
    relation_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProjectKey {
    id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ProjectInfo {
    #[serde(default)]
    scorecard: Option<Scorecard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Scorecard {
    #[serde(default)]
    date: Option<String>,
    overall_score: f64,
    #[serde(default)]
    checks: Vec<ScorecardCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScorecardCheck {
    name: String,
    /// `-1` means the check did not apply to the repo.
    score: i32,
}

/// Health verdicts for `pkg`: a deprecated version and a weak source-repo
/// scorecard. A package deps.dev does not know yields none.
pub(super) fn signals(pkg: &Package, refresh: bool) -> Result<Vec<Verdict>> {
    let system = system(pkg.ecosystem);
    let name = encode(&pkg.name);
    let version = match &pkg.version {
        Some(v) => v.clone(),
        None => {
            let Some(info) =
                fetch::<PackageInfo>(&format!("systems/{system}/packages/{name}"), refresh)?
            else {
                return Ok(Vec::new());
            };
            match info.versions.into_iter().find(|v| v.is_default) {
                Some(v) => v.version_key.version,
                None => return Ok(Vec::new()),
            }
        }
    };
    let Some(info) = fetch::<VersionInfo>(
        &format!(
            "systems/{system}/packages/{name}/versions/{}",
            encode(&version)
        ),
        refresh,
    )?
    else {
        return Ok(Vec::new());
    };
    // The repo link is whatever the package metadata claims, unverified, so a
    // package can borrow a well-run repo's scorecard. That is harmless only
    // while a high score never lowers risk: keep every rule one-directional
    let scorecard = match source_repo(&info) {
        Some(repo) => fetch::<ProjectInfo>(&format!("projects/{}", encode(repo)), refresh)?
            .and_then(|p| p.scorecard),
        None => None,
    };
    Ok(to_verdicts(pkg, &info, scorecard.as_ref()))
}

fn source_repo(info: &VersionInfo) -> Option<&str> {
    info.related_projects
        .iter()
        .find(|p| p.relation_type == "SOURCE_REPO")
        .map(|p| p.project_key.id.as_str())
}

fn to_verdicts(pkg: &Package, info: &VersionInfo, scorecard: Option<&Scorecard>) -> Vec<Verdict> {
    let mut out = Vec::new();
    if info.is_deprecated {
        // A floating launch runs the default version, so its deprecation means
        // the whole package is abandoned rather than one release superseded
        let (level, what) = match pkg.version {
            Some(_) => (RiskLevel::Low, "the pinned version of"),
            None => (RiskLevel::Medium, "the latest version of"),
        };
        out.push(Verdict {
            provider: PROVIDER.to_string(),
            level,
            summary: format!("{what} {} is deprecated", pkg.name),
            audited_at: None,
            categories: vec!["deprecated".to_string()],
        });
    }
    if let Some(sc) = scorecard {
        let level = if sc.overall_score < SCORE_MEDIUM_BELOW {
            Some(RiskLevel::Medium)
        } else if sc.overall_score < SCORE_LOW_BELOW {
            Some(RiskLevel::Low)
        } else {
            None
        };
        if let Some(level) = level {
            out.push(Verdict {
                provider: PROVIDER.to_string(),
                level,
                summary: format!(
                    "source repo of {} scores {:.1}/10 on OpenSSF Scorecard",
                    pkg.name, sc.overall_score
                ),
                audited_at: sc.date.clone(),
                categories: vec!["low-scorecard".to_string()],
            });
        }
        if sc
            .checks
            .iter()
            .any(|c| c.name == "Maintained" && c.score == 0)
        {
            out.push(Verdict {
                provider: PROVIDER.to_string(),
                level: RiskLevel::Low,
                summary: format!("source repo of {} shows no recent activity", pkg.name),
                audited_at: sc.date.clone(),
                categories: vec!["unmaintained".to_string()],
            });
        }
    }
    out
}

fn system(ecosystem: Ecosystem) -> &'static str {
    match ecosystem {
        Ecosystem::Npm => "npm",
        Ecosystem::PyPI => "pypi",
    }
}

/// Percent-encode one path segment: deps.dev takes `@scope/name` and
/// `github.com/owner/repo` as a single segment.
fn encode(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for b in segment.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// GET `path` under the API, cached for 24h. `Ok(None)` is deps.dev's `404`
/// (unknown package, version, or project), cached like any other answer.
fn fetch<T>(path: &str, refresh: bool) -> Result<Option<T>>
where
    T: Serialize + DeserializeOwned,
{
    if !refresh {
        if let Some(hit) = cache::lookup::<Option<T>>(CACHE_NAMESPACE, path) {
            return Ok(hit);
        }
    }
    let resp = http_client()?
        .get(format!("{API_BASE}/{path}"))
        .send()
        .map_err(|e| err(format!("deps.dev request failed: {e}")))?;
    let status = resp.status();
    let found = if status.as_u16() == 404 {
        None
    } else if !status.is_success() {
        return Err(err(format!("deps.dev returned HTTP {status}")));
    } else {
        let text = resp
            .text()
            .map_err(|e| err(format!("failed to read deps.dev response: {e}")))?;
        Some(
            serde_json::from_str::<T>(&text)
                .map_err(|e| err(format!("failed to parse deps.dev response: {e}")))?,
        )
    };
    cache::store(CACHE_NAMESPACE, path, &found);
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(version: Option<&str>) -> Package {
        Package {
            ecosystem: Ecosystem::Npm,
            name: "@acme/server".into(),
            version: version.map(Into::into),
        }
    }

    fn version(deprecated: bool) -> VersionInfo {
        serde_json::from_str(&format!(
            r#"{{"isDeprecated":{deprecated},"relatedProjects":[
                {{"projectKey":{{"id":"github.com/acme/tracker"}},"relationType":"ISSUE_TRACKER"}},
                {{"projectKey":{{"id":"github.com/acme/server"}},"relationType":"SOURCE_REPO"}}
            ]}}"#
        ))
        .unwrap()
    }

    fn scorecard(overall: f64, maintained: i32) -> Scorecard {
        serde_json::from_str(&format!(
            r#"{{"date":"2026-08-24T00:00:00Z","overallScore":{overall},
                "checks":[{{"name":"Maintained","score":{maintained}}},{{"name":"Fuzzing","score":0}}]}}"#
        ))
        .unwrap()
    }

    fn levels(verdicts: Vec<Verdict>) -> Vec<(String, RiskLevel)> {
        verdicts.into_iter().map(|v| (v.summary, v.level)).collect()
    }

    #[test]
    fn source_repo_is_the_source_repo_relation_only() {
        assert_eq!(source_repo(&version(false)), Some("github.com/acme/server"));
        let none: VersionInfo = serde_json::from_str("{}").unwrap();
        assert_eq!(source_repo(&none), None);
    }

    #[test]
    fn deprecation_is_low_when_pinned_and_medium_when_floating() {
        assert_eq!(
            levels(to_verdicts(&pkg(Some("1.0.0")), &version(true), None)),
            vec![(
                "the pinned version of @acme/server is deprecated".to_string(),
                RiskLevel::Low
            )]
        );
        assert_eq!(
            levels(to_verdicts(&pkg(None), &version(true), None)),
            vec![(
                "the latest version of @acme/server is deprecated".to_string(),
                RiskLevel::Medium
            )]
        );
    }

    #[test]
    fn weak_scorecards_add_risk_and_strong_ones_add_nothing() {
        let at = |score| {
            to_verdicts(
                &pkg(Some("1.0.0")),
                &version(false),
                Some(&scorecard(score, 10)),
            )
            .into_iter()
            .map(|v| v.level)
            .collect::<Vec<_>>()
        };
        assert_eq!(at(2.9), vec![RiskLevel::Medium]);
        assert_eq!(at(4.9), vec![RiskLevel::Low]);
        assert!(at(5.0).is_empty());
        assert!(at(9.5).is_empty());
    }

    #[test]
    fn unmaintained_repo_is_flagged_but_not_applicable_checks_are_not() {
        let flagged = to_verdicts(&pkg(None), &version(false), Some(&scorecard(8.0, 0)));
        assert_eq!(
            levels(flagged),
            vec![(
                "source repo of @acme/server shows no recent activity".to_string(),
                RiskLevel::Low
            )]
        );
        assert!(to_verdicts(&pkg(None), &version(false), Some(&scorecard(8.0, -1))).is_empty());
    }

    #[test]
    fn a_healthy_package_without_a_scorecard_yields_nothing() {
        assert!(to_verdicts(&pkg(None), &version(false), None).is_empty());
    }

    #[test]
    fn real_responses_parse() {
        let pkg: PackageInfo = serde_json::from_str(
            r#"{"packageKey":{"system":"NPM","name":"x"},"versions":[
                {"versionKey":{"system":"NPM","name":"x","version":"0.1.0"},"isDefault":false,"isDeprecated":true},
                {"versionKey":{"system":"NPM","name":"x","version":"1.0.0"},"isDefault":true,"isDeprecated":false}
            ]}"#,
        )
        .unwrap();
        let default = pkg.versions.into_iter().find(|v| v.is_default).unwrap();
        assert_eq!(default.version_key.version, "1.0.0");
        let project: ProjectInfo = serde_json::from_str(
            r#"{"projectKey":{"id":"github.com/a/b"},"starsCount":3,"license":"MIT"}"#,
        )
        .unwrap();
        assert!(project.scorecard.is_none());
    }

    #[test]
    fn path_segments_are_percent_encoded() {
        assert_eq!(encode("@acme/server"), "%40acme%2Fserver");
        assert_eq!(encode("github.com/a/b"), "github.com%2Fa%2Fb");
        assert_eq!(encode("1.0.0+build"), "1.0.0%2Bbuild");
    }
}
