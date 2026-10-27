//! Module that contains the [OSV](https://osv.dev) lookup for the package an
//! MCP server launches.
//!
//! OSV merges the GitHub, PyPA, and npm advisory databases with the OpenSSF
//! malicious-packages feed (`MAL-` ids). The query names the version that
//! actually runs (pinned, or resolved by deps.dev for a floating launch) so it
//! returns the advisories affecting it. Only when no version could be resolved
//! does the query go unversioned; that returns every advisory ever filed, so
//! just the malicious-package entries, which condemn the whole package, are
//! kept from it.

use serde::{Deserialize, Serialize};

use crate::error::{err, Result};
use crate::fsops::http_client;

use super::mcp::Package;
use super::{cache, RiskLevel, Verdict};

const QUERY_URL: &str = "https://api.osv.dev/v1/query";
const CACHE_NAMESPACE: &str = "osv";
pub(super) const PROVIDER: &str = "OSV";

/// Upper bound on `next_page_token` follow-ups for one query. OSV pages hold
/// about a thousand advisories, so this is far beyond any real package and
/// only guards against a server that keeps handing back a token.
const MAX_PAGES: usize = 10;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct QueryResponse {
    #[serde(default)]
    vulns: Vec<Vuln>,
    #[serde(default, skip_serializing)]
    next_page_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Vuln {
    id: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    modified: Option<String>,
    #[serde(default)]
    database_specific: Option<DatabaseSpecific>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DatabaseSpecific {
    #[serde(default)]
    severity: Option<String>,
}

/// Every advisory that applies to `pkg` at `version`, one verdict each.
/// `version` is the one that runs; `None` means it could not be resolved.
pub(super) fn vulnerabilities(
    pkg: &Package,
    version: Option<&str>,
    refresh: bool,
) -> Result<Vec<Verdict>> {
    let resolved = Package {
        ecosystem: pkg.ecosystem,
        name: pkg.name.clone(),
        version: version.map(str::to_string),
    };
    let response = query(&resolved, refresh)?;
    Ok(to_verdicts(&resolved, response))
}

fn query(pkg: &Package, refresh: bool) -> Result<QueryResponse> {
    let ecosystem = pkg.ecosystem.osv_name();
    let key = format!(
        "{ecosystem}/{}@{}",
        pkg.name,
        pkg.version.as_deref().unwrap_or("*")
    );
    if !refresh {
        if let Some(hit) = cache::lookup(CACHE_NAMESPACE, &key) {
            return Ok(hit);
        }
    }
    let mut body = serde_json::json!({
        "package": {"name": pkg.name, "ecosystem": ecosystem},
    });
    if let Some(v) = &pkg.version {
        body["version"] = serde_json::json!(v);
    }
    let mut merged = QueryResponse::default();
    for _ in 0..MAX_PAGES {
        let mut page = post(&body)?;
        merged.vulns.append(&mut page.vulns);
        match page.next_page_token {
            Some(token) => body["page_token"] = serde_json::json!(token),
            None => break,
        }
    }
    cache::store(CACHE_NAMESPACE, &key, &merged);
    Ok(merged)
}

fn post(body: &serde_json::Value) -> Result<QueryResponse> {
    let resp = http_client()?
        .post(QUERY_URL)
        .timeout(cache::LOOKUP_TIMEOUT)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .map_err(|e| err(format!("OSV request failed: {e}")))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(err(format!("OSV returned HTTP {status}")));
    }
    let text = resp
        .text()
        .map_err(|e| err(format!("failed to read OSV response: {e}")))?;
    serde_json::from_str(&text).map_err(|e| err(format!("failed to parse OSV response: {e}")))
}

fn to_verdicts(pkg: &Package, response: QueryResponse) -> Vec<Verdict> {
    response
        .vulns
        .into_iter()
        .filter(|v| pkg.version.is_some() || is_malicious(v))
        .map(|v| {
            let malicious = is_malicious(&v);
            let level = if malicious {
                RiskLevel::Critical
            } else {
                v.database_specific
                    .as_ref()
                    .and_then(|d| d.severity.as_deref())
                    .and_then(RiskLevel::from_risk_str)
                    .unwrap_or(RiskLevel::Medium)
            };
            let detail = if malicious {
                format!("{} is a known malicious package", pkg.name)
            } else if v.summary.is_empty() {
                format!("advisory affects {}", pkg.name)
            } else {
                v.summary
            };
            Verdict {
                provider: PROVIDER.to_string(),
                level,
                summary: format!("{}: {detail}", v.id),
                audited_at: v.modified,
                categories: vec![if malicious {
                    "malicious-package"
                } else {
                    "vulnerability"
                }
                .to_string()],
            }
        })
        .collect()
}

fn is_malicious(v: &Vuln) -> bool {
    v.id.starts_with("MAL-")
}

#[cfg(test)]
mod tests {
    use super::super::mcp::Ecosystem;
    use super::*;

    const BODY: &str = r#"{"vulns":[
        {"id":"GHSA-aaaa-bbbb-cccc","summary":"Path traversal in read_file","modified":"2026-05-01T00:00:00Z",
         "database_specific":{"severity":"HIGH"}},
        {"id":"PYSEC-2026-1","summary":"","database_specific":{}},
        {"id":"MAL-2026-42","summary":"Malicious code in pkg","database_specific":{"severity":"CRITICAL"}}
    ],"next_page_token":"abc"}"#;

    fn pkg(version: Option<&str>) -> Package {
        Package {
            ecosystem: Ecosystem::Npm,
            name: "pkg".into(),
            version: version.map(Into::into),
        }
    }

    fn levels(version: Option<&str>) -> Vec<(String, RiskLevel)> {
        let parsed: QueryResponse = serde_json::from_str(BODY).unwrap();
        to_verdicts(&pkg(version), parsed)
            .into_iter()
            .map(|v| (v.summary, v.level))
            .collect()
    }

    #[test]
    fn resolved_versions_keep_every_advisory_with_its_severity() {
        assert_eq!(
            levels(Some("1.0.0")),
            vec![
                (
                    "GHSA-aaaa-bbbb-cccc: Path traversal in read_file".to_string(),
                    RiskLevel::High
                ),
                (
                    "PYSEC-2026-1: advisory affects pkg".to_string(),
                    RiskLevel::Medium
                ),
                (
                    "MAL-2026-42: pkg is a known malicious package".to_string(),
                    RiskLevel::Critical
                ),
            ]
        );
    }

    #[test]
    fn unresolved_versions_keep_only_malicious_package_entries() {
        assert_eq!(
            levels(None),
            vec![(
                "MAL-2026-42: pkg is a known malicious package".to_string(),
                RiskLevel::Critical
            )]
        );
    }

    #[test]
    fn an_empty_answer_parses_to_no_verdicts() {
        let parsed: QueryResponse = serde_json::from_str("{}").unwrap();
        assert!(to_verdicts(&pkg(Some("1.0.0")), parsed).is_empty());
    }

    #[test]
    fn page_tokens_are_read_but_never_cached() {
        let parsed: QueryResponse = serde_json::from_str(BODY).unwrap();
        assert_eq!(parsed.next_page_token.as_deref(), Some("abc"));
        let stored = serde_json::to_value(&parsed).unwrap();
        assert!(stored.get("next_page_token").is_none());
        assert_eq!(stored["vulns"].as_array().unwrap().len(), 3);
    }
}
