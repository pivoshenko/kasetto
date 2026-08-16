//! Module that contains the shared terminal rendering primitives: headers, tree
//! rows, spinners, badges, and JSON output.

use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::colors::{
    ACCENT, ATTENTION, BRAND, CLEAR_LINE, ERROR, INFO, INFRA, RESET, SECONDARY, STRIKE,
    STRIKE_RESET, SUCCESS,
};
use crate::error::Result;

/// Braille spinner frames shared across all TUI surfaces.
pub(crate) const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Cassette brand mark: diamond. Used in the doctor header, the `self update`
/// finalizer (amber), and the `self uninstall` farewell (violet).
pub(crate) const BRAND_GLYPH: &str = "◆";

/// Brand flourish star, trails the `self uninstall` farewell.
pub(crate) const STAR_GLYPH: &str = "✦";

pub(crate) fn animations_enabled(quiet: bool, as_json: bool, plain: bool) -> bool {
    !quiet && !as_json && !plain && std::io::stderr().is_terminal()
}

/// Whether to emit colored output on stdout. Honors `CLICOLOR_FORCE` (set by
/// `--color always`) ahead of TTY / `NO_COLOR` detection.
pub(crate) fn color_stdout_enabled() -> bool {
    if std::env::var_os("CLICOLOR_FORCE").is_some() {
        return true;
    }
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

/// Whether to emit colored diagnostics on stderr. Mirrors
/// [`color_stdout_enabled`] but probes the stderr handle, so `kst sync > out`
/// keeps its `error:` lines colored on the terminal they actually reach. Used
/// by the top-level error handler, which runs outside any command's resolved
/// `plain` flag.
pub(crate) fn color_stderr_enabled() -> bool {
    if std::env::var_os("CLICOLOR_FORCE").is_some() {
        return true;
    }
    std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

/// Print a serializable value as pretty JSON.
pub(crate) fn print_json<T: serde::Serialize>(val: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(val)?);
    Ok(())
}

/// Print a uv-style `tip: <msg>` line in popil sky (`INFO`). Plain mode
/// omits color but keeps the prefix.
pub(crate) fn print_tip(msg: &str, plain: bool) {
    if plain {
        println!("tip: {msg}");
    } else {
        println!("{INFO}{ACCENT}tip:{RESET} {msg}");
    }
}

/// Print a uv-style `error:`-prefixed failure line to stderr, naming the asset
/// that failed. `name` is `None` for a source-level failure, where no single
/// asset is at fault. Used for every asset kind, so a broken MCP or command is
/// named just like a broken skill.
pub(crate) fn eprint_fail(name: Option<&str>, source: &str, reason: Option<&str>, plain: bool) {
    let what = match name {
        Some(n) => format!("failed to install {n} from {source}"),
        None => format!("failed to resolve {source}"),
    };
    let tail = reason.map_or_else(String::new, |r| format!(": {r}"));
    if plain {
        eprintln!("error: {what}{tail}");
    } else {
        eprintln!("{ERROR}{ACCENT}error:{RESET} {what}{SECONDARY}{tail}{RESET}");
    }
}

/// Print a uv-style `warning: <msg>` line to stderr in bold yellow.
pub(crate) fn eprint_warn(msg: &str, plain: bool) {
    if plain {
        eprintln!("warning: {msg}");
    } else {
        eprintln!("{ATTENTION}{ACCENT}warning:{RESET} {msg}");
    }
}

/// Print a uv-style `error: <msg>` line to stderr in bold red.
pub(crate) fn eprint_error(msg: &str, plain: bool) {
    if plain {
        eprintln!("error: {msg}");
    } else {
        eprintln!("{ERROR}{ACCENT}error:{RESET} {msg}");
    }
}

/// Run `operation` while animating a braille spinner on stderr. `transient =
/// true` wipes the spinner line on success (per-asset progress where a final
/// summary reports results); `false` leaves the label printed (long-running
/// single steps). Failure always emits a red `error: <label>` line so the
/// cause isn't lost.
fn spinner_run<T, F>(
    transient: bool,
    enabled: bool,
    plain: bool,
    label: impl Into<String>,
    operation: F,
) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    let label = label.into();
    if !enabled {
        return operation();
    }
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let thread_label = label.clone();
    let handle = thread::spawn(move || {
        let mut idx = 0usize;
        let mut stderr = std::io::stderr();
        while !stop_flag.load(Ordering::Relaxed) {
            let _ = write!(
                stderr,
                "{}{}{}{} {}",
                CLEAR_LINE,
                ATTENTION,
                SPINNER_FRAMES[idx % SPINNER_FRAMES.len()],
                RESET,
                thread_label
            );
            let _ = stderr.flush();
            idx = idx.wrapping_add(1);
            thread::sleep(Duration::from_millis(80));
        }
    });
    let result = operation();
    stop.store(true, Ordering::Relaxed);
    // Best-effort: a panic in the cosmetic spinner thread is intentionally swallowed here so it
    // never surfaces to or aborts the real command whose result we return below
    let _ = handle.join();
    let mut stderr = std::io::stderr();
    match (&result, transient, plain) {
        (Err(_), _, true) => {
            let _ = writeln!(stderr, "error: {}", label);
        }
        (Err(_), _, false) => {
            let _ = writeln!(stderr, "{}{ERROR}{ACCENT}error:{RESET} {label}", CLEAR_LINE);
        }
        (Ok(_), true, _) => {
            let _ = write!(stderr, "{}", CLEAR_LINE);
        }
        (Ok(_), false, true) => {
            let _ = writeln!(stderr, "{}", label);
        }
        (Ok(_), false, false) => {
            let _ = writeln!(stderr, "{}{label}", CLEAR_LINE);
        }
    }
    let _ = stderr.flush();
    result
}

