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

/// KeePass, via the `keepassxc-cli` CLI. Tagged form `${kst:kp:<entry>#<attr>}`
/// (`<attr>` defaults to `Password`) → `keepassxc-cli show -s -a <attr> …`,
/// unlocked with a key-file and/or a master password piped on stdin.
pub(super) struct KeePassSource {
    database: String,
    key_file: Option<String>,
    password: Option<Secret>,
}

impl KeePassSource {
    pub(super) fn new(
        database: String,
        key_file: Option<String>,
        password: Option<Secret>,
    ) -> Self {
        Self {
            database,
            key_file,
            password,
        }
    }
}

impl SecretSource for KeePassSource {
    fn name(&self) -> &'static str {
        "keepass"
    }

    fn handles(&self, r: &SecretRef) -> bool {
        matches!(r.tag.as_deref(), Some("kp") | Some("keepass"))
    }

    fn get(&self, r: &SecretRef) -> Result<Option<Secret>> {
        let (entry, attr) = keepass_entry_attr(&r.payload);
        let args = keepass_args(
            &self.database,
            self.key_file.as_deref(),
            self.password.is_some(),
            entry,
            attr,
        );
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let value = run_cli_stdin(
            "keepassxc-cli",
            &arg_refs,
            self.password.as_ref().map(Secret::expose),
        )?;
        Ok(Some(Secret::new(value)))
    }
}

/// Split a KeePass payload `"<entry>#<attr>"` into entry path and attribute,
/// defaulting the attribute to `Password` when omitted.
fn keepass_entry_attr(payload: &str) -> (&str, &str) {
    match payload.split_once('#') {
        Some((entry, attr)) if !attr.is_empty() => (entry, attr),
        _ => (payload.trim_end_matches('#'), "Password"),
    }
}

/// Build the `keepassxc-cli show` argument list. With no password to pipe the
/// DB is assumed key-file-only, so `--no-password` is added to skip the prompt.
fn keepass_args(
    db: &str,
    key_file: Option<&str>,
    has_password: bool,
    entry: &str,
    attr: &str,
) -> Vec<String> {
    let mut args = vec![
        "show".into(),
        "--quiet".into(),
        "-s".into(),
        "-a".into(),
        attr.into(),
    ];
    if let Some(kf) = key_file {
        args.push("--key-file".into());
        args.push(kf.into());
    }
    if !has_password {
        args.push("--no-password".into());
    }
    args.push(db.into());
    args.push(entry.into());
    args
}

/// AWS Secrets Manager, via the `aws` CLI. `${kst:aws:<secret-id>#<json-key>}`
/// runs `aws secretsmanager get-secret-value`; with `#<json-key>` the secret
/// string is parsed as JSON and that top-level field is returned.
pub(super) struct AwsSecretsSource;

impl SecretSource for AwsSecretsSource {
    fn name(&self) -> &'static str {
        "aws"
    }

    fn handles(&self, r: &SecretRef) -> bool {
        r.tag.as_deref() == Some("aws")
    }

    fn get(&self, r: &SecretRef) -> Result<Option<Secret>> {
        let (secret_id, field) = split_field(&r.payload);
        let raw = run_cli(
            "aws",
            &[
                "secretsmanager",
                "get-secret-value",
                "--secret-id",
                secret_id,
                "--query",
                "SecretString",
                "--output",
                "text",
            ],
        )?;
        Ok(Some(Secret::new(extract_json_field(raw, field)?)))
    }
}

/// Google Cloud Secret Manager, via the `gcloud` CLI.
/// `${kst:gcp:<name>#<json-key>}` runs `gcloud secrets versions access latest
/// --secret=<name>` against the active project; an optional `#<json-key>`
/// extracts a top-level field when the secret is a JSON document.
pub(super) struct GcpSecretsSource;

impl SecretSource for GcpSecretsSource {
    fn name(&self) -> &'static str {
        "gcp"
    }

    fn handles(&self, r: &SecretRef) -> bool {
        r.tag.as_deref() == Some("gcp")
    }

    fn get(&self, r: &SecretRef) -> Result<Option<Secret>> {
        let (name, field) = split_field(&r.payload);
        let raw = run_cli(
            "gcloud",
            &["secrets", "versions", "access", "latest", "--secret", name],
        )?;
        Ok(Some(Secret::new(extract_json_field(raw, field)?)))
    }
}

/// Azure Key Vault, via the `az` CLI. `${kst:az:<vault>/<name>#<json-key>}` runs
/// `az keyvault secret show --vault-name <vault> --name <name>`; an optional
/// `#<json-key>` extracts a top-level field when the secret is a JSON document.
pub(super) struct AzureKeyVaultSource;

