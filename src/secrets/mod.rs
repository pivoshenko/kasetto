//! Secret injection for synced MCP configs.
//!
//! Resolves `${kst_…}` placeholders at sync time from environment variables and
//! a `credentials.yaml` store, so packs can ship `Bearer ${kst_vercel_token}`
//! without committing the value. Injection happens on the in-memory config and
//! is written only to the agent destination — never to the source cache, the
//! stage dir, or `kasetto.lock` (the lock hashes the placeholder source file).

mod source;
mod template;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::{err, Result};
use crate::fsops::dirs_kasetto_config;
use crate::model::{OnMissing, SecretsConfig};
use crate::ui::eprint_warn;

use source::{
    CredentialsFileSource, EnvSource, KeePassSource, OnePasswordSource, SecretSource, VaultSource,
};
pub(crate) use template::has_placeholder;
use template::{substitute, SecretRef};

/// A resolved secret value. `Debug` is redacted so a value never leaks into a
/// log line, panic message, or `--json` report.
pub(crate) struct Secret(String);

impl Secret {
    fn new(v: String) -> Self {
        Self(v)
    }

    fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

/// Resolves `${kst_…}` placeholders against an ordered set of sources.
pub(crate) struct SecretContext {
    sources: Vec<Box<dyn SecretSource>>,
    on_missing: OnMissing,
    plain: bool,
    /// Per-run memo keyed on the placeholder text, so a secret referenced more
    /// than once is resolved once — avoids re-spawning `op`/`vault`/`keepassxc`
    /// (and the repeated biometric prompts that would cause).
    cache: RefCell<HashMap<String, Option<String>>>,
}

impl SecretContext {
    /// A context with no sources — any placeholder is "missing". Used by the
    /// sync-path unit tests that don't exercise injection.
    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self {
            sources: Vec::new(),
            on_missing: OnMissing::Error,
            plain: false,
            cache: RefCell::new(HashMap::new()),
        }
    }

    /// Build the resolver from config + CLI flags. Precedence (first hit wins):
    /// environment variables, then each credential file in order.
    pub(crate) fn from_config(
        cfg: Option<&SecretsConfig>,
        cfg_dir: &Path,
        allow_missing: bool,
        plain: bool,
    ) -> Result<Self> {
        let mut sources: Vec<Box<dyn SecretSource>> = vec![Box::new(EnvSource)];

        let mut files: Vec<PathBuf> = Vec::new();
        if let Ok(dir) = dirs_kasetto_config() {
            files.push(dir.join("credentials.yaml"));
        }
        if let Some(c) = cfg {
            for f in &c.files {
                files.push(resolve_rel(cfg_dir, f));
            }
        }
        for path in &files {
            warn_if_world_readable(path, plain);
            if let Some(src) = CredentialsFileSource::load(path)? {
                sources.push(Box::new(src));
            }
        }

        // External managers: only invoked when a matching `${kst:<tag>:…}` ref
        // appears (their `handles` gates on the tag), so adding op/vault
        // unconditionally costs nothing for env/credentials-only configs.
        sources.push(Box::new(OnePasswordSource));
        sources.push(Box::new(VaultSource));

        // KeePass needs a configured database, so it is added only when the
        // `secrets.keepass` block is present. The master password (if any) is
        // read from the environment, never from the committed config.
        if let Some(kp) = cfg.and_then(|c| c.keepass.as_ref()) {
            let database = crate::fsops::resolve_path(cfg_dir, &kp.database)
                .to_string_lossy()
                .into_owned();
            let key_file = kp.key_file.as_ref().map(|k| {
                crate::fsops::resolve_path(cfg_dir, k)
                    .to_string_lossy()
                    .into_owned()
            });
            let password = std::env::var("KST_KEEPASS_PASSWORD").ok().map(Secret::new);
            sources.push(Box::new(KeePassSource::new(database, key_file, password)));
        }

        let on_missing = if allow_missing {
            OnMissing::Warn
        } else {
            cfg.and_then(|c| c.on_missing).unwrap_or(OnMissing::Error)
        };

        Ok(Self {
            sources,
            on_missing,
            plain,
            cache: RefCell::new(HashMap::new()),
        })
    }

