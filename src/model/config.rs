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
    #[serde(default)]
    pub commands: Vec<CommandSourceSpec>,
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
/// reads the default config path (local `kasetto.yaml`, then global XDG config)
/// as a fallback.
pub(crate) fn resolve_scope(cli_override: Option<Scope>, cfg: Option<&Config>) -> Scope {
    if let Some(s) = cli_override {
        return s;
    }
    if let Some(cfg) = cfg {
        return cfg.resolved_scope();
    }
    if let Ok((cfg, _, _)) = crate::fsops::load_config_any(&crate::default_config_path()) {
        return cfg.resolved_scope();
    }
    Scope::Global
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_scope_prefers_cli_override() {
        assert_eq!(resolve_scope(Some(Scope::Project), None), Scope::Project);
        assert_eq!(resolve_scope(Some(Scope::Global), None), Scope::Global);
    }

    #[test]
    fn config_commands_parses_wildcard() {
        let yaml = r#"
skills: []
commands:
  - source: https://github.com/me/cmds
    commands: "*"
"#;
        let cfg: Config = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(cfg.commands.len(), 1);
        assert!(matches!(
            cfg.commands[0].commands,
            CommandsField::Wildcard(_)
        ));
    }

    #[test]
    fn config_commands_parses_plain_strings_and_objects() {
        let yaml = r#"
skills: []
commands:
  - source: https://github.com/me/cmds
    ref: v1.0
    sub-dir: commands
    commands:
      - review-pr
      - name: deploy
        path: ops
"#;
        let cfg: Config = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(cfg.commands[0].git_ref.as_deref(), Some("v1.0"));
        assert_eq!(cfg.commands[0].sub_dir.as_deref(), Some("commands"));
        let CommandsField::List(ref entries) = cfg.commands[0].commands else {
            panic!("expected list");
        };
        assert_eq!(entries.len(), 2);
        assert!(matches!(&entries[0], CommandEntry::Name(n) if n == "review-pr"));
        assert!(
            matches!(&entries[1], CommandEntry::Obj { name, path: Some(p) } if name == "deploy" && p == "ops")
        );
    }

    #[test]
    fn config_commands_supports_sub_dir_alias() {
        let yaml = r#"
skills: []
commands:
  - source: https://github.com/me/cmds
    sub_dir: nested/commands
    commands: "*"
"#;
        let cfg: Config = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(cfg.commands[0].sub_dir.as_deref(), Some("nested/commands"));
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct SourceSpec {
    pub source: String,
    pub branch: Option<String>,
    /// Pin to a git tag, commit SHA, or any ref. Takes priority over `branch`.
    /// When set, no main/master fallback is attempted.
    #[serde(rename = "ref")]
    pub git_ref: Option<String>,
    /// Optional subdirectory inside the source repo/path to use as the skill root.
    /// Supports both `sub-dir` and `sub_dir` YAML keys.
    #[serde(default, rename = "sub-dir", alias = "sub_dir")]
    pub sub_dir: Option<String>,
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
    /// Mirrors `skills[].skills`: `"*"` to discover all, or a list of names / `{ name, path }`.
    pub mcps: McpsField,
}

impl McpSourceSpec {
    pub(crate) fn as_source_spec(&self) -> SourceSpec {
        SourceSpec {
            source: self.source.clone(),
            branch: self.branch.clone(),
            git_ref: self.git_ref.clone(),
            sub_dir: None,
            skills: SkillsField::Wildcard("*".to_string()),
        }
    }
}

/// The `mcps` field on an `McpSourceSpec` — mirrors `SkillsField` exactly.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum McpsField {
    /// `mcps: "*"` — discover all MCP files in the source.
    #[allow(dead_code)]
    Wildcard(String),
    /// `mcps: [...]` — explicit list of names or `{ name, path }` objects.
    List(Vec<McpEntry>),
}

/// One entry in `mcps[].mcps` — mirrors `SkillTarget`.
///
/// - Plain string `"github"` → `mcps/github.json`
/// - Object `{ name: github, path: tools }` → `tools/github.json`
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum McpEntry {
    Name(String),
    Obj { name: String, path: Option<String> },
}

#[derive(Debug, Deserialize)]
pub(crate) struct CommandSourceSpec {
    pub source: String,
    pub branch: Option<String>,
    #[serde(rename = "ref")]
    pub git_ref: Option<String>,
    #[serde(default, rename = "sub-dir", alias = "sub_dir")]
    pub sub_dir: Option<String>,
    pub commands: CommandsField,
}

impl CommandSourceSpec {
    pub(crate) fn as_source_spec(&self) -> SourceSpec {
        SourceSpec {
            source: self.source.clone(),
            branch: self.branch.clone(),
            git_ref: self.git_ref.clone(),
            sub_dir: self.sub_dir.clone(),
            skills: SkillsField::Wildcard("*".to_string()),
        }
    }
}

/// The `commands` field on a `CommandSourceSpec` — mirrors `McpsField` / `SkillsField`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum CommandsField {
    Wildcard(String),
    List(Vec<CommandEntry>),
}

/// One entry in `commands[].commands` — mirrors `McpEntry`.
///
/// - Plain string `"review-pr"` → resolves through `discover_commands` (namespaced names)
/// - Object `{ name: deploy, path: ops }` → `<path>/<name>.md`
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum CommandEntry {
    Name(String),
    Obj { name: String, path: Option<String> },
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
