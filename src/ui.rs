use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::error::Result;

pub fn animations_enabled(quiet: bool, as_json: bool, plain: bool) -> bool {
    !quiet && !as_json && !plain && std::io::stderr().is_terminal()
}

pub fn with_spinner<T, F>(
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
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let mut idx = 0usize;
        let mut stderr = std::io::stderr();
        while !stop_flag.load(Ordering::Relaxed) {
            let _ = write!(
                stderr,
                "\r\x1b[2K{} {}",
                frames[idx % frames.len()],
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
    let symbol = if result.is_ok() { "✓" } else { "✗" };
    if plain {
        if result.is_ok() {
            let _ = writeln!(stderr, "{} {}", symbol, ok_label);
        } else {
            let _ = writeln!(stderr, "{} {}", symbol, label);
        }
    } else if result.is_ok() {
        let _ = writeln!(stderr, "\r\x1b[2K\x1b[32m{}\x1b[0m {}", symbol, ok_label);
    } else {
        let _ = writeln!(stderr, "\r\x1b[2K\x1b[31m{}\x1b[0m {}", symbol, label);
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

pub fn status_chip(status: &str, plain: bool) -> String {
    if plain {
        return match status {
            "broken" | "source_error" => "[X]".to_string(),
            _ => format!("[{}]", status.to_uppercase()),
        };
    }
    match status {
        "installed" | "updated" | "removed" => format!("\x1b[30;42m {} \x1b[0m", status),
        "unchanged" => format!("\x1b[30;47m {} \x1b[0m", status),
        "would_install" | "would_update" | "would_remove" => {
            format!("\x1b[30;43m {} \x1b[0m", status)
        }
        "broken" | "source_error" => "\x1b[30;41m x \x1b[0m".to_string(),
        _ => format!("\x1b[30;41m {} \x1b[0m", status),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synced_label_rewrites_known_prefixes() {
        assert_eq!(synced_label("Syncing demo"), "Synced demo");
        assert_eq!(synced_label("Checking for updates"), "Checked for updates");
        assert_eq!(synced_label("Updating 1.0.0 -> 1.1.0"), "Updated 1.0.0 -> 1.1.0");
        assert_eq!(synced_label("Loading source"), "Loading source");
    }

    #[test]
    fn status_chip_plain_for_broken_is_x() {
        assert_eq!(status_chip("broken", true), "[X]");
        assert_eq!(status_chip("source_error", true), "[X]");
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
