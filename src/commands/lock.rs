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
//!
//! With `--upgrade-package <name>...` the re-resolve is restricted to sources
//! providing those skills (mirrors `sync --update <name>...`). With `--check`
//! the resolved lock is compared against the on-disk lock and the command
//! exits non-zero on drift, never writing.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::time::Instant;

use crate::colors::{ACCENT, ATTENTION, ERROR, RESET, SECONDARY, SUCCESS};
use crate::error::{err, Result};
use crate::fsops::{
    hash_dir, join_dest_csv, load_config_any, now_unix, resolve_destinations, scope_root,
    select_targets,
};
use crate::lock::{load_lock, save_lock, LockFile};
use crate::model::{resolve_scope, Config, Scope, SkillEntry};
use crate::profile::read_skill_profile_from_dir;
use crate::source::materialize_source;
use crate::ui::{eprint_error, print_json};

pub(crate) struct LockOptions<'a> {
    pub config: Option<&'a str>,
    pub scope_override: Option<Scope>,
    pub as_json: bool,
    pub quiet: u8,
    /// `--check`: verify the lock matches the config without writing; exit 1 on drift.
    pub check: bool,
    /// `--upgrade-package <name>...`: restrict re-resolve to sources providing these skills.
    /// Empty means re-resolve every source (default).
    pub upgrade_only: Vec<String>,
}

