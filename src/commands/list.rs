use serde::Serialize;

use crate::cli::ListKind;
use crate::error::Result;
use crate::fsops::{resolve_dest, scope_root};
use crate::lock::{load_lock, LockFile};
use crate::model::{resolve_scope, InstalledSkill, Scope};
use crate::profile::{format_updated_ago, read_skill_profile};
use crate::state::{load_runtime_state, RuntimeState};
use crate::ui::{
    color_stdout_enabled, print_json, print_section_header, print_source_header, print_tip,
    print_tree_leaf, short_source,
};

#[derive(Clone, Serialize)]
pub(crate) struct AssetEntry {
    pub name: String,
    pub scope: Scope,
    pub pack_file: String,
    pub source: String,
}

pub(crate) fn run(
    as_json: bool,
    kind: ListKind,
    plain: bool,
    quiet: bool,
    scope_override: Option<Scope>,
) -> Result<()> {
    if quiet && !as_json {
        return Ok(());
    }

    let color = !plain && color_stdout_enabled();

    let project_root = std::env::current_dir().unwrap_or_default();
    let merged = scope_override.is_none();
    let (mut skills, mut mcps, mut commands, mut instructions) =
        load_installed_assets(scope_override, &project_root)?;

    if !matches!(kind, ListKind::All | ListKind::Skills) {
        skills.clear();
    }
    if !matches!(kind, ListKind::All | ListKind::Mcps) {
        mcps.clear();
    }
    if !matches!(kind, ListKind::All | ListKind::Commands) {
        commands.clear();
    }
    if !matches!(kind, ListKind::All | ListKind::Instructions) {
        instructions.clear();
    }

    if as_json {
        let output = serde_json::json!({
            "skills": skills,
            "mcps": mcps,
            "commands": commands,
            "instructions": instructions,
            "merged_scopes": merged,
        });
        return print_json(&output);
    }

    let has_anything =
        !skills.is_empty() || !mcps.is_empty() || !commands.is_empty() || !instructions.is_empty();
    if !has_anything {
        println!("Nothing installed.");
        print_tip(
            "run `kasetto init` to scaffold a config, then `kasetto sync` to install skills",
            plain,
        );
        return Ok(());
    }

    let plain = !color;
    print_skills_tree(&skills, plain);
    print_assets_tree("MCP Servers", "connected", &mcps, plain);
    print_assets_tree("Commands", "available", &commands, plain);
    print_assets_tree("Instructions", "active", &instructions, plain);

    Ok(())
}

fn print_skills_tree(skills: &[InstalledSkill], plain: bool) {
    if skills.is_empty() {
        return;
    }
    print_section_header("Skills", Some((skills.len(), "installed")), plain);

    // Group by source, preserve first-seen order, keep skills inside sorted.
    let mut groups: Vec<(String, Vec<&InstalledSkill>)> = Vec::new();
    for s in skills {
        if let Some(g) = groups.iter_mut().find(|(k, _)| k == &s.source) {
            g.1.push(s);
        } else {
            groups.push((s.source.clone(), vec![s]));
        }
    }
    for (source, items) in &groups {
        let repo = short_source(source);
        print_source_header(&repo, Some(items.len()), Some(false), Some(62), plain);
        for (i, s) in items.iter().enumerate() {
            let is_last = i == items.len() - 1;
            let tail = if s.name == s.skill {
                "—".to_string()
            } else {
                s.skill.clone()
            };
            print_tree_leaf(is_last, None, &s.name, false, &tail, 30, plain);
        }
    }
}

fn print_assets_tree(label: &str, unit: &str, rows: &[AssetEntry], plain: bool) {
    if rows.is_empty() {
        return;
    }
    print_section_header(label, Some((rows.len(), unit)), plain);

    let mut groups: Vec<(String, Vec<&AssetEntry>)> = Vec::new();
    for a in rows {
        if let Some(g) = groups.iter_mut().find(|(k, _)| k == &a.source) {
            g.1.push(a);
        } else {
            groups.push((a.source.clone(), vec![a]));
        }
    }
    for (source, items) in &groups {
        let repo = short_source(source);
        print_source_header(&repo, Some(items.len()), Some(false), Some(62), plain);
        for (i, a) in items.iter().enumerate() {
            let is_last = i == items.len() - 1;
            print_tree_leaf(is_last, None, &a.name, false, "—", 30, plain);
        }
    }
}

type InstalledAssets = (
    Vec<InstalledSkill>,
    Vec<AssetEntry>,
    Vec<AssetEntry>,
    Vec<AssetEntry>,
);

