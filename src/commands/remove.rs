//! `kasetto remove` — drop sources (or named entries) from the config, then
//! prune them via sync. Mirrors `add`: kind-tagged, value-taking, repeatable
//! `--skill` / `--mcp` / `--command` flags subtract named entries; a lone `*`
//! value drops that kind's whole entry; no kind flags at all removes the source
//! from every list it appears in.

use std::fs;

use crate::colors::{INFO, RESET};
use crate::error::{err, Result};
use crate::model::Scope;
use crate::source::derive_browse_url;

use crate::fsops::{remove_item, remove_names, RemoveOutcome, Section};

pub(crate) struct RemoveOptions<'a> {
    pub source: &'a str,
    pub skills: &'a [String],
    pub mcps: &'a [String],
    pub commands: &'a [String],
    pub git_ref: Option<&'a str>,
    pub branch: Option<&'a str>,
    pub config: Option<&'a str>,
    pub scope_override: Option<Scope>,
    pub no_sync: bool,
    pub quiet: bool,
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

    // A deep browse URL identifies the same source `add` would have written.
    let source = derive_browse_url(opts.source)
        .map(|d| d.source)
        .unwrap_or_else(|| opts.source.to_string());
    let pin = opts.git_ref.or(opts.branch);

    let kinds = [
        (Section::Skills, opts.skills),
        (Section::Mcps, opts.mcps),
        (Section::Commands, opts.commands),
    ];
    let any_kind = kinds.iter().any(|(_, names)| !names.is_empty());

    let removed = if any_kind {
        remove_by_kind(&mut text, &source, pin, &kinds)?
    } else {
        remove_whole_source(&mut text, &source, pin)?
    };

    fs::write(&path, &text).map_err(|e| err(format!("failed to write {}: {e}", path.display())))?;

    if !opts.quiet {
        for r in &removed {
            print_removed(&r.target, r.section);
        }
    }

    if !opts.no_sync {
        super::source_edit::sync_after(&path, opts.scope_override, opts.quiet, opts.plain)?;
    }
    Ok(())
}

/// No kind flags: drop the source's entry from every list it appears in.
fn remove_whole_source(text: &mut String, source: &str, pin: Option<&str>) -> Result<Vec<Removed>> {
    let mut removed = Vec::new();
    for section in [Section::Skills, Section::Mcps, Section::Commands] {
        let (updated, did) = remove_item(text, section, source, pin)?;
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
    kinds: &[(Section, &[String])],
) -> Result<Vec<Removed>> {
    let mut removed = Vec::new();
    for (section, names) in kinds {
        if names.is_empty() {
            continue;
        }
        if names.len() == 1 && names[0] == "*" {
            let (updated, did) = remove_item(text, *section, source, pin)?;
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
        let (updated, outcome) = remove_names(text, *section, source, pin, names)?;
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

/// Terse, uncolored edit confirmation — only the target is colored; the sync
/// summary that follows carries the prune result (see kasetto-cli-style).
fn print_removed(target: &str, section: &str) {
    if crate::ui::color_stdout_enabled() {
        println!("Removed {INFO}{target}{RESET} from {section}");
    } else {
        println!("Removed {target} from {section}");
    }
}