    /// Recursively inject placeholders into every string in `value`. Errors when
    /// a required secret is missing (unless the policy is `warn`).
    pub(crate) fn inject_value(&self, value: &mut serde_json::Value) -> Result<()> {
        match value {
            serde_json::Value::String(s) => {
                if has_placeholder(s) {
                    *s = self.substitute_str(s)?;
                }
            }
            serde_json::Value::Array(arr) => {
                for v in arr.iter_mut() {
                    self.inject_value(v)?;
                }
            }
            serde_json::Value::Object(map) => {
                for (_, v) in map.iter_mut() {
                    self.inject_value(v)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn substitute_str(&self, s: &str) -> Result<String> {
        substitute(s, |r| {
            if let Some(secret) = self.resolve(r)? {
                return Ok(Some(secret.expose().to_string()));
            }
            let msg = format!(
                "secret {} not found (searched: {})",
                r.display(),
                self.searched(r)
            );
            match self.on_missing {
                OnMissing::Error => Err(err(msg)),
                OnMissing::Warn => {
                    eprint_warn(
                        &format!(
                            "{msg}; writing the literal placeholder to the agent settings \
                             file — the server will not authenticate until you set the secret \
                             and run `kst sync --update`"
                        ),
                        self.plain,
                    );
                    Ok(None)
                }
            }
        })
    }

    fn resolve(&self, r: &SecretRef) -> Result<Option<Secret>> {
        let key = r.display();
        if let Some(cached) = self.cache.borrow().get(&key) {
            return Ok(cached.clone().map(Secret::new));
        }
        let resolved = self.resolve_uncached(r)?;
        self.cache
            .borrow_mut()
            .insert(key, resolved.as_ref().map(|s| s.expose().to_string()));
        Ok(resolved)
    }

    fn resolve_uncached(&self, r: &SecretRef) -> Result<Option<Secret>> {
        let mut handled = false;
        for src in &self.sources {
            if !src.handles(r) {
                continue;
            }
            handled = true;
            if let Some(v) = src.get(r)? {
                return Ok(Some(v));
            }
        }
        // A tagged ref no source claims is either unconfigured (keepass needs a
        // `secrets.keepass` block) or an unknown manager.
        if !handled {
            if let Some(tag) = &r.tag {
                if matches!(tag.as_str(), "kp" | "keepass") {
                    return Err(err(format!(
                        "secret source `{tag}` needs a `secrets.keepass` block with a `database` path"
                    )));
                }
                return Err(err(format!(
                    "secret source `{tag}` is not supported (use `env`, `crd`, `op`, \
                     `vault`, or `kp`, or `${{kst_name}}` for the env -> credentials.yaml chain)"
                )));
            }
        }
        Ok(None)
    }

    /// The sources that apply to `r`, for the "searched: …" diagnostic.
    fn searched(&self, r: &SecretRef) -> String {
        let names: Vec<&str> = self
            .sources
            .iter()
            .filter(|s| s.handles(r))
            .map(|s| s.name())
            .collect();
        if names.is_empty() {
            "no sources".into()
        } else {
            names.join(", ")
        }
    }
}

fn resolve_rel(cfg_dir: &Path, p: &str) -> PathBuf {
    let path = PathBuf::from(p);
    if path.is_absolute() {
        path
    } else {
        cfg_dir.join(path)
    }
}

/// Warn when a file that holds (or now holds) plaintext secrets is group- or
/// world-readable. Used for `credentials.yaml` and for MCP destinations after a
/// resolved secret is written into them.
#[cfg(unix)]
pub(crate) fn warn_if_world_readable(path: &Path, plain: bool) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.permissions().mode() & 0o077 != 0 {
            eprint_warn(
                &format!(
                    "{} is group/world-readable; run `chmod 600 {}`",
                    path.display(),
                    path.display()
                ),
                plain,
            );
        }
    }
}

#[cfg(not(unix))]
pub(crate) fn warn_if_world_readable(_path: &Path, _plain: bool) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_from_yaml(yaml: &str, on_missing: OnMissing) -> SecretContext {
        let root: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        SecretContext {
            sources: vec![Box::new(source::CredentialsFileSource::from_yaml(root))],
            on_missing,
            plain: true,
            cache: RefCell::new(HashMap::new()),
        }
    }

    #[test]
    fn injects_into_nested_json() {
        let ctx = ctx_from_yaml("vercel:\n  token: tok_xyz\n", OnMissing::Error);
        let mut v: serde_json::Value =
            serde_json::json!({"headers": {"Authorization": "Bearer ${kst_vercel__token}"}});
        ctx.inject_value(&mut v).unwrap();
        assert_eq!(v["headers"]["Authorization"], "Bearer tok_xyz");
    }

    #[test]
    fn missing_errors_by_default() {
        let ctx = ctx_from_yaml("other: x\n", OnMissing::Error);
        let mut v: serde_json::Value = serde_json::json!({"k": "${kst_nope}"});
        let e = ctx.inject_value(&mut v).unwrap_err().to_string();
        assert!(e.contains("${kst_nope}"), "error names placeholder: {e}");
        assert!(!e.contains("tok"), "error must not leak a value: {e}");
    }

    #[test]
    fn missing_warn_leaves_placeholder() {
        let ctx = ctx_from_yaml("other: x\n", OnMissing::Warn);
        let mut v: serde_json::Value = serde_json::json!({"k": "${kst_nope}"});
        ctx.inject_value(&mut v).unwrap();
        assert_eq!(v["k"], "${kst_nope}");
    }

    #[test]
    fn secret_debug_is_redacted() {
        let s = Secret::new("super-secret".into());
        assert_eq!(format!("{s:?}"), "Secret(***)");
        assert!(!format!("{s:?}").contains("super-secret"));
    }

    #[test]
    fn injected_value_reaches_destination_via_merge() {
        use std::fs;

        let ctx = ctx_from_yaml("vercel:\n  token: tok_live\n", OnMissing::Error);
        let mut wrap = serde_json::json!({
            "vercel": {
                "url": "https://mcp.vercel.com",
                "headers": {"Authorization": "Bearer ${kst_vercel__token}"}
            }
        });
        ctx.inject_value(&mut wrap).unwrap();
        let map = wrap.as_object().unwrap().clone();

        let dir = crate::fsops::temp_dir("kasetto-secrets-e2e");
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("settings.json");
        let tgt = crate::model::McpSettingsTarget {
            path: target.clone(),
            format: crate::model::McpSettingsFormat::McpServers,
        };
        crate::mcps::merge_mcp_config(&map, &tgt, false).unwrap();

        let written = fs::read_to_string(&target).unwrap();
        assert!(
            written.contains("Bearer tok_live"),
            "value injected: {written}"
        );
        assert!(!written.contains("${kst"), "no placeholder left: {written}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_tagged_source_errors() {
        let ctx = ctx_from_yaml("x: y\n", OnMissing::Error);
        let mut v: serde_json::Value = serde_json::json!({"k": "${kst:gcp:projects/p/secrets/s}"});
        let e = ctx.inject_value(&mut v).unwrap_err().to_string();
        assert!(e.contains("gcp"), "names the unknown source: {e}");
        assert!(e.contains("not supported"), "explains: {e}");
    }

    #[test]
    fn unconfigured_keepass_tag_points_at_config() {
        let ctx = ctx_from_yaml("x: y\n", OnMissing::Error);
        let mut v: serde_json::Value = serde_json::json!({"k": "${kst:kp:GitHub/PAT#Password}"});
        let e = ctx.inject_value(&mut v).unwrap_err().to_string();
        assert!(
            e.contains("secrets.keepass"),
            "points at the config block: {e}"
        );
    }

    #[test]
    fn repeated_placeholder_resolves_once() {
        let ctx = ctx_from_yaml("vercel:\n  token: tok\n", OnMissing::Error);
        let mut v: serde_json::Value =
            serde_json::json!({"a": "${kst_vercel__token}", "b": "${kst_vercel__token}"});
        ctx.inject_value(&mut v).unwrap();
        assert_eq!(
            ctx.cache.borrow().len(),
            1,
            "one memo entry for one placeholder"
        );
    }
}
