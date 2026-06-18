use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::{
    CommandFormat, CommandTarget, InstructionFormat, InstructionTarget, McpSettingsFormat,
    McpSettingsTarget,
};

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub(crate) enum AgentField {
    One(Agent),
    Many(Vec<Agent>),
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Agent {
    #[serde(rename = "amp")]
    Amp,
    #[serde(rename = "antigravity")]
    Antigravity,
    #[serde(rename = "augment")]
    Augment,
    #[serde(rename = "claude-code")]
    ClaudeCode,
    #[serde(rename = "cline")]
    Cline,
    #[serde(rename = "codex")]
    Codex,
    #[serde(rename = "continue")]
    Continue,
    #[serde(rename = "cursor")]
    Cursor,
    #[serde(rename = "gemini-cli")]
    GeminiCli,
    #[serde(rename = "github-copilot")]
    GithubCopilot,
    #[serde(rename = "goose")]
    Goose,
    #[serde(rename = "junie")]
    Junie,
    #[serde(rename = "kiro-cli")]
    KiroCli,
    #[serde(rename = "openclaw")]
    OpenClaw,
    #[serde(rename = "opencode")]
    OpenCode,
    #[serde(rename = "openhands")]
    OpenHands,
    #[serde(rename = "replit")]
    Replit,
    #[serde(rename = "roo")]
    Roo,
    #[serde(rename = "trae")]
    Trae,
    #[serde(rename = "warp")]
    Warp,
    #[serde(rename = "windsurf")]
    Windsurf,
}

/// Every preset value (for clean / enumerating native MCP paths).
pub(crate) const AGENT_PRESETS: &[Agent] = &[
    Agent::Amp,
    Agent::Antigravity,
    Agent::Augment,
    Agent::ClaudeCode,
    Agent::Cline,
    Agent::Codex,
    Agent::Continue,
    Agent::Cursor,
    Agent::GeminiCli,
    Agent::GithubCopilot,
    Agent::Goose,
    Agent::Junie,
    Agent::KiroCli,
    Agent::OpenClaw,
    Agent::OpenCode,
    Agent::OpenHands,
    Agent::Replit,
    Agent::Roo,
    Agent::Trae,
    Agent::Warp,
    Agent::Windsurf,
];

/// Deduped native MCP config files for every known agent (for `clean` manifest wipe).
pub(crate) fn all_mcp_settings_targets(
    home: &Path,
    kasetto_config: &Path,
) -> Vec<McpSettingsTarget> {
    dedup_targets(
        AGENT_PRESETS
            .iter()
            .map(|a| a.mcp_settings_target(home, kasetto_config)),
    )
}

/// Deduped project-level MCP config files for every known agent (for `clean` in project scope).
pub(crate) fn all_mcp_project_targets(project_root: &Path) -> Vec<McpSettingsTarget> {
    dedup_targets(
        AGENT_PRESETS
            .iter()
            .map(|a| a.mcp_project_target(project_root)),
    )
}

fn dedup_targets(iter: impl Iterator<Item = McpSettingsTarget>) -> Vec<McpSettingsTarget> {
    let mut seen = std::collections::HashSet::<PathBuf>::new();
    let mut out: Vec<McpSettingsTarget> = iter.filter(|t| seen.insert(t.path.clone())).collect();
    out.sort_by(|x, y| x.path.cmp(&y.path));
    out
}

fn dedup_command_targets(iter: impl Iterator<Item = Option<CommandTarget>>) -> Vec<CommandTarget> {
    let mut seen = std::collections::HashSet::<PathBuf>::new();
    let mut out: Vec<CommandTarget> = iter
        .flatten()
        .filter(|t| seen.insert(t.path.clone()))
        .collect();
    out.sort_by(|x, y| x.path.cmp(&y.path));
    out
}

/// Deduped global command directories for every known agent.
pub(crate) fn all_command_global_targets(home: &Path) -> Vec<CommandTarget> {
    dedup_command_targets(AGENT_PRESETS.iter().map(|a| a.commands_global_path(home)))
}

/// Deduped project-level command directories for every known agent.
pub(crate) fn all_command_project_targets(project_root: &Path) -> Vec<CommandTarget> {
    dedup_command_targets(
        AGENT_PRESETS
            .iter()
            .map(|a| a.commands_project_path(project_root)),
    )
}

/// Deduped global command directories for a specific set of agents — used by
/// `doctor` to scope the COMMAND DIRECTORIES panel to what the config wires.
pub(crate) fn command_global_targets(home: &Path, agents: &[Agent]) -> Vec<CommandTarget> {
    dedup_command_targets(agents.iter().map(|a| a.commands_global_path(home)))
}

/// Deduped project command directories for a specific set of agents.
pub(crate) fn command_project_targets(project_root: &Path, agents: &[Agent]) -> Vec<CommandTarget> {
    dedup_command_targets(agents.iter().map(|a| a.commands_project_path(project_root)))
}

#[inline]
fn cmd(base: &Path, rel: &str, format: CommandFormat) -> Option<CommandTarget> {
    Some(CommandTarget {
        path: base.join(rel),
        format,
    })
}

#[inline]
fn instruction(base: &Path, rel: &str, format: InstructionFormat) -> Option<InstructionTarget> {
    Some(InstructionTarget {
        path: base.join(rel),
        format,
    })
}

/// VS Code / Copilot user-profile `mcp.json` (not Insiders).
fn vscode_user_mcp_json(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Code/User/mcp.json")
    } else if cfg!(target_os = "windows") {
        let base = std::env::var("APPDATA").unwrap_or_default();
        PathBuf::from(base).join("Code/User/mcp.json")
    } else {
        home.join(".config/Code/User/mcp.json")
    }
}

