mod agent;
mod config;
mod types;

use std::path::PathBuf;

pub(crate) use agent::{all_mcp_project_targets, all_mcp_settings_targets, Agent, AgentField};
pub(crate) use config::{
    resolve_scope, Config, GitPin, Scope, SkillTarget, SkillsField, SourceSpec,
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
