//! Module that contains per-format subagent rendering and destination path derivation.
//!
//! The source is a Claude Code subagent. Every other format keeps only the keys
//! its tool documents and maps tool names into that tool's vocabulary, because
//! the strict loaders (Gemini CLI, Codex, OpenHands) drop a definition that
//! carries an unknown key or tool name instead of ignoring it.

use std::collections::HashSet;
use std::path::PathBuf;

use serde::Serialize;
use serde_yaml::{Mapping, Value};

use crate::error::{err, Result};
use crate::frontmatter::parse;
use crate::model::{AgentFormat, AgentTarget};

/// A transformed subagent plus every source field or tool the format dropped.
pub(crate) struct Rendered {
    pub text: String,
    pub dropped: Vec<String>,
}

pub(crate) struct SourceAgent {
    name: String,
    name_in_frontmatter: bool,
    description: String,
    fields: Mapping,
    frontmatter: String,
    body: String,
}

/// Parse and validate a subagent source. `stem` is the config name, used as the
/// agent name when the frontmatter has none.
pub(crate) fn parse_source(text: &str, stem: &str) -> Result<SourceAgent> {
    let parsed = parse(text)?;
    let Some(frontmatter) = parsed.frontmatter else {
        return Err(err(
            "agent has no YAML frontmatter (a `description` is required)",
        ));
    };
    let fields = match serde_yaml::from_str::<Value>(&frontmatter) {
        Ok(Value::Mapping(m)) => m,
        Ok(Value::Null) => Mapping::new(),
        Ok(_) => return Err(err("agent frontmatter must be a YAML mapping")),
        Err(e) => return Err(err(format!("invalid agent frontmatter YAML: {e}"))),
    };
    let description = str_field(&fields, "description")
        .ok_or_else(|| err("agent frontmatter needs a non-empty `description`"))?
        .to_string();
    let fm_name = str_field(&fields, "name");
    let name = fm_name.unwrap_or(stem).to_string();
    // Claude Code skips an agent whose name contains `:` without saying so
    if name.contains(':') {
        return Err(err(format!("agent name `{name}` must not contain `:`")));
    }
    if parsed.body.trim().is_empty() {
        return Err(err("agent has an empty prompt body"));
    }
    Ok(SourceAgent {
        name,
        name_in_frontmatter: fm_name.is_some(),
        description,
        fields,
        frontmatter,
        body: parsed.body,
    })
}

fn str_field<'a>(fields: &'a Mapping, key: &str) -> Option<&'a str> {
    fields
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// How a format carries the agent name.
#[derive(Clone, Copy)]
enum NameRule {
    Keep,
    /// Lowercase slug, for loaders that reject anything else (Gemini CLI, Junie, Trae).
    Slug,
    /// Identity comes from the filename (OpenCode).
    Omit,
}

/// How a format carries `model`. Claude aliases (`sonnet`, `opus`) mean nothing to
/// most tools, and an unknown model can fail the first prompt, so only formats that
/// resolve them keep the value.
#[derive(Clone, Copy)]
enum ModelRule {
    /// Keep the value; `inherit` is omitted, since these tools would look it up as a
    /// model name and omitting already means the parent's model.
    Keep,
    /// Keep only `inherit`, which the tool documents.
    InheritOnly,
    Drop,
}

#[derive(Clone, Copy)]
enum ToolMap {
    /// Claude tool names pass through (the tool aliases them itself).
    Any,
    /// Claude tool names the tool shares verbatim.
    Only(&'static [&'static str]),
    /// Claude tool name -> the tool's own name.
    Table(&'static [(&'static str, &'static str)]),
}

#[derive(Clone, Copy)]
enum ListStyle {
    List,
    Comma,
}

struct Tools {
    allow: &'static str,
    /// Key for `disallowedTools`, when the tool has a denylist with the same
    /// vocabulary as its allowlist.
    deny: Option<&'static str>,
    map: ToolMap,
    style: ListStyle,
    /// Whether `mcp__server__tool` entries pass through unchanged.
    mcp: bool,
}

struct Spec {
    name: NameRule,
    model: ModelRule,
    color: bool,
    tools: Option<Tools>,
    /// Source keys copied verbatim because the tool shares their meaning.
    copy: &'static [&'static str],
    inject: &'static [(&'static str, &'static str)],
    /// Key that receives a true `background` / `is_background`.
    background: Option<&'static str>,
}