impl SecretSource for AzureKeyVaultSource {
    fn name(&self) -> &'static str {
        "az"
    }

    fn handles(&self, r: &SecretRef) -> bool {
        r.tag.as_deref() == Some("az")
    }

    fn get(&self, r: &SecretRef) -> Result<Option<Secret>> {
        let (path, field) = split_field(&r.payload);
        let (vault, name) = az_vault_name(path)?;
        let raw = run_cli(
            "az",
            &[
                "keyvault",
                "secret",
                "show",
                "--vault-name",
                vault,
                "--name",
                name,
                "--query",
                "value",
                "--output",
                "tsv",
            ],
        )?;
        Ok(Some(Secret::new(extract_json_field(raw, field)?)))
    }
}

/// The `pass`/`gopass` Unix password store, via the `pass` CLI.
/// `${kst:pass:<path>}` runs `pass show <path>` and returns the first line (the
/// password, by the store's convention).
pub(super) struct PassSource;

impl SecretSource for PassSource {
    fn name(&self) -> &'static str {
        "pass"
    }

    fn handles(&self, r: &SecretRef) -> bool {
        r.tag.as_deref() == Some("pass")
    }

    fn get(&self, r: &SecretRef) -> Result<Option<Secret>> {
        let out = run_cli("pass", &["show", &r.payload])?;
        let first = out.lines().next().unwrap_or("").to_string();
        Ok(Some(Secret::new(first)))
    }
}

/// macOS Keychain, via the `security` CLI. `${kst:keychain:<service>#<account>}`
/// runs `security find-generic-password -s <service> [-a <account>] -w` (the
/// `#<account>` is optional).
pub(super) struct KeychainSource;

impl SecretSource for KeychainSource {
    fn name(&self) -> &'static str {
        "keychain"
    }

    fn handles(&self, r: &SecretRef) -> bool {
        r.tag.as_deref() == Some("keychain")
    }

    fn get(&self, r: &SecretRef) -> Result<Option<Secret>> {
        let (service, account) = split_field(&r.payload);
        let mut args = vec!["find-generic-password", "-s", service];
        if let Some(account) = account {
            args.push("-a");
            args.push(account);
        }
        args.push("-w");
        Ok(Some(Secret::new(run_cli("security", &args)?)))
    }
}

/// Split a tagged payload `"<id>#<field>"` into the id and an optional field
/// (shared by the AWS `#<json-key>` and Keychain `#<account>` forms).
fn split_field(payload: &str) -> (&str, Option<&str>) {
    match payload.split_once('#') {
        Some((id, field)) if !field.is_empty() => (id, Some(field)),
        _ => (payload.trim_end_matches('#'), None),
    }
}

/// Return `raw` as-is, or — when `field` is set — parse it as JSON and extract
/// that top-level string field (AWS secrets are often JSON documents).
fn extract_json_field(raw: String, field: Option<&str>) -> Result<String> {
    let Some(key) = field else {
        return Ok(raw);
    };
    let val: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| err(format!("secret is not JSON, cannot extract `{key}`: {e}")))?;
    val.get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| err(format!("JSON secret has no string field `{key}`")))
}