pub(crate) fn run(opts: &LockOptions) -> Result<()> {
    let started = Instant::now();
    let config_path = opts
        .config
        .map_or_else(crate::default_config_path, str::to_string);
    let (cfg, cfg_dir, _label) = load_config_any(&config_path)?;
    let scope = resolve_scope(opts.scope_override, Some(&cfg));
    let destinations = resolve_destinations(&cfg_dir, &cfg, scope)?;
    let root = scope_root(scope, &cfg_dir)?;

    let mut lock = load_lock(scope, &cfg_dir)?;
    let prev_skills = lock.skills.clone();
    let prev_assets = lock.assets.clone();

    // Decide per source whether to re-resolve (default) or carry over existing
    // entries from the on-disk lock (--upgrade-package excluded this source).
    let upgrade_active = |source_url: &str| -> bool {
        if opts.upgrade_only.is_empty() {
            return true;
        }
        prev_skills
            .values()
            .any(|e| e.source == source_url && opts.upgrade_only.contains(&e.skill))
    };

    // Rebuild the skills section from a fresh resolve (re-resolves moving refs,
    // like `sync --update`). Any source error aborts before writing.
    let mut new_skills: BTreeMap<String, SkillEntry> = BTreeMap::new();
    for (i, src) in cfg.skills.iter().enumerate() {
        if !upgrade_active(&src.source) {
            for (id, entry) in prev_skills.iter().filter(|(_, e)| e.source == src.source) {
                new_skills.insert(id.clone(), entry.clone());
            }
            continue;
        }
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
                let (_, description) = read_skill_profile_from_dir(&dir, &name);
                new_skills.insert(
                    format!("{}::{}", src.source, name),
                    SkillEntry {
                        destination: join_dest_csv(&destinations, &name, &root),
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
    let plain = !crate::ui::color_stdout_enabled();

    if opts.check {
        let drift = diff_summary(&prev_skills, &prev_assets, &lock);
        if drift.is_empty() {
            if !opts.as_json && opts.quiet == 0 {
                print_audited(skills_count, source_count, started.elapsed().as_millis());
            } else if opts.as_json {
                print_json(&serde_json::json!({
                    "check": "ok",
                    "skills": skills_count,
                    "sources": source_count,
                }))?;
            }
            return Ok(());
        }
        if opts.as_json {
            let changes: Vec<_> = drift
                .iter()
                .map(|c| serde_json::json!({"status": c.status.label(), "id": c.id}))
                .collect();
            print_json(&serde_json::json!({
                "check": "drift",
                "changes": changes,
            }))?;
        } else {
            eprint_error(
                &format!(
                    "lock is out of date with the config ({} change{} pending); run `kasetto lock` without --check",
                    drift.len(),
                    if drift.len() == 1 { "" } else { "s" },
                ),
                plain,
            );
            if opts.quiet == 0 {
                for c in &drift {
                    eprintln!("  {}", c.render(plain));
                }
            }
        }
        std::process::exit(1);
    }

    let lock_path = save_lock(&mut lock, scope, &cfg_dir)?;

    if opts.quiet == 0 {
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
fn refresh_asset_revisions(lock: &mut LockFile, cfg: &Config) {
    let mut rev_by_source: HashMap<String, String> = HashMap::new();
    for m in &cfg.mcps {
        rev_by_source.insert(m.source.clone(), m.as_source_spec().expected_revision());
    }
    for c in &cfg.commands {
        rev_by_source.insert(c.source.clone(), c.as_source_spec().expected_revision());
    }
    for r in &cfg.instructions {
        rev_by_source.insert(r.source.clone(), r.as_source_spec().expected_revision());
    }
    for asset in lock.assets.values_mut() {
        if let Some(rev) = rev_by_source.get(&asset.source) {
            asset.source_revision = rev.clone();
        }
    }
}

/// One change between the previous lock snapshot and the freshly resolved one.
struct Drift {
    status: DriftStatus,
    id: String,
}

#[derive(Clone, Copy)]
enum DriftStatus {
    Added,
    Removed,
    Updated,
}

impl DriftStatus {
    fn label(self) -> &'static str {
        match self {
            DriftStatus::Added => "added",
            DriftStatus::Removed => "removed",
            DriftStatus::Updated => "updated",
        }
    }

    /// Glyph + color matching the operational sync dialect: `+` SUCCESS,
    /// `−` (U+2212) ERROR, `↑` ATTENTION. Plain mode drops the ANSI.
    fn glyph(self, plain: bool) -> String {
        let (g, color) = match self {
            DriftStatus::Added => ("+", SUCCESS),
            DriftStatus::Removed => ("\u{2212}", ERROR),
            DriftStatus::Updated => ("\u{2191}", ATTENTION),
        };
        if plain {
            g.to_string()
        } else {
            format!("{color}{g}{RESET}")
        }
    }
}

impl Drift {
    fn render(&self, plain: bool) -> String {
        format!("{} {}", self.status.glyph(plain), self.id)
    }
}

/// Compare the rebuilt lock against the previous on-disk snapshot. Order is
/// deterministic via BTreeMap iteration.
fn diff_summary(
    prev_skills: &BTreeMap<String, SkillEntry>,
    prev_assets: &BTreeMap<String, crate::lock::AssetEntry>,
    next: &LockFile,
) -> Vec<Drift> {
    let mut out = Vec::new();
    for (id, prev) in prev_skills {
        match next.skills.get(id) {
            None => out.push(Drift {
                status: DriftStatus::Removed,
                id: id.clone(),
            }),
            Some(now) if now.hash != prev.hash || now.source_revision != prev.source_revision => {
                out.push(Drift {
                    status: DriftStatus::Updated,
                    id: id.clone(),
                });
            }
            _ => {}
        }
    }
    for id in next.skills.keys() {
        if !prev_skills.contains_key(id) {
            out.push(Drift {
                status: DriftStatus::Added,
                id: id.clone(),
            });
        }
    }
    for (id, prev) in prev_assets {
        if let Some(now) = next.assets.get(id) {
            if now.source_revision != prev.source_revision {
                out.push(Drift {
                    status: DriftStatus::Updated,
                    id: id.clone(),
                });
            }
        }
    }
    out
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

/// `--check` no-drift summary: uv-style `Audited` verb so users know the lock
/// was inspected against the config without being written.
fn print_audited(skills: usize, sources: usize, elapsed_ms: u128) {
    let plural = if skills == 1 { "item" } else { "items" };
    if crate::ui::color_stdout_enabled() {
        println!(
            "{SUCCESS}{ACCENT}Audited{RESET} {skills} {plural}{SECONDARY} from {sources} sources in {elapsed_ms}ms{RESET}"
        );
    } else {
        println!("Audited {skills} {plural} from {sources} sources in {elapsed_ms}ms");
    }
}
