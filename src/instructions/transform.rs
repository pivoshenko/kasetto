use std::path::{Path, PathBuf};

use crate::frontmatter::Parsed;
use crate::model::{InstructionFormat, InstructionTarget};

/// Returns the on-disk relative filename for an instruction in a per-instruction directory
/// format. Namespaced names are flattened with `-`. Not used for aggregate
/// formats, where the destination is the shared file in `InstructionTarget.path`.
fn derive_relpath(name: &str, format: InstructionFormat) -> PathBuf {
    let flat = name.replace(':', "-");
    match format {
        InstructionFormat::CursorMdc => PathBuf::from(format!("{flat}.mdc")),
        InstructionFormat::PlainMarkdownDir => PathBuf::from(format!("{flat}.md")),
        // Aggregate formats have no per-instruction relpath; callers must not ask.
        InstructionFormat::AggregateMarkdown => PathBuf::from(format!("{flat}.md")),
    }
}

/// Render the content an instruction contributes for the given format.
///
/// For dir formats this is the full file body. For aggregate formats this is
/// the block content that [`upsert_block`] wraps in managed markers.
pub(crate) fn render(parsed: &Parsed, format: InstructionFormat) -> String {
    match format {
        InstructionFormat::CursorMdc => render_cursor_mdc(parsed),
        // Body only — frontmatter stripped. globs/alwaysApply have no meaning
        // for agents that don't scope instructions, so they are dropped here.
        InstructionFormat::PlainMarkdownDir | InstructionFormat::AggregateMarkdown => {
            parsed.body.clone()
        }
    }
}

/// Cursor MDC: reconstruct the `description` / `globs` / `alwaysApply`
/// frontmatter from whatever the source carried, then the body.
fn render_cursor_mdc(parsed: &Parsed) -> String {
    let meta = InstructionMeta::from(parsed.frontmatter.as_deref());
    let mut fm: Vec<String> = Vec::new();
    if let Some(d) = &meta.description {
        fm.push(format!("description: {d}"));
    }
    if let Some(g) = &meta.globs {
        fm.push(format!("globs: {g}"));
    }
    if let Some(a) = meta.always_apply {
        fm.push(format!("alwaysApply: {a}"));
    }
    if fm.is_empty() {
        return parsed.body.clone();
    }
    format!("---\n{}\n---\n{}", fm.join("\n"), parsed.body)
}

/// The Cursor-relevant fields extracted from an instruction's source frontmatter.
struct InstructionMeta {
    description: Option<String>,
    /// Stored as the rendered string (a CSV when the source used a list).
    globs: Option<String>,
    always_apply: Option<bool>,
}

impl InstructionMeta {
    fn from(frontmatter: Option<&str>) -> Self {
        let mut out = InstructionMeta {
            description: None,
            globs: None,
            always_apply: None,
        };
        let Some(fm) = frontmatter else { return out };
        let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(fm) else {
            return out;
        };
        out.description = value
            .get("description")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        out.always_apply = value.get("alwaysApply").and_then(|v| v.as_bool());
        out.globs = value.get("globs").and_then(globs_to_string);
        out
    }
}

/// Render a `globs` YAML value as a string Cursor accepts: a scalar passes
/// through; a sequence is joined into a comma-separated list.
fn globs_to_string(v: &serde_yaml::Value) -> Option<String> {
    match v {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Sequence(seq) => {
            let parts: Vec<String> = seq
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join(", "))
            }
        }
        _ => None,
    }
}

/// Stable, markdown-comment-safe identity for an instruction's managed block in a shared
/// file. Keyed on `(source, name)` so two sources contributing a same-named instruction
/// (e.g. `style`) get distinct blocks. Recomputable at removal time from the
/// lock entry's `source` + `name`.
pub(crate) fn block_id(source: &str, name: &str) -> String {
    let digest = crate::fsops::hash_str(&format!("{source}::{name}"));
    format!("{name}-{}", &digest[..8])
}

fn markers(id: &str) -> (String, String) {
    (
        format!("<!-- kasetto:instruction:{id} START -->"),
        format!("<!-- kasetto:instruction:{id} END -->"),
    )
}

/// Locate the managed block: the first `start` marker, then the first `end`
/// marker that follows it. Returns `(block_start, block_end_exclusive)`.
///
/// Scoping the END search to *after* START is what keeps this correct — a naive
/// independent `find(end)` would match an earlier block's END (or a stray END in
/// someone's prose) and slice the wrong region. Combined with the content-hashed
/// block id, a collision with a marker embedded in an instruction body is implausible.
fn find_block(existing: &str, start: &str, end: &str) -> Option<(usize, usize)> {
    let s = existing.find(start)?;
    let after_start = s + start.len();
    let e = existing[after_start..].find(end)? + after_start;
    Some((s, e + end.len()))
}

