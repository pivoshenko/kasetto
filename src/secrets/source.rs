//! Secret sources: where injected values come from. Resolution walks the
//! sources, skipping any that don't `handle` the ref, and the first hit wins.
//! Chain refs (`${KST_NAME}`) go to env + credential files (env first); tagged
//! refs (`${KST:op:…}` / `${KST:vault:…}`) go to the matching external manager.

use std::path::Path;
use std::process::Command;

use serde_yaml::Value as Yaml;

use crate::error::{err, Result};

use super::template::SecretRef;
use super::Secret;

pub(super) trait SecretSource {
    /// Short label used in "secret not found (searched: …)" diagnostics.
    fn name(&self) -> &'static str;
    /// Whether this source is responsible for `r` (by ref form / tag).
    fn handles(&self, r: &SecretRef) -> bool;
    fn get(&self, r: &SecretRef) -> Result<Option<Secret>>;
}

/// Process environment variables, keyed by the flat `KST_…` placeholder name.
pub(super) struct EnvSource;

impl SecretSource for EnvSource {
    fn name(&self) -> &'static str {
        "env"
    }

    fn handles(&self, r: &SecretRef) -> bool {
        r.tag.is_none()
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

    fn handles(&self, r: &SecretRef) -> bool {
        r.tag.is_none()
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

/// 1Password, via the `op` CLI. Tagged form `${KST:op:<vault>/<item>/<field>}`
/// (or a full `${KST:op://<vault>/<item>/<field>}` URI) → `op read op://…`.
pub(super) struct OnePasswordSource;

impl SecretSource for OnePasswordSource {
    fn name(&self) -> &'static str {
        "1password (op)"
    }

    fn handles(&self, r: &SecretRef) -> bool {
        r.tag.as_deref() == Some("op")
    }

    fn get(&self, r: &SecretRef) -> Result<Option<Secret>> {
        let uri = op_uri(&r.payload);
        let value = run_cli("op", &["read", "--no-newline", &uri])?;
        Ok(Some(Secret::new(value)))
    }
}

/// HashiCorp Vault, via the `vault` CLI. Tagged form
/// `${KST:vault:<kv-path>#<field>}` → `vault kv get -field=<field> <kv-path>`.
pub(super) struct VaultSource;

impl SecretSource for VaultSource {
    fn name(&self) -> &'static str {
        "vault"
    }

    fn handles(&self, r: &SecretRef) -> bool {
        r.tag.as_deref() == Some("vault")
    }

    fn get(&self, r: &SecretRef) -> Result<Option<Secret>> {
        let (path, field) = vault_path_field(&r.payload)?;
        let value = run_cli("vault", &["kv", "get", &format!("-field={field}"), path])?;
        Ok(Some(Secret::new(value)))
    }
}

/// Build the canonical `op://vault/item/field` URI from a tagged payload, which
/// may already carry the `//` (from `${KST:op://…}`) or omit it (`${KST:op:…}`).
fn op_uri(payload: &str) -> String {
    match payload.strip_prefix("//") {
        Some(rest) => format!("op://{rest}"),
        None => format!("op://{payload}"),
    }
}

/// Split a Vault payload `"<kv-path>#<field>"` into its path and field parts.
fn vault_path_field(payload: &str) -> Result<(&str, &str)> {
    match payload.split_once('#') {
        Some((path, field)) if !path.is_empty() && !field.is_empty() => Ok((path, field)),
        _ => Err(err(format!(
            "vault secret `{payload}` must be `<kv-path>#<field>` (e.g. secret/myapp#token)"
        ))),
    }
}

/// Run a secret-manager CLI and return its trimmed stdout. The resolved value
/// is captured, never echoed; failures surface the CLI's stderr (a diagnostic,
/// not the value) so a missing binary or auth error is actionable.
fn run_cli(bin: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(bin).args(args).output().map_err(|e| {
        err(format!(
            "failed to run `{bin}` (is it installed and on PATH?): {e}"
        ))
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(err(format!("`{bin}` failed: {}", stderr.trim())));
    }
    let value = String::from_utf8_lossy(&output.stdout);
    Ok(value.trim_end_matches(['\n', '\r']).to_string())
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
            payload: String::new(),
        }
    }

    fn tagged_ref(tag: &str, payload: &str) -> SecretRef {
        SecretRef {
            flat_key: String::new(),
            segments: Vec::new(),
            tag: Some(tag.to_string()),
            payload: payload.to_string(),
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

    #[test]
    fn op_uri_normalizes_both_forms() {
        assert_eq!(op_uri("Private/GitHub/token"), "op://Private/GitHub/token");
        assert_eq!(
            op_uri("//Private/GitHub/token"),
            "op://Private/GitHub/token"
        );
    }

    #[test]
    fn vault_path_field_splits_on_hash() {
        assert_eq!(
            vault_path_field("secret/myapp#token").unwrap(),
            ("secret/myapp", "token")
        );
        assert!(vault_path_field("secret/myapp").is_err());
        assert!(vault_path_field("#token").is_err());
        assert!(vault_path_field("secret/myapp#").is_err());
    }

    #[test]
    fn handles_routes_by_form() {
        let chain = ref_for("KST_X");
        let op = tagged_ref("op", "Private/GitHub/token");
        let vault = tagged_ref("vault", "secret/app#token");

        assert!(EnvSource.handles(&chain) && !EnvSource.handles(&op));
        assert!(OnePasswordSource.handles(&op) && !OnePasswordSource.handles(&chain));
        assert!(VaultSource.handles(&vault) && !VaultSource.handles(&op));
    }
}
