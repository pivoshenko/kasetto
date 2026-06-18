//! `kasetto add` — append one or more sources to the config, then sync them in.
//!
//! Kind-tagged repeatable flags (`--skill` / `--mcp` / `--command`) select both
//! the asset kind and the named entries, so a single `add` can touch several
//! sections of `kasetto.yaml` at once (a repo that ships skills + MCPs +
//! commands). The source may be a plain repo URL, a `<source>@<ref>` shorthand,
//! or a deep `blob`/`tree` browse URL — the latter is decomposed into `source` +
//! `ref`/`branch` + `sub-dir` (+ skill name for a `SKILL.md` link); explicit
//! flags override the derived pieces.

use std::fs;
use std::path::{Path, PathBuf};

use crate::colors::{INFO, RESET};
use crate::error::{err, Result};
use crate::fsops::{
    insert_item, item_exists, now_unix, select_targets, Pin, Section, Selector, SourceItem,
};
use crate::model::{Scope, SkillTarget, SkillsField, SourceSpec};
use crate::source::{derive_browse_url, materialize_source, BrowseDerived};
use crate::ui::{print_json, print_tip};

pub(crate) struct AddOptions<'a> {
    pub source: &'a str,
    pub skills: &'a [String],
    pub mcps: &'a [String],
    pub commands: &'a [String],
    pub instructions: &'a [String],
    pub git_ref: Option<&'a str>,
    pub branch: Option<&'a str>,
    pub sub_dir: Option<&'a str>,
    pub config: Option<&'a str>,
    pub scope_override: Option<Scope>,
    pub no_verify: bool,
    pub no_sync: bool,
    pub dry_run: bool,
    pub locked: bool,
    pub as_json: bool,
    pub quiet: u8,
    pub plain: bool,
}

/// One resolved section edit: which list, and the entry to insert there.
struct SectionEdit {
    section: Section,
    item: SourceItem,
}

pub(crate) fn run(opts: &AddOptions) -> Result<()> {
    if opts.git_ref.is_some() && opts.branch.is_some() {
        return Err(err("--ref and --branch are mutually exclusive"));
    }
    // `--locked` forbids fetching, but a brand-new source has no lock entry yet
    // — the follow-up sync would fail mid-flight after the manifest edit. Reject
    // the combination up front and point at the two valid workflows. cargo
    // follows the same "lock would need updating but --locked was passed" model.
    if opts.locked && !opts.no_sync {
        return Err(err(
            "`--locked` on `add` requires `--no-sync` — a newly added source \
             cannot be installed without fetching. Either pass `--no-sync --locked` \
             (edit the manifest only, then run `kasetto lock` + `kasetto sync --locked` \
             to install offline), or drop `--locked` to fetch the new source now.",
        ));
    }
    let path = super::source_edit::resolve_local_config_path(opts.config)?;

    // Strip cargo/uv-style `@<ref>` shorthand off the positional before any
    // URL decomposition. Explicit `--ref`/`--branch` win if also passed.
    let (raw_source, at_ref) = super::source_edit::split_at_ref(opts.source);
    if at_ref.is_some() && (opts.git_ref.is_some() || opts.branch.is_some()) {
        return Err(err(
            "`@<ref>` shorthand conflicts with --ref/--branch; pass only one",
        ));
    }

    // Decompose a deep browse URL; explicit flags below take precedence.
    let derived = derive_browse_url(&raw_source).unwrap_or_else(|| BrowseDerived {
        source: raw_source.clone(),
        ..Default::default()
    });
    let source = derived.source.clone();
    let pin = resolve_pin(opts, &derived, at_ref.as_deref());
    let sub_dir = opts
        .sub_dir
        .map(str::to_string)
        .or_else(|| derived.sub_dir.clone());

    let edits = plan_edits(opts, &source, &pin, sub_dir.as_deref(), &derived);

    let mut text = if path.exists() {
        fs::read_to_string(&path)
            .map_err(|e| err(format!("failed to read {}: {e}", path.display())))?
    } else {
        "# Kasetto - https://github.com/pivoshenko/kasetto\n".to_string()
    };

    for edit in &edits {
        if item_exists(&text, edit.section, &edit.item) {
            return Err(err(format!(
                "`{source}` is already in `{}:`; edit it directly or `kasetto remove` it first",
                edit.section.key()
            )));
        }
    }

    if opts.dry_run {
        emit_result(opts, &source, &edits, true)?;
        return Ok(());
    }

    if !opts.no_verify {
        verify_source(&source, &pin, sub_dir.as_deref(), &edits, &path)?;
    }

    for edit in &edits {
        text = insert_item(&text, edit.section, &edit.item)?;
    }
    fs::write(&path, &text).map_err(|e| err(format!("failed to write {}: {e}", path.display())))?;

    emit_result(opts, &source, &edits, false)?;

    if !opts.no_sync {
        super::source_edit::sync_after(
            &path,
            opts.scope_override,
            opts.quiet,
            opts.plain,
            opts.locked,
        )?;
    } else if !opts.as_json && opts.quiet == 0 {
        print_tip("run `kasetto sync` to install the new source", opts.plain);
    }
    Ok(())
}

fn resolve_pin(opts: &AddOptions, derived: &BrowseDerived, at_ref: Option<&str>) -> Pin {
    if let Some(r) = opts.git_ref {
        return Pin::Ref(r.to_string());
    }
    if let Some(b) = opts.branch {
        return Pin::Branch(b.to_string());
    }
    if let Some(r) = at_ref {
        return Pin::Ref(r.to_string());
    }
    if let Some(r) = &derived.git_ref {
        return Pin::Ref(r.clone());
    }
    if let Some(b) = &derived.branch {
        return Pin::Branch(b.clone());
    }
    Pin::None
}

