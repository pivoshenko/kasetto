//! Secret sources: where injected values come from. Resolution walks the
//! sources, skipping any that don't `handle` the ref, and the first hit wins.
//! Chain refs (`${kst_name}`) go to env + credential files (env first); tagged
//! refs route to one explicit source — `${kst:env:…}`/`${kst:crd:…}` reuse the
//! env/credentials sources, `${kst:op:…}`/`${kst:vault:…}` the external CLIs.

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

/// Process environment variables. Chain refs key off the flat `kst_…` name
/// (also tried uppercased, the conventional `KST_…` form); the explicit
/// `${kst:env:NAME}` tag looks up `NAME` verbatim.
pub(super) struct EnvSource;

impl SecretSource for EnvSource {
    fn name(&self) -> &'static str {
        "env"
    }

    fn handles(&self, r: &SecretRef) -> bool {
        r.tag.is_none() || r.tag.as_deref() == Some("env")
    }

    fn get(&self, r: &SecretRef) -> Result<Option<Secret>> {
        // Explicit `${kst:env:NAME}` → look up NAME exactly as written.
        if r.tag.as_deref() == Some("env") {
            return Ok(std::env::var(&r.payload).ok().map(Secret::new));
        }
        // Chain `${kst_name}` → the lowercase key as written, then uppercased so
        // a lowercase placeholder still resolves a conventional UPPER_CASE var.
        if let Ok(v) = std::env::var(&r.flat_key) {
            return Ok(Some(Secret::new(v)));
        }
        Ok(std::env::var(r.flat_key.to_ascii_uppercase())
            .ok()
            .map(Secret::new))
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
        r.tag.is_none() || r.tag.as_deref() == Some("crd")
    }

    fn get(&self, r: &SecretRef) -> Result<Option<Secret>> {
        // Explicit `${kst:crd:a/b/c}` → descend the `/`-separated path.
        if r.tag.as_deref() == Some("crd") {
            let segs: Vec<&str> = r.payload.split('/').filter(|s| !s.is_empty()).collect();
            return Ok(descend(&self.root, &segs).map(Secret::new));
        }
        // Chain: a flat top-level key (case-insensitive), then the nested
        // `__`-separated path.
        if let Some(v) = lookup_key(&self.root, &r.flat_key).and_then(Yaml::as_str) {
            return Ok(Some(Secret::new(v.to_string())));
        }
        let segs: Vec<&str> = r.segments.iter().map(String::as_str).collect();
        Ok(descend(&self.root, &segs).map(Secret::new))
    }
}

/// Case-insensitive single-key lookup in a YAML mapping.
fn lookup_key<'a>(node: &'a Yaml, key: &str) -> Option<&'a Yaml> {
    let Yaml::Mapping(m) = node else {
        return None;
    };
    m.iter()
        .find(|(k, _)| k.as_str().is_some_and(|s| s.eq_ignore_ascii_case(key)))
        .map(|(_, v)| v)
}

/// Walk `segments` from `root` (each matched case-insensitively) and return the
/// leaf as a string. Returns `None` if any segment is missing or the leaf is
/// not a scalar string.
fn descend(root: &Yaml, segments: &[&str]) -> Option<String> {
    let mut cur = root;
    for seg in segments {
        cur = lookup_key(cur, seg)?;
    }
    cur.as_str().map(str::to_string)
}

/// 1Password, via the `op` CLI. Tagged form `${kst:op:<vault>/<item>/<field>}`
/// (or a full `${kst:op://<vault>/<item>/<field>}` URI) → `op read op://…`.
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
/// `${kst:vault:<kv-path>#<field>}` → `vault kv get -field=<field> <kv-path>`.
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
/// may already carry the `//` (from `${kst:op://…}`) or omit it (`${kst:op:…}`).
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
        let name = inner.strip_prefix("kst_").unwrap_or(inner);
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
    fn credentials_flat_key_case_insensitive() {
        // A lowercase `${kst_github_token}` resolves an UPPER_CASE flat key.
        let yaml: Yaml = serde_yaml::from_str("KST_GITHUB_TOKEN: ghp_abc\n").unwrap();
        let src = CredentialsFileSource::from_yaml(yaml);
        let got = src.get(&ref_for("kst_github_token")).unwrap().unwrap();
        assert_eq!(got.expose(), "ghp_abc");
    }

    #[test]
    fn credentials_nested_case_insensitive() {
        let yaml: Yaml = serde_yaml::from_str("vercel:\n  token: tok_123\n").unwrap();
        let src = CredentialsFileSource::from_yaml(yaml);
        let got = src.get(&ref_for("kst_vercel__token")).unwrap().unwrap();
        assert_eq!(got.expose(), "tok_123");
    }

    #[test]
    fn credentials_tagged_crd_slash_path() {
        let yaml: Yaml = serde_yaml::from_str("vercel:\n  token: tok_crd\n").unwrap();
        let src = CredentialsFileSource::from_yaml(yaml);
        let got = src.get(&tagged_ref("crd", "vercel/token")).unwrap().unwrap();
        assert_eq!(got.expose(), "tok_crd");
    }

    #[test]
    fn credentials_missing_returns_none() {
        let yaml: Yaml = serde_yaml::from_str("other: x\n").unwrap();
        let src = CredentialsFileSource::from_yaml(yaml);
        assert!(src.get(&ref_for("kst_vercel__token")).unwrap().is_none());
    }

    #[test]
    fn env_tag_reads_verbatim_and_chain_falls_back_to_uppercase() {
        std::env::set_var("PLAIN_ENV_SECRET", "from_env_tag");
        let got = EnvSource.get(&tagged_ref("env", "PLAIN_ENV_SECRET")).unwrap();
        assert_eq!(got.unwrap().expose(), "from_env_tag");
        std::env::remove_var("PLAIN_ENV_SECRET");

        // Chain `${kst_chain_only}` resolves the uppercased `KST_CHAIN_ONLY`.
        std::env::set_var("KST_CHAIN_ONLY", "from_chain");
        let got = EnvSource.get(&ref_for("kst_chain_only")).unwrap();
        assert_eq!(got.unwrap().expose(), "from_chain");
        std::env::remove_var("KST_CHAIN_ONLY");
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
        let chain = ref_for("kst_x");
        let env_tag = tagged_ref("env", "FOO");
        let crd_tag = tagged_ref("crd", "a/b");
        let op = tagged_ref("op", "Private/GitHub/token");
        let vault = tagged_ref("vault", "secret/app#token");
        let crd_src = CredentialsFileSource::from_yaml(serde_yaml::from_str("{}").unwrap());

        // Env claims the chain and the explicit `env:` tag, nothing else.
        assert!(EnvSource.handles(&chain) && EnvSource.handles(&env_tag));
        assert!(!EnvSource.handles(&crd_tag) && !EnvSource.handles(&op));
        // Credentials claims the chain and the explicit `crd:` tag.
        assert!(crd_src.handles(&chain) && crd_src.handles(&crd_tag));
        assert!(!crd_src.handles(&env_tag) && !crd_src.handles(&op));
        // External managers claim only their own tag.
        assert!(OnePasswordSource.handles(&op) && !OnePasswordSource.handles(&chain));
        assert!(VaultSource.handles(&vault) && !VaultSource.handles(&op));
    }
}