/// Strip a single leading newline (LF or CRLF) so a replaced block doesn't
/// accrete blank lines, tolerant of CRLF files (e.g. a Windows-saved CLAUDE.md).
fn strip_leading_newline(s: &str) -> &str {
    s.strip_prefix("\r\n")
        .or_else(|| s.strip_prefix('\n'))
        .unwrap_or(s)
}

/// Insert or replace the managed block for `id` in `existing`, preserving every
/// byte outside the block (user content and other instructions' blocks).
pub(crate) fn upsert_block(existing: &str, id: &str, content: &str) -> String {
    let (start, end) = markers(id);
    let body = content.trim_matches('\n');
    let block = format!("{start}\n{body}\n{end}\n");

    if let Some((s, region_end)) = find_block(existing, &start, &end) {
        let mut out = String::with_capacity(existing.len());
        out.push_str(&existing[..s]);
        out.push_str(&block);
        out.push_str(strip_leading_newline(&existing[region_end..]));
        return out;
    }

    let mut out = existing.to_string();
    if !out.is_empty() {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }
    out.push_str(&block);
    out
}

/// Remove the managed block for `id` from `existing`, collapsing the surrounding
/// blank lines so the rest of the file stays tidy. A no-op when the block is
/// absent.
pub(crate) fn remove_block(existing: &str, id: &str) -> String {
    let (start, end) = markers(id);
    let Some((s, region_end)) = find_block(existing, &start, &end) else {
        return existing.to_string();
    };
    let before = existing[..s].trim_end_matches(['\r', '\n']);
    let after = existing[region_end..].trim_start_matches(['\r', '\n']);
    match (before.is_empty(), after.is_empty()) {
        (true, true) => String::new(),
        (true, false) => after.to_string(),
        (false, true) => format!("{before}\n"),
        (false, false) => format!("{before}\n\n{after}"),
    }
}

/// Absolute path an instruction is written to for the given target. For aggregate
/// formats this is the shared file; for dir formats it joins the derived name.
pub(crate) fn destination_path(target: &InstructionTarget, name: &str) -> PathBuf {
    if target.format.is_aggregate() {
        target.path.clone()
    } else {
        target.path.join(derive_relpath(name, target.format))
    }
}

/// Whether an instruction's output is already present at the destination: for dir
/// formats the file exists; for aggregate formats the managed block is present
/// in the shared file (so a user-deleted block triggers a reinstall).
pub(crate) fn dest_present(target: &InstructionTarget, name: &str, source: &str) -> bool {
    let dest = destination_path(target, name);
    if !dest.is_file() {
        return false;
    }
    if !target.format.is_aggregate() {
        return true;
    }
    let (start, _) = markers(&block_id(source, name));
    std::fs::read_to_string(&dest)
        .map(|text| text.contains(&start))
        .unwrap_or(false)
}

