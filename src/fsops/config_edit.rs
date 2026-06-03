//! Surgical, comment-preserving edits to a local kasetto YAML config.
//!
//! `add` / `remove` rewrite the user's `kasetto.yaml`. A serde round-trip would
//! drop every comment and reorder keys, so instead we edit the raw lines:
//! insert appends a list item under the right section; remove deletes a whole
//! item's line-range, or a named entry's lines within one (pruning the item
//! when its last name goes). Everything else in the file survives byte-for-byte.

use crate::error::{err, Result};

/// The top-level list a source entry lives under.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Section {
    Skills,
    Mcps,
    Commands,
}

impl Section {
    /// Top-level YAML key (also the per-item selector field name).
    pub(crate) fn key(self) -> &'static str {
        match self {
            Section::Skills => "skills",
            Section::Mcps => "mcps",
            Section::Commands => "commands",
        }
    }

    /// Singular noun used in user-facing messages and the matching CLI flag.
    pub(crate) fn singular(self) -> &'static str {
        match self {
            Section::Skills => "skill",
            Section::Mcps => "mcp",
            Section::Commands => "command",
        }
    }
}

/// What a `remove_names` call did to the matched entry.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RemoveOutcome {
    /// No entry for that source in the section.
    NotFound,
    /// Every named entry was removed, so the whole source item was dropped.
    WholeItem,
    /// These names were removed; the source item was kept.
    Names(Vec<String>),
}

/// How a source pins its revision.
#[derive(Clone, Debug)]
pub(crate) enum Pin {
    Ref(String),
    Branch(String),
    None,
}

impl Pin {
    /// The pin value as it appears in the lookup (ref or branch string).
    fn value(&self) -> Option<&str> {
        match self {
            Pin::Ref(r) | Pin::Branch(r) => Some(r.as_str()),
            Pin::None => None,
        }
    }
}

/// Which entries to install from the source: all (`"*"`) or a named list.
#[derive(Clone, Debug)]
pub(crate) enum Selector {
    Wildcard,
    Names(Vec<String>),
}

/// A source entry to insert into a section.
#[derive(Clone, Debug)]
pub(crate) struct SourceItem {
    pub source: String,
    pub pin: Pin,
    pub sub_dir: Option<String>,
    pub selector: Selector,
}

/// One parsed list item within a section, plus the line range it occupies.
struct ParsedItem {
    start: usize,
    end: usize,
    indent: usize,
    source: Option<String>,
    pin: Option<String>,
    sub_dir: Option<String>,
}

/// Insert `item` as the last entry under `section.key()`. Creates the section
/// when absent. Comments and unrelated lines are preserved verbatim.
pub(crate) fn insert_item(text: &str, section: Section, item: &SourceItem) -> Result<String> {
    let mut lines = split_lines(text);
    let key = section.key();

    match find_top_level(&lines, key) {
        Some(idx) => {
            let inline = section_inline_value(&lines[idx], key);
            if let Some(val) = &inline {
                if !val.is_empty() && val != "[]" && val != "{}" {
                    return Err(err(format!(
                        "`{key}:` is an inline list; reformat it as a block list before editing"
                    )));
                }
                // Normalize `key: []` / `key:` into a block header.
                lines[idx] = format!("{key}:");
            }

            let sec_end = next_top_level(&lines, idx + 1);
            let items = parse_items(&lines, idx + 1, sec_end);
            let indent = items.first().map(|it| it.indent).unwrap_or(2);

            // Append after the last item's real content, letting trailing blank
            // lines and floating comments stay below the freshly inserted item.
            let mut insert_at = items.last().map(|it| it.end).unwrap_or(idx + 1);
            while insert_at > idx + 1 {
                let prev = lines[insert_at - 1].trim_start();
                if prev.is_empty() || prev.starts_with('#') {
                    insert_at -= 1;
                } else {
                    break;
                }
            }

            let block = render_item(item, indent, section);
            splice(&mut lines, insert_at, block);
        }
        None => {
            // Append a fresh section at the end of the file.
            if let Some(last) = lines.last() {
                if !last.trim().is_empty() {
                    lines.push(String::new());
                }
            }
            lines.push(format!("{key}:"));
            for l in render_item(item, 2, section) {
                lines.push(l);
            }
        }
    }

    Ok(join_lines(lines, text))
}