const GEMINI_TOOLS: &[(&str, &str)] = &[
    ("Read", "read_file"),
    ("Write", "write_file"),
    ("Edit", "replace"),
    ("MultiEdit", "replace"),
    ("Grep", "grep_search"),
    ("Glob", "glob"),
    ("LS", "list_directory"),
    ("Bash", "run_shell_command"),
    ("WebFetch", "web_fetch"),
    ("WebSearch", "google_web_search"),
    ("TodoWrite", "write_todos"),
    ("Skill", "activate_skill"),
    ("AskUserQuestion", "ask_user"),
];

const AUGMENT_TOOLS: &[(&str, &str)] = &[
    ("Read", "read"),
    ("Bash", "bash"),
    ("Edit", "edit"),
    ("MultiEdit", "edit"),
    ("Write", "write"),
    ("Grep", "grep"),
    ("Glob", "find"),
    ("LS", "ls"),
    ("WebFetch", "web-fetch"),
];

// `LS` is left out: Cline's only directory-listing alias resolves to its shell tool
const CLINE_TOOLS: &[(&str, &str)] = &[
    ("Read", "read_files"),
    ("Grep", "search_codebase"),
    ("Glob", "search_codebase"),
    ("Bash", "run_commands"),
    ("Edit", "editor"),
    ("MultiEdit", "editor"),
    ("Write", "editor"),
    ("WebFetch", "fetch_web_content"),
    ("Skill", "skills"),
    ("AskUserQuestion", "ask_question"),
];

const KIRO_TOOLS: &[(&str, &str)] = &[
    ("Read", "read"),
    ("Grep", "read"),
    ("Glob", "read"),
    ("LS", "read"),
    ("Edit", "write"),
    ("MultiEdit", "write"),
    ("Write", "write"),
    ("NotebookEdit", "write"),
    ("Bash", "shell"),
    ("WebFetch", "web"),
    ("WebSearch", "web"),
];

// Only the documented tool names: OpenHands fails to build an agent naming an
// unknown tool
const OPENHANDS_TOOLS: &[(&str, &str)] = &[
    ("Read", "file_editor"),
    ("Edit", "file_editor"),
    ("MultiEdit", "file_editor"),
    ("Write", "file_editor"),
    ("Bash", "terminal"),
    ("TodoWrite", "task_tracker"),
];

const JUNIE_TOOLS: &[&str] = &[
    "Read",
    "Bash",
    "Glob",
    "Grep",
    "Write",
    "Edit",
    "WebSearch",
    "WebFetch",
    "AskUserQuestion",
];

const TRAE_TOOLS: &[&str] = &[
    "Bash",
    "Edit",
    "Glob",
    "Grep",
    "Read",
    "Skill",
    "TodoWrite",
    "WebFetch",
    "WebSearch",
    "Write",
    "LSP",
];

const ZCODE_TOOLS: &[&str] = &[
    "Read",
    "Grep",
    "Glob",
    "Bash",
    "Edit",
    "Write",
    "WebFetch",
    "WebSearch",
    "TodoWrite",
];

const fn plain(name: NameRule, model: ModelRule) -> Spec {
    Spec {
        name,
        model,
        color: false,
        tools: None,
        copy: &[],
        inject: &[],
        background: None,
    }
}

const fn tools(map: ToolMap, deny: Option<&'static str>, style: ListStyle, mcp: bool) -> Tools {
    Tools {
        allow: "tools",
        deny,
        map,
        style,
        mcp,
    }
}