#[inline]
fn mcp_servers_target(base: &Path, rel: &str) -> McpSettingsTarget {
    McpSettingsTarget {
        path: base.join(rel),
        format: McpSettingsFormat::McpServers,
    }
}

impl Agent {
    pub(crate) fn global_path(self, home: &Path) -> PathBuf {
        match self {
            Agent::Amp | Agent::Replit => home.join(".config/agents/skills"),
            Agent::Antigravity => home.join(".gemini/antigravity/skills"),
            Agent::Augment => home.join(".augment/skills"),
            Agent::ClaudeCode => home.join(".claude/skills"),
            Agent::Cline | Agent::Warp => home.join(".agents/skills"),
            Agent::Codex => home.join(".codex/skills"),
            Agent::Continue => home.join(".continue/skills"),
            Agent::Cursor => home.join(".cursor/skills"),
            Agent::GeminiCli => home.join(".gemini/skills"),
            Agent::GithubCopilot => home.join(".copilot/skills"),
            Agent::Goose => home.join(".config/goose/skills"),
            Agent::Junie => home.join(".junie/skills"),
            Agent::KiroCli => home.join(".kiro/skills"),
            Agent::OpenClaw => home.join(".openclaw/skills"),
            Agent::OpenCode => home.join(".config/opencode/skills"),
            Agent::OpenHands => home.join(".openhands/skills"),
            Agent::Roo => home.join(".roo/skills"),
            Agent::Trae => home.join(".trae/skills"),
            Agent::Windsurf => home.join(".codeium/windsurf/skills"),
        }
    }

    /// Native MCP config location and merge format for this agent.
    pub(crate) fn mcp_settings_target(
        self,
        home: &Path,
        _kasetto_config: &Path,
    ) -> McpSettingsTarget {
        match self {
            Agent::ClaudeCode => mcp_servers_target(home, ".claude.json"),
            Agent::Cursor => mcp_servers_target(home, ".cursor/mcp.json"),
            Agent::GithubCopilot => McpSettingsTarget {
                path: vscode_user_mcp_json(home),
                format: McpSettingsFormat::VsCodeServers,
            },
            Agent::GeminiCli => mcp_servers_target(home, ".gemini/settings.json"),
            Agent::Roo => mcp_servers_target(home, ".roo/mcp_settings.json"),
            Agent::Windsurf => mcp_servers_target(home, ".codeium/windsurf/mcp_config.json"),
            Agent::Cline => {
                mcp_servers_target(home, ".cline/data/settings/cline_mcp_settings.json")
            }
            Agent::Continue => mcp_servers_target(home, ".continue/mcpServers/kasetto.json"),
            Agent::Amp | Agent::Replit => mcp_servers_target(home, ".config/agents/mcp.json"),
            Agent::Antigravity => mcp_servers_target(home, ".gemini/antigravity/mcp.json"),
            Agent::Augment => mcp_servers_target(home, ".augment/mcp.json"),
            Agent::Warp => mcp_servers_target(home, ".warp/mcp.json"),
            Agent::Codex => McpSettingsTarget {
                path: home.join(".codex/config.toml"),
                format: McpSettingsFormat::CodexToml,
            },
            Agent::Goose => mcp_servers_target(home, ".config/goose/mcp.json"),
            Agent::Junie => mcp_servers_target(home, ".junie/mcp.json"),
            Agent::KiroCli => mcp_servers_target(home, ".kiro/mcp.json"),
            Agent::OpenClaw => mcp_servers_target(home, ".openclaw/mcp.json"),
            Agent::OpenCode => McpSettingsTarget {
                path: home.join(".config/opencode/opencode.json"),
                format: McpSettingsFormat::OpenCode,
            },
            Agent::OpenHands => mcp_servers_target(home, ".openhands/mcp.json"),
            Agent::Trae => mcp_servers_target(home, ".trae/mcp.json"),
        }
    }

