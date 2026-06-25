//! Scanner for `${KST_…}` secret placeholders in synced asset values.
//!
//! Two forms: the chain form `${KST_NAME}` (resolved against env + credential
//! files) and the tagged form `${KST:<source>:<ref>}` (routed to an external
//! secret manager — `op` / `vault`). Only the `KST_` / `KST:` sentinel is
//! recognised — bare `${VAR}` that an agent or shell must expand at
//! server-launch time is passed through untouched. Hand-rolled to avoid a
//! `regex` dependency.

use crate::error::{err, Result};

/// A parsed `${KST…}` placeholder.
#[derive(Debug, PartialEq)]
pub(crate) struct SecretRef {
    /// Flat, env-style key including the `KST_` prefix (e.g. `KST_VERCEL__TOKEN`).
    pub flat_key: String,
    /// Nested lookup path from splitting the post-`KST_` name on `__`.
    pub segments: Vec<String>,
    /// Explicit source tag for the `${KST:<tag>:<ref>}` form (`op`, `vault`).
    pub tag: Option<String>,
    /// Source-specific reference for the tagged form (the `<ref>` after the tag).
    pub payload: String,
}

impl SecretRef {
    /// Placeholder label for diagnostics — a locator, never a resolved value.
    pub(crate) fn display(&self) -> String {
        match &self.tag {
            Some(t) => format!("${{KST:{t}:{}}}", self.payload),
            None => format!("${{{}}}", self.flat_key),
        }
    }
}

/// Whether `s` contains at least one `${KST` sentinel (cheap pre-check).
pub(crate) fn has_placeholder(s: &str) -> bool {
    s.contains("${KST")
}

/// Substitute every `${KST_…}` placeholder in `input`. `lookup` returns
/// `Some(value)` to replace, `None` to leave the placeholder literal. Non-`KST`
/// `${…}` and malformed sentinels are passed through untouched.
pub(crate) fn substitute<F>(input: &str, mut lookup: F) -> Result<String>
where
    F: FnMut(&SecretRef) -> Result<Option<String>>,
{
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = rest.find("${KST") {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + "${KST".len()..]; // text after the sentinel prefix
        let kind = after.chars().next();
        // A valid placeholder continues with `_` (chain) or `:` (tagged) and has
        // a closing brace. Anything else is copied verbatim.
        match (kind, after.find('}')) {
            (Some('_') | Some(':'), Some(close)) => {
                let inner = &after[..close];
                let r = parse_ref(inner)?;
                match lookup(&r)? {
                    Some(v) => out.push_str(&v),
                    None => {
                        out.push_str("${KST");
                        out.push_str(inner);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            _ => {
                out.push_str("${KST");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    Ok(out)
}

/// Parse the inner text after `${KST` (before `}`): `_VERCEL__TOKEN` (chain) or
/// `:vault:secret/path#field` / `:op://vault/item/field` (tagged).
fn parse_ref(inner: &str) -> Result<SecretRef> {
    if let Some(tagged) = inner.strip_prefix(':') {
        let (tag, payload) = tagged.split_once(':').ok_or_else(|| {
            err(format!(
                "tagged secret `${{KST:{tagged}}}` must be `${{KST:<source>:<ref>}}`"
            ))
        })?;
        if tag.is_empty() || payload.is_empty() {
            return Err(err("tagged secret needs a non-empty source and ref"));
        }
        return Ok(SecretRef {
            flat_key: String::new(),
            segments: Vec::new(),
            tag: Some(tag.to_string()),
            payload: payload.to_string(),
        });
    }
    let name = inner.strip_prefix('_').unwrap_or(inner);
    if name.is_empty() {
        return Err(err("empty secret placeholder `${KST_}`"));
    }
    Ok(SecretRef {
        flat_key: format!("KST_{name}"),
        segments: name.split("__").map(str::to_string).collect(),
        tag: None,
        payload: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lookup that resolves any chain placeholder to its flat key, uppercased
    /// and wrapped in `<>` so substitutions are visible in assertions.
    fn echo(r: &SecretRef) -> Result<Option<String>> {
        Ok(Some(format!("<{}>", r.flat_key)))
    }

    #[test]
    fn substitutes_a_bare_placeholder() {
        let out = substitute("Bearer ${KST_VERCEL_TOKEN}", echo).unwrap();
        assert_eq!(out, "Bearer <KST_VERCEL_TOKEN>");
    }

    #[test]
    fn parses_double_underscore_nesting() {
        let r = parse_ref("_VERCEL__TOKEN").unwrap();
        assert_eq!(r.flat_key, "KST_VERCEL__TOKEN");
        assert_eq!(r.segments, vec!["VERCEL", "TOKEN"]);
    }

    #[test]
    fn single_underscore_is_one_segment() {
        let r = parse_ref("_VERCEL_TOKEN").unwrap();
        assert_eq!(r.segments, vec!["VERCEL_TOKEN"]);
    }

    #[test]
    fn substitutes_multiple_in_one_string() {
        let out = substitute("${KST_A}:${KST_B}", echo).unwrap();
        assert_eq!(out, "<KST_A>:<KST_B>");
    }

    #[test]
    fn leaves_non_kst_placeholders_untouched() {
        let out = substitute("${HOME}/bin and ${PATH}", echo).unwrap();
        assert_eq!(out, "${HOME}/bin and ${PATH}");
    }

    #[test]
    fn leaves_lookalike_sentinel_untouched() {
        // `${KSTUFF}` is not a placeholder (no `_`/`:` after `KST`).
        let out = substitute("${KSTUFF}", echo).unwrap();
        assert_eq!(out, "${KSTUFF}");
    }

    #[test]
    fn leaves_unterminated_sentinel_literal() {
        let out = substitute("trailing ${KST_NOPE", echo).unwrap();
        assert_eq!(out, "trailing ${KST_NOPE");
    }

    #[test]
    fn missing_leaves_placeholder_when_lookup_returns_none() {
        let out = substitute("x ${KST_GONE} y", |_| Ok(None)).unwrap();
        assert_eq!(out, "x ${KST_GONE} y");
    }

    #[test]
    fn tagged_vault_form_parses_tag_and_payload() {
        let r = parse_ref(":vault:secret/data/path#field").unwrap();
        assert_eq!(r.tag.as_deref(), Some("vault"));
        assert_eq!(r.payload, "secret/data/path#field");
    }

    #[test]
    fn tagged_op_uri_keeps_full_payload() {
        let r = parse_ref(":op://Private/GitHub/token").unwrap();
        assert_eq!(r.tag.as_deref(), Some("op"));
        assert_eq!(r.payload, "//Private/GitHub/token");
    }

    #[test]
    fn tagged_form_without_ref_errors() {
        assert!(parse_ref(":vault").is_err());
        assert!(parse_ref(":vault:").is_err());
    }
}
