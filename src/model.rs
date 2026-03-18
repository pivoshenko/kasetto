use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub destination: Option<String>,
    pub agent: Option<Agent>,
    pub skills: Vec<SourceSpec>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    #[serde(rename = "amp")]
    Amp,
    #[serde(rename = "kimi-cli")]
    KimiCli,
    #[serde(rename = "replit")]
    Replit,
    #[serde(rename = "universal")]
    Universal,
    #[serde(rename = "antigravity")]
    Antigravity,
    #[serde(rename = "augment")]
    Augment,
    #[serde(rename = "claude-code", alias = "claude")]
    ClaudeCode,
    #[serde(rename = "openclaw")]
    OpenClaw,
    #[serde(rename = "cline")]
    Cline,
    #[serde(rename = "warp")]
    Warp,
    #[serde(rename = "codebuddy")]
    CodeBuddy,
    #[serde(rename = "codex")]
    Codex,
    #[serde(rename = "command-code")]
    CommandCode,
    #[serde(rename = "continue")]
    Continue,
    #[serde(rename = "cortex")]
    Cortex,
    #[serde(rename = "crush")]
    Crush,
    #[serde(rename = "cursor")]
    Cursor,
    #[serde(rename = "deepagents")]
    DeepAgents,
    #[serde(rename = "droid")]
    Droid,
    #[serde(rename = "gemini-cli")]
    GeminiCli,
    #[serde(rename = "github-copilot")]
    GithubCopilot,
    #[serde(rename = "goose")]
    Goose,
    #[serde(rename = "junie")]
    Junie,
    #[serde(rename = "iflow-cli")]
    IflowCli,
    #[serde(rename = "kilo")]
    Kilo,
    #[serde(rename = "kiro-cli")]
    KiroCli,
    #[serde(rename = "kode")]
    Kode,
    #[serde(rename = "mcpjam")]
    McpJam,
    #[serde(rename = "mistral-vibe")]
    MistralVibe,
    #[serde(rename = "mux")]
    Mux,
    #[serde(rename = "opencode")]
    OpenCode,
    #[serde(rename = "openhands")]
    OpenHands,
    #[serde(rename = "pi")]
    Pi,
    #[serde(rename = "qoder")]
    Qoder,
    #[serde(rename = "qwen-code")]
    QwenCode,
    #[serde(rename = "roo")]
    Roo,
    #[serde(rename = "trae")]
    Trae,
    #[serde(rename = "trae-cn")]
    TraeCn,
    #[serde(rename = "windsurf")]
    Windsurf,
    #[serde(rename = "zencoder")]
    Zencoder,
    #[serde(rename = "neovate")]
    Neovate,
    #[serde(rename = "pochi")]
    Pochi,
    #[serde(rename = "adal")]
    Adal,
}

impl Agent {
    pub fn global_path(self, home: &Path) -> PathBuf {
        match self {
            Agent::Amp | Agent::KimiCli | Agent::Replit | Agent::Universal => {
                home.join(".config/agents/skills")
            }
            Agent::Antigravity => home.join(".gemini/antigravity/skills"),
            Agent::Augment => home.join(".augment/skills"),
            Agent::ClaudeCode => home.join(".claude/skills"),
            Agent::OpenClaw => home.join(".openclaw/skills"),
            Agent::Cline | Agent::Warp => home.join(".agents/skills"),
            Agent::CodeBuddy => home.join(".codebuddy/skills"),
            Agent::Codex => home.join(".codex/skills"),
            Agent::CommandCode => home.join(".commandcode/skills"),
            Agent::Continue => home.join(".continue/skills"),
            Agent::Cortex => home.join(".snowflake/cortex/skills"),
            Agent::Crush => home.join(".config/crush/skills"),
            Agent::Cursor => home.join(".cursor/skills"),
            Agent::DeepAgents => home.join(".deepagents/agent/skills"),
            Agent::Droid => home.join(".factory/skills"),
            Agent::GeminiCli => home.join(".gemini/skills"),
            Agent::GithubCopilot => home.join(".copilot/skills"),
            Agent::Goose => home.join(".config/goose/skills"),
            Agent::Junie => home.join(".junie/skills"),
            Agent::IflowCli => home.join(".iflow/skills"),
            Agent::Kilo => home.join(".kilocode/skills"),
            Agent::KiroCli => home.join(".kiro/skills"),
            Agent::Kode => home.join(".kode/skills"),
            Agent::McpJam => home.join(".mcpjam/skills"),
            Agent::MistralVibe => home.join(".vibe/skills"),
            Agent::Mux => home.join(".mux/skills"),
            Agent::OpenCode => home.join(".config/opencode/skills"),
            Agent::OpenHands => home.join(".openhands/skills"),
            Agent::Pi => home.join(".pi/agent/skills"),
            Agent::Qoder => home.join(".qoder/skills"),
            Agent::QwenCode => home.join(".qwen/skills"),
            Agent::Roo => home.join(".roo/skills"),
            Agent::Trae => home.join(".trae/skills"),
            Agent::TraeCn => home.join(".trae-cn/skills"),
            Agent::Windsurf => home.join(".codeium/windsurf/skills"),
            Agent::Zencoder => home.join(".zencoder/skills"),
            Agent::Neovate => home.join(".neovate/skills"),
            Agent::Pochi => home.join(".pochi/skills"),
            Agent::Adal => home.join(".adal/skills"),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SourceSpec {
    pub source: String,
    pub branch: Option<String>,
    pub skills: SkillsField,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SkillsField {
    Wildcard(String),
    List(Vec<SkillTarget>),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SkillTarget {
    Name(String),
    Obj { name: String, path: Option<String> },
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct SkillEntry {
    pub destination: String,
    pub hash: String,
    pub skill: String,
    #[serde(default)]
    pub description: String,
    pub source: String,
    pub source_revision: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    pub version: u8,
    pub last_run: Option<String>,
    pub skills: BTreeMap<String, SkillEntry>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            last_run: None,
            skills: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Serialize, Default)]
pub struct Summary {
    pub installed: usize,
    pub updated: usize,
    pub removed: usize,
    pub unchanged: usize,
    pub broken: usize,
    pub failed: usize,
}

#[derive(Debug, Serialize)]
pub struct Action {
    pub source: Option<String>,
    pub skill: Option<String>,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub run_id: String,
    pub config: String,
    pub destination: String,
    pub dry_run: bool,
    pub summary: Summary,
    pub actions: Vec<Action>,
}

#[derive(Debug, Serialize, Clone)]
pub struct InstalledSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub skill: String,
    pub destination: String,
    pub hash: String,
    pub source_revision: String,
    pub updated_at: String,
    pub updated_ago: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct FailedInstall {
    pub skill: String,
    pub source: String,
    pub reason: String,
}
