//! `kasetto remove` — drop sources (or named entries) from the config, then
//! prune them via sync. Mirrors `add`: kind-tagged, value-taking, repeatable
//! `--skill` / `--mcp` / `--command` flags subtract named entries; a lone `*`
//! value drops that kind's whole entry; no kind flags at all removes the source
//! from every list it appears in.

use std::fs;

use crate::colors::{INFO, RESET};
use crate::error::{err, Result};
use crate::model::Scope;
use crate::source::{derive_browse_url, BrowseDerived};
use crate::ui::{print_json, print_tip};

use crate::fsops::{remove_item, remove_names, RemoveOutcome, Section};

pub(crate) struct RemoveOptions<'a> {
    pub source: &'a str,
    pub skills: &'a [String],
    pub mcps: &'a [String],
    pub commands: &'a [String],
    pub git_ref: Option<&'a str>,
    pub branch: Option<&'a str>,
    pub sub_dir: Option<&'a str>,
    pub config: Option<&'a str>,
    pub scope_override: Option<Scope>,
    pub no_sync: bool,
    pub dry_run: bool,
    pub locked: bool,
    pub as_json: bool,
    pub quiet: u8,
    pub plain: bool,
}

/// One applied edit, for the confirmation line: what was dropped and from where.
struct Removed {
    target: String,
    section: &'static str,
}

pub(crate) fn run(opts: &RemoveOptions) -> Result<()> {
    let path = super::source_edit::resolve_local_config_path(opts.config)?;
    if !path.exists() {
        return Err(err(format!("config not found: {}", path.display())));
    }
    let mut text = fs::read_to_string(&path)
        .map_err(|e| err(format!("failed to read {}: {e}", path.display())))?;

    // Strip a trailing `@<ref>` shorthand so `remove foo@v1` matches `--ref v1`.
    let (raw_source, at_ref) = super::source_edit::split_at_ref(opts.source);
    if at_ref.is_some() && (opts.git_ref.is_some() || opts.branch.is_some()) {
        return Err(err(
            "`@<ref>` shorthand conflicts with --ref/--branch; pass only one",
        ));
    }

    // A deep browse URL writes `source` + `ref`/`branch` + `sub-dir` (see
    // `add`); honor the same identity here so a pasted deep URL targets the
    // exact entry. Explicit flags override the derived pieces.
    let derived = derive_browse_url(&raw_source).unwrap_or_else(|| BrowseDerived {
        source: raw_source.clone(),
        ..Default::default()
    });
    let source = derived.source.clone();
    let derived_pin = derived.git_ref.as_deref().or(derived.branch.as_deref());
    let pin = opts
        .git_ref
        .or(opts.branch)
        .or(at_ref.as_deref())
        .or(derived_pin);
    let sub_dir = opts.sub_dir.or(derived.sub_dir.as_deref());

    let kinds = [
        (Section::Skills, opts.skills),
        (Section::Mcps, opts.mcps),
        (Section::Commands, opts.commands),
    ];
    let any_kind = kinds.iter().any(|(_, names)| !names.is_empty());

    let removed = if any_kind {
        remove_by_kind(&mut text, &source, pin, sub_dir, &kinds)?
    } else {
        remove_whole_source(&mut text, &source, pin, sub_dir)?
    };

    if opts.dry_run {
        emit_result(opts, &removed, true)?;
        return Ok(());
    }

    fs::write(&path, &text).map_err(|e| err(format!("failed to write {}: {e}", path.display())))?;
    emit_result(opts, &removed, false)?;

    if !opts.no_sync {
        super::source_edit::sync_after(
            &path,
            opts.scope_override,
            opts.quiet,
            opts.plain,
            opts.locked,
        )?;
    } else if !opts.as_json && opts.quiet == 0 {
        print_tip("run `kasetto sync` to prune the removed assets", opts.plain);
    }
    Ok(())
}

/// No kind flags: drop the source's entry from every list it appears in.
fn remove_whole_source(
    text: &mut String,
    source: &str,
    pin: Option<&str>,
    sub_dir: Option<&str>,
) -> Result<Vec<Removed>> {
    let mut removed = Vec::new();
    for section in [Section::Skills, Section::Mcps, Section::Commands] {
        // MCP entries never carry sub-dir; don't let a deep-URL sub-dir filter
        // them out when the user is dropping the source from every list.
        let section_sub = if section == Section::Mcps {
            None
        } else {
            sub_dir
        };
        let (updated, did) = remove_item(text, section, source, pin, section_sub)?;
        if did {
            *text = updated;
            removed.push(Removed {
                target: source.to_string(),
                section: section.key(),
            });
        }
    }
    if removed.is_empty() {
        return Err(err(format!(
            "`{source}` not found in any list (entries inherited via `extends` must be removed in the parent)"
        )));
    }
    Ok(removed)
}

/// Named kind flags: subtract names per section. A lone `*` value drops the
/// whole entry for that kind. An explicitly named section with no entry errors.
fn remove_by_kind(
    text: &mut String,
    source: &str,
    pin: Option<&str>,
    sub_dir: Option<&str>,
    kinds: &[(Section, &[String])],
) -> Result<Vec<Removed>> {
    let mut removed = Vec::new();
    for (section, names) in kinds {
        if names.is_empty() {
            continue;
        }
        // MCP entries never carry sub-dir (the schema has no such field there);
        // pass None to avoid filtering MCPs out when sub-dir came from a deep URL.
        let section_sub = if *section == Section::Mcps {
            None
        } else {
            sub_dir
        };
        if names.len() == 1 && names[0] == "*" {
            let (updated, did) = remove_item(text, *section, source, pin, section_sub)?;
            if !did {
                return Err(err(format!("`{source}` not found in `{}:`", section.key())));
            }
            *text = updated;
            removed.push(Removed {
                target: source.to_string(),
                section: section.key(),
            });
            continue;
        }
        let (updated, outcome) = remove_names(text, *section, source, pin, section_sub, names)?;
        match outcome {
            RemoveOutcome::NotFound => {
                return Err(err(format!("`{source}` not found in `{}:`", section.key())));
            }
            RemoveOutcome::WholeItem => {
                *text = updated;
                removed.push(Removed {
                    target: source.to_string(),
                    section: section.key(),
                });
            }
            RemoveOutcome::Names(ns) => {
                *text = updated;
                removed.push(Removed {
                    target: ns.join(", "),
                    section: section.key(),
                });
            }
        }
    }
    Ok(removed)
}

/// JSON or text confirmation. JSON emits a structured list; text prints one
/// uncolored line per removal (only the target is colored), matching the
/// kasetto-cli-style edit-line conventions.
fn emit_result(opts: &RemoveOptions, removed: &[Removed], dry: bool) -> Result<()> {
    if opts.as_json {
        let items: Vec<_> = removed
            .iter()
            .map(|r| serde_json::json!({"target": r.target, "section": r.section}))
            .collect();
        print_json(&serde_json::json!({
            "action": if dry { "would_remove" } else { "removed" },
            "items": items,
            "dry_run": dry,
        }))?;
        return Ok(());
    }
    if opts.quiet > 0 {
        return Ok(());
    }
    // Present continuous for the in-progress edit line (cargo precedent); the
    // sync summary that follows carries the past-tense `Removed N items` closer.
    let verb = if dry { "Would remove" } else { "Removing" };
    for r in removed {
        if crate::ui::color_stdout_enabled() {
            println!("{verb} {INFO}{}{RESET} from {}", r.target, r.section);
        } else {
            println!("{verb} {} from {}", r.target, r.section);
        }
    }
    Ok(())
}