fn spec(format: AgentFormat) -> Spec {
    use ListStyle::{Comma, List};
    use ModelRule::{Drop, InheritOnly, Keep};
    use NameRule::{Keep as KeepName, Omit, Slug};
    match format {
        AgentFormat::ClaudeCode | AgentFormat::Goose | AgentFormat::CodexToml => {
            plain(KeepName, Drop)
        }
        AgentFormat::Cursor => Spec {
            copy: &["readonly"],
            background: Some("is_background"),
            ..plain(KeepName, InheritOnly)
        },
        AgentFormat::Copilot => Spec {
            tools: Some(tools(ToolMap::Any, None, List, false)),
            ..plain(KeepName, Drop)
        },
        AgentFormat::Gemini => Spec {
            tools: Some(tools(ToolMap::Table(GEMINI_TOOLS), None, List, false)),
            ..plain(Slug, InheritOnly)
        },
        AgentFormat::OpenCode => Spec {
            inject: &[("mode", "subagent")],
            ..plain(Omit, Drop)
        },
        AgentFormat::Augment => Spec {
            color: true,
            tools: Some(tools(
                ToolMap::Table(AUGMENT_TOOLS),
                Some("disabled_tools"),
                List,
                false,
            )),
            ..plain(KeepName, Keep)
        },
        AgentFormat::Cline => Spec {
            tools: Some(tools(ToolMap::Table(CLINE_TOOLS), None, List, false)),
            ..plain(KeepName, Drop)
        },
        AgentFormat::Junie => Spec {
            tools: Some(tools(
                ToolMap::Only(JUNIE_TOOLS),
                Some("disallowedTools"),
                List,
                false,
            )),
            copy: &["maxTurns", "skills"],
            ..plain(Slug, Keep)
        },
        AgentFormat::Kiro => Spec {
            tools: Some(tools(ToolMap::Table(KIRO_TOOLS), None, List, false)),
            ..plain(KeepName, Drop)
        },
        AgentFormat::OpenHands => Spec {
            tools: Some(tools(ToolMap::Table(OPENHANDS_TOOLS), None, List, false)),
            ..plain(KeepName, Drop)
        },
        AgentFormat::Trae => Spec {
            tools: Some(tools(
                ToolMap::Only(TRAE_TOOLS),
                Some("disallowedTools"),
                Comma,
                true,
            )),
            ..plain(Slug, Drop)
        },
        AgentFormat::ZCode => Spec {
            tools: Some(tools(
                ToolMap::Only(ZCODE_TOOLS),
                Some("disallowedTools"),
                List,
                true,
            )),
            ..plain(KeepName, InheritOnly)
        },
    }
}

fn ext(format: AgentFormat) -> &'static str {
    match format {
        AgentFormat::CodexToml => "toml",
        AgentFormat::Copilot => "agent.md",
        AgentFormat::Cline => "yaml",
        _ => "md",
    }
}

pub(crate) fn destination_path(target: &AgentTarget, name: &str) -> PathBuf {
    target.path.join(format!("{name}.{}", ext(target.format)))
}

pub(crate) fn render(text: &str, stem: &str, format: AgentFormat) -> Result<Rendered> {
    let agent = parse_source(text, stem)?;
    match format {
        AgentFormat::ClaudeCode => Ok(Rendered {
            text: render_claude(&agent)?,
            dropped: Vec::new(),
        }),
        AgentFormat::CodexToml => render_codex(&agent),
        _ => render_frontmatter(&agent, &spec(format)),
    }
}

/// The source verbatim, with `name` injected when the frontmatter lacks it:
/// Claude Code skips an agent without a name instead of using the filename.
fn render_claude(agent: &SourceAgent) -> Result<String> {
    let mut fm = String::new();
    if !agent.name_in_frontmatter {
        fm.push_str(&yaml_entry("name", Value::String(agent.name.clone()))?);
    }
    fm.push_str(&agent.frontmatter);
    Ok(format!(
        "---\n{}\n---\n{}",
        fm.trim_end_matches('\n'),
        agent.body
    ))
}

fn yaml_entry(key: &str, value: Value) -> Result<String> {
    let mut m = Mapping::new();
    m.insert(Value::String(key.into()), value);
    serde_yaml::to_string(&m).map_err(|e| err(format!("failed to serialize agent: {e}")))
}

#[derive(Serialize)]
struct CodexAgent<'a> {
    name: &'a str,
    description: &'a str,
    developer_instructions: &'a str,
}

fn render_codex(agent: &SourceAgent) -> Result<Rendered> {
    let text = toml::to_string(&CodexAgent {
        name: &agent.name,
        description: &agent.description,
        developer_instructions: agent.body.trim(),
    })
    .map_err(|e| err(format!("failed to serialize Codex agent: {e}")))?;
    let carried = ["name", "description", "model"];
    let mut dropped = Vec::new();
    if str_field(&agent.fields, "model").is_some_and(|m| m != "inherit") {
        dropped.push("model".to_string());
    }
    push_uncarried(&agent.fields, &carried, &mut dropped);
    Ok(Rendered { text, dropped })
}