/// Split an Azure payload `"<vault>/<name>"` into its vault and secret name.
fn az_vault_name(payload: &str) -> Result<(&str, &str)> {
    match payload.split_once('/') {
        Some((vault, name)) if !vault.is_empty() && !name.is_empty() => Ok((vault, name)),
        _ => Err(err(format!(
            "azure secret `{payload}` must be `<vault>/<name>` (e.g. my-vault/db-password)"
        ))),
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
    // Same `#` split as `split_field`, but the field is mandatory for vault.
    match split_field(payload) {
        (path, Some(field)) if !path.is_empty() => Ok((path, field)),
        _ => Err(err(format!(
            "vault secret `{payload}` must be `<kv-path>#<field>` (e.g. secret/myapp#token)"
        ))),
    }
}

/// Run a secret-manager CLI and return its trimmed stdout. The resolved value
/// is captured, never echoed; failures surface the CLI's stderr (a diagnostic,
/// not the value) so a missing binary or auth error is actionable.
fn run_cli(bin: &str, args: &[&str]) -> Result<String> {
    run_cli_stdin(bin, args, None)
}

/// Like [`run_cli`] but optionally pipes `stdin` to the process (used to feed a
/// KeePass master password without exposing it on the command line). The piped
/// value is never echoed; only the CLI's stderr surfaces on failure.
fn run_cli_stdin(bin: &str, args: &[&str], stdin: Option<&str>) -> Result<String> {
    use std::process::Stdio;

    let mut cmd = Command::new(bin);
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    // Pipe when we have input to feed; otherwise close stdin so a CLI that would
    // prompt interactively fails fast instead of hanging the sync on a tty read.
    cmd.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let mut child = cmd.spawn().map_err(|e| {
        err(format!(
            "failed to run `{bin}` (is it installed and on PATH?): {e}"
        ))
    })?;
    if let Some(data) = stdin {
        use std::io::Write;
        // Taking and dropping the handle closes the pipe (EOF) after the write.
        if let Some(mut pipe) = child.stdin.take() {
            pipe.write_all(data.as_bytes())
                .and_then(|()| pipe.write_all(b"\n"))
                .map_err(|e| err(format!("failed to pass input to `{bin}`: {e}")))?;
        }
    }
    // Safe to write all input before reading output because secret-manager
    // responses are tiny. Don't reuse this for a CLI that streams large output
    // before draining stdin — that could deadlock on a full pipe buffer.
    let output = child
        .wait_with_output()
        .map_err(|e| err(format!("`{bin}` did not complete: {e}")))?;
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
        let got = src
            .get(&tagged_ref("crd", "vercel/token"))
            .unwrap()
            .unwrap();
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
        let got = EnvSource
            .get(&tagged_ref("env", "PLAIN_ENV_SECRET"))
            .unwrap();
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
        let kp = tagged_ref("kp", "GitHub/PAT#Password");
        let crd_src = CredentialsFileSource::from_yaml(serde_yaml::from_str("{}").unwrap());
        let kp_src = KeePassSource::new("db.kdbx".into(), None, None);

        // Env claims the chain and the explicit `env:` tag, nothing else.
        assert!(EnvSource.handles(&chain) && EnvSource.handles(&env_tag));
        assert!(!EnvSource.handles(&crd_tag) && !EnvSource.handles(&op));
        // Credentials claims the chain and the explicit `crd:` tag.
        assert!(crd_src.handles(&chain) && crd_src.handles(&crd_tag));
        assert!(!crd_src.handles(&env_tag) && !crd_src.handles(&op));
        // External managers claim only their own tag.
        assert!(OnePasswordSource.handles(&op) && !OnePasswordSource.handles(&chain));
        assert!(VaultSource.handles(&vault) && !VaultSource.handles(&op));
        assert!(kp_src.handles(&kp) && kp_src.handles(&tagged_ref("keepass", "x#y")));
        assert!(!kp_src.handles(&chain) && !kp_src.handles(&vault));
    }

    #[test]
    fn split_field_separates_optional_field() {
        assert_eq!(
            split_field("prod/db#password"),
            ("prod/db", Some("password"))
        );
        assert_eq!(split_field("prod/db"), ("prod/db", None));
        assert_eq!(split_field("prod/db#"), ("prod/db", None));
    }

    #[test]
    fn extract_json_field_returns_raw_or_key() {
        assert_eq!(extract_json_field("plain".into(), None).unwrap(), "plain");
        let json = r#"{"username":"u","password":"p"}"#.to_string();
        assert_eq!(
            extract_json_field(json.clone(), Some("password")).unwrap(),
            "p"
        );
        assert!(extract_json_field(json, Some("missing")).is_err());
        assert!(extract_json_field("not-json".into(), Some("k")).is_err());
    }

    #[test]
    fn az_vault_name_requires_both_parts() {
        assert_eq!(az_vault_name("my-vault/db").unwrap(), ("my-vault", "db"));
        assert!(az_vault_name("no-slash").is_err());
        assert!(az_vault_name("/db").is_err());
        assert!(az_vault_name("vault/").is_err());
    }

    #[test]
    fn new_providers_route_by_tag() {
        let chain = ref_for("kst_x");
        let cases: [(&dyn SecretSource, &str); 5] = [
            (&AwsSecretsSource, "aws"),
            (&GcpSecretsSource, "gcp"),
            (&AzureKeyVaultSource, "az"),
            (&PassSource, "pass"),
            (&KeychainSource, "keychain"),
        ];
        for (src, tag) in cases {
            assert!(src.handles(&tagged_ref(tag, "x")), "{tag} claims its tag");
            assert!(!src.handles(&chain), "{tag} ignores the chain form");
            assert!(
                !src.handles(&tagged_ref("other", "x")),
                "{tag} ignores other tags"
            );
        }
    }

    #[test]
    fn keepass_entry_attr_defaults_to_password() {
        assert_eq!(
            keepass_entry_attr("GitHub/PAT#Token"),
            ("GitHub/PAT", "Token")
        );
        assert_eq!(keepass_entry_attr("GitHub/PAT"), ("GitHub/PAT", "Password"));
        assert_eq!(
            keepass_entry_attr("GitHub/PAT#"),
            ("GitHub/PAT", "Password")
        );
    }

    #[test]
    fn keepass_args_add_no_password_only_without_a_password() {
        // Key-file only → `--key-file` and `--no-password` (skip the prompt).
        let kf = keepass_args("db.kdbx", Some("k.keyx"), false, "GitHub/PAT", "Password");
        assert!(kf.windows(2).any(|w| w == ["--key-file", "k.keyx"]));
        assert!(kf.iter().any(|a| a == "--no-password"));
        // Password piped → no `--no-password`.
        let pw = keepass_args("db.kdbx", None, true, "GitHub/PAT", "Password");
        assert!(!pw.iter().any(|a| a == "--no-password"));
        assert_eq!(pw.last().unwrap(), "GitHub/PAT");
    }
}
