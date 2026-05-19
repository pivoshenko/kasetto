/// Default config file in the current directory when `--config` is omitted.
pub(crate) const DEFAULT_CONFIG_FILENAME: &str = "kasetto.yaml";
/// Default config file under the Kasetto XDG config directory (`init --global` writes here).
pub(crate) const DEFAULT_GLOBAL_CONFIG_FILENAME: &str = "kasetto.yaml";
/// Kasetto preferences file under the XDG config directory.
/// May contain a `config:` key pointing to a remote or absolute config path.
pub(crate) const PREFERENCES_FILENAME: &str = "config.yaml";

#[derive(serde::Deserialize)]
struct Preferences {
    source: Option<String>,
}

/// Resolve the default config path used when `--config` is omitted.
///
/// Priority:
/// 1. `$KASETTO_CONFIG` env var
/// 2. `source:` key in `$XDG_CONFIG_HOME/kasetto/config.yaml` (preferences file)
/// 3. `./kasetto.yaml` (local project config)
/// 4. `$XDG_CONFIG_HOME/kasetto/kasetto.yaml` (global config)
/// 5. `./kasetto.yaml` fallback
pub(crate) fn default_config_path() -> String {
    let env_var = std::env::var("KASETTO_CONFIG")
        .ok()
        .filter(|v| !v.is_empty());
    let prefs_path = crate::fsops::dirs_kasetto_config()
        .ok()
        .map(|d| d.join(PREFERENCES_FILENAME));
    let local_exists = std::path::Path::new(DEFAULT_CONFIG_FILENAME).exists();
    let global_path = crate::fsops::dirs_kasetto_config()
        .ok()
        .map(|d| d.join(DEFAULT_GLOBAL_CONFIG_FILENAME));
    resolve_config_path(
        env_var,
        prefs_path.as_deref(),
        local_exists,
        global_path.as_deref(),
    )
}

fn resolve_config_path(
    env_var: Option<String>,
    prefs_path: Option<&std::path::Path>,
    local_exists: bool,
    global_path: Option<&std::path::Path>,
) -> String {
    if let Some(v) = env_var {
        return v;
    }

    if let Some(path) = prefs_path {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(prefs) = serde_yaml::from_str::<Preferences>(&text) {
                if let Some(cfg) = prefs.source {
                    return cfg;
                }
            }
        }
    }

    if local_exists {
        return DEFAULT_CONFIG_FILENAME.to_string();
    }

    if let Some(global) = global_path {
        if global.exists() {
            return global.to_string_lossy().to_string();
        }
    }

    DEFAULT_CONFIG_FILENAME.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn env_var_takes_highest_priority() {
        let result = resolve_config_path(
            Some("https://example.com/team.yaml".into()),
            None,
            true,
            None,
        );
        assert_eq!(result, "https://example.com/team.yaml");
    }

    #[test]
    fn preferences_file_source_used_when_no_env_var() {
        let dir = temp_dir("kasetto-prefs");
        fs::create_dir_all(&dir).unwrap();
        let prefs = dir.join("config.yaml");
        fs::write(&prefs, "source: https://example.com/remote.yaml\n").unwrap();

        let result = resolve_config_path(None, Some(&prefs), false, None);
        assert_eq!(result, "https://example.com/remote.yaml");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn env_var_beats_preferences_file() {
        let dir = temp_dir("kasetto-prefs-priority");
        fs::create_dir_all(&dir).unwrap();
        let prefs = dir.join("config.yaml");
        fs::write(&prefs, "source: https://example.com/prefs.yaml\n").unwrap();

        let result = resolve_config_path(
            Some("https://example.com/env.yaml".into()),
            Some(&prefs),
            false,
            None,
        );
        assert_eq!(result, "https://example.com/env.yaml");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn local_kasetto_yaml_used_when_no_env_or_prefs() {
        let result = resolve_config_path(None, None, true, None);
        assert_eq!(result, DEFAULT_CONFIG_FILENAME);
    }

    #[test]
    fn global_config_used_when_local_absent() {
        let dir = temp_dir("kasetto-global");
        fs::create_dir_all(&dir).unwrap();
        let global = dir.join("kasetto.yaml");
        fs::write(&global, "agent: claude-code\nskills: []\n").unwrap();

        let result = resolve_config_path(None, None, false, Some(&global));
        assert_eq!(result, global.to_string_lossy());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn falls_back_to_local_filename_when_nothing_exists() {
        let result = resolve_config_path(None, None, false, None);
        assert_eq!(result, DEFAULT_CONFIG_FILENAME);
    }

    #[test]
    fn missing_prefs_file_is_skipped_silently() {
        let dir = temp_dir("kasetto-no-prefs");
        let missing = dir.join("config.yaml");

        let result = resolve_config_path(None, Some(&missing), true, None);
        assert_eq!(result, DEFAULT_CONFIG_FILENAME);
    }

    #[test]
    fn prefs_file_without_source_key_is_skipped() {
        let dir = temp_dir("kasetto-prefs-no-source");
        fs::create_dir_all(&dir).unwrap();
        let prefs = dir.join("config.yaml");
        fs::write(&prefs, "some_other_key: value\n").unwrap();

        let result = resolve_config_path(None, Some(&prefs), true, None);
        assert_eq!(result, DEFAULT_CONFIG_FILENAME);

        let _ = fs::remove_dir_all(&dir);
    }
}

mod app;
mod banner;
mod cli;
mod colors;
mod commands;
mod error;
mod fsops;
mod home;
mod list;
mod lock;
mod mcps;
mod model;
mod profile;
mod prompts;
mod source;
mod tui;
mod ui;
mod update_notifier;

pub use app::run;
pub use error::Result;