    /// Project-local skills directory for this agent, relative to `project_root`.
    pub(crate) fn project_path(self, project_root: &Path) -> PathBuf {
        match self {
            Agent::Amp | Agent::Replit => project_root.join(".agents/skills"),
            Agent::Antigravity => project_root.join(".gemini/antigravity/skills"),
            Agent::Augment => project_root.join(".augment/skills"),
            Agent::ClaudeCode => project_root.join(".claude/skills"),
            Agent::Cline | Agent::Warp => project_root.join(".agents/skills"),
            Agent::Codex => project_root.join(".codex/skills"),
            Agent::Continue => project_root.join(".continue/skills"),
            Agent::Cursor => project_root.join(".cursor/skills"),
            Agent::GeminiCli => project_root.join(".gemini/skills"),
            Agent::GithubCopilot => project_root.join(".copilot/skills"),
            Agent::Goose => project_root.join(".goose/skills"),
            Agent::Junie => project_root.join(".junie/skills"),
            Agent::KiroCli => project_root.join(".kiro/skills"),
            Agent::OpenClaw => project_root.join(".openclaw/skills"),
            Agent::OpenCode => project_root.join(".opencode/skills"),
            Agent::OpenHands => project_root.join(".openhands/skills"),
            Agent::Roo => project_root.join(".roo/skills"),
            Agent::Trae => project_root.join(".trae/skills"),
            Agent::Windsurf => project_root.join(".windsurf/skills"),
        }
    }

    /// Project-local MCP config location and merge format for this agent.
    pub(crate) fn mcp_project_target(self, project_root: &Path) -> McpSettingsTarget {
        match self {
            Agent::ClaudeCode => McpSettingsTarget {
                path: project_root.join(".mcp.json"),
                format: McpSettingsFormat::McpServers,
            },
            Agent::Cursor => mcp_servers_target(project_root, ".cursor/mcp.json"),
            Agent::GithubCopilot => McpSettingsTarget {
                path: project_root.join(".vscode/mcp.json"),
                format: McpSettingsFormat::VsCodeServers,
            },
            Agent::GeminiCli => mcp_servers_target(project_root, ".gemini/settings.json"),
            Agent::Roo => mcp_servers_target(project_root, ".roo/mcp.json"),
            Agent::Windsurf => mcp_servers_target(project_root, ".windsurf/mcp.json"),
            Agent::Cline => mcp_servers_target(project_root, ".cline_mcp_servers.json"),
            Agent::Continue => {
                mcp_servers_target(project_root, ".continue/mcpServers/kasetto.json")
            }
            Agent::Codex => McpSettingsTarget {
                path: project_root.join(".codex/config.toml"),
                format: McpSettingsFormat::CodexToml,
            },
            Agent::Amp => mcp_servers_target(project_root, ".amp/mcp.json"),
            Agent::Trae => mcp_servers_target(project_root, ".trae/mcp.json"),
            Agent::Junie => mcp_servers_target(project_root, ".junie/mcp/mcp.json"),
            Agent::KiroCli => mcp_servers_target(project_root, ".kiro/settings/mcp.json"),
            Agent::OpenCode => McpSettingsTarget {
                path: project_root.join(".opencode/opencode.json"),
                format: McpSettingsFormat::OpenCode,
            },
            Agent::Antigravity
            | Agent::Augment
            | Agent::Goose
            | Agent::OpenClaw
            | Agent::OpenHands
            | Agent::Replit
            | Agent::Warp => mcp_servers_target(project_root, ".mcp.json"),
        }
    }