/// Find the single section item matching `source` (and `pin`, when given).
/// `Ok(None)` for no match; `Err` when ambiguous (same source, differing pins).
fn find_match<'a>(
    items: &'a [ParsedItem],
    source: &str,
    pin: Option<&str>,
    key: &str,
) -> Result<Option<&'a ParsedItem>> {
    let matches: Vec<&ParsedItem> = items
        .iter()
        .filter(|it| it.source.as_deref() == Some(source))
        .filter(|it| match pin {
            Some(p) => it.pin.as_deref() == Some(p),
            None => true,
        })
        .collect();
    match matches.len() {
        0 => Ok(None),
        1 => Ok(Some(matches[0])),
        _ => Err(err(format!(
            "multiple `{source}` entries in `{key}:`; pass --ref or --branch to disambiguate"
        ))),
    }
}

/// Remove the whole source entry matching `source` (and `pin`, when given)
/// from `section`. Returns `(new_text, removed)`. Errors when the match is
/// ambiguous (multiple entries with the same source but different pins).
pub(crate) fn remove_item(
    text: &str,
    section: Section,
    source: &str,
    pin: Option<&str>,
) -> Result<(String, bool)> {
    let mut lines = split_lines(text);
    let key = section.key();

    let Some(idx) = find_top_level(&lines, key) else {
        return Ok((text.to_string(), false));
    };
    let sec_end = next_top_level(&lines, idx + 1);
    let items = parse_items(&lines, idx + 1, sec_end);

    match find_match(&items, source, pin, key)? {
        None => Ok((text.to_string(), false)),
        Some(it) => {
            let (start, end) = (it.start, it.end);
            lines.drain(start..end);
            Ok((join_lines(lines, text), true))
        }
    }
}

/// Remove specific `names` from the source entry's selector list in `section`.
/// When the last name goes, the whole entry is dropped (`WholeItem`). Errors if
/// the entry is a wildcard (`skills: "*"` — nothing to subtract), if it uses
/// object-form (`{name, path}`) entries (edit those by hand), or if any
/// requested name is absent.
pub(crate) fn remove_names(
    text: &str,
    section: Section,
    source: &str,
    pin: Option<&str>,
    names: &[String],
) -> Result<(String, RemoveOutcome)> {
    let mut lines = split_lines(text);
    let key = section.key();
    let singular = section.singular();

    let Some(idx) = find_top_level(&lines, key) else {
        return Ok((text.to_string(), RemoveOutcome::NotFound));
    };
    let sec_end = next_top_level(&lines, idx + 1);
    let items = parse_items(&lines, idx + 1, sec_end);
    let Some(it) = find_match(&items, source, pin, key)? else {
        return Ok((text.to_string(), RemoveOutcome::NotFound));
    };
    let (istart, iend) = (it.start, it.end);

    // Locate the selector field line (`skills:` / `mcps:` / `commands:`).
    let mut field = None;
    #[allow(clippy::needless_range_loop)]
    for i in istart..iend {
        if let Some((k, v)) = field_kv(&lines[i]) {
            if k == key {
                field = Some((i, v, indent_of(&lines[i])));
                break;
            }
        }
    }
    let Some((fidx, fval, findent)) = field else {
        return Err(err(format!(
            "`{source}` entry in `{key}:` has no `{key}:` list"
        )));
    };
    if !fval.is_empty() {
        return Err(err(format!(
            "`{source}` uses a wildcard in `{key}:`; remove the whole entry with `--{singular} \"*\"`"
        )));
    }

    // Collect the scalar name lines under the selector field.
    let mut name_lines: Vec<(usize, String)> = Vec::new();
    let mut has_object = false;
    #[allow(clippy::needless_range_loop)]
    for i in fidx + 1..iend {
        let t = lines[i].trim_start();
        if t.is_empty() {
            continue;
        }
        if indent_of(&lines[i]) <= findent {
            break;
        }
        if let Some(rest) = t.strip_prefix("- ") {
            let body = rest.trim();
            if body.contains(':') {
                has_object = true;
            }
            name_lines.push((i, body.trim_matches('"').trim_matches('\'').to_string()));
        }
    }
    if has_object {
        return Err(err(format!(
            "`{source}` `{key}:` list has object-form entries; edit the config directly to remove a name"
        )));
    }

    let have: Vec<&str> = name_lines.iter().map(|(_, n)| n.as_str()).collect();
    let missing: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|n| !have.contains(n))
        .collect();
    if !missing.is_empty() {
        return Err(err(format!(
            "{singular} `{}` not found in `{source}` ({key}: {})",
            missing.join("`, `"),
            have.join(", ")
        )));
    }

    let remaining = name_lines
        .iter()
        .filter(|(_, n)| !names.contains(n))
        .count();
    if remaining == 0 {
        // Removing every name empties the list — drop the whole entry.
        lines.drain(istart..iend);
        return Ok((join_lines(lines, text), RemoveOutcome::WholeItem));
    }

    let mut to_delete: Vec<usize> = name_lines
        .iter()
        .filter(|(_, n)| names.contains(n))
        .map(|(i, _)| *i)
        .collect();
    to_delete.sort_unstable();
    for i in to_delete.into_iter().rev() {
        lines.remove(i);
    }
    Ok((
        join_lines(lines, text),
        RemoveOutcome::Names(names.to_vec()),
    ))
}

