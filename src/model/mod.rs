//! Package that contains the data model: config schema, agent registry, and lock-file types.

mod agent;
mod config;
pub(crate) mod extend;
mod types;

use std::path::PathBuf;

pub(crate) use agent::{
    agent_global_targets, agent_project_targets, all_agent_global_targets,
    all_agent_project_targets, all_command_global_targets, all_command_project_targets,
    all_instruction_global_targets, all_instruction_project_targets, all_mcp_project_targets,
    all_mcp_settings_targets, all_skill_global_targets, all_skill_project_targets,
    command_global_targets, command_project_targets, instruction_global_targets,
    instruction_project_targets, mcp_settings_project_targets, mcp_settings_targets,
    skill_global_targets, skill_project_targets, Agent, AgentField,
};
pub(crate) use config::{
    resolve_scope, AgentEntry, AgentSourceSpec, AgentsField, CommandEntry, CommandsField, Config,
    GitPin, InstructionEntry, InstructionsField, McpEntry, McpsField, PluginSourceSpec,
    PluginsField, Scope, SkillTarget, SkillsField, SourceSpec,
};
pub(crate) use config::{
    CommandSourceSpec, InlineMcpSpec, InstructionSourceSpec, McpSourceSpec, McpSpec, INLINE_SOURCE,
};
pub(crate) use config::{OnMissing, SecretsConfig};
pub(crate) use types::{
    Action, AuditGate, AuditViolation, InstalledSkill, Report, SkillEntry, State, Summary,
    SyncFailure, LOCK_VERSION,
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
    /// ZCode config JSON: `{ "mcp": { "servers": { ... } } }` (nested under `mcp.servers`).
    ZCode,
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
    /// Flat `<name>.md` with YAML frontmatter preserved (Pi prompt templates).
    MarkdownFlatFrontmatter,
    /// Markdown body only, frontmatter stripped.
    MarkdownPlain,
    /// `<name>.prompt.md`, frontmatter preserved (GitHub Copilot).
    PromptMd,
    /// `<name>.prompt` (Continue Dev), frontmatter preserved, `invokable: true` injected.
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

/// On-disk shape Kasetto emits for a subagent on a given agent. Each variant
/// keeps only the frontmatter keys its tool documents, because several tools
/// (Gemini CLI, Codex) refuse to load a definition carrying an unknown key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum AgentFormat {
    /// Claude Code `<name>.md`: the source verbatim (`name` injected when absent).
    ClaudeCode,
    /// Cursor `<name>.md`: `name`, `description`, `readonly`, `is_background`.
    Cursor,
    /// GitHub Copilot `<name>.agent.md`: `name`, `description`, `tools` (Claude
    /// tool names resolve through Copilot's documented aliases).
    Copilot,
    /// OpenAI Codex `<name>.toml`: `name`, `description`, body as `developer_instructions`.
    CodexToml,
    /// Gemini CLI `<name>.md` (strict schema): `name`, `description`, `tools` mapped to
    /// Gemini tool names as a YAML list.
    Gemini,
    /// OpenCode `<name>.md`: `description` and `mode: subagent` (the default `primary`
    /// would make it a persona rather than a delegate).
    OpenCode,
    /// Augment `<name>.md`: `name`, `description`, `color`, `model`, and `tools` /
    /// `disabled_tools` mapped to Auggie's lowercase built-ins.
    Augment,
    /// Cline `<name>.yaml` holding frontmatter + body: `name`, `description`, `tools`
    /// mapped to Cline tool ids.
    Cline,
    /// Goose `<name>.md`: `name`, `description` (Goose reads nothing else).
    Goose,
    /// JetBrains Junie `<name>.md`: `name`, `description`, `model`, `maxTurns`, `skills`,
    /// and `tools` / `disallowedTools` (the Claude tool names Junie shares).
    Junie,
    /// Kiro `<name>.md`: `name`, `description`, `tools` mapped to Kiro categories.
    Kiro,
    /// OpenHands `<name>.md`: `name`, `description`, `tools` mapped to OpenHands tools.
    OpenHands,
    /// Trae `<name>.md`: `name`, `description`, `tools` / `disallowedTools` as
    /// comma-separated strings.
    Trae,
    /// ZCode `<name>.md`: `name`, `description`, `model: inherit`, `tools` /
    /// `disallowedTools`.
    ZCode,
}

/// Destination directory and write format for subagent sync / clean.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AgentTarget {
    pub path: PathBuf,
    pub format: AgentFormat,
}

/// Settings file a tool reads plugin marketplaces (`extraKnownMarketplaces`) and
/// enabled plugins (`enabledPlugins`) from. Only Claude Code and Copilot CLI share
/// that shape and fetch the plugin themselves; every other plugin system needs a
/// pre-placed clone or its own CLI to install one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PluginTarget {
    pub path: PathBuf,
}

/// On-disk shape Kasetto emits for an instruction on a given agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum InstructionFormat {
    /// Plain Markdown body merged into a single shared file (`CLAUDE.md`,
    /// `AGENTS.md`, `GEMINI.md`, `.github/copilot-instructions.md`, ...) via
    /// managed comment-block markers so multiple instructions and hand edits coexist.
    AggregateMarkdown,
    /// `<name>.mdc` per instruction: Cursor MDC frontmatter (`description`, `globs`,
    /// `alwaysApply`) reconstructed from the source, then the body.
    CursorMdc,
    /// `<name>.md` per instruction: Markdown body only, frontmatter stripped.
    PlainMarkdownDir,
}

impl InstructionFormat {
    /// Whether the target is a single shared file that instructions merge into (as
    /// opposed to a directory holding one file per instruction).
    pub(crate) fn is_aggregate(self) -> bool {
        matches!(self, InstructionFormat::AggregateMarkdown)
    }
}

/// Destination (shared file or per-instruction directory) and write format for instruction
/// sync / clean. `path` is a file when `format.is_aggregate()`, else a directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InstructionTarget {
    pub path: PathBuf,
    pub format: InstructionFormat,
}