fn render_frontmatter(agent: &SourceAgent, spec: &Spec) -> Result<Rendered> {
    let fields = &agent.fields;
    let mut out = Mapping::new();
    let mut carried: Vec<&str> = vec!["name", "description"];
    let mut dropped = Vec::new();

    match spec.name {
        NameRule::Keep => insert(&mut out, "name", Value::String(agent.name.clone())),
        NameRule::Slug => {
            let slug = slug(&agent.name);
            if slug != agent.name {
                dropped.push(format!("name (written as `{slug}`)"));
            }
            insert(&mut out, "name", Value::String(slug));
        }
        NameRule::Omit => {}
    }
    insert(
        &mut out,
        "description",
        Value::String(agent.description.clone()),
    );
    for (k, v) in spec.inject {
        insert(&mut out, k, Value::String((*v).to_string()));
    }

    if let Some(model) = str_field(fields, "model") {
        carried.push("model");
        let inherit = model == "inherit";
        match spec.model {
            ModelRule::Keep if !inherit => insert(&mut out, "model", Value::String(model.into())),
            ModelRule::InheritOnly if inherit => {
                insert(&mut out, "model", Value::String(model.into()))
            }
            ModelRule::Keep => {}
            ModelRule::InheritOnly | ModelRule::Drop if !inherit => dropped.push("model".into()),
            ModelRule::InheritOnly | ModelRule::Drop => {}
        }
    }

    if spec.color {
        if let Some(color) = fields.get("color") {
            carried.push("color");
            insert(&mut out, "color", color.clone());
        }
    }

    if let Some(key) = spec.background {
        for src in ["background", "is_background"] {
            if let Some(v) = fields.get(src) {
                carried.push(src);
                if v.as_bool() == Some(true) {
                    insert(&mut out, key, Value::Bool(true));
                }
            }
        }
    }

    if let Some(t) = &spec.tools {
        for (src, dst) in [("tools", Some(t.allow)), ("disallowedTools", t.deny)] {
            let (Some(value), Some(dst)) = (fields.get(src), dst) else {
                continue;
            };
            carried.push(src);
            let names = tool_names(value);
            let (mapped, lost) = map_tools(&names, t);
            dropped.extend(lost.into_iter().map(|n| format!("{src}: {n}")));
            if mapped.is_empty() {
                if src == "tools" && !names.is_empty() {
                    dropped.push(
                        "tools (nothing left to restrict to, so the agent gets every tool)".into(),
                    );
                }
                continue;
            }
            let list = match t.style {
                ListStyle::List => Value::Sequence(mapped.into_iter().map(Value::String).collect()),
                ListStyle::Comma => Value::String(mapped.join(", ")),
            };
            insert(&mut out, dst, list);
        }
    }

    for key in spec.copy {
        if let Some(v) = fields.get(*key) {
            carried.push(key);
            insert(&mut out, key, v.clone());
        }
    }

    push_uncarried(fields, &carried, &mut dropped);
    let yaml =
        serde_yaml::to_string(&out).map_err(|e| err(format!("failed to serialize agent: {e}")))?;
    Ok(Rendered {
        text: format!("---\n{yaml}---\n{}", agent.body),
        dropped,
    })
}

fn insert(out: &mut Mapping, key: &str, value: Value) {
    out.insert(Value::String(key.into()), value);
}

/// Record every remaining source key with a non-empty value as dropped.
fn push_uncarried(fields: &Mapping, carried: &[&str], dropped: &mut Vec<String>) {
    for (k, v) in fields {
        let Some(k) = k.as_str() else { continue };
        let empty = match v {
            Value::Null => true,
            Value::String(s) => s.trim().is_empty(),
            Value::Sequence(s) => s.is_empty(),
            Value::Mapping(m) => m.is_empty(),
            _ => false,
        };
        if !empty && !carried.contains(&k) {
            dropped.push(k.to_string());
        }
    }
}