/// True when an entry with the exact identity `(source, pin, sub_dir)` already
/// exists in the section. Mirrors the `extends` merge identity.
pub(crate) fn item_exists(text: &str, section: Section, item: &SourceItem) -> bool {
    let lines = split_lines(text);
    let key = section.key();
    let Some(idx) = find_top_level(&lines, key) else {
        return false;
    };
    let sec_end = next_top_level(&lines, idx + 1);
    let want_pin = item.pin.value().unwrap_or("").to_string();
    let want_sub = item.sub_dir.clone().unwrap_or_default();
    parse_items(&lines, idx + 1, sec_end).iter().any(|it| {
        it.source.as_deref() == Some(item.source.as_str())
            && it.pin.clone().unwrap_or_default() == want_pin
            && it.sub_dir.clone().unwrap_or_default() == want_sub
    })
}

fn render_item(item: &SourceItem, indent: usize, section: Section) -> Vec<String> {
    let pad = " ".repeat(indent);
    let field_pad = " ".repeat(indent + 2);
    let mut out = vec![format!("{pad}- source: {}", item.source)];
    match &item.pin {
        Pin::Ref(r) => out.push(format!("{field_pad}ref: {r}")),
        Pin::Branch(b) => out.push(format!("{field_pad}branch: {b}")),
        Pin::None => {}
    }
    if let Some(sub) = &item.sub_dir {
        out.push(format!("{field_pad}sub-dir: {sub}"));
    }
    let field = section.key();
    match &item.selector {
        Selector::Wildcard => out.push(format!("{field_pad}{field}: \"*\"")),
        Selector::Names(names) => {
            out.push(format!("{field_pad}{field}:"));
            for n in names {
                out.push(format!("{field_pad}  - {n}"));
            }
        }
    }
    out
}

// --- line helpers ---------------------------------------------------------

fn split_lines(text: &str) -> Vec<String> {
    text.lines().map(String::from).collect()
}

fn join_lines(lines: Vec<String>, original: &str) -> String {
    let mut out = lines.join("\n");
    if original.ends_with('\n') || original.is_empty() {
        out.push('\n');
    }
    out
}

fn splice(lines: &mut Vec<String>, at: usize, new: Vec<String>) {
    let tail = lines.split_off(at);
    lines.extend(new);
    lines.extend(tail);
}

/// Returns the bare key of a top-level mapping line (column 0, `key:`), if any.
fn is_top_level_key(line: &str) -> Option<&str> {
    if line.starts_with([' ', '\t', '#', '-']) {
        return None;
    }
    let t = line.trim_end();
    if t.is_empty() {
        return None;
    }
    let colon = t.find(':')?;
    let key = &t[..colon];
    if key.is_empty() || key.contains(char::is_whitespace) {
        return None;
    }
    Some(key)
}

fn find_top_level(lines: &[String], key: &str) -> Option<usize> {
    lines.iter().position(|l| is_top_level_key(l) == Some(key))
}

