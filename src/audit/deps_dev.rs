//! Module that contains the [deps.dev](https://deps.dev) lookups for the package
//! an MCP server launches.
//!
//! deps.dev adds the health signals OSV does not carry: whether the publisher
//! deprecated the version that runs, and the OpenSSF Scorecard of the source
//! repository the package names. It also resolves which version a floating
//! launch runs today (the registry default), which the OSV query then uses.
//! Its advisories mirror OSV and are ignored here so nothing is counted twice.
//! Every rule only ever adds risk: a missing scorecard or an unknown package
//! produces no verdict at all.
//!
//! The three endpoints are separate calls so the orchestrator in the parent
//! module can run them in phases and fetch each distinct version and project
//! once per run.

use serde::{Deserialize, Serialize};

use crate::error::Result;

use super::cache::{cached_get, encode_segment};
use super::mcp::{Ecosystem, Package};
use super::{RiskLevel, Verdict};

const API_BASE: &str = "https://api.deps.dev/v3";
const CACHE_NAMESPACE: &str = "deps.dev";
pub(super) const PROVIDER: &str = "deps.dev";

/// Scorecard overall scores (0-10) below which the source repo adds a verdict.
const SCORE_MEDIUM_BELOW: f64 = 3.0;
const SCORE_LOW_BELOW: f64 = 5.0;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageInfo {
    #[serde(default)]
    versions: Vec<VersionSummary>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VersionSummary {
    version_key: VersionKey,
    #[serde(default)]
    is_default: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct VersionKey {
    version: String,
}

/// What the cache keeps of a `GetPackage` answer: the default version alone,
/// not the full version list a popular package carries.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DefaultVersion {
    version: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VersionInfo {
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
pub(super) struct Scorecard {
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

/// The version a floating launch of `pkg` runs today: the registry default.
/// `Ok(None)` when deps.dev does not know the package or it has no default.
pub(super) fn default_version(pkg: &Package, refresh: bool) -> Result<Option<String>> {
    let url = format!("{API_BASE}/{}", package_path(pkg));
    let found = cached_get(
        PROVIDER,
        CACHE_NAMESPACE,
        &url,
        refresh,
        |info: PackageInfo| DefaultVersion {
            version: info
                .versions
                .into_iter()
                .find(|v| v.is_default)
                .map(|v| v.version_key.version),
        },
    )?;
    Ok(found.and_then(|d| d.version))
}

/// Deprecation status and related projects of one version of `pkg`.
/// `Ok(None)` when deps.dev does not know that version.
pub(super) fn version(pkg: &Package, version: &str, refresh: bool) -> Result<Option<VersionInfo>> {
    let url = format!(
        "{API_BASE}/{}/versions/{}",
        package_path(pkg),
        encode_segment(version)
    );
    cached_get(
        PROVIDER,
        CACHE_NAMESPACE,
        &url,
        refresh,
        std::convert::identity,
    )
}

/// The OpenSSF Scorecard of a project (`github.com/owner/repo`), if deps.dev
/// knows the project and Scorecard has run on it.
pub(super) fn scorecard(project: &str, refresh: bool) -> Result<Option<Scorecard>> {
    let url = format!("{API_BASE}/projects/{}", encode_segment(project));
    let found: Option<ProjectInfo> = cached_get(
        PROVIDER,
        CACHE_NAMESPACE,
        &url,
        refresh,
        std::convert::identity,
    )?;
    Ok(found.and_then(|p| p.scorecard))
}

/// The project id of the source repo a version claims, if any. The link is
/// whatever the package metadata says, unverified, so a package can borrow a
/// well-run repo's scorecard. That is harmless only while a high score never
/// lowers risk: every rule in [`to_verdicts`] is one-directional.
pub(super) fn source_repo(info: &VersionInfo) -> Option<&str> {
    info.related_projects
        .iter()
        .find(|p| p.relation_type == "SOURCE_REPO")
        .map(|p| p.project_key.id.as_str())
}

/// Health verdicts for `pkg` given the version that runs and its source repo's
/// scorecard. `pkg` is the launch as configured, so a floating launch is still
/// reported as such even though a concrete version was looked up.
pub(super) fn to_verdicts(
    pkg: &Package,
    info: &VersionInfo,
    scorecard: Option<&Scorecard>,
) -> Vec<Verdict> {
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

fn package_path(pkg: &Package) -> String {
    format!(
        "systems/{}/packages/{}",
        system(pkg.ecosystem),
        encode_segment(&pkg.name)
    )
}

fn system(ecosystem: Ecosystem) -> &'static str {
    match ecosystem {
        Ecosystem::Npm => "npm",
        Ecosystem::PyPI => "pypi",
    }
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

    fn version_info(deprecated: bool) -> VersionInfo {
        serde_json::from_str(&format!(
            r#"{{"isDeprecated":{deprecated},"relatedProjects":[
                {{"projectKey":{{"id":"github.com/acme/tracker"}},"relationType":"ISSUE_TRACKER"}},
                {{"projectKey":{{"id":"github.com/acme/server"}},"relationType":"SOURCE_REPO"}}
            ]}}"#
        ))
        .unwrap()
    }

    fn scorecard_fixture(overall: f64, maintained: i32) -> Scorecard {
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
        assert_eq!(
            source_repo(&version_info(false)),
            Some("github.com/acme/server")
        );
        let none: VersionInfo = serde_json::from_str("{}").unwrap();
        assert_eq!(source_repo(&none), None);
    }

    #[test]
    fn deprecation_is_low_when_pinned_and_medium_when_floating() {
        assert_eq!(
            levels(to_verdicts(&pkg(Some("1.0.0")), &version_info(true), None)),
            vec![(
                "the pinned version of @acme/server is deprecated".to_string(),
                RiskLevel::Low
            )]
        );
        assert_eq!(
            levels(to_verdicts(&pkg(None), &version_info(true), None)),
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
                &version_info(false),
                Some(&scorecard_fixture(score, 10)),
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
        let flagged = to_verdicts(
            &pkg(None),
            &version_info(false),
            Some(&scorecard_fixture(8.0, 0)),
        );
        assert_eq!(
            levels(flagged),
            vec![(
                "source repo of @acme/server shows no recent activity".to_string(),
                RiskLevel::Low
            )]
        );
        assert!(to_verdicts(
            &pkg(None),
            &version_info(false),
            Some(&scorecard_fixture(8.0, -1))
        )
        .is_empty());
    }

    #[test]
    fn a_healthy_package_without_a_scorecard_yields_nothing() {
        assert!(to_verdicts(&pkg(None), &version_info(false), None).is_empty());
    }

    #[test]
    fn real_responses_parse_and_only_the_default_version_is_kept() {
        let info: PackageInfo = serde_json::from_str(
            r#"{"packageKey":{"system":"NPM","name":"x"},"versions":[
                {"versionKey":{"system":"NPM","name":"x","version":"0.1.0"},"isDefault":false,"isDeprecated":true},
                {"versionKey":{"system":"NPM","name":"x","version":"1.0.0"},"isDefault":true,"isDeprecated":false}
            ]}"#,
        )
        .unwrap();
        let default = info.versions.into_iter().find(|v| v.is_default).unwrap();
        assert_eq!(default.version_key.version, "1.0.0");
        let kept = serde_json::to_string(&DefaultVersion {
            version: Some(default.version_key.version),
        })
        .unwrap();
        assert_eq!(kept, r#"{"version":"1.0.0"}"#);
        let project: ProjectInfo = serde_json::from_str(
            r#"{"projectKey":{"id":"github.com/a/b"},"starsCount":3,"license":"MIT"}"#,
        )
        .unwrap();
        assert!(project.scorecard.is_none());
    }

    #[test]
    fn package_paths_encode_the_name_as_one_segment() {
        assert_eq!(
            package_path(&pkg(None)),
            "systems/npm/packages/%40acme%2Fserver"
        );
        let py = Package {
            ecosystem: Ecosystem::PyPI,
            name: "mcp-server".into(),
            version: None,
        };
        assert_eq!(package_path(&py), "systems/pypi/packages/mcp-server");
    }
}
