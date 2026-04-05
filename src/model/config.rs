use serde::{Deserialize, Serialize};

use super::{Agent, AgentField};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Scope {
    #[default]
    #[serde(rename = "global")]
    Global,
    #[serde(rename = "project")]
    Project,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub destination: Option<String>,
    #[serde(default)]
    pub scope: Option<Scope>,
    #[serde(default)]
    pub agent: Option<AgentField>,
    #[serde(default)]
    pub skills: Vec<SourceSpec>,
    #[serde(default)]
    pub mcps: Vec<McpSourceSpec>,
}

impl Config {
    pub(crate) fn agents(&self) -> Vec<Agent> {
        match &self.agent {
            Some(AgentField::One(a)) => vec![*a],
            Some(AgentField::Many(v)) => v.clone(),
            None => vec![],
        }
    }

    pub(crate) fn resolved_scope(&self) -> Scope {
        self.scope.unwrap_or_default()
    }
}

/// Resolve the effective scope: CLI override > config YAML `scope:` field > Global default.
///
/// When a `Config` is already loaded, pass it directly. Otherwise the function
/// reads `kasetto.yaml` from the current directory as a fallback.
pub(crate) fn resolve_scope(cli_override: Option<Scope>, cfg: Option<&Config>) -> Scope {
    if let Some(s) = cli_override {
        return s;
    }
    if let Some(cfg) = cfg {
        return cfg.resolved_scope();
    }
    if let Ok(text) = std::fs::read_to_string(crate::DEFAULT_CONFIG_FILENAME) {
        if let Ok(cfg) = serde_yaml::from_str::<Config>(&text) {
            return cfg.resolved_scope();
        }
    }
    Scope::Global
}

#[derive(Debug, Deserialize)]
pub(crate) struct SourceSpec {
    pub source: String,
    pub branch: Option<String>,
    /// Pin to a git tag, commit SHA, or any ref. Takes priority over `branch`.
    /// When set, no main/master fallback is attempted.
    #[serde(rename = "ref")]
    pub git_ref: Option<String>,
    pub skills: SkillsField,
}

/// What the user specified to identify a version of the source.
pub(crate) enum GitPin {
    /// Explicit ref (tag, SHA, etc.) -- no fallback.
    Ref(String),
    /// Explicit branch name -- no fallback.
    Branch(String),
    /// Nothing specified -- try "main", fall back to "master".
    Default,
}

impl SourceSpec {
    /// Resolve the effective git pin: `ref` > `branch` > default.
    pub(crate) fn git_pin(&self) -> GitPin {
        if let Some(r) = &self.git_ref {
            GitPin::Ref(r.clone())
        } else if let Some(b) = &self.branch {
            GitPin::Branch(b.clone())
        } else {
            GitPin::Default
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct McpSourceSpec {
    pub source: String,
    pub branch: Option<String>,
    #[serde(rename = "ref")]
    pub git_ref: Option<String>,
    /// Explicit path to an MCP JSON file within the repo (e.g. `.mcp.json`).
    /// When set, skips auto-discovery and uses this file directly.
    pub path: Option<String>,
}

impl McpSourceSpec {
    pub(crate) fn as_source_spec(&self) -> SourceSpec {
        SourceSpec {
            source: self.source.clone(),
            branch: self.branch.clone(),
            git_ref: self.git_ref.clone(),
            skills: SkillsField::Wildcard("*".to_string()),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum SkillsField {
    Wildcard(String),
    List(Vec<SkillTarget>),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum SkillTarget {
    Name(String),
    Obj { name: String, path: Option<String> },
}
