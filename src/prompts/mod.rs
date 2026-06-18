//! Command (slash-command / prompt template) parsing and per-agent transforms.

mod transform;

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{err, Result};
use crate::frontmatter::parse;
use crate::model::CommandTarget;

pub(crate) use transform::destination_path;

/// Read a Markdown command file at `source`, parse it, and write the transformed
/// output into `target.path` under the format-derived relative filename.
///
/// Returns the absolute path of the written file.
pub(crate) fn apply_command(source: &Path, target: &CommandTarget, name: &str) -> Result<PathBuf> {
    let text = fs::read_to_string(source).map_err(|e| {
        err(format!(
            "failed to read command source {}: {e}",
            source.display()
        ))
    })?;
    let parsed = parse(&text)?;
    let rendered = transform::render(&parsed, target.format);
    let dest = transform::destination_path(target, name);
    transform::ensure_parent_dirs(&dest)?;
    fs::write(&dest, rendered)
        .map_err(|e| err(format!("failed to write command {}: {e}", dest.display())))?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsops::temp_dir;
    use crate::model::CommandFormat;

    #[test]
    fn apply_command_writes_nested_markdown() {
        let src_dir = temp_dir("kasetto-pr-src");
        fs::create_dir_all(&src_dir).unwrap();
        let src = src_dir.join("commit.md");
        fs::write(&src, "---\ndescription: hi\n---\nbody\n").unwrap();

        let dst_dir = temp_dir("kasetto-pr-dst");
        let target = CommandTarget {
            path: dst_dir.clone(),
            format: CommandFormat::MarkdownFrontmatter,
        };
        let out = apply_command(&src, &target, "git:commit").unwrap();
        assert!(out.ends_with("git/commit.md"));
        let text = fs::read_to_string(&out).unwrap();
        assert!(text.contains("description: hi"));

        let _ = fs::remove_dir_all(&src_dir);
        let _ = fs::remove_dir_all(&dst_dir);
    }

    #[test]
    fn apply_command_writes_gemini_toml() {
        let src_dir = temp_dir("kasetto-pr-gem");
        fs::create_dir_all(&src_dir).unwrap();
        let src = src_dir.join("deploy.md");
        fs::write(&src, "---\ndescription: ship it\n---\nrun $ARGUMENTS\n").unwrap();

        let dst_dir = temp_dir("kasetto-pr-gem-dst");
        let target = CommandTarget {
            path: dst_dir.clone(),
            format: CommandFormat::GeminiToml,
        };
        let out = apply_command(&src, &target, "deploy").unwrap();
        assert!(out.ends_with("deploy.toml"));
        let text = fs::read_to_string(&out).unwrap();
        assert!(text.contains("description = \"ship it\""));
        assert!(text.contains("prompt = \"\"\""));

        let _ = fs::remove_dir_all(&src_dir);
        let _ = fs::remove_dir_all(&dst_dir);
    }
}