    /// Global commands directory and write format for this agent, if supported.
    pub(crate) fn commands_global_path(self, home: &Path) -> Option<CommandTarget> {
        match self {
            Agent::ClaudeCode => cmd(home, ".claude/commands", CommandFormat::MarkdownFrontmatter),
            Agent::Windsurf => cmd(
                home,
                ".codeium/windsurf/global_workflows",
                CommandFormat::MarkdownFrontmatter,
            ),
            Agent::OpenCode => cmd(
                home,
                ".config/opencode/commands",
                CommandFormat::MarkdownFrontmatter,
            ),
            Agent::Continue => cmd(home, ".continue/prompts", CommandFormat::PromptFile),
            Agent::Amp => cmd(
                home,
                ".config/amp/commands",
                CommandFormat::MarkdownFrontmatter,
            ),
            Agent::Augment => cmd(
                home,
                ".augment/commands",
                CommandFormat::MarkdownFrontmatter,
            ),
            Agent::Roo => cmd(home, ".roo/commands", CommandFormat::MarkdownFrontmatter),
            Agent::Codex => cmd(home, ".codex/prompts", CommandFormat::MarkdownFrontmatter),
            Agent::GeminiCli => cmd(home, ".gemini/commands", CommandFormat::GeminiToml),
            Agent::Cursor
            | Agent::Cline
            | Agent::GithubCopilot
            | Agent::Junie
            | Agent::OpenHands
            | Agent::Antigravity
            | Agent::Goose
            | Agent::KiroCli
            | Agent::OpenClaw
            | Agent::Replit
            | Agent::Trae
            | Agent::Warp => None,
        }
    }

    /// Project-local commands directory and write format for this agent, if supported.
    pub(crate) fn commands_project_path(self, project_root: &Path) -> Option<CommandTarget> {
        match self {
            Agent::ClaudeCode => cmd(
                project_root,
                ".claude/commands",
                CommandFormat::MarkdownFrontmatter,
            ),
            Agent::Cursor => cmd(
                project_root,
                ".cursor/commands",
                CommandFormat::MarkdownPlain,
            ),
            Agent::Windsurf => cmd(
                project_root,
                ".windsurf/workflows",
                CommandFormat::MarkdownFrontmatter,
            ),
            Agent::Cline => cmd(
                project_root,
                ".clinerules/workflows",
                CommandFormat::MarkdownPlain,
            ),
            Agent::OpenCode => cmd(
                project_root,
                ".opencode/commands",
                CommandFormat::MarkdownFrontmatter,
            ),
            Agent::Continue => cmd(project_root, ".continue/prompts", CommandFormat::PromptFile),
            Agent::GithubCopilot => cmd(project_root, ".github/prompts", CommandFormat::PromptMd),
            Agent::Amp => cmd(
                project_root,
                ".agents/commands",
                CommandFormat::MarkdownFrontmatter,
            ),
            Agent::Augment => cmd(
                project_root,
                ".augment/commands",
                CommandFormat::MarkdownFrontmatter,
            ),
            Agent::Roo => cmd(
                project_root,
                ".roo/commands",
                CommandFormat::MarkdownFrontmatter,
            ),
            Agent::Junie => cmd(
                project_root,
                ".junie/commands",
                CommandFormat::MarkdownFrontmatter,
            ),
            Agent::OpenHands => cmd(
                project_root,
                ".openhands/microagents",
                CommandFormat::MarkdownFrontmatter,
            ),
            Agent::GeminiCli => cmd(project_root, ".gemini/commands", CommandFormat::GeminiToml),
            Agent::Antigravity
            | Agent::Codex
            | Agent::Goose
            | Agent::KiroCli
            | Agent::OpenClaw
            | Agent::Replit
            | Agent::Trae
            | Agent::Warp => None,
        }
    }