/// Spinner that clears its line on success (per-asset progress).
pub(crate) fn with_spinner_transient<T, F>(
    enabled: bool,
    plain: bool,
    label: impl Into<String>,
    operation: F,
) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    spinner_run(true, enabled, plain, label, operation)
}

/// Spinner that leaves the label printed on success (single long step).
pub(crate) fn with_spinner<T, F>(
    enabled: bool,
    plain: bool,
    label: impl Into<String>,
    operation: F,
) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    spinner_run(false, enabled, plain, label, operation)
}

/// Pick the singular or plural noun for `n`. Every count the CLI narrates goes
/// through here; summary lines that hardcoded the plural used to print
/// `Resolved 1 sources`.
pub(crate) fn pluralize(n: usize, one: &'static str, many: &'static str) -> &'static str {
    if n == 1 {
        one
    } else {
        many
    }
}

/// Strip the URL scheme + leading `www.` so source labels read like
/// `github.com/org/repo` instead of `https://github.com/org/repo`.
pub(crate) fn short_source(source: &str) -> String {
    let s = source
        .strip_prefix("https://")
        .or_else(|| source.strip_prefix("http://"))
        .unwrap_or(source);
    s.strip_prefix("www.").unwrap_or(s).to_string()
}

/// Single-glyph prefix for a per-asset sync action. The cassette design
/// dialect: `+` install (green), `↑` update (amber), `−` remove (red),
/// `✓` unchanged (faint), `!` broken/failed (red). Plain colored (no bold)
/// per terminal.jsx Row glyph styling.
pub(crate) fn action_glyph(status: &str, plain: bool) -> String {
    let (glyph, color): (&str, &str) = match status {
        "installed" | "would_install" => ("+", SUCCESS),
        "updated" | "would_update" => ("↑", ATTENTION),
        "removed" | "would_remove" => ("−", ERROR),
        "unchanged" => ("✓", INFRA),
        "broken" | "source_error" => ("!", ERROR),
        _ => ("?", ERROR),
    };
    if plain {
        glyph.to_string()
    } else {
        format!("{color}{glyph}{RESET}")
    }
}

