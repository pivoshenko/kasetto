use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::colors::{
    ACCENT, CHIP_ERROR, CHIP_NEUTRAL, CHIP_SUCCESS, CHIP_WARNING, CLEAR_LINE, ERROR, RESET,
    SECONDARY, SUCCESS,
};
use crate::error::Result;

/// Braille spinner frames shared across all TUI surfaces.
pub(crate) const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Unicode symbols for success and failure indicators.
pub(crate) const SYM_OK: &str = "✓";
pub(crate) const SYM_FAIL: &str = "✗";

pub(crate) fn animations_enabled(quiet: bool, as_json: bool, plain: bool) -> bool {
    !quiet && !as_json && !plain && std::io::stderr().is_terminal()
}

// ---------------------------------------------------------------------------
// Shared output helpers
// ---------------------------------------------------------------------------

/// Print a serializable value as pretty JSON.
pub(crate) fn print_json<T: serde::Serialize>(val: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(val)?);
    Ok(())
}

/// Print `Label: value` with optional ANSI color.
pub(crate) fn print_field(label: &str, value: &str, color: bool) {
    if color {
        println!("{ACCENT}{label}: {RESET}{value}");
    } else {
        println!("{label}: {value}");
    }
}

/// Print `Label:` header with optional ANSI color.
pub(crate) fn print_label(label: &str, color: bool) {
    if color {
        println!("{ACCENT}{label}:{RESET}");
    } else {
        println!("{label}:");
    }
}

/// Print `Label: count` section header with optional ANSI color.
pub(crate) fn print_section_header(title: &str, count: usize, color: bool) {
    if color {
        println!("{ACCENT}{title}: {count}{RESET}");
    } else {
        println!("{title}: {count}");
    }
}

/// Print a failure line to stderr with optional ANSI color.
pub(crate) fn eprint_fail(name: &str, source: &str, plain: bool) {
    if plain {
        eprintln!("{SYM_FAIL} Failed {name} {source}");
    } else {
        eprintln!(
            "{ERROR}{SYM_FAIL}{RESET} Failed {ACCENT}{name}{RESET} {SECONDARY}{source}{RESET}"
        );
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
                "{}{} {}",
                CLEAR_LINE,
                SPINNER_FRAMES[idx % SPINNER_FRAMES.len()],
                thread_label
            );
            let _ = stderr.flush();
            idx = idx.wrapping_add(1);
            thread::sleep(Duration::from_millis(80));
        }
    });

    let result = operation();
    stop.store(true, Ordering::Relaxed);
    let _ = handle.join();

    let mut stderr = std::io::stderr();
    let symbol = if result.is_ok() { SYM_OK } else { SYM_FAIL };
    if plain {
        if result.is_ok() {
            let _ = writeln!(stderr, "{} {}", symbol, ok_label);
        } else {
            let _ = writeln!(stderr, "{} {}", symbol, label);
        }
    } else if result.is_ok() {
        let _ = writeln!(
            stderr,
            "{}{}{}{} {}",
            CLEAR_LINE, SUCCESS, symbol, RESET, ok_label
        );
    } else {
        let _ = writeln!(
            stderr,
            "{}{}{}{} {}",
            CLEAR_LINE, ERROR, symbol, RESET, label
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

pub(crate) fn status_chip(status: &str, plain: bool) -> String {
    if plain {
        return match status {
            "broken" | "source_error" => format!("[{SYM_FAIL}]"),
            _ => format!("[{}]", status.to_uppercase()),
        };
    }
    match status {
        "installed" | "updated" | "removed" => format!("{} {status} {}", CHIP_SUCCESS, RESET),
        "unchanged" => format!("{} {status} {}", CHIP_NEUTRAL, RESET),
        "would_install" | "would_update" | "would_remove" => {
            format!("{} {status} {}", CHIP_WARNING, RESET)
        }
        "broken" | "source_error" => format!("{} {SYM_FAIL} {}", CHIP_ERROR, RESET),
        _ => format!("{} {status} {}", CHIP_ERROR, RESET),
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
    fn status_chip_plain_for_broken_is_fail_symbol() {
        assert_eq!(status_chip("broken", true), "[✗]");
        assert_eq!(status_chip("source_error", true), "[✗]");
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
