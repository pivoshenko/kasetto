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
//!   for risky launch shapes (see [`mcp`]), plus, for the npm / PyPI package a
//!   server runs, a [deps.dev](https://deps.dev) lookup that resolves the version
//!   a floating launch runs today and carries its deprecation status and its
//!   source repo's OpenSSF Scorecard, and an [OSV](https://osv.dev) lookup for
//!   the advisories affecting that version, which also carries the OpenSSF
//!   malicious-packages feed.
//!
//! An asset that cannot be judged (no audit yet, not indexed, lookup failed, a
//! partner answer nobody can classify) is reported as unverified and never
//! counts as a breach of the severity threshold, so a flaky network can't block
//! a sync by default; `fail-on-unverified` is the opt-in that makes it one.
//! Lookups run on their own I/O-sized pool, and each distinct package and
//! source repo is fetched once per run however many servers share it.

mod cache;
mod deps_dev;
mod mcp;
mod osv;
mod skills_sh;

pub(crate) use skills_sh::REPO_LEVEL_NOTE;

use std::collections::HashMap;
use std::path::Path;

use clap::ValueEnum;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::error::{err, Result};
use crate::fsops::{dirs_home, dirs_kasetto_cache, dirs_kasetto_config};
use crate::lock::LockFile;
use crate::model::{
    all_mcp_project_targets, all_mcp_settings_targets, McpSettingsTarget, Scope, INLINE_SOURCE,
};

use mcp::Package;

/// Every provider an MCP verdict can come from, in the order a report label
/// lists them.
pub(crate) const MCP_PROVIDERS: &[&str] = &[mcp::PROVIDER, osv::PROVIDER, deps_dev::PROVIDER];

/// Upper bound on concurrent lookups. Audit requests are many and small, so
/// the pool is sized for I/O rather than inheriting rayon's CPU-sized global one.
const MAX_LOOKUP_THREADS: usize = 16;

/// Normalized severity, ordered least to most severe. A partner answer that
/// cannot be classified is missing data, not a level: it is dropped with a
/// warning and leaves the asset unverified rather than flagged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RiskLevel {
    Safe,
    Low,
    Medium,
    High,
    Critical,
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
        }
    }
}

/// The severity a gate fails on: `audit --fail-on`, `sync --audit`, and
/// `audit.fail-on` in the config all take this one type. It is deliberately
/// narrower than [`RiskLevel`]: `safe` would fail every audited asset, so it is
/// not a threshold, and a config naming it is rejected at load time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[clap(rename_all = "lowercase")]
pub(crate) enum AuditThreshold {
    Low,
    Medium,
    High,
    Critical,
}

