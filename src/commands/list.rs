use std::io::IsTerminal;

use crate::banner::print_banner;
use crate::colors::{RESET, SECONDARY, WARNING_EMPHASIS};
use crate::error::Result;
use crate::list::{browse as browse_list, mcp_asset_entries, BrowseInput};
use crate::lock::load_lock;
use crate::model::{resolve_scope, InstalledSkill, Scope};
use crate::profile::{format_updated_ago, list_color_enabled, read_skill_profile};
use crate::ui::{print_json, print_section_header};

pub(crate) fn run(as_json: bool, scope_override: Option<Scope>) -> Result<()> {
    let scope = resolve_scope(scope_override, None);
    let project_root = std::env::current_dir().unwrap_or_default();
    let lock = load_lock(scope, &project_root)?;

    let state = lock.state();
    let managed_mcps = lock.list_installed_mcps();

    let mut skills = Vec::new();
    for (id, entry) in &state.skills {
        let (name, fallback_description) = read_skill_profile(&entry.destination, &entry.skill);
        let description = if entry.description.trim().is_empty() {
            fallback_description
        } else {
            entry.description.clone()
        };
        let updated_ago = format_updated_ago(&entry.updated_at);
        skills.push(InstalledSkill {
            id: id.clone(),
            name,
            description,
            source: entry.source.clone(),
            skill: entry.skill.clone(),
            destination: entry.destination.clone(),
            hash: entry.hash.clone(),
            source_revision: entry.source_revision.clone(),
            updated_at: entry.updated_at.clone(),
            updated_ago,
        });
    }
    skills.sort_by_cached_key(|s| s.name.to_lowercase());

    if as_json {
        let output = serde_json::json!({
            "skills": skills,
            "mcps": managed_mcps,
        });
        return print_json(&output);
    }

    let has_anything = !skills.is_empty() || !managed_mcps.is_empty();

    if !has_anything {
        print_banner();
        println!("Nothing installed.");
        return Ok(());
    }

    if std::io::stdout().is_terminal() && std::env::var_os("NO_TUI").is_none() {
        let mcps = mcp_asset_entries(&managed_mcps);
        browse_list(&BrowseInput { skills, mcps })?;
        return Ok(());
    }

    print_banner();
    let color = list_color_enabled();

    if !skills.is_empty() {
        print_section_header("Skills", skills.len(), color);
        println!();
        for item in &skills {
            if color {
                println!(
                    "  {WARNING_EMPHASIS}{}{RESET}  {SECONDARY}updated {} ({}){RESET}",
                    item.name, item.updated_ago, item.updated_at
                );
            } else {
                println!(
                    "  {}  updated {} ({})",
                    item.name, item.updated_ago, item.updated_at
                );
            }
            println!("    {}", item.description);
            println!("    source: {}", item.source);
            println!();
        }
    }

    if !managed_mcps.is_empty() {
        print_section_header("MCP Servers", managed_mcps.len(), color);
        for name in &managed_mcps {
            println!("  {name}");
        }
        println!();
    }

    Ok(())
}
