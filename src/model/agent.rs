use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::{McpSettingsFormat, McpSettingsTarget};

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
}