/// `tools` as Claude Code accepts it: a comma-separated string or a YAML list.
fn tool_names(value: &Value) -> Vec<String> {
    match value {
        Value::String(s) => s
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        Value::Sequence(items) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

/// Map Claude tool names into the format's vocabulary. Returns (mapped, unmapped).
/// An argument form such as `Bash(git *)` maps by its base name and is reported as
/// lost too, since dropping the argument widens a scoped allowlist to the whole tool.
fn map_tools(names: &[String], t: &Tools) -> (Vec<String>, Vec<String>) {
    let mut mapped: Vec<String> = Vec::new();
    let mut lost = Vec::new();
    let mut seen = HashSet::new();
    for raw in names {
        let base = raw.split('(').next().unwrap_or(raw).trim();
        let out = if base.starts_with("mcp__") {
            t.mcp.then(|| base.to_string())
        } else {
            match t.map {
                ToolMap::Any => Some(base.to_string()),
                ToolMap::Only(list) => list.contains(&base).then(|| base.to_string()),
                ToolMap::Table(table) => table
                    .iter()
                    .find(|(claude, _)| *claude == base)
                    .map(|(_, own)| (*own).to_string()),
            }
        };
        match out {
            Some(name) => {
                if base != raw.trim() {
                    lost.push(format!(
                        "{raw} (written as `{name}`, its argument restriction is lost)"
                    ));
                }
                if seen.insert(name.clone()) {
                    mapped.push(name);
                }
            }
            None => lost.push(raw.clone()),
        }
    }
    (mapped, lost)
}

/// Lowercase, `[a-z0-9-]` only, no leading or trailing hyphen.
fn slug(name: &str) -> String {
    let lowered: String = name
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = lowered.trim_matches('-');
    if trimmed.is_empty() {
        "agent".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // One source exercising every mapped and dropped field
    const RICH: &str = "---\nname: Code_Reviewer\ndescription: Reviews diffs for bugs\ntools: Read, Grep, Glob, Bash(git *), WebSearch, mcp__github__get_pr\ndisallowedTools: Write\nmodel: sonnet\ncolor: blue\nbackground: true\nmaxTurns: 5\n---\nYou review code.\n";

    // pstack's comment-sicko frontmatter, the minimal shape
    const MINIMAL: &str = "---\nname: comment-sicko\ndescription: A deranged comment-hater that savors deletion and condemns workaround code.\n---\n# Comment Sicko\n\nI hate comments.\n";

    fn golden(format: AgentFormat, src: &str, text: &str, dropped: &[&str]) {
        let r = render(src, "stem", format).unwrap();
        assert_eq!(r.text, text, "text for {format:?}");
        assert_eq!(r.dropped, dropped, "dropped for {format:?}");
    }

    #[test]
    fn claude_code_is_verbatim() {
        golden(AgentFormat::ClaudeCode, RICH, RICH, &[]);
    }

    #[test]
    fn claude_code_injects_missing_name() {
        golden(
            AgentFormat::ClaudeCode,
            "---\ndescription: d\n---\nbody\n",
            "---\nname: stem\ndescription: d\n---\nbody\n",
            &[],
        );
    }

    #[test]
    fn cursor_keeps_background_and_drops_tools() {
        golden(
            AgentFormat::Cursor,
            RICH,
            "---\nname: Code_Reviewer\ndescription: Reviews diffs for bugs\nis_background: true\n---\nYou review code.\n",
            &["model", "tools", "disallowedTools", "color", "maxTurns"],
        );
    }

    #[test]
    fn cursor_carries_pstack_is_background() {
        golden(
            AgentFormat::Cursor,
            "---\nname: poteto-agent\ndescription: Delegate\nis_background: true\n---\nbody\n",
            "---\nname: poteto-agent\ndescription: Delegate\nis_background: true\n---\nbody\n",
            &[],
        );
    }

    #[test]
    fn copilot_passes_claude_tool_names() {
        golden(
            AgentFormat::Copilot,
            RICH,
            "---\nname: Code_Reviewer\ndescription: Reviews diffs for bugs\ntools:\n- Read\n- Grep\n- Glob\n- Bash\n- WebSearch\n---\nYou review code.\n",
            &["model", "tools: Bash(git *) (written as `Bash`, its argument restriction is lost)", "tools: mcp__github__get_pr", "disallowedTools", "color", "background", "maxTurns"],
        );
    }

    #[test]
    fn codex_moves_body_into_developer_instructions() {
        let r = render(RICH, "stem", AgentFormat::CodexToml).unwrap();
        let parsed: toml::Table = toml::from_str(&r.text).unwrap();
        assert_eq!(parsed["name"].as_str(), Some("Code_Reviewer"));
        assert_eq!(
            parsed["description"].as_str(),
            Some("Reviews diffs for bugs")
        );
        assert_eq!(
            parsed["developer_instructions"].as_str(),
            Some("You review code.")
        );
        assert_eq!(parsed.len(), 3);
        assert_eq!(
            r.dropped,
            [
                "model",
                "tools",
                "disallowedTools",
                "color",
                "background",
                "maxTurns"
            ]
        );
    }

    #[test]
    fn codex_body_with_triple_quotes_round_trips() {
        let src = "---\ndescription: d\n---\nSay \"\"\" and ''' freely.\nLine two.\n";
        let r = render(src, "q", AgentFormat::CodexToml).unwrap();
        let parsed: toml::Table = toml::from_str(&r.text).unwrap();
        assert_eq!(
            parsed["developer_instructions"].as_str(),
            Some("Say \"\"\" and ''' freely.\nLine two.")
        );
    }

    #[test]
    fn gemini_slugs_name_and_maps_tools() {
        golden(
            AgentFormat::Gemini,
            RICH,
            "---\nname: code-reviewer\ndescription: Reviews diffs for bugs\ntools:\n- read_file\n- grep_search\n- glob\n- run_shell_command\n- google_web_search\n---\nYou review code.\n",
            &["name (written as `code-reviewer`)", "model", "tools: Bash(git *) (written as `run_shell_command`, its argument restriction is lost)", "tools: mcp__github__get_pr", "disallowedTools", "color", "background", "maxTurns"],
        );
    }

    #[test]
    fn opencode_marks_subagent_mode() {
        golden(
            AgentFormat::OpenCode,
            MINIMAL,
            "---\ndescription: A deranged comment-hater that savors deletion and condemns workaround code.\nmode: subagent\n---\n# Comment Sicko\n\nI hate comments.\n",
            &[],
        );
    }

    #[test]
    fn augment_keeps_color_model_and_maps_both_lists() {
        golden(
            AgentFormat::Augment,
            RICH,
            "---\nname: Code_Reviewer\ndescription: Reviews diffs for bugs\nmodel: sonnet\ncolor: blue\ntools:\n- read\n- grep\n- find\n- bash\ndisabled_tools:\n- write\n---\nYou review code.\n",
            &["tools: Bash(git *) (written as `bash`, its argument restriction is lost)", "tools: WebSearch", "tools: mcp__github__get_pr", "background", "maxTurns"],
        );
    }

    #[test]
    fn cline_maps_to_cline_ids() {
        golden(
            AgentFormat::Cline,
            RICH,
            "---\nname: Code_Reviewer\ndescription: Reviews diffs for bugs\ntools:\n- read_files\n- search_codebase\n- run_commands\n---\nYou review code.\n",
            &["model", "tools: Bash(git *) (written as `run_commands`, its argument restriction is lost)", "tools: WebSearch", "tools: mcp__github__get_pr", "disallowedTools", "color", "background", "maxTurns"],
        );
    }

    #[test]
    fn goose_keeps_name_and_description_only() {
        golden(
            AgentFormat::Goose,
            RICH,
            "---\nname: Code_Reviewer\ndescription: Reviews diffs for bugs\n---\nYou review code.\n",
            &["model", "tools", "disallowedTools", "color", "background", "maxTurns"],
        );
    }

    #[test]
    fn junie_shares_claude_tool_names() {
        golden(
            AgentFormat::Junie,
            RICH,
            "---\nname: code-reviewer\ndescription: Reviews diffs for bugs\nmodel: sonnet\ntools:\n- Read\n- Grep\n- Glob\n- Bash\n- WebSearch\ndisallowedTools:\n- Write\nmaxTurns: 5\n---\nYou review code.\n",
            &["name (written as `code-reviewer`)", "tools: Bash(git *) (written as `Bash`, its argument restriction is lost)", "tools: mcp__github__get_pr", "color", "background"],
        );
    }

    #[test]
    fn kiro_collapses_tools_into_categories() {
        golden(
            AgentFormat::Kiro,
            RICH,
            "---\nname: Code_Reviewer\ndescription: Reviews diffs for bugs\ntools:\n- read\n- shell\n- web\n---\nYou review code.\n",
            &["model", "tools: Bash(git *) (written as `shell`, its argument restriction is lost)", "tools: mcp__github__get_pr", "disallowedTools", "color", "background", "maxTurns"],
        );
    }

    #[test]
    fn openhands_emits_only_documented_tools() {
        golden(
            AgentFormat::OpenHands,
            RICH,
            "---\nname: Code_Reviewer\ndescription: Reviews diffs for bugs\ntools:\n- file_editor\n- terminal\n---\nYou review code.\n",
            &["model", "tools: Grep", "tools: Glob", "tools: Bash(git *) (written as `terminal`, its argument restriction is lost)", "tools: WebSearch", "tools: mcp__github__get_pr", "disallowedTools", "color", "background", "maxTurns"],
        );
    }

    #[test]
    fn trae_writes_comma_strings_and_keeps_mcp() {
        golden(
            AgentFormat::Trae,
            RICH,
            "---\nname: code-reviewer\ndescription: Reviews diffs for bugs\ntools: Read, Grep, Glob, Bash, WebSearch, mcp__github__get_pr\ndisallowedTools: Write\n---\nYou review code.\n",
            &["name (written as `code-reviewer`)", "model", "tools: Bash(git *) (written as `Bash`, its argument restriction is lost)", "color", "background", "maxTurns"],
        );
    }

    #[test]
    fn zcode_keeps_inherit_and_tool_lists() {
        let src = RICH.replace("model: sonnet", "model: inherit");
        golden(
            AgentFormat::ZCode,
            &src,
            "---\nname: Code_Reviewer\ndescription: Reviews diffs for bugs\nmodel: inherit\ntools:\n- Read\n- Grep\n- Glob\n- Bash\n- WebSearch\n- mcp__github__get_pr\ndisallowedTools:\n- Write\n---\nYou review code.\n",
            &["tools: Bash(git *) (written as `Bash`, its argument restriction is lost)", "color", "background", "maxTurns"],
        );
    }

    #[test]
    fn unmappable_tools_warn_that_the_restriction_is_gone() {
        let src = "---\ndescription: d\ntools: WebSearch\n---\nbody\n";
        let r = render(src, "s", AgentFormat::Cline).unwrap();
        assert!(!r.text.contains("tools"));
        assert_eq!(
            r.dropped,
            [
                "tools: WebSearch",
                "tools (nothing left to restrict to, so the agent gets every tool)"
            ]
        );
    }

    #[test]
    fn model_inherit_is_never_reported_as_dropped() {
        let src = "---\ndescription: d\nmodel: inherit\n---\nbody\n";
        for format in [AgentFormat::Goose, AgentFormat::Augment, AgentFormat::Junie] {
            let r = render(src, "s", format).unwrap();
            assert!(r.dropped.is_empty(), "{format:?}");
            assert!(!r.text.contains("model"), "{format:?}");
        }
    }

    #[test]
    fn source_validation_rejects_unusable_agents() {
        assert!(parse_source("no frontmatter\n", "s").is_err());
        assert!(parse_source("---\nname: x\n---\nbody\n", "s").is_err());
        assert!(parse_source("---\ndescription: d\n---\n  \n", "s").is_err());
        assert!(parse_source("---\nname: a:b\ndescription: d\n---\nbody\n", "s").is_err());
        assert!(parse_source("---\n- list\n---\nbody\n", "s").is_err());
    }

    #[test]
    fn destination_uses_format_extension() {
        let t = |format| AgentTarget {
            path: PathBuf::from("/x"),
            format,
        };
        assert_eq!(
            destination_path(&t(AgentFormat::Copilot), "r"),
            PathBuf::from("/x/r.agent.md")
        );
        assert_eq!(
            destination_path(&t(AgentFormat::CodexToml), "r"),
            PathBuf::from("/x/r.toml")
        );
        assert_eq!(
            destination_path(&t(AgentFormat::Cline), "r"),
            PathBuf::from("/x/r.yaml")
        );
        assert_eq!(
            destination_path(&t(AgentFormat::Gemini), "r"),
            PathBuf::from("/x/r.md")
        );
    }

    #[test]
    fn slug_normalizes() {
        assert_eq!(slug("Code_Reviewer"), "code-reviewer");
        assert_eq!(slug("-x-"), "x");
        assert_eq!(slug("__"), "agent");
    }
}
