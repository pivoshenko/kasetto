//! Package that contains the security audit shared by `kasetto audit` and the
//! `sync` gate.
//!
//! Two asset kinds are covered, each from the source that can actually judge it:
//!
//! - **skills** take the verdicts [skills.sh](https://skills.sh) publishes from
//!   several independent partners (Snyk, Socket, Gen Agent Trust Hub, ...). They
//!   routinely disagree, so a skill's headline is its *worst* partner verdict and
//!   the full breakdown stays available. Verdicts are repo-level, not pinned to
//!   the locked commit, and only `github.com` sources are indexed.
//! - **MCP servers** are judged from their installed definition: offline rules
//!   for risky launch shapes (see [`mcp`]), plus an [OSV](https://osv.dev) lookup
//!   for the npm / PyPI package a server runs, which also carries the OpenSSF
//!   malicious-packages feed, and a [deps.dev](https://deps.dev) lookup for its
//!   deprecation status and its source repo's OpenSSF Scorecard.
//!
//! An asset that cannot be judged (no audit yet, not indexed, lookup failed) is
//! reported as such and never counts as a breach, so a flaky network can't
//! block a sync.

mod cache;
mod deps_dev;
mod mcp;
mod osv;
mod skills_sh;

use std::path::Path;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::fsops::{dirs_home, dirs_kasetto_config};
use crate::lock::LockFile;
use crate::model::{
    all_mcp_project_targets, all_mcp_settings_targets, McpSettingsTarget, Scope, INLINE_SOURCE,
};

/// Normalized severity, ordered least to most severe. `Unknown` sorts *above*
/// `Critical` so a verdict that cannot be classified is treated as the worst
/// case by [`AssetAudit::worst`] and by the gate: fail safe, never silent.
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
    /// Map a published severity string (case-insensitive). `NONE` / `SAFE`
    /// collapse to [`RiskLevel::Safe`]; unrecognized values return `None` so the
    /// caller can pick its own fallback.
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

/// Which kind of installed asset an audit row describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AssetKind {
    Skill,
    Mcp,
}

/// One provider's judgement of an asset, severity already normalized.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Verdict {
    pub provider: String,
    pub level: RiskLevel,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audited_at: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
}

/// Every verdict gathered for one asset, plus the caveats of how they were
/// gathered (an MCP server whose OSV lookup failed is still audited by the
/// offline rules, but says so).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct AssetAudit {
    pub verdicts: Vec<Verdict>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl AssetAudit {
    /// The most severe verdict: the asset's headline severity. `None` only when
    /// no provider reported.
    pub(crate) fn worst(&self) -> Option<RiskLevel> {
        self.verdicts.iter().map(|v| v.level).max()
    }

    /// The provider behind the headline severity, for one-line reports.
    pub(crate) fn worst_provider(&self) -> Option<&str> {
        let worst = self.worst()?;
        self.verdicts
            .iter()
            .find(|v| v.level == worst)
            .map(|v| v.provider.as_str())
    }
}

/// The audit result for one installed asset.
#[derive(Debug, Clone)]
pub(crate) enum AuditOutcome {
    /// At least one provider returned a verdict.
    Audited(AssetAudit),
    /// Nothing could judge the asset; the reason is user-facing
    /// (`no audit yet`, `not on skills.sh`, `not installed`).
    Unaudited(&'static str),
    /// The lookup failed (network / transport), so severity is unverifiable.
    Error(String),
}

/// An installed asset to audit, read from the lock (and, for MCP servers, the
/// agent config the server was merged into).
#[derive(Debug, Clone)]
pub(crate) struct AuditTarget {
    pub kind: AssetKind,
    /// Skill directory name, or MCP server name.
    pub name: String,
    /// Configured source URL (or `<inline>` for config-declared MCP servers).
    pub source: String,
    /// The installed server definition; `None` for skills, and for a server the
    /// lock tracks but no agent config holds any more.
    pub definition: Option<serde_json::Value>,
}

/// One audited asset: the target plus its outcome.
#[derive(Debug, Clone)]
pub(crate) struct AuditResult {
    pub target: AuditTarget,
    pub outcome: AuditOutcome,
}

impl AuditResult {
    /// Severity when audited, else `None`.
    pub(crate) fn worst(&self) -> Option<RiskLevel> {
        match &self.outcome {
            AuditOutcome::Audited(a) => a.worst(),
            _ => None,
        }
    }

