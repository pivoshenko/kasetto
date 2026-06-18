use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::frontmatter::Parsed;
use crate::model::{CommandFormat, CommandTarget};

/// Returns the on-disk relative filename for a command, given its name and format.
///
/// Namespaced names with `:` map to nested subdirectories for formats that keep
/// the original Markdown shape. Plain formats flatten namespaces with `-`.
fn derive_relpath(name: &str, format: CommandFormat) -> PathBuf {
    match format {
        CommandFormat::MarkdownFrontmatter => name_to_nested_path(name, "md"),
        CommandFormat::MarkdownPlain => PathBuf::from(format!("{}.md", flatten_name(name))),
        CommandFormat::PromptMd => PathBuf::from(format!("{}.prompt.md", flatten_name(name))),
        CommandFormat::PromptFile => PathBuf::from(format!("{}.prompt", flatten_name(name))),
        CommandFormat::GeminiToml => PathBuf::from(format!("{}.toml", flatten_name(name))),
    }
}

fn name_to_nested_path(name: &str, ext: &str) -> PathBuf {
    let mut parts: Vec<&str> = name.split(':').filter(|p| !p.is_empty()).collect();
    let Some(last) = parts.pop() else {
        return PathBuf::from(format!("command.{ext}"));
    };
    let mut path = PathBuf::new();
    for p in parts {
        path.push(p);
    }
    path.push(format!("{last}.{ext}"));
    path
}

fn flatten_name(name: &str) -> String {
    name.replace(':', "-")
}

/// Render `parsed` to bytes for the given `format`.
pub(crate) fn render(parsed: &Parsed, format: CommandFormat) -> String {
    match format {
        CommandFormat::MarkdownFrontmatter | CommandFormat::PromptMd => {
            if let Some(fm) = &parsed.frontmatter {
                format!("---\n{}\n---\n{}", fm, parsed.body)
            } else {
                parsed.body.clone()
            }
        }
        CommandFormat::MarkdownPlain => parsed.body.clone(),
        CommandFormat::PromptFile => render_prompt_file(parsed),
        CommandFormat::GeminiToml => render_gemini_toml(parsed),
    }
}

fn render_prompt_file(parsed: &Parsed) -> String {
    // Continue Dev `.prompt` files use a YAML preamble between `---` fences with `invokable: true`.
    let body = parsed.body.replace("$ARGUMENTS", "{{{ input }}}");
    let mut preamble: Vec<String> = Vec::new();
    if let Some(fm) = &parsed.frontmatter {
        for line in fm.lines() {
            if line.trim().is_empty() {
                continue;
            }
            preamble.push(line.to_string());
        }
    }
    let has_invokable = preamble
        .iter()
        .any(|line| line.trim_start().starts_with("invokable:"));
    if !has_invokable {
        preamble.push("invokable: true".to_string());
    }
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&preamble.join("\n"));
    out.push('\n');
    out.push_str("---\n");
    out.push_str(&body);
    out
}

fn render_gemini_toml(parsed: &Parsed) -> String {
    let description = parsed.description().unwrap_or_default();
    let body = parsed.body.trim_end_matches('\n').to_string();
    let mut out = String::new();
    if !description.is_empty() {
        out.push_str(&format!("description = {}\n", toml_string(&description)));
    }
    out.push_str("prompt = \"\"\"\n");
    out.push_str(&body);
    out.push_str("\n\"\"\"\n");
    out
}

fn toml_string(s: &str) -> String {
    // Basic TOML string escape — sufficient for description one-liners.
    let escaped = s
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    format!("\"{escaped}\"")
}

/// Resolve where this command should be written under `target.path` and return the absolute path.
pub(crate) fn destination_path(target: &CommandTarget, name: &str) -> PathBuf {
    target.path.join(derive_relpath(name, target.format))
}

/// Ensure parent directories of `path` exist.
pub(crate) fn ensure_parent_dirs(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontmatter::parse;

    fn sample() -> Parsed {
        parse("---\ndescription: do thing\nargument-hint: <n>\n---\nUse $ARGUMENTS here.\n")
            .unwrap()
    }

    #[test]
    fn nested_paths_for_markdown_frontmatter() {
        let p = derive_relpath("git:commit", CommandFormat::MarkdownFrontmatter);
        assert_eq!(p, PathBuf::from("git/commit.md"));
        let p2 = derive_relpath("commit", CommandFormat::MarkdownFrontmatter);
        assert_eq!(p2, PathBuf::from("commit.md"));
    }

    #[test]
    fn flat_names_for_other_formats() {
        assert_eq!(
            derive_relpath("git:commit", CommandFormat::MarkdownPlain),
            PathBuf::from("git-commit.md")
        );
        assert_eq!(
            derive_relpath("git:commit", CommandFormat::PromptMd),
            PathBuf::from("git-commit.prompt.md")
        );
        assert_eq!(
            derive_relpath("git:commit", CommandFormat::PromptFile),
            PathBuf::from("git-commit.prompt")
        );
        assert_eq!(
            derive_relpath("git:commit", CommandFormat::GeminiToml),
            PathBuf::from("git-commit.toml")
        );
    }

    #[test]
    fn markdown_frontmatter_round_trip() {
        let p = sample();
        let rendered = render(&p, CommandFormat::MarkdownFrontmatter);
        assert!(rendered.starts_with("---\n"));
        assert!(rendered.contains("description: do thing"));
        assert!(rendered.contains("Use $ARGUMENTS here."));
    }

    #[test]
    fn markdown_plain_strips_frontmatter() {
        let p = sample();
        let rendered = render(&p, CommandFormat::MarkdownPlain);
        assert!(!rendered.contains("description:"));
        assert!(rendered.contains("Use $ARGUMENTS here."));
    }

    #[test]
    fn prompt_md_preserves_frontmatter() {
        let p = sample();
        let rendered = render(&p, CommandFormat::PromptMd);
        assert!(rendered.starts_with("---\n"));
        assert!(rendered.contains("description: do thing"));
    }

    #[test]
    fn prompt_file_injects_invokable_and_rewrites_arguments() {
        let p = sample();
        let rendered = render(&p, CommandFormat::PromptFile);
        assert!(rendered.contains("invokable: true"));
        assert!(rendered.contains("{{{ input }}}"));
        assert!(!rendered.contains("$ARGUMENTS"));
    }

    #[test]
    fn prompt_file_does_not_double_invokable() {
        let parsed = parse("---\ninvokable: false\n---\nx\n").unwrap();
        let rendered = render(&parsed, CommandFormat::PromptFile);
        let count = rendered.matches("invokable:").count();
        assert_eq!(count, 1);
        assert!(rendered.contains("invokable: false"));
    }

    #[test]
    fn gemini_toml_emits_description_and_prompt() {
        let p = sample();
        let rendered = render(&p, CommandFormat::GeminiToml);
        assert!(rendered.contains("description = \"do thing\""));
        assert!(rendered.contains("prompt = \"\"\""));
        assert!(rendered.contains("Use $ARGUMENTS here."));
    }
}
