//! `kasetto lock` — resolve and pin the config into `kasetto.lock` without
//! installing to destinations.
//!
//! Skills are materialized and hashed from the source tree. Because a skill is
//! installed as a verbatim recursive copy, that source-tree hash equals the
//! hash a later `sync` computes at the destination — so the lock is immediately
//! offline-ready (`sync --locked` works with zero fetches afterward).
//!
//! MCP and command assets cannot be hashed without applying their merge /
//! transform, so `lock` only refreshes their resolved revision pins; their
//! content hash fills in on the next real `sync`.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::time::Instant;

use crate::colors::{ACCENT, RESET, SECONDARY, SUCCESS};
use crate::error::{err, Result};
use crate::fsops::{
    hash_dir, load_config_any, now_unix, relativize_dest, resolve_destinations, scope_root,
    select_targets,
};
use crate::lock::{load_lock, save_lock};
use crate::model::{resolve_scope, Config, Scope, SkillEntry};
use crate::profile::read_skill_profile_from_dir;
use crate::source::materialize_source;
use crate::ui::print_json;

pub(crate) struct LockOptions<'a> {
    pub config: Option<&'a str>,
    pub scope_override: Option<Scope>,
    pub as_json: bool,
    pub quiet: bool,
}

pub(crate) fn run(opts: &LockOptions) -> Result<()> {
    let started = Instant::now();
    let config_path = opts
        .config
        .map(str::to_string)
        .unwrap_or_else(crate::default_config_path);
    let (cfg, cfg_dir, _label) = load_config_any(&config_path)?;
    let scope = resolve_scope(opts.scope_override, Some(&cfg));
    let destinations = resolve_destinations(&cfg_dir, &cfg, scope)?;
    let root = scope_root(scope, &cfg_dir)?;

    let mut lock = load_lock(scope, &cfg_dir)?;

    // Rebuild the skills section from a fresh resolve (re-resolves moving refs,
    // like `sync --update`). Any source error aborts before writing.
    let mut new_skills: BTreeMap<String, SkillEntry> = BTreeMap::new();
    for (i, src) in cfg.skills.iter().enumerate() {
        let stage = std::env::temp_dir().join(format!("kasetto-lock-{}-{}", now_unix(), i));
        let materialized = materialize_source(src, &cfg_dir, &stage)?;
        let select = select_targets(
            &src.skills,
            &materialized.available,
            &materialized.source_root,
        );

        let result = select.and_then(|(targets, broken)| {
            if let Some(b) = broken.first() {
                return Err(err(format!(
                    "skill `{}` not found in {}",
                    b.name, src.source
                )));
            }
            for (name, dir) in targets {
                let hash = hash_dir(&dir)?;
                let dest = destinations[0].join(&name);
                let (_, description) = read_skill_profile_from_dir(&dir, &name);
                new_skills.insert(
                    format!("{}::{}", src.source, name),
                    SkillEntry {
                        destination: relativize_dest(&dest, &root),
                        hash,
                        skill: name.clone(),
                        description,
                        source: src.source.clone(),
                        source_revision: materialized.source_revision.clone(),
                        scope: Some(scope),
                    },
                );
            }
            Ok(())
        });

        if let Some(cleanup) = materialized.cleanup_dir {
            let _ = fs::remove_dir_all(cleanup);
        }
        result?;
    }
    lock.skills = new_skills;

    refresh_asset_revisions(&mut lock, &cfg);

    let skills_count = lock.skills.len();
    let source_count = cfg.skills.len();
    let lock_path = save_lock(&mut lock, scope, &cfg_dir)?;

    if !opts.quiet {
        if opts.as_json {
            print_json(&serde_json::json!({
                "skills": skills_count,
                "sources": source_count,
                "lock": lock_path.display().to_string(),
            }))?;
        } else {
            print_locked(skills_count, source_count, started.elapsed().as_millis());
        }
    }
    Ok(())
}

/// Refresh the resolved revision pin on already-tracked MCP/command assets to
/// match the current config. No content hash is recomputed (see module docs).
fn refresh_asset_revisions(lock: &mut crate::lock::LockFile, cfg: &Config) {
    let mut rev_by_source: HashMap<String, String> = HashMap::new();
    for m in &cfg.mcps {
        rev_by_source.insert(m.source.clone(), m.as_source_spec().expected_revision());
    }
    for c in &cfg.commands {
        rev_by_source.insert(c.source.clone(), c.as_source_spec().expected_revision());
    }
    for asset in lock.assets.values_mut() {
        if let Some(rev) = rev_by_source.get(&asset.source) {
            asset.source_revision = rev.clone();
        }
    }
}

/// Past-tense summary verb in sync rhythm: green `Locked`, default-fg count,
/// dim source tail + timing. No chip strip (nothing was installed).
fn print_locked(skills: usize, sources: usize, elapsed_ms: u128) {
    let plural = if skills == 1 { "item" } else { "items" };
    if crate::ui::color_stdout_enabled() {
        println!(
            "{SUCCESS}{ACCENT}Locked{RESET} {skills} {plural}{SECONDARY} from {sources} sources in {elapsed_ms}ms{RESET}"
        );
    } else {
        println!("Locked {skills} {plural} from {sources} sources in {elapsed_ms}ms");
    }
}