    /// `true` when the asset could not be fully judged: unaudited, failed, or
    /// audited with a warning about a missing provider.
    pub(crate) fn is_unverified(&self) -> bool {
        match &self.outcome {
            AuditOutcome::Audited(a) => !a.warnings.is_empty(),
            AuditOutcome::Unaudited(_) | AuditOutcome::Error(_) => true,
        }
    }
}

/// Collect every installed skill and MCP server recorded in `lock`, reading each
/// server's definition from the scope's agent configs. `names` narrows the set
/// to assets whose name (or MCP pack name) matches; empty keeps everything.
pub(crate) fn collect_targets(
    lock: &LockFile,
    scope: Scope,
    scope_root: &Path,
    names: &[String],
) -> Result<Vec<AuditTarget>> {
    let wanted = |n: &str| names.is_empty() || names.iter().any(|w| w == n);
    let mut out: Vec<AuditTarget> = lock
        .skills
        .values()
        .filter(|e| wanted(&e.skill))
        .map(|e| AuditTarget {
            kind: AssetKind::Skill,
            name: e.skill.clone(),
            source: e.source.clone(),
            definition: None,
        })
        .collect();

    let settings = mcp_settings_for_scope(scope, scope_root)?;
    for asset in lock.assets.values().filter(|a| a.kind == "mcp") {
        let pack_wanted = wanted(&asset.name);
        for server in asset.destination.split(',').filter(|s| !s.is_empty()) {
            if !pack_wanted && !wanted(server) {
                continue;
            }
            out.push(AuditTarget {
                kind: AssetKind::Mcp,
                name: server.to_string(),
                source: asset.source.clone(),
                definition: settings
                    .iter()
                    .find_map(|t| crate::mcps::read_server_definition(server, t)),
            });
        }
    }

    out.sort_by(|a, b| (a.kind as u8, &a.source, &a.name).cmp(&(b.kind as u8, &b.source, &b.name)));
    out.dedup_by(|a, b| a.kind == b.kind && a.name == b.name && a.source == b.source);
    Ok(out)
}

/// Every MCP settings file kasetto may have merged servers into for `scope`,
/// independent of which agents the current config names (the lock may predate
/// an agent change).
fn mcp_settings_for_scope(scope: Scope, scope_root: &Path) -> Result<Vec<McpSettingsTarget>> {
    Ok(match scope {
        Scope::Project => all_mcp_project_targets(scope_root),
        Scope::Global => all_mcp_settings_targets(&dirs_home()?, &dirs_kasetto_config()?),
    })
}

/// Audit every target in parallel, preserving input order. `refresh` bypasses
/// the 24h lookup cache.
pub(crate) fn audit_all(targets: Vec<AuditTarget>, refresh: bool) -> Vec<AuditResult> {
    targets
        .into_par_iter()
        .map(|target| {
            let outcome = audit_target(&target, refresh);
            AuditResult { target, outcome }
        })
        .collect()
}

fn audit_target(target: &AuditTarget, refresh: bool) -> AuditOutcome {
    match target.kind {
        AssetKind::Skill => skills_sh::audit_skill(&target.name, &target.source, refresh),
        AssetKind::Mcp => match &target.definition {
            Some(def) => audit_mcp_server(def, refresh),
            None => AuditOutcome::Unaudited("not installed"),
        },
    }
}

/// Audit one installed MCP server definition: the offline rules always run;
/// the package it launches is then checked against OSV and deps.dev. A failed
/// lookup leaves the other verdicts standing and adds a warning instead.
fn audit_mcp_server(def: &serde_json::Value, refresh: bool) -> AuditOutcome {
    let spec = mcp::ServerSpec::from_definition(def);
    let mut audit = AssetAudit {
        verdicts: mcp::check(&spec),
        warnings: Vec::new(),
    };
    if let Some(pkg) = mcp::launched_package(&spec) {
        match osv::vulnerabilities(&pkg, refresh) {
            Ok(found) => audit.verdicts.extend(found),
            Err(e) => audit.warnings.push(format!("OSV lookup failed: {e}")),
        }
        match deps_dev::signals(&pkg, refresh) {
            Ok(found) => audit.verdicts.extend(found),
            Err(e) => audit.warnings.push(format!("deps.dev lookup failed: {e}")),
        }
    }
    if audit.verdicts.is_empty() {
        audit.verdicts.push(Verdict {
            provider: mcp::PROVIDER.to_string(),
            level: RiskLevel::Safe,
            summary: "no risky launch options found".to_string(),
            audited_at: None,
            categories: Vec::new(),
        });
    }
    AuditOutcome::Audited(audit)
}

/// User-facing label for a source: `inline` for config-declared MCP servers.
pub(crate) fn source_label(source: &str) -> String {
    if source == INLINE_SOURCE {
        "inline (kasetto.yaml)".to_string()
    } else {
        crate::ui::short_source(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lock::AssetEntry;
    use crate::model::SkillEntry;

    fn verdict(provider: &str, level: RiskLevel) -> Verdict {
        Verdict {
            provider: provider.into(),
            level,
            summary: String::new(),
            audited_at: None,
            categories: Vec::new(),
        }
    }

    #[test]
    fn risk_str_normalizes_aliases_and_case() {
        assert_eq!(RiskLevel::from_risk_str("none"), Some(RiskLevel::Safe));
        assert_eq!(RiskLevel::from_risk_str(" med "), Some(RiskLevel::Medium));
        assert_eq!(
            RiskLevel::from_risk_str("MODERATE"),
            Some(RiskLevel::Medium)
        );
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
    fn worst_is_the_max_verdict_and_names_its_provider() {
        let audit = AssetAudit {
            verdicts: vec![
                verdict("Socket", RiskLevel::Safe),
                verdict("Snyk", RiskLevel::High),
                verdict("ZeroLeaks", RiskLevel::Low),
            ],
            warnings: Vec::new(),
        };
        assert_eq!(audit.worst(), Some(RiskLevel::High));
        assert_eq!(audit.worst_provider(), Some("Snyk"));
        let empty = AssetAudit {
            verdicts: Vec::new(),
            warnings: Vec::new(),
        };
        assert_eq!(empty.worst(), None);
    }

    #[test]
    fn mcp_server_without_findings_is_audited_safe() {
        let def = serde_json::json!({"type": "http", "url": "https://example.com/mcp"});
        let AuditOutcome::Audited(audit) = audit_mcp_server(&def, false) else {
            panic!("expected audited")
        };
        assert_eq!(audit.worst(), Some(RiskLevel::Safe));
        assert!(audit.warnings.is_empty());
    }

    #[test]
    fn collect_targets_reads_skills_and_mcp_servers_from_the_lock() {
        let root = crate::fsops::temp_dir("audit-collect");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join(".mcp.json"),
            r#"{"mcpServers":{"gh":{"type":"http","url":"https://api.example.com/mcp"}}}"#,
        )
        .unwrap();

        let mut lock = LockFile::default();
        lock.skills.insert(
            "k".into(),
            SkillEntry {
                skill: "pdf".into(),
                source: "https://github.com/anthropics/skills".into(),
                ..Default::default()
            },
        );
        lock.assets.insert(
            "mcp::pack".into(),
            AssetEntry {
                kind: "mcp".into(),
                name: "pack.json".into(),
                source: "https://github.com/o/r".into(),
                destination: "gh,gone".into(),
                ..Default::default()
            },
        );

        let all = collect_targets(&lock, Scope::Project, &root, &[]).unwrap();
        let names: Vec<_> = all.iter().map(|t| (t.kind, t.name.as_str())).collect();
        assert_eq!(
            names,
            vec![
                (AssetKind::Skill, "pdf"),
                (AssetKind::Mcp, "gh"),
                (AssetKind::Mcp, "gone"),
            ]
        );
        assert!(all[1].definition.is_some());
        assert!(all[2].definition.is_none());

        let only_pack =
            collect_targets(&lock, Scope::Project, &root, &["pack.json".into()]).unwrap();
        assert_eq!(only_pack.len(), 2);
        let only_gh = collect_targets(&lock, Scope::Project, &root, &["gh".into()]).unwrap();
        assert_eq!(only_gh.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unverified_covers_every_outcome_that_could_not_judge() {
        let target = AuditTarget {
            kind: AssetKind::Mcp,
            name: "x".into(),
            source: INLINE_SOURCE.into(),
            definition: None,
        };
        let with = |outcome| AuditResult {
            target: target.clone(),
            outcome,
        };
        assert!(with(AuditOutcome::Unaudited("not installed")).is_unverified());
        assert!(with(AuditOutcome::Error("boom".into())).is_unverified());
        assert!(with(AuditOutcome::Audited(AssetAudit {
            verdicts: vec![verdict("kasetto", RiskLevel::Safe)],
            warnings: vec!["OSV lookup failed".into()],
        }))
        .is_unverified());
        assert!(!with(AuditOutcome::Audited(AssetAudit {
            verdicts: vec![verdict("kasetto", RiskLevel::Safe)],
            warnings: Vec::new(),
        }))
        .is_unverified());
    }
}