/// Width of the status-label column in a source-grouped tree. The label
/// vocabulary is closed (`added`/`updated`/`removed`/`unchanged`/`broken`, and
/// dry-run reuses the same five words), so this is a constant rather than
/// something measured from the rows — no name can ever push the column.
pub(crate) const STATUS_LABEL_W: usize = 9;

/// Past-tense status verb alone, colored per role. The word that sits in the
/// [`STATUS_LABEL_W`] column, left of the asset name.
pub(crate) fn status_label(status: &str, plain: bool) -> String {
    let (label, color): (&str, &str) = match status {
        "installed" | "would_install" => ("added", SUCCESS),
        "updated" | "would_update" => ("updated", ATTENTION),
        "removed" | "would_remove" => ("removed", ERROR),
        "unchanged" => ("unchanged", INFRA),
        "broken" | "source_error" => ("broken", ERROR),
        other => return other.to_string(),
    };
    if plain {
        label.to_string()
    } else {
        format!("{color}{label}{RESET}")
    }
}

/// Dim version metadata that trails the asset name: `v1.0.0` on an install,
/// `2.1.0 → 2.2.0` on an update, empty otherwise. Split out from
/// [`status_label`] because it follows the name (uv-style) while the label
/// precedes it.
pub(crate) fn status_detail(
    status: &str,
    version_from: Option<&str>,
    version_to: Option<&str>,
    plain: bool,
) -> String {
    let text = match status {
        "installed" | "would_install" => version_to.map(|v| format!("v{v}")),
        "updated" | "would_update" => match (version_from, version_to) {
            (Some(f), Some(t)) => Some(format!("{f} → {t}")),
            _ => None,
        },
        _ => None,
    };
    match (text, plain) {
        (None, _) => String::new(),
        (Some(t), true) => t,
        (Some(t), false) => format!("{SECONDARY}{t}{RESET}"),
    }
}

/// Amber, uppercase, letter-spaced section header per design: `SKILLS   23 installed`.
/// `count_unit` is `(count, "installed")` for inline metadata, always separated
/// by the same three spaces. Body content follows immediately, no trailing blank.
///
/// `lead_blank` inserts the separating blank line above the header. Pass `false`
/// for the first section a command prints, so no command's output opens on a
/// blank line, and `true` for every section after it. Panels whose header always
/// trails other content (the `doctor` groups, which sit under its head line)
/// pass `true` throughout.
pub(crate) fn print_section_header(
    label: &str,
    count_unit: Option<(usize, &str)>,
    lead_blank: bool,
    plain: bool,
) {
    if lead_blank {
        println!();
    }
    let label_up = label.to_uppercase();
    if plain {
        match count_unit {
            Some((n, unit)) => println!("{label_up}   {n} {unit}"),
            None => println!("{label_up}"),
        }
        return;
    }
    match count_unit {
        Some((n, unit)) => {
            println!("{ACCENT}{ATTENTION}{label_up}{RESET}   {SECONDARY}{n} {unit}{RESET}")
        }
        None => println!("{ACCENT}{ATTENTION}{label_up}{RESET}"),
    }
}

