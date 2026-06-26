//! Scanner for `${kst_…}` secret placeholders in synced asset values.
//!
//! Two forms: the chain form `${kst_name}` (resolved against env + credential
//! files) and the tagged form `${kst:<source>:<ref>}` routed to one explicit
//! source — `env`, `crd` (credentials.yaml), `op`, or `vault`. Only the
//! lowercase `kst_` / `kst:` sentinel is recognised — bare `${VAR}` that an
//! agent or shell must expand at server-launch time is passed through
//! untouched. Hand-rolled to avoid a `regex` dependency.

use crate::error::{err, Result};

/// The literal placeholder prefix every secret reference opens with.
const SENTINEL: &str = "${kst";

/// A parsed `${kst…}` placeholder.
#[derive(Debug, PartialEq)]
pub(crate) struct SecretRef {
    /// Flat, env-style key including the `kst_` prefix (e.g. `kst_vercel__token`).
    /// Only set for the chain form; empty for tagged refs.
    pub flat_key: String,
    /// Nested lookup path from splitting the post-`kst_` name on `__` (chain form).
    pub segments: Vec<String>,
    /// Explicit source tag for `${kst:<tag>:<ref>}` (`env`, `crd`, `op`, `vault`).
    pub tag: Option<String>,
    /// Source-specific reference for the tagged form (the `<ref>` after the tag).
    pub payload: String,
}

impl SecretRef {
    /// Placeholder label for diagnostics — a locator, never a resolved value.
    pub(crate) fn display(&self) -> String {
        match &self.tag {
            Some(t) => format!("${{kst:{t}:{}}}", self.payload),
            None => format!("${{{}}}", self.flat_key),
        }
    }
}

/// Whether `s` contains at least one `${kst` sentinel (cheap pre-check).
pub(crate) fn has_placeholder(s: &str) -> bool {
    s.contains(SENTINEL)
}

/// Substitute every `${kst…}` placeholder in `input`. `lookup` returns
/// `Some(value)` to replace, `None` to leave the placeholder literal. Non-`kst`
/// `${…}` and malformed sentinels are passed through untouched.
pub(crate) fn substitute<F>(input: &str, mut lookup: F) -> Result<String>
where
    F: FnMut(&SecretRef) -> Result<Option<String>>,
{
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = rest.find(SENTINEL) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + SENTINEL.len()..]; // text after the sentinel prefix
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
                        out.push_str(SENTINEL);
                        out.push_str(inner);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            _ => {
                out.push_str(SENTINEL);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    Ok(out)
}

/// Parse the inner text after `${kst` (before `}`): `_vercel__token` (chain) or
/// `:env:NAME` / `:crd:vercel/token` / `:vault:secret/path#field` /
/// `:op://vault/item/field` (tagged).
fn parse_ref(inner: &str) -> Result<SecretRef> {
    if let Some(tagged) = inner.strip_prefix(':') {
        let (tag, payload) = tagged.split_once(':').ok_or_else(|| {
            err(format!(
                "tagged secret `${{kst:{tagged}}}` must be `${{kst:<source>:<ref>}}`"
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
        return Err(err("empty secret placeholder `${kst_}`"));
    }
    Ok(SecretRef {
        flat_key: format!("kst_{name}"),
        segments: name.split("__").map(str::to_string).collect(),
        tag: None,
        payload: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lookup that echoes a chain placeholder's flat key wrapped in `<>` so
    /// substitutions are visible in assertions.
    fn echo(r: &SecretRef) -> Result<Option<String>> {
        Ok(Some(format!("<{}>", r.flat_key)))
    }

    #[test]
    fn substitutes_a_bare_placeholder() {
        let out = substitute("Bearer ${kst_vercel_token}", echo).unwrap();
        assert_eq!(out, "Bearer <kst_vercel_token>");
    }

    #[test]
    fn parses_double_underscore_nesting() {
        let r = parse_ref("_vercel__token").unwrap();
        assert_eq!(r.flat_key, "kst_vercel__token");
        assert_eq!(r.segments, vec!["vercel", "token"]);
    }

    #[test]
    fn single_underscore_is_one_segment() {
        let r = parse_ref("_vercel_token").unwrap();
        assert_eq!(r.segments, vec!["vercel_token"]);
    }

    #[test]
    fn substitutes_multiple_in_one_string() {
        let out = substitute("${kst_a}:${kst_b}", echo).unwrap();
        assert_eq!(out, "<kst_a>:<kst_b>");
    }

    #[test]
    fn leaves_non_kst_placeholders_untouched() {
        let out = substitute("${HOME}/bin and ${PATH}", echo).unwrap();
        assert_eq!(out, "${HOME}/bin and ${PATH}");
    }

    #[test]
    fn leaves_uppercase_sentinel_untouched() {
        // The sentinel is strictly lowercase; `${KST_FOO}` is a foreign var.
        let out = substitute("${KST_FOO}", echo).unwrap();
        assert_eq!(out, "${KST_FOO}");
    }

    #[test]
    fn leaves_lookalike_sentinel_untouched() {
        // `${kstuff}` is not a placeholder (no `_`/`:` after `kst`).
        let out = substitute("${kstuff}", echo).unwrap();
        assert_eq!(out, "${kstuff}");
    }

    #[test]
    fn leaves_unterminated_sentinel_literal() {
        let out = substitute("trailing ${kst_nope", echo).unwrap();
        assert_eq!(out, "trailing ${kst_nope");
    }

    #[test]
    fn missing_leaves_placeholder_when_lookup_returns_none() {
        let out = substitute("x ${kst_gone} y", |_| Ok(None)).unwrap();
        assert_eq!(out, "x ${kst_gone} y");
    }

    #[test]
    fn tagged_env_form_parses_tag_and_payload() {
        let r = parse_ref(":env:VERCEL_TOKEN").unwrap();
        assert_eq!(r.tag.as_deref(), Some("env"));
        assert_eq!(r.payload, "VERCEL_TOKEN");
    }

    #[test]
    fn tagged_crd_form_keeps_slash_path() {
        let r = parse_ref(":crd:vercel/token").unwrap();
        assert_eq!(r.tag.as_deref(), Some("crd"));
        assert_eq!(r.payload, "vercel/token");
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
