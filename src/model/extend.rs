//! YAML-level merge for `extends` config inheritance.
//!
//! Top-level scalar fields (`destination`, `scope`, `agent`) replace.
//! `skills` and `mcps` lists merge by identity tuple — same identity
//! replaces, otherwise appends.

use serde_yaml::{Mapping, Value};

/// Strip and return the `extends` field from a config Value.
/// Accepts a single string or a sequence of strings.
pub(crate) fn extract_extends(v: &mut Value) -> Vec<String> {
    let Value::Mapping(map) = v else {
        return Vec::new();
    };
    let Some(raw) = map.remove("extends") else {
        return Vec::new();
    };
    match raw {
        Value::String(s) => vec![s],
        Value::Sequence(seq) => seq
            .into_iter()
            .filter_map(|item| match item {
                Value::String(s) => Some(s),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Merge `overlay` on top of `base`. Both should be top-level config mappings.
/// Returns `overlay` unchanged if either side is not a mapping.
pub(crate) fn merge_yaml(base: Value, overlay: Value) -> Value {
    let Value::Mapping(mut out) = base else {
        return overlay;
    };
    let overlay_map = match overlay {
        Value::Mapping(m) => m,
        other => return other,
    };
    for (key, ov_val) in overlay_map {
        let key_str = key.as_str().unwrap_or("");
        let is_source_list = matches!(key_str, "skills" | "mcps" | "commands" | "rules");
        match (is_source_list.then(|| out.remove(&key)).flatten(), ov_val) {
            (Some(Value::Sequence(base_seq)), Value::Sequence(ov_seq)) => {
                out.insert(key, Value::Sequence(merge_source_list(base_seq, ov_seq)));
            }
            (_, ov) => {
                out.insert(key, ov);
            }
        }
    }
    Value::Mapping(out)
}

/// Identity-aware merge for skills/mcps lists.
/// Identity = `(source, ref|branch|"", sub_dir|"")`.
/// Same-identity entries are replaced wholesale by overlay; new entries appended.
fn merge_source_list(base: Vec<Value>, overlay: Vec<Value>) -> Vec<Value> {
    let mut out: Vec<Value> = base;
    for ov in overlay {
        let ov_id = identity_of(&ov);
        if let Some(pos) = out.iter().position(|b| identity_of(b) == ov_id) {
            out[pos] = ov;
        } else {
            out.push(ov);
        }
    }
    out
}

fn identity_of(entry: &Value) -> (String, String, String) {
    let Value::Mapping(m) = entry else {
        return (String::new(), String::new(), String::new());
    };
    let source = string_field(m, "source").unwrap_or_default();
    let pin = string_field(m, "ref")
        .or_else(|| string_field(m, "branch"))
        .unwrap_or_default();
    let sub_dir = string_field(m, "sub-dir")
        .or_else(|| string_field(m, "sub_dir"))
        .unwrap_or_default();
    (source, pin, sub_dir)
}

fn string_field(m: &Mapping, key: &str) -> Option<String> {
    m.get(key).and_then(Value::as_str).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yaml(s: &str) -> Value {
        serde_yaml::from_str(s).expect("parse yaml")
    }

    #[test]
    fn extract_extends_string() {
        let mut v = yaml("extends: ../base.yaml\nskills: []\n");
        assert_eq!(extract_extends(&mut v), vec!["../base.yaml".to_string()]);
        // Field is removed from the value.
        assert!(
            matches!(&v, Value::Mapping(m) if !m.contains_key(Value::String("extends".into())))
        );
    }

    #[test]
    fn extract_extends_list() {
        let mut v = yaml("extends:\n  - a.yaml\n  - https://x/b.yaml\nskills: []\n");
        assert_eq!(extract_extends(&mut v), vec!["a.yaml", "https://x/b.yaml"]);
    }

    #[test]
    fn extract_extends_absent() {
        let mut v = yaml("skills: []\n");
        assert!(extract_extends(&mut v).is_empty());
    }

    #[test]
    fn merge_replaces_scalars() {
        let base = yaml("scope: global\nagent: cursor\nskills: []\n");
        let overlay = yaml("scope: project\nskills: []\n");
        let merged = merge_yaml(base, overlay);
        assert_eq!(merged.get("scope").and_then(Value::as_str), Some("project"));
        assert_eq!(merged.get("agent").and_then(Value::as_str), Some("cursor"));
    }

    #[test]
    fn merge_appends_distinct_skills_sources() {
        let base = yaml("skills:\n  - source: https://x/a\n    skills: \"*\"\n");
        let overlay = yaml("skills:\n  - source: https://x/b\n    skills: \"*\"\n");
        let merged = merge_yaml(base, overlay);
        let Value::Sequence(seq) = merged.get("skills").unwrap() else {
            panic!("expected sequence")
        };
        assert_eq!(seq.len(), 2);
    }

    #[test]
    fn merge_overrides_same_identity() {
        let base = yaml("skills:\n  - source: https://x/a\n    ref: v1\n    skills: \"*\"\n");
        let overlay =
            yaml("skills:\n  - source: https://x/a\n    ref: v1\n    skills:\n      - one\n");
        let merged = merge_yaml(base, overlay);
        let Value::Sequence(seq) = merged.get("skills").unwrap() else {
            panic!("expected sequence")
        };
        assert_eq!(seq.len(), 1);
        // overlay's narrower skills list wins
        assert!(matches!(seq[0].get("skills").unwrap(), Value::Sequence(_)));
    }

    #[test]
    fn merge_keeps_distinct_refs_as_separate_entries() {
        let base = yaml("skills:\n  - source: https://x/a\n    ref: v1\n    skills: \"*\"\n");
        let overlay = yaml("skills:\n  - source: https://x/a\n    ref: v2\n    skills: \"*\"\n");
        let merged = merge_yaml(base, overlay);
        let Value::Sequence(seq) = merged.get("skills").unwrap() else {
            panic!("expected sequence")
        };
        assert_eq!(seq.len(), 2);
    }

    #[test]
    fn merge_keeps_distinct_sub_dirs_as_separate_entries() {
        let base =
            yaml("skills:\n  - source: https://x/a\n    sub-dir: pack-a\n    skills: \"*\"\n");
        let overlay =
            yaml("skills:\n  - source: https://x/a\n    sub-dir: pack-b\n    skills: \"*\"\n");
        let merged = merge_yaml(base, overlay);
        let Value::Sequence(seq) = merged.get("skills").unwrap() else {
            panic!("expected sequence")
        };
        assert_eq!(seq.len(), 2);
    }

    #[test]
    fn merge_mcps_uses_same_rules() {
        let base = yaml("mcps:\n  - source: https://x/a\n    ref: v1\n    mcps: \"*\"\n");
        let overlay =
            yaml("mcps:\n  - source: https://x/a\n    ref: v1\n    mcps:\n      - github\n");
        let merged = merge_yaml(base, overlay);
        let Value::Sequence(seq) = merged.get("mcps").unwrap() else {
            panic!("expected sequence")
        };
        assert_eq!(seq.len(), 1);
    }

    #[test]
    fn merge_commands_uses_same_rules() {
        let base = yaml("commands:\n  - source: https://x/a\n    ref: v1\n    commands: \"*\"\n");
        let overlay = yaml(
            "commands:\n  - source: https://x/a\n    ref: v1\n    commands:\n      - review\n",
        );
        let merged = merge_yaml(base, overlay);
        let Value::Sequence(seq) = merged.get("commands").unwrap() else {
            panic!("expected sequence")
        };
        assert_eq!(seq.len(), 1);
    }

    #[test]
    fn merge_rules_uses_same_rules() {
        let base = yaml("rules:\n  - source: https://x/a\n    ref: v1\n    rules: \"*\"\n");
        let overlay =
            yaml("rules:\n  - source: https://x/a\n    ref: v1\n    rules:\n      - style\n");
        let merged = merge_yaml(base, overlay);
        let Value::Sequence(seq) = merged.get("rules").unwrap() else {
            panic!("expected sequence")
        };
        assert_eq!(seq.len(), 1);
    }

    #[test]
    fn merge_rules_appends_distinct_sources() {
        let base = yaml("rules:\n  - source: https://x/a\n    rules: \"*\"\n");
        let overlay = yaml("rules:\n  - source: https://x/b\n    rules: \"*\"\n");
        let merged = merge_yaml(base, overlay);
        let Value::Sequence(seq) = merged.get("rules").unwrap() else {
            panic!("expected sequence")
        };
        assert_eq!(seq.len(), 2);
    }

    #[test]
    fn merge_commands_appends_distinct_sources() {
        let base = yaml("commands:\n  - source: https://x/a\n    commands: \"*\"\n");
        let overlay = yaml("commands:\n  - source: https://x/b\n    commands: \"*\"\n");
        let merged = merge_yaml(base, overlay);
        let Value::Sequence(seq) = merged.get("commands").unwrap() else {
            panic!("expected sequence")
        };
        assert_eq!(seq.len(), 2);
    }

    #[test]
    fn merge_keeps_base_only_keys() {
        let base = yaml("destination: ./skills\nskills: []\n");
        let overlay = yaml("scope: project\nskills: []\n");
        let merged = merge_yaml(base, overlay);
        assert_eq!(
            merged.get("destination").and_then(Value::as_str),
            Some("./skills")
        );
        assert_eq!(merged.get("scope").and_then(Value::as_str), Some("project"));
    }
}
