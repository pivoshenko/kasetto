use std::io::IsTerminal;

use crate::banner::print_banner;
use crate::colors::{RESET, SECONDARY, WARNING_EMPHASIS};
use crate::error::Result;
use crate::fsops::{resolve_dest, scope_root};
use crate::list::{
    browse as browse_list, command_asset_entries, mcp_asset_entries, AssetEntry, BrowseInput,
};
use crate::lock::{load_lock, LockFile};
use crate::model::{resolve_scope, InstalledSkill, Scope};
use crate::profile::{format_updated_ago, read_skill_profile};
use crate::state::{load_runtime_state, RuntimeState};
use crate::ui::{animations_enabled, print_json, print_section_header};

pub(crate) fn run(
    as_json: bool,
    plain: bool,
    quiet: bool,
    scope_override: Option<Scope>,
) -> Result<()> {
    if quiet && !as_json {
        return Ok(());
    }

    let animate = animations_enabled(quiet, as_json, plain);
    let color = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() && !plain;

    let project_root = std::env::current_dir().unwrap_or_default();
    let merged = scope_override.is_none();
    let (skills, mcps, commands) = load_skills_mcps_commands(scope_override, &project_root)?;

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
        if !quiet {
            if plain || !animate {
                println!("kasetto | カセット");
            } else {
                print_banner();
            }
            println!("Nothing installed.");
        }
        return Ok(());
    }

    if std::io::stdout().is_terminal() && std::env::var_os("NO_TUI").is_none() && !plain {
        browse_list(&BrowseInput {
            skills,
            mcps,
            commands,
            plain,
        })?;
        return Ok(());
    }

    if !quiet {
        if plain || !animate {
            println!("kasetto | カセット");
        } else {
            print_banner();
        }
    }

    print_skills(&skills, merged, color);
    print_mcps(&mcps, merged, color);
    print_commands(&commands, merged, color);

    Ok(())
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

fn print_skills(skills: &[InstalledSkill], merged: bool, color: bool) {
    if skills.is_empty() {
        return;
    }
    print_section_header("Skills", skills.len(), color);
    println!();
    for item in skills {
        let scope_note = if merged {
            format!(" [{}]", scope_label(item.scope))
        } else {
            String::new()
        };
        if color {
            println!(
                "  {WARNING_EMPHASIS}{}{}{RESET}  {SECONDARY}updated {} ({}){RESET}",
                item.name, scope_note, item.updated_ago, item.updated_at,
            );
        } else {
            println!(
                "  {}{}  updated {} ({})",
                item.name, scope_note, item.updated_ago, item.updated_at,
            );
        }
        println!("    {}", item.description);
        println!("    source: {}", item.source);
        println!();
    }
}

fn print_mcps(mcps: &[AssetEntry], merged: bool, color: bool) {
    if mcps.is_empty() {
        return;
    }
    print_section_header("MCP Servers", mcps.len(), color);
    println!();
    for m in mcps {
        let scope_note = if merged {
            format!(" [{}]", scope_label(m.scope))
        } else {
            String::new()
        };
        if m.pack_file.is_empty() && m.source.is_empty() {
            println!("  {}{}", m.name, scope_note);
        } else if m.pack_file.is_empty() {
            println!("  {}{}  source {}", m.name, scope_note, m.source);
        } else {
            println!(
                "  {}{}  pack {}  source {}",
                m.name, scope_note, m.pack_file, m.source
            );
        }
    }
    println!();
}

fn print_commands(commands: &[AssetEntry], merged: bool, color: bool) {
    if commands.is_empty() {
        return;
    }
    print_section_header("Commands", commands.len(), color);
    println!();
    for c in commands {
        let scope_note = if merged {
            format!(" [{}]", scope_label(c.scope))
        } else {
            String::new()
        };
        if c.source.is_empty() {
            println!("  {}{}", c.name, scope_note);
        } else if color {
            println!(
                "  {WARNING_EMPHASIS}{}{}{RESET}  {SECONDARY}source {}{RESET}",
                c.name, scope_note, c.source
            );
        } else {
            println!("  {}{}  source {}", c.name, scope_note, c.source);
        }
    }
    println!();
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
