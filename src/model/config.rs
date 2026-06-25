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

/// Deserialized `kasetto.yaml`: the full sync request (destination, scope, agents, and sources).
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
    #[serde(default)]
    pub instructions: Vec<InstructionSourceSpec>,
    /// Optional secret-injection settings. Carries no secret *values* (it is
    /// committed) — only policy and extra credential-file paths.
    #[serde(default)]
    pub secrets: Option<SecretsConfig>,
}

/// `secrets:` config block. Values live in `credentials.yaml` / env, never here.
#[derive(Debug, Deserialize, Default)]
pub(crate) struct SecretsConfig {
    /// What to do when a `${KST_…}` placeholder can't be resolved. Default `error`.
    #[serde(default)]
    pub on_missing: Option<OnMissing>,
    /// Extra credential files (relative to the config dir, or absolute), searched
    /// after the default `$XDG_CONFIG_HOME/kasetto/credentials.yaml`.
    #[serde(default)]
    pub files: Vec<String>,
}

/// Behavior when a referenced secret can't be resolved.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OnMissing {
    Error,
    Warn,
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
    fn config_instructions_parses_wildcard() {
        let yaml = r#"
skills: []
instructions:
  - source: https://github.com/me/rules
    instructions: "*"
"#;
        let cfg: Config = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(cfg.instructions.len(), 1);
        assert!(matches!(
            cfg.instructions[0].instructions,
            InstructionsField::Wildcard(_)
        ));
    }

    #[test]
    fn config_instructions_parses_plain_strings_and_objects() {
        let yaml = r#"
skills: []
instructions:
  - source: https://github.com/me/rules
    ref: v1.0
    sub-dir: instructions
    instructions:
      - style
      - name: security
        path: house
"#;
        let cfg: Config = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(cfg.instructions[0].git_ref.as_deref(), Some("v1.0"));
        assert_eq!(cfg.instructions[0].sub_dir.as_deref(), Some("instructions"));
        let InstructionsField::List(ref entries) = cfg.instructions[0].instructions else {
            panic!("expected list");
        };
        assert_eq!(entries.len(), 2);
        assert!(matches!(&entries[0], InstructionEntry::Name(n) if n == "style"));
        assert!(
            matches!(&entries[1], InstructionEntry::Obj { name, path: Some(p) } if name == "security" && p == "house")
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

/// A skill source: where to fetch from and which skills to install.
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

    /// Same label `source::materialize_source` records as `source_revision`
    /// for this spec. Used by the `needs_fetch` paths to detect when a user
    /// retargeted a source (changed `ref` / `branch`) so we don't skip the
    /// fetch just because the old destination still hashes correctly.
    pub(crate) fn expected_revision(&self) -> String {
        if !self.source.contains("://") {
            return "local".into();
        }
        match self.git_pin() {
            GitPin::Ref(r) => format!("ref:{r}"),
            GitPin::Branch(b) => format!("branch:{b}"),
            GitPin::Default => "branch:main".into(),
        }
    }
}

/// An MCP source: where to fetch from and which MCP servers to install.
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

/// A command source: where to fetch from and which slash commands to install.
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

/// A instructions source: where to fetch from and which instruction files to install.
#[derive(Debug, Deserialize)]
pub(crate) struct InstructionSourceSpec {
    pub source: String,
    pub branch: Option<String>,
    #[serde(rename = "ref")]
    pub git_ref: Option<String>,
    #[serde(default, rename = "sub-dir", alias = "sub_dir")]
    pub sub_dir: Option<String>,
    pub instructions: InstructionsField,
}

impl InstructionSourceSpec {
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

/// The `instructions` field on a `InstructionSourceSpec` — mirrors `CommandsField`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum InstructionsField {
    Wildcard(String),
    List(Vec<InstructionEntry>),
}

/// One entry in `instructions[].instructions` — mirrors `CommandEntry`.
///
/// - Plain string `"style"` → resolves through `discover_instructions` (namespaced names)
/// - Object `{ name: style, path: house }` → `<path>/style.{md,mdc}`
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum InstructionEntry {
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