/// Ensure parent directories of `path` exist.
pub(crate) fn ensure_parent_dirs(path: &Path) -> crate::error::Result<()> {
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
        parse("---\ndescription: house style\nglobs: \"*.rs\"\nalwaysApply: true\n---\nUse tabs.\n")
            .unwrap()
    }

    #[test]
    fn relpaths_flatten_namespaces() {
        assert_eq!(
            derive_relpath("house:style", InstructionFormat::CursorMdc),
            PathBuf::from("house-style.mdc")
        );
        assert_eq!(
            derive_relpath("house:style", InstructionFormat::PlainMarkdownDir),
            PathBuf::from("house-style.md")
        );
    }

    #[test]
    fn cursor_mdc_reconstructs_frontmatter() {
        let rendered = render(&sample(), InstructionFormat::CursorMdc);
        assert!(rendered.starts_with("---\n"));
        assert!(rendered.contains("description: house style"));
        assert!(rendered.contains("globs: *.rs"));
        assert!(rendered.contains("alwaysApply: true"));
        assert!(rendered.contains("Use tabs."));
    }

    #[test]
    fn cursor_mdc_joins_globs_list() {
        let p = parse("---\nglobs:\n  - \"*.ts\"\n  - \"*.tsx\"\n---\nbody\n").unwrap();
        let rendered = render(&p, InstructionFormat::CursorMdc);
        assert!(rendered.contains("globs: *.ts, *.tsx"));
    }

    #[test]
    fn plain_dir_strips_frontmatter() {
        let rendered = render(&sample(), InstructionFormat::PlainMarkdownDir);
        assert!(!rendered.contains("description:"));
        assert!(rendered.contains("Use tabs."));
    }

    #[test]
    fn block_id_differs_per_source() {
        assert_ne!(
            block_id("https://x/a", "style"),
            block_id("https://x/b", "style")
        );
    }

    #[test]
    fn upsert_into_empty_file() {
        let out = upsert_block("", "style-abc", "instruction body");
        assert_eq!(
            out,
            "<!-- kasetto:instruction:style-abc START -->\ninstruction body\n<!-- kasetto:instruction:style-abc END -->\n"
        );
    }

    #[test]
    fn upsert_appends_and_preserves_user_content() {
        let existing = "# CLAUDE.md\n\nMy own notes.\n";
        let out = upsert_block(existing, "style-abc", "instruction body");
        assert!(out.starts_with("# CLAUDE.md\n\nMy own notes.\n"));
        assert!(out.contains("<!-- kasetto:instruction:style-abc START -->"));
        assert!(out.contains("instruction body"));
    }

    #[test]
    fn upsert_replaces_existing_block_only() {
        let existing = upsert_block("user text.\n", "style-abc", "v1");
        let updated = upsert_block(&existing, "style-abc", "v2");
        assert!(updated.contains("user text."));
        assert!(updated.contains("v2"));
        assert!(!updated.contains("v1"));
        // Exactly one block.
        assert_eq!(updated.matches("style-abc START").count(), 1);
    }

    #[test]
    fn remove_block_keeps_surrounding_content() {
        let existing = upsert_block("user text.\n", "style-abc", "instruction body");
        let after = remove_block(&existing, "style-abc");
        assert!(after.contains("user text."));
        assert!(!after.contains("style-abc"));
        assert!(!after.contains("instruction body"));
    }

    #[test]
    fn two_instructions_coexist_in_one_file() {
        let one = upsert_block("", "a-1", "alpha");
        let two = upsert_block(&one, "b-2", "beta");
        assert!(two.contains("alpha"));
        assert!(two.contains("beta"));
        // Removing one leaves the other.
        let only_b = remove_block(&two, "a-1");
        assert!(!only_b.contains("alpha"));
        assert!(only_b.contains("beta"));
    }

    #[test]
    fn updating_first_block_does_not_corrupt_second() {
        // Regression: a naive independent find(END) would match the FIRST END
        // and slice the wrong region when updating the first of two blocks.
        let two = upsert_block(&upsert_block("top.\n", "a-1", "alpha"), "b-2", "beta");
        let updated = upsert_block(&two, "a-1", "alpha2");
        assert!(updated.contains("top."));
        assert!(updated.contains("alpha2"));
        assert!(updated.contains("beta"));
        assert_eq!(updated.matches("a-1 START").count(), 1);
        assert_eq!(updated.matches("b-2 START").count(), 1);
        assert_eq!(updated.matches("b-2 END").count(), 1);
    }

    #[test]
    fn body_containing_a_different_marker_string_is_safe() {
        // An instruction that documents kasetto's marker format (a DIFFERENT id) must not
        // confuse block detection for this id.
        let body = "see <!-- kasetto:instruction:other-xyz END --> for the format";
        let out = upsert_block("user.\n", "style-abc", body);
        // Update this block; the embedded other-id marker must not truncate it.
        let updated = upsert_block(&out, "style-abc", "clean body");
        assert!(updated.contains("clean body"));
        assert!(!updated.contains("see <!--"));
        assert!(updated.contains("user."));
        assert_eq!(updated.matches("style-abc START").count(), 1);
    }

    #[test]
    fn crlf_file_does_not_accrete_carriage_returns_on_update() {
        let existing = "# CLAUDE.md\r\n\r\nUser notes.\r\n";
        let v1 = upsert_block(existing, "style-abc", "v1");
        let v2 = upsert_block(&v1, "style-abc", "v2");
        assert!(v2.contains("User notes."));
        assert!(v2.contains("v2"));
        assert!(!v2.contains("v1"));
        // No stray blank-line / CR accretion across updates.
        assert!(!v2.contains("\r\n\r\n\r\n"));
        assert_eq!(v2.matches("style-abc START").count(), 1);
    }

    #[test]
    fn empty_body_round_trips() {
        let out = upsert_block("", "style-abc", "\n\n");
        assert_eq!(
            out,
            "<!-- kasetto:instruction:style-abc START -->\n\n<!-- kasetto:instruction:style-abc END -->\n"
        );
        let removed = remove_block(&out, "style-abc");
        assert_eq!(removed, "");
    }
}