    /// Project-local instructions destination and write format for this agent.
    ///
    /// Project paths are verified against each agent's official docs (early
    /// 2026). Aggregate formats are single shared instruction files; dir formats
    /// hold one file per instruction. `OpenClaw` has no project-scoped instructions concept
    /// (its `AGENTS.md` lives in the machine-level workspace — see
    /// `instructions_global_path`).
    pub(crate) fn instructions_project_path(
        self,
        project_root: &Path,
    ) -> Option<InstructionTarget> {
        use InstructionFormat::{AggregateMarkdown as Agg, CursorMdc, PlainMarkdownDir as Dir};
        match self {
            // Aggregate single-file instruction files.
            Agent::ClaudeCode => instruction(project_root, "CLAUDE.md", Agg),
            Agent::Codex => instruction(project_root, "AGENTS.md", Agg),
            Agent::OpenCode => instruction(project_root, "AGENTS.md", Agg),
            Agent::Amp => instruction(project_root, "AGENTS.md", Agg),
            // Antigravity's native file is GEMINI.md (takes precedence over AGENTS.md).
            Agent::Antigravity => instruction(project_root, "GEMINI.md", Agg),
            Agent::GeminiCli => instruction(project_root, "GEMINI.md", Agg),
            Agent::GithubCopilot => {
                instruction(project_root, ".github/copilot-instructions.md", Agg)
            }
            // Junie's current primary file is .junie/AGENTS.md (.junie/guidelines.md is legacy).
            Agent::Junie => instruction(project_root, ".junie/AGENTS.md", Agg),
            Agent::Goose => instruction(project_root, ".goosehints", Agg),
            Agent::Warp => instruction(project_root, "WARP.md", Agg),
            // Replit Agent reads replit.md, not AGENTS.md.
            Agent::Replit => instruction(project_root, "replit.md", Agg),
            // OpenHands' always-on repo instructions live in this single file.
            Agent::OpenHands => instruction(project_root, ".openhands/microagents/repo.md", Agg),
            // Cursor MDC: per-instruction files with reconstructed frontmatter.
            Agent::Cursor => instruction(project_root, ".cursor/rules", CursorMdc),
            // Per-instruction plain-markdown directories.
            Agent::Windsurf => instruction(project_root, ".windsurf/rules", Dir),
            Agent::Cline => instruction(project_root, ".clinerules", Dir),
            Agent::Continue => instruction(project_root, ".continue/rules", Dir),
            Agent::Roo => instruction(project_root, ".roo/rules", Dir),
            Agent::Augment => instruction(project_root, ".augment/rules", Dir),
            Agent::KiroCli => instruction(project_root, ".kiro/steering", Dir),
            Agent::Trae => instruction(project_root, ".trae/rules", Dir),
            // OpenClaw has no per-project instructions file (workspace-level only).
            Agent::OpenClaw => None,
        }
    }

