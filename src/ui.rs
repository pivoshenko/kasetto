use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::colors::{
    ACCENT, ACCENT_WARM, ATTENTION, CLEAR_LINE, ERROR, INFO, RESET, SECONDARY, SUCCESS,
};
use crate::error::Result;

/// Braille spinner frames shared across all TUI surfaces.
pub(crate) const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];


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

/// Print `key  value` rows with keys padded to a common width so values
/// align vertically. Keys render bold (no color); values default. This is
/// the canonical `kst doctor` field layout.
pub(crate) fn print_panel(rows: &[(&str, &str)], color: bool) {
    let key_w = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (k, v) in rows {
        if color {
            println!("{ACCENT}{k:<key_w$}{RESET}  {v}");
        } else {
            println!("{k:<key_w$}  {v}");
        }
    }
}

/// Print a bold (no color), no-colon group header preceded by a blank line.
/// Used to separate sections in `kst doctor` and `kst clean --dry-run`.
pub(crate) fn print_group_header(title: &str, color: bool) {
    println!();
    if color {
        println!("{ACCENT}{title}{RESET}");
    } else {
        println!("{title}");
    }
}

/// Print a uv-style `tip: <msg>` line in popil sky (`INFO`). Plain mode
/// omits color but keeps the prefix.
pub(crate) fn print_tip(msg: &str, plain: bool) {
    if plain {
        println!("tip: {msg}");
    } else {
        println!("{INFO}\x1b[1mtip:{RESET} {msg}");
    }
}

/// Print a uv-style `error:`-prefixed failure line to stderr.
pub(crate) fn eprint_fail(name: &str, source: &str, plain: bool) {
    if plain {
        eprintln!("error: failed to install {name} from {source}");
    } else {
        eprintln!("{ERROR}\x1b[1merror:{RESET} failed to install {name} from {source}");
    }
}

pub(crate) fn with_spinner<T, F>(
    enabled: bool,
    plain: bool,
    label: impl Into<String>,
    operation: F,
) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    let label = label.into();
    let ok_label = synced_label(&label);
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
                ACCENT_WARM,
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
    if plain {
        if result.is_ok() {
            let _ = writeln!(stderr, "{}", ok_label);
        } else {
            let _ = writeln!(stderr, "error: {}", label);
        }
    } else if result.is_ok() {
        let _ = writeln!(
            stderr,
            "{}{}\x1b[1m{}{}",
            CLEAR_LINE, SUCCESS, ok_label, RESET
        );
    } else {
        let _ = writeln!(
            stderr,
            "{}{}\x1b[1merror:{} {}",
            CLEAR_LINE, ERROR, RESET, label
        );
    }
    let _ = stderr.flush();

    result
}

fn synced_label(label: &str) -> String {
    if let Some(rest) = label.strip_prefix("Syncing ") {
        return format!("Synced {}", rest);
    }
    if let Some(rest) = label.strip_prefix("Checking ") {
        return format!("Checked {}", rest);
    }
    if let Some(rest) = label.strip_prefix("Updating ") {
        return format!("Updated {}", rest);
    }
    label.to_string()
}

/// Single-glyph prefix for a per-asset sync action, uv-style: `+` install,
/// `~` update, `-` remove, `=` unchanged, `!` broken/failed. Bold + colored
/// when `plain` is false.
pub(crate) fn action_glyph(status: &str, plain: bool) -> String {
    let (glyph, color) = match status {
        "installed" | "would_install" => ('+', SUCCESS),
        "updated" | "would_update" => ('~', ATTENTION),
        "removed" | "would_remove" => ('-', ERROR),
        "unchanged" => ('=', SECONDARY),
        "broken" | "source_error" => ('!', ERROR),
        _ => ('?', ERROR),
    };
    if plain {
        glyph.to_string()
    } else {
        format!("\x1b[1m{color}{glyph}{RESET}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synced_label_rewrites_known_prefixes() {
        assert_eq!(synced_label("Syncing demo"), "Synced demo");
        assert_eq!(synced_label("Checking for updates"), "Checked for updates");
        assert_eq!(
            synced_label("Updating 1.0.0 -> 1.1.0"),
            "Updated 1.0.0 -> 1.1.0"
        );
        assert_eq!(synced_label("Loading source"), "Loading source");
    }

    #[test]
    fn action_glyph_plain_returns_single_char() {
        assert_eq!(action_glyph("installed", true), "+");
        assert_eq!(action_glyph("updated", true), "~");
        assert_eq!(action_glyph("removed", true), "-");
        assert_eq!(action_glyph("unchanged", true), "=");
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