impl AuditThreshold {
    pub(crate) fn level(self) -> RiskLevel {
        match self {
            AuditThreshold::Low => RiskLevel::Low,
            AuditThreshold::Medium => RiskLevel::Medium,
            AuditThreshold::High => RiskLevel::High,
            AuditThreshold::Critical => RiskLevel::Critical,
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
#[derive(Debug, Clone, Default, Serialize)]
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
    /// At least one provider answered. The verdicts can still be empty when
    /// every answer was dropped as unclassifiable; the warnings then say so.
    Audited(AssetAudit),
    /// Nothing could judge the asset; the reason is user-facing
    /// (`no audit yet`, `not on skills.sh`, `not installed`).
    Unaudited(&'static str),
    /// The lookup failed (network / transport), so severity is unverifiable.
    Error(String),
}

/// An installed asset to audit, read from the lock (and, for MCP servers, the
/// agent config the server was merged into).
pub(crate) struct AuditTarget {
    pub kind: AssetKind,
    /// Skill directory name, or MCP server name.
    pub name: String,
    /// Configured source URL (or `<inline>` for config-declared MCP servers).
    pub source: String,
    /// The launch-relevant fields of the installed server, the only part of the
    /// definition the rules need; `None` for skills, and for a server the lock
    /// tracks but no agent config holds any more.
    spec: Option<mcp::ServerSpec>,
}

impl AuditTarget {
    /// A target with no installed definition: a skill, or an MCP server that is
    /// no longer in any agent config.
    pub(crate) fn new(kind: AssetKind, name: impl Into<String>, source: impl Into<String>) -> Self {
        AuditTarget {
            kind,
            name: name.into(),
            source: source.into(),
            spec: None,
        }
    }
}

/// One audited asset: the target plus its outcome.
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
    /// audited with a warning about a missing provider or with no usable
    /// verdict at all.
    pub(crate) fn is_unverified(&self) -> bool {
        match &self.outcome {
            AuditOutcome::Audited(a) => !a.warnings.is_empty() || a.verdicts.is_empty(),
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
        .map(|e| AuditTarget::new(AssetKind::Skill, &e.skill, &e.source))
        .collect();

    let settings = mcp_settings_for_scope(scope, scope_root)?;
    for asset in lock.assets.values().filter(|a| a.kind == "mcp") {
        let pack_wanted = wanted(&asset.name);
        for server in asset.destination.split(',').filter(|s| !s.is_empty()) {
            if !pack_wanted && !wanted(server) {
                continue;
            }
            // Only the launch fields leave this function: the raw definition
            // holds resolved secrets and must not travel with the target
            let spec = settings
                .iter()
                .find_map(|t| crate::mcps::read_server_definition(server, t))
                .map(|def| mcp::ServerSpec::from_definition(&def));
            out.push(AuditTarget {
                kind: AssetKind::Mcp,
                name: server.to_string(),
                source: asset.source.clone(),
                spec,
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

/// Remove every cached audit lookup, so the next run asks every provider again.
pub(crate) fn clear_cache() -> Result<()> {
    cache::clear_in(&dirs_kasetto_cache()?)
        .map_err(|e| err(format!("failed to clear the audit cache: {e}")))
}

/// Audit every target, preserving input order. `refresh` bypasses the 24h
/// lookup cache. Lookups run on a dedicated pool sized to the targets, capped
/// at [`MAX_LOOKUP_THREADS`].
pub(crate) fn audit_all(targets: Vec<AuditTarget>, refresh: bool) -> Vec<AuditResult> {
    let threads = targets.len().clamp(1, MAX_LOOKUP_THREADS);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|i| format!("kst-audit-{i}"))
        .build();
    match pool {
        Ok(pool) => pool.install(|| audit_in_pool(targets, refresh)),
        Err(_) => audit_in_pool(targets, refresh),
    }
}

/// Skills fan out one lookup each; MCP servers are grouped by the package they
/// launch so each package is looked up once, then every server reads its
/// package's report.
fn audit_in_pool(targets: Vec<AuditTarget>, refresh: bool) -> Vec<AuditResult> {
    let packages = distinct_packages(&targets);
    let (skills, reports) = rayon::join(
        || {
            targets
                .par_iter()
                .map(|t| match t.kind {
                    AssetKind::Skill => Some(skills_sh::audit_skill(&t.name, &t.source, refresh)),
                    AssetKind::Mcp => None,
                })
                .collect::<Vec<_>>()
        },
        || audit_packages(&packages, refresh),
    );
    targets
        .into_iter()
        .zip(skills)
        .map(|(target, skill)| {
            let outcome = skill.unwrap_or_else(|| match &target.spec {
                Some(spec) => audit_mcp_server(spec, &reports),
                None => AuditOutcome::Unaudited("not installed"),
            });
            AuditResult { target, outcome }
        })
        .collect()
}

/// Audit one installed MCP server: the offline rules always run, and the
/// package it launches contributes the verdicts and caveats of its shared
/// report.
fn audit_mcp_server(spec: &mcp::ServerSpec, reports: &HashMap<String, AssetAudit>) -> AuditOutcome {
    let mut audit = AssetAudit {
        verdicts: mcp::check(spec),
        warnings: Vec::new(),
    };
    if let Some(report) = mcp::launched_package(spec).and_then(|p| reports.get(&package_key(&p))) {
        audit.verdicts.extend(report.verdicts.iter().cloned());
        audit.warnings.extend(report.warnings.iter().cloned());
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

/// The packages the MCP targets launch, each once, in first-seen order.
fn distinct_packages(targets: &[AuditTarget]) -> Vec<Package> {
    let mut out: Vec<Package> = Vec::new();
    for pkg in targets
        .iter()
        .filter_map(|t| t.spec.as_ref())
        .filter_map(mcp::launched_package)
    {
        if !out.contains(&pkg) {
            out.push(pkg);
        }
    }
    out
}

/// Report key for a package. `Package` has no `Hash`, and the string reads the
/// same as the OSV cache key.
fn package_key(pkg: &Package) -> String {
    format!(
        "{}/{}@{}",
        pkg.ecosystem.osv_name(),
        pkg.name,
        pkg.version.as_deref().unwrap_or("*")
    )
}

/// One registry call's outcome. The error is a string because the crate error
/// is not `Clone` and one lookup feeds every server launching the package.
type Lookup<T> = std::result::Result<T, String>;

/// Everything the registries said about one package, every call kept so the
/// assembly can turn each failure into a warning.
struct PackageLookup {
    /// The version that runs: the pin, else deps.dev's default. `Ok(None)` when
    /// deps.dev does not know the package.
    version: Lookup<Option<String>>,
    osv: Lookup<Vec<Verdict>>,
    info: Lookup<Option<deps_dev::VersionInfo>>,
}

/// Resolve the version that runs, then query OSV and deps.dev for it
/// concurrently. Without a version deps.dev has nothing to judge and OSV falls
/// back to its unversioned, malicious-only query.
fn lookup_package(pkg: &Package, refresh: bool) -> PackageLookup {
    let version = match &pkg.version {
        Some(v) => Ok(Some(v.clone())),
        None => deps_dev::default_version(pkg, refresh).map_err(|e| e.to_string()),
    };
    let runs = version.as_ref().ok().and_then(|v| v.as_deref());
    let (osv, info) = rayon::join(
        || osv::vulnerabilities(pkg, runs, refresh).map_err(|e| e.to_string()),
        || match runs {
            Some(v) => deps_dev::version(pkg, v, refresh).map_err(|e| e.to_string()),
            None => Ok(None),
        },
    );
    PackageLookup { version, osv, info }
}

/// Audit every distinct package in parallel, then every distinct source repo,
/// and assemble one shared report per package.
fn audit_packages(packages: &[Package], refresh: bool) -> HashMap<String, AssetAudit> {
    let lookups: Vec<PackageLookup> = packages
        .par_iter()
        .map(|p| lookup_package(p, refresh))
        .collect();
    let mut repos: Vec<String> = lookups
        .iter()
        .filter_map(|l| l.info.as_ref().ok()?.as_ref())
        .filter_map(deps_dev::source_repo)
        .map(str::to_string)
        .collect();
    repos.sort_unstable();
    repos.dedup();
    let scorecards: HashMap<String, Lookup<Option<deps_dev::Scorecard>>> = repos
        .into_par_iter()
        .map(|repo| {
            let found = deps_dev::scorecard(&repo, refresh).map_err(|e| e.to_string());
            (repo, found)
        })
        .collect();
    packages
        .iter()
        .zip(lookups)
        .map(|(pkg, lookup)| {
            let scorecard = lookup
                .info
                .as_ref()
                .ok()
                .and_then(|i| i.as_ref())
                .and_then(deps_dev::source_repo)
                .and_then(|repo| scorecards.get(repo));
            (package_key(pkg), assemble(pkg, &lookup, scorecard))
        })
        .collect()
}

/// Turn the raw lookups into verdicts, with one warning per provider that
/// failed anywhere in its chain.
fn assemble(
    pkg: &Package,
    lookup: &PackageLookup,
    scorecard: Option<&Lookup<Option<deps_dev::Scorecard>>>,
) -> AssetAudit {
    let mut audit = AssetAudit::default();
    match &lookup.osv {
        Ok(found) => audit.verdicts.extend(found.iter().cloned()),
        Err(e) => audit.warnings.push(lookup_warning(osv::PROVIDER, e)),
    }
    let mut deps_dev_failure = lookup.version.as_ref().err();
    match &lookup.info {
        Ok(Some(info)) => {
            let scorecard = match scorecard {
                Some(Ok(found)) => found.as_ref(),
                Some(Err(e)) => {
                    deps_dev_failure = Some(e);
                    None
                }
                None => None,
            };
            audit
                .verdicts
                .extend(deps_dev::to_verdicts(pkg, info, scorecard));
        }
        Ok(None) => {}
        Err(e) => deps_dev_failure = Some(e),
    }
    if let Some(e) = deps_dev_failure {
        audit.warnings.push(lookup_warning(deps_dev::PROVIDER, e));
    }
    audit
}

fn lookup_warning(provider: &str, error: &str) -> String {
    format!("{provider} lookup failed: {error}")
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

    fn spec(def: serde_json::Value) -> mcp::ServerSpec {
        mcp::ServerSpec::from_definition(&def)
    }

    fn npx(package: &str) -> mcp::ServerSpec {
        spec(serde_json::json!({"command": "npx", "args": ["-y", package]}))
    }

    fn mcp_target(name: &str, spec: Option<mcp::ServerSpec>) -> AuditTarget {
        AuditTarget {
            kind: AssetKind::Mcp,
            name: name.into(),
            source: INLINE_SOURCE.into(),
            spec,
        }
    }

    fn lookup(
        version: Lookup<Option<&str>>,
        osv: Lookup<Vec<Verdict>>,
        info: Lookup<Option<&str>>,
    ) -> PackageLookup {
        PackageLookup {
            version: version.map(|v| v.map(str::to_string)),
            osv,
            info: info.map(|i| i.map(|json| serde_json::from_str(json).unwrap())),
        }
    }

    const DEPRECATED: &str = r#"{"isDeprecated":true,"relatedProjects":[
        {"projectKey":{"id":"github.com/acme/server"},"relationType":"SOURCE_REPO"}]}"#;

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
    fn threshold_values_map_to_their_level_and_nothing_else_parses() {
        let parsed: Vec<AuditThreshold> =
            serde_yaml::from_str("[low, medium, high, critical]").expect("four values");
        let levels: Vec<RiskLevel> = parsed.iter().map(|t| t.level()).collect();
        assert_eq!(
            levels,
            [
                RiskLevel::Low,
                RiskLevel::Medium,
                RiskLevel::High,
                RiskLevel::Critical
            ]
        );
        for bad in ["safe", "unknown", "HIGH"] {
            assert!(
                serde_yaml::from_str::<AuditThreshold>(bad).is_err(),
                "{bad}"
            );
        }
        let cli: Vec<String> = AuditThreshold::value_variants()
            .iter()
            .filter_map(|t| t.to_possible_value())
            .map(|p| p.get_name().to_string())
            .collect();
        assert_eq!(cli, ["low", "medium", "high", "critical"]);
    }

    #[test]
    fn ordering_runs_from_safe_to_critical() {
        assert!(RiskLevel::Safe < RiskLevel::Low);
        assert!(RiskLevel::Low < RiskLevel::Medium);
        assert!(RiskLevel::Medium < RiskLevel::High);
        assert!(RiskLevel::High < RiskLevel::Critical);
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
        let empty = AssetAudit::default();
        assert_eq!(empty.worst(), None);
    }

    #[test]
    fn mcp_server_without_findings_is_audited_safe() {
        let spec = spec(serde_json::json!({"type": "http", "url": "https://example.com/mcp"}));
        let AuditOutcome::Audited(audit) = audit_mcp_server(&spec, &HashMap::new()) else {
            panic!("expected audited")
        };
        assert_eq!(audit.worst(), Some(RiskLevel::Safe));
        assert!(audit.warnings.is_empty());
    }

    #[test]
    fn mcp_server_takes_its_package_report_verdicts_and_warnings() {
        let spec = npx("pkg@1.0.0");
        let key = package_key(&mcp::launched_package(&spec).unwrap());
        assert_eq!(key, "npm/pkg@1.0.0");
        let reports = HashMap::from([(
            key,
            AssetAudit {
                verdicts: vec![verdict(osv::PROVIDER, RiskLevel::Critical)],
                warnings: vec!["deps.dev lookup failed: timeout".into()],
            },
        )]);
        let AuditOutcome::Audited(audit) = audit_mcp_server(&spec, &reports) else {
            panic!("expected audited")
        };
        assert_eq!(audit.worst(), Some(RiskLevel::Critical));
        assert_eq!(audit.worst_provider(), Some(osv::PROVIDER));
        assert_eq!(audit.warnings.len(), 1);
    }

    #[test]
    fn distinct_packages_dedupes_servers_launching_the_same_package() {
        let targets = vec![
            mcp_target("a", Some(npx("pkg"))),
            mcp_target("b", Some(npx("pkg"))),
            mcp_target("c", Some(npx("pkg@1.0.0"))),
            mcp_target("d", Some(npx("other"))),
            mcp_target("e", None),
            AuditTarget::new(AssetKind::Skill, "pdf", "https://github.com/o/r"),
        ];
        let keys: Vec<String> = distinct_packages(&targets)
            .iter()
            .map(package_key)
            .collect();
        assert_eq!(keys, vec!["npm/pkg@*", "npm/pkg@1.0.0", "npm/other@*"]);
    }

    #[test]
    fn assemble_turns_each_failed_chain_into_one_warning() {
        let pkg = mcp::launched_package(&npx("pkg")).unwrap();

        let clean = assemble(
            &pkg,
            &lookup(
                Ok(Some("1.0.0")),
                Ok(vec![verdict("OSV", RiskLevel::High)]),
                Ok(Some(DEPRECATED)),
            ),
            Some(&Ok(None)),
        );
        assert_eq!(clean.verdicts.len(), 2);
        assert!(clean.warnings.is_empty());

        let osv_down = assemble(
            &pkg,
            &lookup(Ok(Some("1.0.0")), Err("timeout".into()), Ok(None)),
            None,
        );
        assert!(osv_down.verdicts.is_empty());
        assert_eq!(
            osv_down.warnings,
            vec!["OSV lookup failed: timeout".to_string()]
        );

        let unresolved = assemble(
            &pkg,
            &lookup(Err("HTTP 500".into()), Ok(Vec::new()), Ok(None)),
            None,
        );
        assert_eq!(
            unresolved.warnings,
            vec!["deps.dev lookup failed: HTTP 500".to_string()]
        );

        let scorecard_down = assemble(
            &pkg,
            &lookup(Ok(Some("1.0.0")), Ok(Vec::new()), Ok(Some(DEPRECATED))),
            Some(&Err("HTTP 503".into())),
        );
        assert_eq!(scorecard_down.verdicts.len(), 1);
        assert_eq!(
            scorecard_down.warnings,
            vec!["deps.dev lookup failed: HTTP 503".to_string()]
        );

        let unknown = assemble(&pkg, &lookup(Ok(None), Ok(Vec::new()), Ok(None)), None);
        assert!(unknown.verdicts.is_empty());
        assert!(unknown.warnings.is_empty());
    }

    #[test]
    fn mcp_providers_lists_every_provider_once() {
        assert_eq!(MCP_PROVIDERS, &["kasetto", "OSV", "deps.dev"]);
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
        assert_eq!(
            all[1].spec.as_ref().and_then(|s| s.url.as_deref()),
            Some("https://api.example.com/mcp")
        );
        assert!(all[2].spec.is_none());

        let only_pack =
            collect_targets(&lock, Scope::Project, &root, &["pack.json".into()]).unwrap();
        assert_eq!(only_pack.len(), 2);
        let only_gh = collect_targets(&lock, Scope::Project, &root, &["gh".into()]).unwrap();
        assert_eq!(only_gh.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unverified_covers_every_outcome_that_could_not_judge() {
        let with = |outcome| AuditResult {
            target: AuditTarget::new(AssetKind::Mcp, "x", INLINE_SOURCE),
            outcome,
        };
        assert!(with(AuditOutcome::Unaudited("not installed")).is_unverified());
        assert!(with(AuditOutcome::Error("boom".into())).is_unverified());
        assert!(with(AuditOutcome::Audited(AssetAudit {
            verdicts: vec![verdict("kasetto", RiskLevel::Safe)],
            warnings: vec!["OSV lookup failed".into()],
        }))
        .is_unverified());
        assert!(with(AuditOutcome::Audited(AssetAudit::default())).is_unverified());
        assert!(!with(AuditOutcome::Audited(AssetAudit {
            verdicts: vec![verdict("kasetto", RiskLevel::Safe)],
            warnings: Vec::new(),
        }))
        .is_unverified());
    }
}