/// Build the per-section edits from the kind flags, with these defaults:
/// - named `--skill`/`--mcp`/`--command` → that section as a list (a lone `*`
///   value becomes a wildcard);
/// - a `SKILL.md` browse URL with no `--skill` flags → a one-skill list;
/// - nothing specified at all → `skills: "*"` (the common "add this pack" case).
///
/// MCP entries never carry `sub-dir` (the schema has no such field there).
fn plan_edits(
    opts: &AddOptions,
    source: &str,
    pin: &Pin,
    sub_dir: Option<&str>,
    derived: &BrowseDerived,
) -> Vec<SectionEdit> {
    let skill_names: Vec<String> = if !opts.skills.is_empty() {
        opts.skills.to_vec()
    } else if let Some(name) = &derived.skill_name {
        vec![name.clone()]
    } else {
        Vec::new()
    };
    let nothing_specified = skill_names.is_empty()
        && opts.mcps.is_empty()
        && opts.commands.is_empty()
        && opts.instructions.is_empty();

    let mut edits = Vec::new();
    let mut push = |section: Section, selector: Selector| {
        let item_sub = if section == Section::Mcps {
            None
        } else {
            sub_dir.map(str::to_string)
        };
        edits.push(SectionEdit {
            section,
            item: SourceItem {
                source: source.to_string(),
                pin: pin.clone(),
                sub_dir: item_sub,
                selector,
            },
        });
    };

    if !skill_names.is_empty() {
        push(Section::Skills, selector_from(&skill_names));
    } else if nothing_specified {
        push(Section::Skills, Selector::Wildcard);
    }
    if !opts.mcps.is_empty() {
        push(Section::Mcps, selector_from(opts.mcps));
    }
    if !opts.commands.is_empty() {
        push(Section::Commands, selector_from(opts.commands));
    }
    if !opts.instructions.is_empty() {
        push(Section::Instructions, selector_from(opts.instructions));
    }
    edits
}

/// A lone `*` value means "discover everything"; any other list stays explicit.
fn selector_from(names: &[String]) -> Selector {
    if names.len() == 1 && names[0] == "*" {
        Selector::Wildcard
    } else {
        Selector::Names(names.to_vec())
    }
}

/// Fetch the source once to confirm it resolves before touching the config; for
/// named skill entries also assert each skill exists. MCP/command names are
/// validated later at sync time.
fn verify_source(
    source: &str,
    pin: &Pin,
    sub_dir: Option<&str>,
    edits: &[SectionEdit],
    config_path: &Path,
) -> Result<()> {
    let spec = SourceSpec {
        source: source.to_string(),
        branch: match pin {
            Pin::Branch(b) => Some(b.clone()),
            _ => None,
        },
        git_ref: match pin {
            Pin::Ref(r) => Some(r.clone()),
            _ => None,
        },
        sub_dir: sub_dir.map(str::to_string),
        skills: SkillsField::Wildcard("*".to_string()),
    };
    let cfg_dir = config_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let stage = std::env::temp_dir().join(format!("kasetto-add-{}", now_unix()));

    let materialized = materialize_source(&spec, &cfg_dir, &stage)?;

    let mut name_error = None;
    if let Some(names) = named_skills(edits) {
        let sf = SkillsField::List(names.iter().cloned().map(SkillTarget::Name).collect());
        match select_targets(&sf, &materialized.available, &materialized.source_root) {
            Ok((_, broken)) => {
                if let Some(b) = broken.first() {
                    name_error = Some(err(format!("skill `{}` not found in {source}", b.name)));
                }
            }
            Err(e) => name_error = Some(e),
        }
    }

    if let Some(dir) = materialized.cleanup_dir {
        let _ = fs::remove_dir_all(dir);
    }
    match name_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// The explicit skill names from the skills edit, if it is a named list.
fn named_skills(edits: &[SectionEdit]) -> Option<&Vec<String>> {
    edits
        .iter()
        .find_map(|e| match (&e.section, &e.item.selector) {
            (Section::Skills, Selector::Names(names)) => Some(names),
            _ => None,
        })
}

/// Print the action confirmation in the requested format. JSON output is
/// structured; text output is the terse, source-only-colored line that the
/// sync summary follows (see kasetto-cli-style).
fn emit_result(opts: &AddOptions, source: &str, edits: &[SectionEdit], dry: bool) -> Result<()> {
    if opts.as_json {
        let mut sections: Vec<&str> = edits.iter().map(|e| e.section.key()).collect();
        sections.dedup();
        print_json(&serde_json::json!({
            "action": if dry { "would_add" } else { "added" },
            "source": source,
            "sections": sections,
            "dry_run": dry,
        }))?;
        return Ok(());
    }
    if opts.quiet > 0 {
        return Ok(());
    }
    // Present continuous for the in-progress edit line — cargo says
    // `Adding serde v1.0... to dependencies`; the sync summary that follows
    // (`Installed N items in 84ms`) carries the past-tense closer.
    let verb = if dry { "Would add" } else { "Adding" };
    let mut sections: Vec<&str> = edits.iter().map(|e| e.section.key()).collect();
    sections.dedup();
    let sections = sections.join(", ");
    if crate::ui::color_stdout_enabled() {
        println!("{verb} {INFO}{source}{RESET} to {sections}");
    } else {
        println!("{verb} {source} to {sections}");
    }
    Ok(())
}