/// Per-source header: status glyph + cyan `org/repo`. Optional right-aligned
/// faint item count padded to `right_col_at` characters. `done = Some(true)`
/// → ✓ green, `Some(false)` → • faint idle, `None` → no leading glyph.
pub(crate) fn print_source_header(
    repo: &str,
    count: Option<usize>,
    done: Option<bool>,
    right_col_at: Option<usize>,
    plain: bool,
) {
    let glyph_plain = match done {
        Some(true) => "✓",
        Some(false) => "•",
        None => " ",
    };
    if plain {
        match (count, right_col_at) {
            (Some(n), Some(col)) => {
                let prefix = format!("{glyph_plain} {repo}");
                let pad = right_col_pad(col, prefix.chars().count() + n.to_string().len());
                println!("{prefix}{}{n}", " ".repeat(pad));
            }
            (Some(n), None) => println!("{glyph_plain} {repo}   {n}"),
            (None, _) => println!("{glyph_plain} {repo}"),
        }
        return;
    }
    let glyph_colored = match done {
        Some(true) => format!("{SUCCESS}{glyph_plain}{RESET}"),
        Some(false) => format!("{INFRA}{glyph_plain}{RESET}"),
        None => " ".to_string(),
    };
    match (count, right_col_at) {
        (Some(n), Some(col)) => {
            // Visible width = "✓ repo" + " " + "N"; pad whitespace between repo and N
            let visible_prefix = 1 + 1 + repo.chars().count();
            let n_str = n.to_string();
            let pad = right_col_pad(col, visible_prefix + n_str.chars().count());
            println!(
                "{glyph_colored} {INFO}{repo}{RESET}{}{INFRA}{n_str}{RESET}",
                " ".repeat(pad)
            );
        }
        (Some(n), None) => println!("{glyph_colored} {INFO}{repo}{RESET}   {INFRA}{n}{RESET}"),
        (None, _) => println!("{glyph_colored} {INFO}{repo}{RESET}"),
    }
}

/// Whitespace between a row's left text and its right-aligned tail, so the
/// tail lands on column `col`. Never returns 0: a row whose content already
/// overruns `col` keeps a two-space gutter instead of fusing `pathwritable`.
fn right_col_pad(col: usize, content_w: usize) -> usize {
    col.saturating_sub(content_w).max(2)
}

/// Status tree leaf, uv-style: `├─ ↑ updated    blog-write  2.1.0 → 2.2.0`.
/// Branch, then the status glyph, then the status label in a fixed
/// [`STATUS_LABEL_W`] column, then the name, then optional dim detail. Every
/// column left of the name is constant width, so names always start at the
/// same offset no matter how long any row's name is — the reason this shape is
/// preferred over a right-aligned status column.
pub(crate) fn print_status_leaf(
    is_last: bool,
    status: &str,
    name: &str,
    detail: &str,
    plain: bool,
) {
    let branch = if is_last { "└─" } else { "├─" };
    let glyph = action_glyph(status, plain);
    let label = status_label(status, plain);
    // The colored label carries ANSI, so pad from the plain word's width
    let label_pad = STATUS_LABEL_W.saturating_sub(status_label(status, true).chars().count());
    let name_styled = if plain || !matches!(status, "removed" | "would_remove") {
        name.to_string()
    } else {
        format!("{INFRA}{STRIKE}{name}{STRIKE_RESET}{RESET}")
    };
    let detail_part = if detail.is_empty() {
        String::new()
    } else {
        format!("  {detail}")
    };
    let branch_styled = if plain {
        branch.to_string()
    } else {
        format!("{INFRA}{branch}{RESET}")
    };
    println!(
        "{branch_styled} {glyph} {label}{}  {name_styled}{detail_part}",
        " ".repeat(label_pad)
    );
}

/// Tree leaf row: `├─` for non-last, `└─` for last, then the name. The branch
/// is dim; the name inherits the terminal foreground.
pub(crate) fn print_tree_leaf(is_last: bool, name: &str, plain: bool) {
    let branch = if is_last { "└─" } else { "├─" };
    if plain {
        println!("{branch} {name}");
        return;
    }
    println!("{INFRA}{branch}{RESET} {name}");
}