    /// Global / user-level instructions destination for this agent.
    ///
    /// Most are documented official paths; a few are community-reported and not
    /// yet in official docs (notably Cursor `~/.cursor/rules`, Cline
    /// `~/Documents/Cline/Rules`) — harmless where a given build ignores them.
    /// Returns `None` for agents whose global instructions are UI-managed (Warp, Trae)
    /// or have no documented on-disk location (Continue, OpenHands, Replit).
    pub(crate) fn instructions_global_path(self, home: &Path) -> Option<InstructionTarget> {
        use InstructionFormat::{AggregateMarkdown as Agg, CursorMdc, PlainMarkdownDir as Dir};
        match self {
            Agent::ClaudeCode => instruction(home, ".claude/CLAUDE.md", Agg),
            Agent::Codex => instruction(home, ".codex/AGENTS.md", Agg),
            Agent::GeminiCli => instruction(home, ".gemini/GEMINI.md", Agg),
            Agent::Antigravity => instruction(home, ".gemini/GEMINI.md", Agg),
            Agent::Junie => instruction(home, ".junie/AGENTS.md", Agg),
            Agent::OpenCode => instruction(home, ".config/opencode/AGENTS.md", Agg),
            Agent::Amp => instruction(home, ".config/amp/AGENTS.md", Agg),
            Agent::Goose => instruction(home, ".config/goose/.goosehints", Agg),
            Agent::GithubCopilot => instruction(home, ".copilot/copilot-instructions.md", Agg),
            Agent::OpenClaw => instruction(home, ".openclaw/workspace/AGENTS.md", Agg),
            Agent::Windsurf => instruction(home, ".codeium/windsurf/memories/global_rules.md", Agg),
            Agent::KiroCli => instruction(home, ".kiro/steering", Dir),
            Agent::Augment => instruction(home, ".augment/rules", Dir),
            Agent::Roo => instruction(home, ".roo/rules", Dir),
            Agent::Cline => instruction(home, "Documents/Cline/Rules", Dir),
            // Community-supported (not yet in official docs): global ~/.cursor/rules
            // is read as .mdc by recent Cursor builds; harmless where it isn't.
            Agent::Cursor => instruction(home, ".cursor/rules", CursorMdc),
            // UI-managed or no documented on-disk global instructions.
            Agent::Warp | Agent::Trae | Agent::Continue | Agent::OpenHands | Agent::Replit => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_global_path_known_agents() {
        let home = Path::new("/tmp/home");
        assert_eq!(
            Agent::ClaudeCode.commands_global_path(home).unwrap().path,
            home.join(".claude/commands")
        );
        assert_eq!(
            Agent::Windsurf.commands_global_path(home).unwrap().path,
            home.join(".codeium/windsurf/global_workflows")
        );
        assert_eq!(
            Agent::GeminiCli.commands_global_path(home).unwrap().format,
            CommandFormat::GeminiToml
        );
        assert!(Agent::Cursor.commands_global_path(home).is_none());
        assert!(Agent::Trae.commands_global_path(home).is_none());
    }

    #[test]
    fn commands_project_path_known_agents() {
        let pr = Path::new("/work");
        assert_eq!(
            Agent::Cursor.commands_project_path(pr).unwrap().path,
            pr.join(".cursor/commands")
        );
        assert_eq!(
            Agent::Cursor.commands_project_path(pr).unwrap().format,
            CommandFormat::MarkdownPlain
        );
        assert_eq!(
            Agent::GithubCopilot
                .commands_project_path(pr)
                .unwrap()
                .format,
            CommandFormat::PromptMd
        );
        assert!(Agent::Codex.commands_project_path(pr).is_none());
        assert!(Agent::Warp.commands_project_path(pr).is_none());
    }

    #[test]
    fn instructions_project_path_known_agents() {
        let pr = Path::new("/work");
        assert_eq!(
            Agent::ClaudeCode
                .instructions_project_path(pr)
                .unwrap()
                .path,
            pr.join("CLAUDE.md")
        );
        assert!(Agent::ClaudeCode
            .instructions_project_path(pr)
            .unwrap()
            .format
            .is_aggregate());
        assert_eq!(
            Agent::Cursor.instructions_project_path(pr).unwrap().format,
            InstructionFormat::CursorMdc
        );
        assert_eq!(
            Agent::Windsurf
                .instructions_project_path(pr)
                .unwrap()
                .format,
            InstructionFormat::PlainMarkdownDir
        );
        assert_eq!(
            Agent::Codex.instructions_project_path(pr).unwrap().path,
            pr.join("AGENTS.md")
        );
        // Doc-verified corrections.
        assert_eq!(
            Agent::Replit.instructions_project_path(pr).unwrap().path,
            pr.join("replit.md")
        );
        assert_eq!(
            Agent::Antigravity
                .instructions_project_path(pr)
                .unwrap()
                .path,
            pr.join("GEMINI.md")
        );
        assert_eq!(
            Agent::OpenHands.instructions_project_path(pr).unwrap().path,
            pr.join(".openhands/microagents/repo.md")
        );
        assert_eq!(
            Agent::Junie.instructions_project_path(pr).unwrap().path,
            pr.join(".junie/AGENTS.md")
        );
        // OpenClaw has no per-project instructions concept.
        assert!(Agent::OpenClaw.instructions_project_path(pr).is_none());
        // Every other preset resolves to a project instructions target.
        for a in AGENT_PRESETS.iter().filter(|a| **a != Agent::OpenClaw) {
            assert!(
                a.instructions_project_path(pr).is_some(),
                "{a:?} missing project instructions"
            );
        }
    }

    #[test]
    fn instructions_global_path_for_agents_with_global_location() {
        let home = Path::new("/tmp/home");
        assert_eq!(
            Agent::ClaudeCode
                .instructions_global_path(home)
                .unwrap()
                .path,
            home.join(".claude/CLAUDE.md")
        );
        assert_eq!(
            Agent::Junie.instructions_global_path(home).unwrap().path,
            home.join(".junie/AGENTS.md")
        );
        assert_eq!(
            Agent::OpenClaw.instructions_global_path(home).unwrap().path,
            home.join(".openclaw/workspace/AGENTS.md")
        );
        // Cursor global ~/.cursor/rules (community-supported .mdc dir).
        let cursor = Agent::Cursor.instructions_global_path(home).unwrap();
        assert_eq!(cursor.path, home.join(".cursor/rules"));
        assert_eq!(cursor.format, InstructionFormat::CursorMdc);
        // Warp / Trae globals are UI-managed → no syncable file.
        assert!(Agent::Warp.instructions_global_path(home).is_none());
        assert!(Agent::Trae.instructions_global_path(home).is_none());
    }

    #[test]
    fn all_command_global_targets_dedupes_and_sorts() {
        let home = Path::new("/tmp/home");
        let all = all_command_global_targets(home);
        assert!(!all.is_empty());
        for w in all.windows(2) {
            assert!(w[0].path <= w[1].path);
        }
    }
}
