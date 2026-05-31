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

/// Cassette brand mark — diamond. Used in the doctor header, the `self update`
/// finalizer (amber), and the `self uninstall` farewell (violet).
pub(crate) const BRAND_GLYPH: &str = "◆";

/// Brand flourish star — accompanies the wordmark tagline and the farewell.
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

/// Print a serializable value as pretty JSON.
pub(crate) fn print_json<T: serde::Serialize>(val: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(val)?);
    Ok(())
}

/// Print an amber-uppercase, letter-spaced section header preceded by a blank
/// line — the cassette section grammar. Used by `kst doctor`.
pub(crate) fn print_group_header(title: &str, color: bool) {
    println!();
    if color {
        println!("{ACCENT}{ATTENTION}{}{RESET}", title.to_uppercase());
    } else {
        println!("{}", title.to_uppercase());
    }
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

/// Print a uv-style `error:`-prefixed failure line to stderr.
pub(crate) fn eprint_fail(name: &str, source: &str, plain: bool) {
    if plain {
        eprintln!("error: failed to install {name} from {source}");
    } else {
        eprintln!("{ERROR}{ACCENT}error:{RESET} failed to install {name} from {source}");
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
    // never surfaces to or aborts the real command whose result we return below.
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

/// Strip the URL scheme + leading `www.` so source labels read like
/// `github.com/org/repo` instead of `https://github.com/org/repo`.
pub(crate) fn short_source(source: &str) -> String {
    let s = source
        .strip_prefix("https://")
        .or_else(|| source.strip_prefix("http://"))
        .unwrap_or(source);
    s.strip_prefix("www.").unwrap_or(s).to_string()
}

/// Single-glyph prefix for a per-asset sync action — the cassette design
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

/// Past-tense status verb + dim metadata tail per design — pairs with
/// [`action_glyph`] in source-grouped trees. Returns the colored tail (e.g.
/// `updated  2.1.0 → 2.2.0`, `added  v1.0.0`, `removed`, `unchanged`).
pub(crate) fn status_tail(status: &str, version_from: Option<&str>, version_to: Option<&str>, plain: bool) -> String {
    if plain {
        return match status {
            "installed" | "would_install" => format!("added{}", version_to.map(|v| format!("  v{v}")).unwrap_or_default()),
            "updated" | "would_update" => match (version_from, version_to) {
                (Some(f), Some(t)) => format!("updated  {f} → {t}"),
                _ => "updated".to_string(),
            },
            "removed" | "would_remove" => "removed".to_string(),
            "unchanged" => "unchanged".to_string(),
            "broken" | "source_error" => "broken".to_string(),
            _ => status.to_string(),
        };
    }
    match status {
        "installed" | "would_install" => {
            let tail = version_to.map(|v| format!("{SECONDARY}  v{v}{RESET}")).unwrap_or_default();
            format!("{SUCCESS}added{RESET}{tail}")
        }
        "updated" | "would_update" => match (version_from, version_to) {
            (Some(f), Some(t)) => format!("{ATTENTION}updated{RESET}{SECONDARY}  {f} → {t}{RESET}"),
            _ => format!("{ATTENTION}updated{RESET}"),
        },
        "removed" | "would_remove" => format!("{ERROR}removed{RESET}"),
        "unchanged" => format!("{INFRA}unchanged{RESET}"),
        "broken" | "source_error" => format!("{ERROR}broken{RESET}"),
        _ => status.to_string(),
    }
}

/// Amber, uppercase, letter-spaced section header per design — `SKILLS   23 installed`.
/// `count_unit` is `(count, "installed")` for inline metadata.
pub(crate) fn print_section_header(label: &str, count_unit: Option<(usize, &str)>, plain: bool) {
    if plain {
        match count_unit {
            Some((n, unit)) => println!("{label}   {n} {unit}"),
            None => println!("{label}"),
        }
        return;
    }
    let label_up = label.to_uppercase();
    match count_unit {
        Some((n, unit)) => println!("{ACCENT}{ATTENTION}{label_up}{RESET}   {SECONDARY}{n} {unit}{RESET}"),
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
                let pad = col.saturating_sub(prefix.len() + n.to_string().len());
                println!("{prefix}{}{n}", " ".repeat(pad));
            }
            (Some(n), None) => println!("{glyph_plain} {repo}    {n}"),
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
            // Visible width = "✓ repo" + " " + "N"; pad whitespace between repo and N.
            let visible_prefix = 1 + 1 + repo.chars().count();
            let n_str = n.to_string();
            let pad = col.saturating_sub(visible_prefix + n_str.len());
            println!(
                "{glyph_colored} {INFO}{repo}{RESET}{}{INFRA}{n_str}{RESET}",
                " ".repeat(pad)
            );
        }
        (Some(n), None) => println!("{glyph_colored} {INFO}{repo}{RESET}   {INFRA}{n}{RESET}"),
        (None, _) => println!("{glyph_colored} {INFO}{repo}{RESET}"),
    }
}

/// Tree leaf row: `├─` for non-last, `└─` for last, then optional glyph,
/// then name padded to `name_width` chars (foreground or strike-through for
/// removed), then dim tail. `name_width = 0` means no padding (use 2-space
/// gutter instead).
pub(crate) fn print_tree_leaf(
    is_last: bool,
    glyph: Option<&str>,
    name: &str,
    name_strike: bool,
    tail: &str,
    name_width: usize,
    plain: bool,
) {
    let branch = if is_last { "└─" } else { "├─" };
    let name_visible_w = name.chars().count();
    let name_pad = name_width.saturating_sub(name_visible_w);
    if plain {
        let g = glyph.map(|g| format!(" {g}")).unwrap_or_default();
        let padded_name = if name_width == 0 {
            name.to_string()
        } else {
            format!("{name}{}", " ".repeat(name_pad))
        };
        let t = if tail.is_empty() { String::new() } else { format!("  {tail}") };
        println!("{branch}{g} {padded_name}{t}");
        return;
    }
    let name_styled = if name_strike {
        format!("{INFRA}{STRIKE}{name}{STRIKE_RESET}{RESET}")
    } else {
        name.to_string()
    };
    let padded_name = if name_width == 0 {
        name_styled
    } else {
        format!("{name_styled}{}", " ".repeat(name_pad))
    };
    let glyph_part = glyph.map(|g| format!(" {g}")).unwrap_or_default();
    let tail_part = if tail.is_empty() { String::new() } else { format!("  {tail}") };
    println!("{INFRA}{branch}{RESET}{glyph_part} {padded_name}{tail_part}");
}

/// Render the per-action summary chips shown beneath a sync's lead verb line:
/// `● N updated  ● N added  ● N removed  ● N unchanged`. The dot inherits the
/// role color (amber/green/red/dim) and the count is bold.
pub(crate) fn print_sync_chips(
    updated: usize,
    added: usize,
    removed: usize,
    unchanged: usize,
    plain: bool,
) {
    if plain {
        println!(
            "  {updated} updated  {added} added  {removed} removed  {unchanged} unchanged"
        );
        return;
    }
    println!(
        "  {ATTENTION}●{RESET} {updated} {SECONDARY}updated{RESET}  \
         {SUCCESS}●{RESET} {added} {SECONDARY}added{RESET}  \
         {ERROR}●{RESET} {removed} {SECONDARY}removed{RESET}  \
         {INFRA}●{RESET} {unchanged} {SECONDARY}unchanged{RESET}"
    );
}

/// `◆ kasetto vX.Y.Z                          ✓ healthy` — doctor head per
/// design. Amber diamond + bold `kasetto`, dim version, right-aligned green
/// `✓ healthy` badge at column ~62.
pub(crate) fn print_doctor_head(version: &str, healthy: bool, plain: bool) {
    const COL: usize = 62;
    let left_plain = format!("◆ kasetto v{version}");
    let badge_plain = if healthy { "✓ healthy" } else { "✗ issues" };
    let pad = COL.saturating_sub(left_plain.chars().count() + badge_plain.chars().count());
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
pub(crate) fn print_doctor_kv(key: &str, value: &str, key_w: usize, value_color: Option<&str>, plain: bool) {
    if plain {
        println!("{key:<key_w$}  {value}");
        return;
    }
    match value_color {
        Some(c) => println!("{key:<key_w$}  {c}{value}{RESET}"),
        None => println!("{key:<key_w$}  {value}"),
    }
}

/// `✓ Sentence` — a single check row in the CHECKS section.
pub(crate) fn print_check(passed: bool, label: &str, plain: bool) {
    let glyph = if passed { "✓" } else { "✗" };
    if plain {
        println!("{glyph} {label}");
        return;
    }
    let color = if passed { SUCCESS } else { ERROR };
    println!("{color}{glyph}{RESET} {label}");
}

/// `✓ ~/.foo/bar               writable` — command-directory row with
/// right-aligned faint trailing tag.
pub(crate) fn print_dir_row(path: &str, writable: bool, plain: bool) {
    const COL: usize = 62;
    let tag = if writable { "writable" } else { "not writable" };
    let path_relative = relativize_home(path);
    let visible_left = 1 + 1 + path_relative.chars().count();
    let pad = COL.saturating_sub(visible_left + tag.chars().count());
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

/// `◆ Updated to vNEW  was vOLD` — self update finalizer (amber diamond).
pub(crate) fn print_update_closer(new: &str, old: &str, plain: bool) {
    if plain {
        println!("Updated to v{new}  was v{old}");
        return;
    }
    println!(
        "{ATTENTION}{BRAND_GLYPH}{RESET} {ACCENT}Updated to v{new}{RESET}{SECONDARY}  was v{old}{RESET}"
    );
}

/// Violet `◆ kasetto vX uninstalled` + amber `またね` farewell — self uninstall closer.
pub(crate) fn print_uninstall_closer(version: &str, plain: bool) {
    if plain {
        println!("kasetto v{version} uninstalled");
        println!("Thanks for using kasetto.  またね");
        return;
    }
    println!(
        "{BRAND}{BRAND_GLYPH}{RESET} {ACCENT}kasetto v{version} uninstalled{RESET}"
    );
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
    fn with_spinner_disabled_executes_operation_and_returns_result() {
        let result = with_spinner(false, true, "Syncing demo", || {
            Ok::<_, crate::error::Error>(42)
        })
        .expect("operation");
        assert_eq!(result, 42);
    }
}