/// Render the per-action summary chips shown beneath a sync's lead verb line:
/// `● N updated  ● N added  ● N removed  ● N unchanged`. The dot inherits the
/// role color (amber/green/red/dim) and the count is bold.
///
/// `broken` covers everything that did not land (missing assets plus source
/// resolution failures), matching the single `broken` word the tree labels both
/// with. Unlike the other four it is omitted at zero, so a clean sync keeps the
/// four-chip strip and the red chip only ever appears as a signal.
pub(crate) fn print_sync_chips(
    updated: usize,
    added: usize,
    removed: usize,
    unchanged: usize,
    broken: usize,
    plain: bool,
) {
    if plain {
        let tail = if broken > 0 {
            format!("  {broken} broken")
        } else {
            String::new()
        };
        println!(
            "  {updated} updated  {added} added  {removed} removed  {unchanged} unchanged{tail}"
        );
        return;
    }
    let tail = if broken > 0 {
        format!("  {ERROR}●{RESET} {broken} {SECONDARY}broken{RESET}")
    } else {
        String::new()
    };
    println!(
        "  {ATTENTION}●{RESET} {updated} {SECONDARY}updated{RESET}  \
         {SUCCESS}●{RESET} {added} {SECONDARY}added{RESET}  \
         {ERROR}●{RESET} {removed} {SECONDARY}removed{RESET}  \
         {INFRA}●{RESET} {unchanged} {SECONDARY}unchanged{RESET}{tail}"
    );
}

/// `◆ kasetto vX.Y.Z                          ✓ healthy`. Doctor head per
/// design. Amber diamond + bold `kasetto`, dim version, right-aligned green
/// `✓ healthy` badge at column ~62.
pub(crate) fn print_doctor_head(version: &str, healthy: bool, plain: bool) {
    const COL: usize = 62;
    let left_plain = format!("◆ kasetto v{version}");
    let badge_plain = if healthy { "✓ healthy" } else { "✗ issues" };
    let pad = right_col_pad(
        COL,
        left_plain.chars().count() + badge_plain.chars().count(),
    );
    if plain {
        println!("{left_plain}{}{badge_plain}", " ".repeat(pad));
        return;
    }
    let badge = if healthy {
        format!("{SUCCESS}✓ healthy{RESET}")
    } else {
        format!("{ERROR}✗ issues{RESET}")
    };
    println!(
        "{ATTENTION}{ACCENT}{BRAND_GLYPH} kasetto{RESET} {SECONDARY}v{version}{RESET}{}{badge}",
        " ".repeat(pad),
    );
}

/// Replace `$HOME` prefix in `path` with `~`. Anything else returns unchanged.
pub(crate) fn relativize_home(path: &str) -> String {
    if let Ok(home) = std::env::var("HOME") {
        if let Some(stripped) = path.strip_prefix(&home) {
            if stripped.is_empty() {
                return "~".to_string();
            }
            return format!("~{stripped}");
        }
    }
    path.to_string()
}

/// Print a `KEY  value` row for the cassette doctor panel: key in foreground
/// (no color), padded to `key_w` chars, value in the supplied color (default
/// foreground; pass `Some(ATTENTION)` for INVENTORY counts).
pub(crate) fn print_doctor_kv(
    key: &str,
    value: &str,
    key_w: usize,
    value_color: Option<&str>,
    plain: bool,
) {
    if plain {
        println!("{key:<key_w$}  {value}");
        return;
    }
    match value_color {
        Some(c) => println!("{key:<key_w$}  {c}{value}{RESET}"),
        None => println!("{key:<key_w$}  {value}"),
    }
}

/// `✓ Sentence`. A single check row in the CHECKS section.
pub(crate) fn print_check(passed: bool, label: &str, plain: bool) {
    let glyph = if passed { "✓" } else { "✗" };
    if plain {
        println!("{glyph} {label}");
        return;
    }
    let color = if passed { SUCCESS } else { ERROR };
    println!("{color}{glyph}{RESET} {label}");
}

/// `✓ ~/.foo/bar               writable`. Command-directory row with
/// right-aligned faint trailing tag.
pub(crate) fn print_dir_row(path: &str, writable: bool, plain: bool) {
    const COL: usize = 62;
    let tag = if writable { "writable" } else { "not writable" };
    let path_relative = relativize_home(path);
    let visible_left = 1 + 1 + path_relative.chars().count();
    let pad = right_col_pad(COL, visible_left + tag.chars().count());
    if plain {
        let glyph = if writable { "✓" } else { "✗" };
        println!("{glyph} {path_relative}{}{tag}", " ".repeat(pad));
        return;
    }
    let (glyph_color, tag_color) = if writable {
        (SUCCESS, INFRA)
    } else {
        (ERROR, ERROR)
    };
    let glyph = if writable { "✓" } else { "✗" };
    println!(
        "{glyph_color}{glyph}{RESET} {SECONDARY}{path_relative}{RESET}{}{tag_color}{tag}{RESET}",
        " ".repeat(pad)
    );
}