fn next_top_level(lines: &[String], from: usize) -> usize {
    (from..lines.len())
        .find(|&i| is_top_level_key(&lines[i]).is_some())
        .unwrap_or(lines.len())
}

/// The inline value after `key:` on a top-level line (e.g. `[]` for `skills: []`).
fn section_inline_value(line: &str, key: &str) -> Option<String> {
    let t = line.trim_end();
    let rest = t.strip_prefix(key)?.trim_start();
    let rest = rest.strip_prefix(':')?;
    Some(rest.trim().to_string())
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn is_dash(line: &str) -> bool {
    let t = line.trim_start();
    t == "-" || t.starts_with("- ")
}

/// Parse the sequence items directly under a section. Items are the dash lines
/// at the shallowest dash indentation; each spans up to the next sibling item
/// (or the section end). Nested sequences (deeper dashes) stay inside an item.
fn parse_items(lines: &[String], sec_start: usize, sec_end: usize) -> Vec<ParsedItem> {
    let dashes: Vec<usize> = (sec_start..sec_end)
        .filter(|&i| is_dash(&lines[i]))
        .collect();
    if dashes.is_empty() {
        return Vec::new();
    }
    let item_indent = dashes.iter().map(|&i| indent_of(&lines[i])).min().unwrap();
    let starts: Vec<usize> = dashes
        .into_iter()
        .filter(|&i| indent_of(&lines[i]) == item_indent)
        .collect();

    let mut items = Vec::with_capacity(starts.len());
    for (k, &start) in starts.iter().enumerate() {
        let end = starts.get(k + 1).copied().unwrap_or(sec_end);
        let (source, pin, sub_dir) = extract_fields(lines, start, end);
        items.push(ParsedItem {
            start,
            end,
            indent: item_indent,
            source,
            pin,
            sub_dir,
        });
    }
    items
}

fn extract_fields(
    lines: &[String],
    start: usize,
    end: usize,
) -> (Option<String>, Option<String>, Option<String>) {
    let mut source = None;
    let mut pin = None;
    let mut sub_dir = None;
    for line in &lines[start..end] {
        let Some((key, val)) = field_kv(line) else {
            continue;
        };
        match key.as_str() {
            "source" if source.is_none() => source = Some(val),
            "ref" | "branch" if pin.is_none() => pin = Some(val),
            "sub-dir" | "sub_dir" if sub_dir.is_none() => sub_dir = Some(val),
            _ => {}
        }
    }
    (source, pin, sub_dir)
}

/// Parse a `key: value` pair from a line, tolerating a leading `- ` dash and
/// surrounding quotes on the value. Returns `None` for valueless / dashed-list
/// lines like `- alpha` or `skills:`.
fn field_kv(line: &str) -> Option<(String, String)> {
    let mut t = line.trim_start();
    if let Some(rest) = t.strip_prefix("- ") {
        t = rest.trim_start();
    } else if t == "-" {
        return None;
    }
    let colon = t.find(':')?;
    let key = t[..colon].trim().to_string();
    if key.is_empty() || key.contains(char::is_whitespace) {
        return None;
    }
    let val = t[colon + 1..]
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string();
    Some((key, val))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wildcard(source: &str) -> SourceItem {
        SourceItem {
            source: source.to_string(),
            pin: Pin::None,
            sub_dir: None,
            selector: Selector::Wildcard,
        }
    }

    #[test]
    fn insert_appends_under_existing_section_preserving_comments() {
        let text = "# my config\nskills:\n  - source: https://x/a\n    skills: \"*\"\n";
        let out = insert_item(text, Section::Skills, &wildcard("https://x/b")).unwrap();
        assert_eq!(
            out,
            "# my config\n\
             skills:\n\
             \x20 - source: https://x/a\n\
             \x20\x20\x20 skills: \"*\"\n\
             \x20 - source: https://x/b\n\
             \x20\x20\x20 skills: \"*\"\n"
        );
    }

    #[test]
    fn insert_creates_section_when_absent() {
        let text = "agent: claude-code\n";
        let out = insert_item(text, Section::Mcps, &wildcard("https://x/m")).unwrap();
        assert_eq!(
            out,
            "agent: claude-code\n\nmcps:\n  - source: https://x/m\n    mcps: \"*\"\n"
        );
    }

    #[test]
    fn insert_normalizes_inline_empty_list() {
        let text = "skills: []\n";
        let out = insert_item(text, Section::Skills, &wildcard("https://x/a")).unwrap();
        assert_eq!(out, "skills:\n  - source: https://x/a\n    skills: \"*\"\n");
    }

    #[test]
    fn insert_into_empty_file() {
        let out = insert_item("", Section::Commands, &wildcard("https://x/c")).unwrap();
        assert_eq!(
            out,
            "commands:\n  - source: https://x/c\n    commands: \"*\"\n"
        );
    }

    #[test]
    fn insert_with_ref_and_named_list() {
        let item = SourceItem {
            source: "https://x/a".into(),
            pin: Pin::Ref("v2.0".into()),
            sub_dir: Some("pack".into()),
            selector: Selector::Names(vec!["alpha".into(), "beta".into()]),
        };
        let out = insert_item("skills: []\n", Section::Skills, &item).unwrap();
        assert_eq!(
            out,
            "skills:\n\
             \x20 - source: https://x/a\n\
             \x20\x20\x20 ref: v2.0\n\
             \x20\x20\x20 sub-dir: pack\n\
             \x20\x20\x20 skills:\n\
             \x20\x20\x20\x20\x20 - alpha\n\
             \x20\x20\x20\x20\x20 - beta\n"
        );
    }

    #[test]
    fn insert_keeps_trailing_comment_after_new_item() {
        let text = "skills:\n  - source: https://x/a\n    skills: \"*\"\n\n# trailing note\nagent: cursor\n";
        let out = insert_item(text, Section::Skills, &wildcard("https://x/b")).unwrap();
        assert!(out.contains("- source: https://x/b"));
        // The trailing comment + next key remain after the inserted item.
        let b_pos = out.find("https://x/b").unwrap();
        let note_pos = out.find("# trailing note").unwrap();
        assert!(b_pos < note_pos);
        assert!(out.contains("\nagent: cursor\n"));
    }

    #[test]
    fn remove_deletes_only_the_matching_item() {
        let text = "skills:\n  - source: https://x/a\n    skills: \"*\"\n  - source: https://x/b\n    skills: \"*\"\n";
        let (out, removed) = remove_item(text, Section::Skills, "https://x/a", None).unwrap();
        assert!(removed);
        assert_eq!(out, "skills:\n  - source: https://x/b\n    skills: \"*\"\n");
    }

    #[test]
    fn remove_absent_source_is_noop() {
        let text = "skills:\n  - source: https://x/a\n    skills: \"*\"\n";
        let (out, removed) = remove_item(text, Section::Skills, "https://x/z", None).unwrap();
        assert!(!removed);
        assert_eq!(out, text);
    }

    #[test]
    fn remove_ambiguous_without_pin_errors() {
        let text = "skills:\n  - source: https://x/a\n    ref: v1\n    skills: \"*\"\n  - source: https://x/a\n    ref: v2\n    skills: \"*\"\n";
        let err = remove_item(text, Section::Skills, "https://x/a", None).unwrap_err();
        assert!(err.to_string().contains("disambiguate"));
    }

    #[test]
    fn remove_disambiguates_by_pin() {
        let text = "skills:\n  - source: https://x/a\n    ref: v1\n    skills: \"*\"\n  - source: https://x/a\n    ref: v2\n    skills: \"*\"\n";
        let (out, removed) = remove_item(text, Section::Skills, "https://x/a", Some("v1")).unwrap();
        assert!(removed);
        assert!(out.contains("ref: v2"));
        assert!(!out.contains("ref: v1"));
    }

    #[test]
    fn item_exists_matches_full_identity() {
        let text =
            "skills:\n  - source: https://x/a\n    ref: v1\n    sub-dir: pack\n    skills: \"*\"\n";
        let same = SourceItem {
            source: "https://x/a".into(),
            pin: Pin::Ref("v1".into()),
            sub_dir: Some("pack".into()),
            selector: Selector::Wildcard,
        };
        assert!(item_exists(text, Section::Skills, &same));

        let diff_ref = SourceItem {
            pin: Pin::Ref("v2".into()),
            ..same.clone()
        };
        assert!(!item_exists(text, Section::Skills, &diff_ref));
    }

    #[test]
    fn remove_names_subtracts_one_keeps_entry() {
        let text = "skills:\n  - source: https://x/a\n    skills:\n      - alpha\n      - beta\n";
        let (out, outcome) = remove_names(
            text,
            Section::Skills,
            "https://x/a",
            None,
            &["alpha".into()],
        )
        .unwrap();
        assert_eq!(outcome, RemoveOutcome::Names(vec!["alpha".into()]));
        assert_eq!(
            out,
            "skills:\n  - source: https://x/a\n    skills:\n      - beta\n"
        );
    }

    #[test]
    fn remove_names_last_name_drops_whole_entry() {
        let text = "skills:\n  - source: https://x/a\n    skills:\n      - alpha\n";
        let (out, outcome) = remove_names(
            text,
            Section::Skills,
            "https://x/a",
            None,
            &["alpha".into()],
        )
        .unwrap();
        assert_eq!(outcome, RemoveOutcome::WholeItem);
        assert_eq!(out, "skills:\n");
    }

    #[test]
    fn remove_names_missing_name_errors_without_mutating() {
        let text = "skills:\n  - source: https://x/a\n    skills:\n      - alpha\n";
        let err = remove_names(
            text,
            Section::Skills,
            "https://x/a",
            None,
            &["ghost".into()],
        )
        .unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn remove_names_on_wildcard_errors() {
        let text = "skills:\n  - source: https://x/a\n    skills: \"*\"\n";
        let err = remove_names(
            text,
            Section::Skills,
            "https://x/a",
            None,
            &["alpha".into()],
        )
        .unwrap_err();
        assert!(err.to_string().contains("wildcard"));
    }

    #[test]
    fn remove_names_object_form_errors() {
        let text = "skills:\n  - source: https://x/a\n    skills:\n      - name: alpha\n        path: lib\n";
        let err = remove_names(
            text,
            Section::Skills,
            "https://x/a",
            None,
            &["alpha".into()],
        )
        .unwrap_err();
        assert!(err.to_string().contains("object-form"));
    }

    #[test]
    fn remove_names_absent_source_is_not_found() {
        let text = "skills:\n  - source: https://x/a\n    skills:\n      - alpha\n";
        let (out, outcome) = remove_names(
            text,
            Section::Skills,
            "https://x/z",
            None,
            &["alpha".into()],
        )
        .unwrap();
        assert_eq!(outcome, RemoveOutcome::NotFound);
        assert_eq!(out, text);
    }

    #[test]
    fn remove_last_item_leaves_bare_section_header() {
        // Dropping the final entry of a section leaves the section header
        // behind without trailing whitespace; a later `insert_item` can repopulate.
        let text = "skills:\n  - source: https://x/a\n    skills: \"*\"\n";
        let (out, removed) = remove_item(text, Section::Skills, "https://x/a", None).unwrap();
        assert!(removed);
        assert_eq!(out, "skills:\n");
        let reinserted = insert_item(&out, Section::Skills, &wildcard("https://x/b")).unwrap();
        assert_eq!(
            reinserted,
            "skills:\n  - source: https://x/b\n    skills: \"*\"\n"
        );
    }

    #[test]
    fn remove_last_named_item_collapses_then_can_be_reused() {
        // The same invariant for the names path: stripping the last name drops
        // the entry, leaving only the section header.
        let text = "mcps:\n  - source: https://x/a\n    mcps:\n      - foo\n";
        let (out, outcome) =
            remove_names(text, Section::Mcps, "https://x/a", None, &["foo".into()]).unwrap();
        assert_eq!(outcome, RemoveOutcome::WholeItem);
        assert_eq!(out, "mcps:\n");
    }

    #[test]
    fn remove_then_insert_round_trips_indentation() {
        // Four-space-indented list must round-trip without breaking the sequence.
        let text = "skills:\n    - source: https://x/a\n      skills: \"*\"\n";
        let out = insert_item(text, Section::Skills, &wildcard("https://x/b")).unwrap();
        // New item adopts the existing 4-space dash indent.
        assert!(out.contains("\n    - source: https://x/b"));
        assert!(out.contains("\n      skills: \"*\""));
    }
}
