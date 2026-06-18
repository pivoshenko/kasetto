//! Rule (CLAUDE.md / .cursor/rules / AGENTS.md …) parsing and per-agent transforms.
//!
//! A rule source is Markdown with optional YAML frontmatter, the same shape as a
//! slash command. The divergence from commands is the destination: some agents
//! take a single shared file that many rules merge into (`AggregateMarkdown`,
//! handled with managed `<!-- kasetto:rule:ID … -->` comment blocks so user
//! hand-edits survive), while others take a directory of one file per rule.

mod transform;

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{err, Result};
use crate::frontmatter::parse;
use crate::fsops::resolve_dest;
use crate::model::RuleTarget;

pub(crate) use transform::{dest_present, destination_path};

/// Read a rule source file, parse it, and write the transformed output to
/// `target`. For dir formats this writes a per-rule file; for aggregate formats
/// it inserts/updates this rule's managed block in the shared file, preserving
/// everything else. `source_url` + `name` key the managed block.
///
/// Returns the absolute path of the file that was written.
pub(crate) fn apply_rule(
    source: &Path,
    target: &RuleTarget,
    source_url: &str,
    name: &str,
) -> Result<PathBuf> {
    let text = fs::read_to_string(source).map_err(|e| {
        err(format!(
            "failed to read rule source {}: {e}",
            source.display()
        ))
    })?;
    let parsed = parse(&text)?;
    let content = transform::render(&parsed, target.format);
    let dest = destination_path(target, name);
    transform::ensure_parent_dirs(&dest)?;

    if target.format.is_aggregate() {
        let existing = fs::read_to_string(&dest).unwrap_or_default();
        let merged =
            transform::upsert_block(&existing, &transform::block_id(source_url, name), &content);
        fs::write(&dest, merged)
            .map_err(|e| err(format!("failed to write rule {}: {e}", dest.display())))?;
    } else {
        fs::write(&dest, content)
            .map_err(|e| err(format!("failed to write rule {}: {e}", dest.display())))?;
    }
    Ok(dest)
}

/// The token stored in the lock `destination` CSV for one written rule file:
/// `agg:<rel>` for a shared aggregate file (teardown strips this rule's block)
/// or `file:<rel>` for a standalone per-rule file (teardown deletes it).
pub(crate) fn dest_token(target: &RuleTarget, rel: &str) -> String {
    if target.format.is_aggregate() {
        format!("agg:{rel}")
    } else {
        format!("file:{rel}")
    }
}

/// Reverse one stored destination token: strip the rule's managed block from a
/// shared aggregate file (never deleting the user-owned file), or delete a
/// standalone per-rule file. `source_url` + `name` recompute the block id.
pub(crate) fn teardown_dest(token: &str, source_url: &str, name: &str, root: &Path) {
    // Only the known `agg:`/`file:` prefixes are stripped — splitting on the
    // first `:` would mangle an absolute Windows destination (`C:\…`), which
    // `relativize_dest` stores verbatim when the dest is outside the scope root.
    if let Some(rel) = token.strip_prefix("agg:") {
        let path = resolve_dest(rel, root);
        if path.is_file() {
            if let Ok(text) = fs::read_to_string(&path) {
                let stripped =
                    transform::remove_block(&text, &transform::block_id(source_url, name));
                let _ = fs::write(&path, stripped);
            }
        }
    } else {
        // `file:<rel>` (or a bare path, for forward-compat) — delete the file.
        let rel = token.strip_prefix("file:").unwrap_or(token);
        let path = resolve_dest(rel, root);
        if path.is_file() {
            let _ = fs::remove_file(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsops::temp_dir;
    use crate::model::RuleFormat;

    #[test]
    fn apply_rule_writes_cursor_mdc_file() {
        let src_dir = temp_dir("kasetto-rule-src");
        fs::create_dir_all(&src_dir).unwrap();
        let src = src_dir.join("style.mdc");
        fs::write(&src, "---\ndescription: hi\nglobs: \"*.rs\"\n---\nbody\n").unwrap();

        let dst_dir = temp_dir("kasetto-rule-dst");
        let target = RuleTarget {
            path: dst_dir.clone(),
            format: RuleFormat::CursorMdc,
        };
        let out = apply_rule(&src, &target, "https://x/a", "style").unwrap();
        assert!(out.ends_with("style.mdc"));
        let text = fs::read_to_string(&out).unwrap();
        assert!(text.contains("description: hi"));
        assert!(text.contains("globs: *.rs"));

        let _ = fs::remove_dir_all(&src_dir);
        let _ = fs::remove_dir_all(&dst_dir);
    }

    #[test]
    fn apply_rule_aggregate_preserves_user_content_then_teardown() {
        let src_dir = temp_dir("kasetto-rule-agg-src");
        fs::create_dir_all(&src_dir).unwrap();
        let src = src_dir.join("style.md");
        fs::write(&src, "---\ndescription: hi\n---\nUse tabs.\n").unwrap();

        let proj = temp_dir("kasetto-rule-agg-proj");
        fs::create_dir_all(&proj).unwrap();
        let claude = proj.join("CLAUDE.md");
        fs::write(&claude, "# Project\n\nUser paragraph.\n").unwrap();

        let target = RuleTarget {
            path: claude.clone(),
            format: RuleFormat::AggregateMarkdown,
        };
        let out = apply_rule(&src, &target, "https://x/a", "style").unwrap();
        assert_eq!(out, claude);
        let text = fs::read_to_string(&claude).unwrap();
        assert!(text.contains("User paragraph."));
        assert!(text.contains("Use tabs."));
        assert!(dest_present(&target, "style", "https://x/a"));

        // Re-apply is idempotent (still one block).
        apply_rule(&src, &target, "https://x/a", "style").unwrap();
        let text2 = fs::read_to_string(&claude).unwrap();
        assert_eq!(text2.matches("kasetto:rule").count(), 2); // START + END

        // Teardown strips the block but keeps the user file + content.
        let token = dest_token(&target, "CLAUDE.md");
        teardown_dest(&token, "https://x/a", "style", &proj);
        let after = fs::read_to_string(&claude).unwrap();
        assert!(after.contains("User paragraph."));
        assert!(!after.contains("Use tabs."));
        assert!(claude.is_file());

        let _ = fs::remove_dir_all(&src_dir);
        let _ = fs::remove_dir_all(&proj);
    }
}