/// `◆ Updated to vNEW  was vOLD`. Self update finalizer (amber diamond).
pub(crate) fn print_update_closer(new: &str, old: &str, plain: bool) {
    if plain {
        println!("Updated to v{new}  was v{old}");
        return;
    }
    println!(
        "{ATTENTION}{BRAND_GLYPH}{RESET} {ACCENT}Updated to v{new}{RESET}{SECONDARY}  was v{old}{RESET}"
    );
}

/// Violet `◆ kasetto vX uninstalled` + amber `またね` farewell. Self uninstall closer.
pub(crate) fn print_uninstall_closer(version: &str, plain: bool) {
    if plain {
        println!("kasetto v{version} uninstalled");
        println!("Thanks for using kasetto.  またね");
        return;
    }
    println!("{BRAND}{BRAND_GLYPH}{RESET} {ACCENT}kasetto v{version} uninstalled{RESET}");
    println!(
        "  {SECONDARY}Thanks for using kasetto.{RESET}  {ATTENTION}またね{RESET}  {SECONDARY}{STAR_GLYPH}{RESET}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_glyph_plain_uses_design_dialect() {
        assert_eq!(action_glyph("installed", true), "+");
        assert_eq!(action_glyph("updated", true), "↑");
        assert_eq!(action_glyph("removed", true), "−");
        assert_eq!(action_glyph("unchanged", true), "✓");
        assert_eq!(action_glyph("broken", true), "!");
        assert_eq!(action_glyph("source_error", true), "!");
    }

    #[test]
    fn status_labels_all_fit_the_fixed_column() {
        // The whole point of a constant column: no label may overflow it, or
        // names stop starting at the same offset
        for status in [
            "installed",
            "would_install",
            "updated",
            "would_update",
            "removed",
            "would_remove",
            "unchanged",
            "broken",
            "source_error",
        ] {
            let w = status_label(status, true).chars().count();
            assert!(w <= STATUS_LABEL_W, "{status} label is {w} wide");
        }
        assert_eq!(
            status_label("unchanged", true).chars().count(),
            STATUS_LABEL_W
        );
    }

    #[test]
    fn status_detail_carries_versions_and_is_empty_otherwise() {
        assert_eq!(
            status_detail("installed", None, Some("1.0.0"), true),
            "v1.0.0"
        );
        assert_eq!(
            status_detail("updated", Some("2.1.0"), Some("2.2.0"), true),
            "2.1.0 → 2.2.0"
        );
        assert_eq!(status_detail("updated", None, None, true), "");
        assert_eq!(status_detail("unchanged", None, None, true), "");
        assert_eq!(status_detail("removed", None, None, true), "");
    }

    #[test]
    fn right_col_pad_keeps_a_gutter_when_content_overruns_the_column() {
        assert_eq!(right_col_pad(62, 40), 22);
        // overrun: never 0, or the tail fuses onto the left text
        assert_eq!(right_col_pad(62, 62), 2);
        assert_eq!(right_col_pad(62, 90), 2);
    }

    #[test]
    fn pluralize_switches_only_on_one() {
        assert_eq!(pluralize(0, "source", "sources"), "sources");
        assert_eq!(pluralize(1, "source", "sources"), "source");
        assert_eq!(pluralize(2, "source", "sources"), "sources");
    }

    #[test]
    fn with_spinner_disabled_executes_operation_and_returns_result() {
        let result = with_spinner(false, true, "Syncing demo", || {
            Ok::<_, crate::error::Error>(42)
        })
        .expect("operation");
        assert_eq!(result, 42);
    }
}
