//! Secret sources: where injected values come from. Resolution walks the
//! sources in precedence order (env first, then credential files) and the first
//! hit wins.

use std::path::Path;

use serde_yaml::Value as Yaml;

use crate::error::{err, Result};

use super::template::SecretRef;
use super::Secret;

pub(super) trait SecretSource {
    /// Short label used in "secret not found (searched: …)" diagnostics.
    fn name(&self) -> &'static str;
    fn get(&self, r: &SecretRef) -> Result<Option<Secret>>;
}

/// Process environment variables, keyed by the flat `KST_…` placeholder name.
pub(super) struct EnvSource;

impl SecretSource for EnvSource {
    fn name(&self) -> &'static str {
        "env"
    }

    fn get(&self, r: &SecretRef) -> Result<Option<Secret>> {
        match std::env::var(&r.flat_key) {
            Ok(v) => Ok(Some(Secret::new(v))),
            Err(_) => Ok(None),
        }
    }
}

/// A parsed `credentials.yaml`, resolved by flat top-level key or nested path.
pub(super) struct CredentialsFileSource {
    root: Yaml,
}

impl CredentialsFileSource {
    /// Load and parse the file. Returns `Ok(None)` when the file does not exist
    /// (an absent credential store is not an error — env may satisfy everything).
    pub(super) fn load(path: &Path) -> Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| err(format!("failed to read {}: {e}", path.display())))?;
        let root: Yaml = serde_yaml::from_str(&text)
            .map_err(|e| err(format!("invalid credentials YAML {}: {e}", path.display())))?;
        Ok(Some(Self { root }))
    }

    #[cfg(test)]
    pub(super) fn from_yaml(root: Yaml) -> Self {
        Self { root }
    }
}

impl SecretSource for CredentialsFileSource {
    fn name(&self) -> &'static str {
        "credentials.yaml"
    }

    fn get(&self, r: &SecretRef) -> Result<Option<Secret>> {
        let Yaml::Mapping(map) = &self.root else {
            return Ok(None);
        };
        // 1) Flat top-level key, e.g. `KST_GITHUB_TOKEN: "…"`.
        if let Some(v) = map.get(r.flat_key.as_str()).and_then(Yaml::as_str) {
            return Ok(Some(Secret::new(v.to_string())));
        }
        // 2) Nested path, matching each `__`-separated segment case-insensitively.
        let mut cur = &self.root;
        for seg in &r.segments {
            let Yaml::Mapping(m) = cur else {
                return Ok(None);
            };
            let next = m
                .iter()
                .find(|(k, _)| k.as_str().is_some_and(|s| s.eq_ignore_ascii_case(seg)));
            match next {
                Some((_, v)) => cur = v,
                None => return Ok(None),
            }
        }
        Ok(cur.as_str().map(|s| Secret::new(s.to_string())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::template::SecretRef;

    fn ref_for(inner: &str) -> SecretRef {
        let name = inner.strip_prefix("KST_").unwrap_or(inner);
        SecretRef {
            flat_key: inner.to_string(),
            segments: name.split("__").map(str::to_string).collect(),
            tag: None,
        }
    }

    #[test]
    fn credentials_flat_key() {
        let yaml: Yaml = serde_yaml::from_str("KST_GITHUB_TOKEN: ghp_abc\n").unwrap();
        let src = CredentialsFileSource::from_yaml(yaml);
        let got = src.get(&ref_for("KST_GITHUB_TOKEN")).unwrap().unwrap();
        assert_eq!(got.expose(), "ghp_abc");
    }

    #[test]
    fn credentials_nested_case_insensitive() {
        let yaml: Yaml = serde_yaml::from_str("vercel:\n  token: tok_123\n").unwrap();
        let src = CredentialsFileSource::from_yaml(yaml);
        let got = src.get(&ref_for("KST_VERCEL__TOKEN")).unwrap().unwrap();
        assert_eq!(got.expose(), "tok_123");
    }

    #[test]
    fn credentials_missing_returns_none() {
        let yaml: Yaml = serde_yaml::from_str("other: x\n").unwrap();
        let src = CredentialsFileSource::from_yaml(yaml);
        assert!(src.get(&ref_for("KST_VERCEL__TOKEN")).unwrap().is_none());
    }
}