fn load_installed_assets(
    scope_override: Option<Scope>,
    project_root: &std::path::Path,
) -> Result<InstalledAssets> {
    if let Some(s) = scope_override {
        let scope = resolve_scope(Some(s), None);
        let lock = load_lock(scope, project_root)?;
        let runtime = load_runtime_state(scope, project_root)?;
        return Ok((
            installed_skills_from_lock(&lock, &runtime, scope, project_root, false),
            mcp_asset_entries(&lock, scope),
            kind_asset_entries(&lock, scope, "command"),
            kind_asset_entries(&lock, scope, "instructions"),
        ));
    }
    let global_lock = load_lock(Scope::Global, project_root)?;
    let project_lock = load_lock(Scope::Project, project_root)?;
    let global_runtime = load_runtime_state(Scope::Global, project_root)?;
    let project_runtime = load_runtime_state(Scope::Project, project_root)?;
    let mut skills = installed_skills_from_lock(
        &global_lock,
        &global_runtime,
        Scope::Global,
        project_root,
        true,
    );
    skills.extend(installed_skills_from_lock(
        &project_lock,
        &project_runtime,
        Scope::Project,
        project_root,
        true,
    ));
    skills.sort_by_cached_key(|s| (scope_ord(s.scope), s.name.to_lowercase()));
    let mut mcps = mcp_asset_entries(&global_lock, Scope::Global);
    mcps.extend(mcp_asset_entries(&project_lock, Scope::Project));
    mcps.sort_by_cached_key(|m| (m.name.to_lowercase(), scope_ord(m.scope)));
    let mut commands = kind_asset_entries(&global_lock, Scope::Global, "command");
    commands.extend(kind_asset_entries(&project_lock, Scope::Project, "command"));
    commands.sort_by_cached_key(|m| (m.name.to_lowercase(), scope_ord(m.scope)));
    let mut instructions = kind_asset_entries(&global_lock, Scope::Global, "instructions");
    instructions.extend(kind_asset_entries(
        &project_lock,
        Scope::Project,
        "instructions",
    ));
    instructions.sort_by_cached_key(|m| (m.name.to_lowercase(), scope_ord(m.scope)));
    Ok((skills, mcps, commands, instructions))
}

fn kind_asset_entries(lock: &LockFile, scope: Scope, kind: &str) -> Vec<AssetEntry> {
    let mut out: Vec<AssetEntry> = lock
        .assets
        .iter()
        .filter(|(_, a)| a.kind == kind)
        .map(|(_, a)| AssetEntry {
            name: a.name.clone(),
            scope,
            pack_file: String::new(),
            source: a.source.clone(),
        })
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

fn mcp_asset_entries(lock: &LockFile, scope: Scope) -> Vec<AssetEntry> {
    let mut out = Vec::new();
    for name in lock.list_installed_mcps() {
        let (pack_file, source) = lock
            .assets
            .iter()
            .filter(|(_, a)| a.kind == "mcp")
            .find(|(_, a)| a.destination.split(',').any(|s| !s.is_empty() && s == name))
            .map(|(_, a)| (a.name.clone(), a.source.clone()))
            .unwrap_or_default();
        out.push(AssetEntry {
            name,
            scope,
            pack_file,
            source,
        });
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

fn scope_ord(s: Scope) -> u8 {
    match s {
        Scope::Global => 0,
        Scope::Project => 1,
    }
}

fn scope_label(s: Scope) -> &'static str {
    match s {
        Scope::Global => "global",
        Scope::Project => "project",
    }
}

fn skill_display_id(lock_scope: Scope, raw_id: &str, composite: bool) -> String {
    if composite {
        format!("{}::{}", scope_label(lock_scope), raw_id)
    } else {
        raw_id.to_string()
    }
}

fn installed_skills_from_lock(
    lock: &LockFile,
    runtime: &RuntimeState,
    lock_scope: Scope,
    project_root: &std::path::Path,
    composite_ids: bool,
) -> Vec<InstalledSkill> {
    let state = lock.state();
    let root = scope_root(lock_scope, project_root).unwrap_or_else(|_| project_root.to_path_buf());
    let mut skills = Vec::new();
    for (id, entry) in &state.skills {
        let abs_dest = resolve_dest(&entry.destination, &root);
        let abs_dest_str = abs_dest.to_string_lossy().to_string();
        let (name, fallback_description) = read_skill_profile(&abs_dest_str, &entry.skill);
        let description = if entry.description.trim().is_empty() {
            fallback_description
        } else {
            entry.description.clone()
        };
        let updated_at = runtime.updated_at(id);
        let updated_ago = format_updated_ago(&updated_at);
        let effective_scope = entry.scope.unwrap_or(lock_scope);
        skills.push(InstalledSkill {
            id: skill_display_id(lock_scope, id, composite_ids),
            scope: effective_scope,
            name,
            description,
            source: entry.source.clone(),
            skill: entry.skill.clone(),
            destination: abs_dest_str,
            hash: entry.hash.clone(),
            source_revision: entry.source_revision.clone(),
            updated_at,
            updated_ago,
        });
    }
    skills.sort_by_cached_key(|s| s.name.to_lowercase());
    skills
}
