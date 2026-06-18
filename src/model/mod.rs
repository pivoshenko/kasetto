mod agent;
mod config;
pub(crate) mod extend;
mod types;

use std::path::PathBuf;

pub(crate) use agent::{
    all_command_global_targets, all_command_project_targets, all_mcp_project_targets,
    all_mcp_settings_targets, command_global_targets, command_project_targets, Agent, AgentField,
};
pub(crate) use config::{
    resolve_scope, CommandEntry, CommandsField, Config, GitPin, McpEntry, McpsField, RuleEntry,
    RulesField, Scope, SkillTarget, SkillsField, SourceSpec,
};
pub(crate) use config::{CommandSourceSpec, McpSourceSpec, RuleSourceSpec};
pub(crate) use types::{
    Action, InstalledSkill, Report, SkillEntry, State, Summary, SyncFailure, LOCK_VERSION,
};

/// How Kasetto merges pack `mcpServers` into an agent config file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum McpSettingsFormat {
    /// `{ "mcpServers": { ... } }` (Claude, Cursor, Gemini CLI, Roo, Cline, etc.).
    McpServers,
    /// VS Code / GitHub Copilot user `mcp.json`: `{ "servers": { ... } }`.
    VsCodeServers,
    /// OpenCode `opencode.json`: `{ "mcp": { "name": { "type": "local"|"remote", ... } } }`.
    OpenCode,
    /// OpenAI Codex `~/.codex/config.toml` (`[mcp_servers.name]` tables).
    CodexToml,
}

/// Destination file and merge format for MCP sync / clean.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct McpSettingsTarget {
    pub path: PathBuf,
    pub format: McpSettingsFormat,
}

/// On-disk shape Kasetto emits for a command on a given agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CommandFormat {
    /// Verbatim Markdown with YAML frontmatter (Claude Code style).
    MarkdownFrontmatter,
    /// Markdown body only — frontmatter stripped.
    MarkdownPlain,
    /// `<name>.prompt.md` — frontmatter preserved (GitHub Copilot).
    PromptMd,
    /// `<name>.prompt` (Continue Dev) — frontmatter preserved, `invokable: true` injected.
    PromptFile,
    /// `<name>.toml` (Gemini CLI custom commands).
    GeminiToml,
}

/// Destination directory and write format for command sync / clean.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommandTarget {
    pub path: PathBuf,
    pub format: CommandFormat,
}

/// On-disk shape Kasetto emits for a rule on a given agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum RuleFormat {
    /// Plain Markdown body merged into a single shared file (`CLAUDE.md`,
    /// `AGENTS.md`, `GEMINI.md`, `.github/copilot-instructions.md`, …) via
    /// managed comment-block markers so multiple rules and hand edits coexist.
    AggregateMarkdown,
    /// `<name>.mdc` per rule — Cursor MDC frontmatter (`description`, `globs`,
    /// `alwaysApply`) reconstructed from the source, then the body.
    CursorMdc,
    /// `<name>.md` per rule — Markdown body only, frontmatter stripped.
    PlainMarkdownDir,
}

impl RuleFormat {
    /// Whether the target is a single shared file that rules merge into (as
    /// opposed to a directory holding one file per rule).
    pub(crate) fn is_aggregate(self) -> bool {
        matches!(self, RuleFormat::AggregateMarkdown)
    }
}

/// Destination (shared file or per-rule directory) and write format for rule
/// sync / clean. `path` is a file when `format.is_aggregate()`, else a directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuleTarget {
    pub path: PathBuf,
    pub format: RuleFormat,
}
