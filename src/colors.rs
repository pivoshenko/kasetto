//! ANSI SGR sequences and clap help styling.
//!
//! Operational output (`sync`, `list`, `doctor`, `clean`, …) and help text use
//! the basic ANSI 16-color palette so the CLI inherits the user's terminal
//! theme — same convention as `cargo` and `uv`. The brand banner is the only
//! surface that opts into 24-bit truecolor (popil lavender, see `banner.rs`).
//!
//! # Semantic roles
//!
//! | Name | Use |
//! |------|-----|
//! | [`ACCENT`] | Bold (no color) — labels, prompts, command names in prose |
//! | [`ACCENT_WARM`] | Bold cyan — spinner glyph, in-progress emphasis |
//! | [`ATTENTION`] | Yellow — `warning:` prefix and dry-run verbs |
//! | [`SECONDARY`] | Dim — hints, metadata, help examples |
//! | [`INFO`] | Cyan — `tip:` / `note:` prefixes |
//! | [`SUCCESS`] | Green — `Installed`, `Created`, `Audited` lead verbs |
//! | [`WARNING`] | Yellow (alias of ATTENTION) |
//! | [`WARNING_EMPHASIS`] | Bold yellow — emphasized warnings, table row labels |
//! | [`ERROR`] | Red — `error:` prefix and failure lead verbs |

use clap::builder::styling::{AnsiColor, Effects, Style, Styles};

/// Clap help styling — bold green headers (cargo / uv convention), bold
/// (un-colored) literals, default placeholders.
pub(crate) fn clap_styles() -> Styles {
    Styles::styled()
        .header(Style::new().fg_color(Some(AnsiColor::Green.into())) | Effects::BOLD)
        .usage(Style::new().fg_color(Some(AnsiColor::Green.into())) | Effects::BOLD)
        .literal(Style::new() | Effects::BOLD)
        .placeholder(Style::new())
}

/// CUU - cursor up `n` rows (ECMA-48 `CSI n A`).
pub(crate) fn ansi_cursor_up(rows: u16) -> String {
    format!("\x1b[{}A", rows)
}

/// CHA - cursor horizontal absolute; **1-based** column (`CSI n G`, xterm-style).
pub(crate) fn ansi_cursor_column_1based(column: u16) -> String {
    format!("\x1b[{}G", column.max(1))
}

/// Reset all attributes (`SGR 0`).
pub(crate) const RESET: &str = "\x1b[0m";

// Basic ANSI 16-color SGR literals. The terminal theme decides the hue.

/// Bold (no foreground color) — generic highlight for labels, prompts, paths.
pub(crate) const ACCENT: &str = "\x1b[1m";
/// Bold cyan — spinner glyph and in-progress emphasis (matches uv's progress tone).
pub(crate) const ACCENT_WARM: &str = "\x1b[1;36m";
/// Plain yellow — `warning:` prefix, dry-run verbs.
pub(crate) const ATTENTION: &str = "\x1b[33m";

/// Dim attribute — hints, metadata, dim labels. Maps to whatever
/// `--fg-subtle`-equivalent the user's terminal theme provides.
pub(crate) const SECONDARY: &str = "\x1b[2m";

pub(crate) const SUCCESS: &str = "\x1b[32m";
pub(crate) const ERROR: &str = "\x1b[31m";
pub(crate) const WARNING: &str = "\x1b[33m";
pub(crate) const WARNING_EMPHASIS: &str = "\x1b[1;33m";
pub(crate) const INFO: &str = "\x1b[36m";

/// Carriage return + clear to end of line.
pub(crate) const CLEAR_LINE: &str = "\r\x1b[2K";

/// Clap `after_help`: bold green "Examples:" header, dim example lines.
#[macro_export]
macro_rules! cli_examples {
    ($($line:literal),* $(,)?) => {
        concat!(
            "\x1b[1;32mExamples:\x1b[0m\n",
            $(
                concat!("  \x1b[2m", $line, "\x1b[0m\n"),
            )*
        )
    };
}
