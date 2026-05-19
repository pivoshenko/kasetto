mod agent;
mod config;
pub(crate) mod extend;
mod types;

use std::path::PathBuf;

pub(crate) use agent::{
    all_command_global_targets, all_command_project_targets, all_mcp_project_targets,
    all_mcp_settings_targets, Agent, AgentField,
};
#[cfg(test)]
pub(crate) use config::CommandSourceSpec;
pub(crate) use config::{
    resolve_scope, CommandEntry, CommandsField, Config, GitPin, McpEntry, McpsField, Scope,
    SkillTarget, SkillsField, SourceSpec,
};
pub(crate) use types::{Action, InstalledSkill, Report, SkillEntry, State, Summary, SyncFailure};

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
