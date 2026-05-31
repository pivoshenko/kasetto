use serde::Serialize;

use crate::cli::ListKind;
use crate::colors::{ACCENT, RESET, SECONDARY};
use crate::error::Result;
use crate::fsops::{resolve_dest, scope_root};
use crate::lock::{load_lock, LockFile};
use crate::model::{resolve_scope, InstalledSkill, Scope};
use crate::profile::{format_updated_ago, read_skill_profile};
use crate::state::{load_runtime_state, RuntimeState};
use crate::ui::{color_stdout_enabled, print_json, print_tip, short_source};

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
    let (mut skills, mut mcps, mut commands) =
        load_skills_mcps_commands(scope_override, &project_root)?;

    if !matches!(kind, ListKind::All | ListKind::Skills) {
        skills.clear();
    }
    if !matches!(kind, ListKind::All | ListKind::Mcps) {
        mcps.clear();
    }
    if !matches!(kind, ListKind::All | ListKind::Commands) {
        commands.clear();
    }

    if as_json {
        let output = serde_json::json!({
            "skills": skills,
            "mcps": mcps,
            "commands": commands,
            "merged_scopes": merged,
        });
        return print_json(&output);
    }

    let has_anything = !skills.is_empty() || !mcps.is_empty() || !commands.is_empty();
    if !has_anything {
        println!("Nothing installed.");
        print_tip(
            "run `kst init` to scaffold a config, then `kst sync` to install skills",
            plain,
        );
        return Ok(());
    }

    print_skills_table(&skills, merged, color);
    print_assets_table("MCP Servers", &mcps, merged, color);
    print_assets_table("Commands", &commands, merged, color);

    Ok(())
}

fn print_skills_table(skills: &[InstalledSkill], merged: bool, color: bool) {
    if skills.is_empty() {
        return;
    }
    let name_w = column_width("NAME", skills.iter().map(|s| s.name.as_str()));
    let scope_w = if merged {
        column_width("SCOPE", skills.iter().map(|s| scope_label(s.scope)))
    } else {
        0
    };
    let updated_w = column_width("UPDATED", skills.iter().map(|s| s.updated_ago.as_str()));

    print_header(
        &[
            ("NAME", name_w),
            ("SCOPE", scope_w),
            ("UPDATED", updated_w),
            ("SOURCE", 0),
        ],
        color,
    );

    for s in skills {
        let scope_cell = if merged { scope_label(s.scope) } else { "" };
        let source_cell = short_source(&s.source);
        let row = format_row(&[
            (&s.name, name_w),
            (scope_cell, scope_w),
            (&s.updated_ago, updated_w),
            (&source_cell, 0),
        ]);
        println!("{}", row);
    }
    println!();
}

fn print_assets_table(title: &str, rows: &[AssetEntry], merged: bool, color: bool) {
    if rows.is_empty() {
        return;
    }
    if color {
        println!("{ACCENT}{}{RESET}", title);
    } else {
        println!("{}", title);
    }

    let name_w = column_width("NAME", rows.iter().map(|a| a.name.as_str()));
    let scope_w = if merged {
        column_width("SCOPE", rows.iter().map(|a| scope_label(a.scope)))
    } else {
        0
    };

    print_header(
        &[("NAME", name_w), ("SCOPE", scope_w), ("SOURCE", 0)],
        color,
    );

    for a in rows {
        let scope_cell = if merged { scope_label(a.scope) } else { "" };
        let source_cell = short_source(&a.source);
        println!(
            "{}",
            format_row(&[
                (&a.name, name_w),
                (scope_cell, scope_w),
                (&source_cell, 0),
            ])
        );
    }
    println!();
}

fn column_width<'a>(header: &'a str, values: impl Iterator<Item = &'a str>) -> usize {
    values.map(str::len).max().unwrap_or(0).max(header.len())
}

fn print_header(cols: &[(&str, usize)], color: bool) {
    let row = format_row(
        &cols
            .iter()
            .map(|(label, width)| (*label, *width))
            .collect::<Vec<_>>(),
    );
    if color {
        println!("{SECONDARY}{}{RESET}", row);
    } else {
        println!("{}", row);
    }
}

fn format_row(cells: &[(&str, usize)]) -> String {
    let mut out = String::new();
    let last = cells.len().saturating_sub(1);
    for (i, (value, width)) in cells.iter().enumerate() {
        if *width == 0 && i != last {
            continue;
        }
        if i == last {
            out.push_str(value);
        } else if *width > 0 {
            out.push_str(&format!("{:<width$}", value, width = width));
            out.push_str("  ");
        }
    }
    out
}

fn load_skills_mcps_commands(
    scope_override: Option<Scope>,
    project_root: &std::path::Path,
) -> Result<(Vec<InstalledSkill>, Vec<AssetEntry>, Vec<AssetEntry>)> {
    if let Some(s) = scope_override {
        let scope = resolve_scope(Some(s), None);
        let lock = load_lock(scope, project_root)?;
        let runtime = load_runtime_state(scope, project_root)?;
        return Ok((
            installed_skills_from_lock(&lock, &runtime, scope, project_root, false),
            mcp_asset_entries(&lock, scope),
            command_asset_entries(&lock, scope),
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
    let mut commands = command_asset_entries(&global_lock, Scope::Global);
    commands.extend(command_asset_entries(&project_lock, Scope::Project));
    commands.sort_by_cached_key(|m| (m.name.to_lowercase(), scope_ord(m.scope)));
    Ok((skills, mcps, commands))
}

fn command_asset_entries(lock: &LockFile, scope: Scope) -> Vec<AssetEntry> {
    let mut out: Vec<AssetEntry> = lock
        .assets
        .iter()
        .filter(|(_, a)| a.kind == "command")
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
