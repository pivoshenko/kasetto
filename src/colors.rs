//! ANSI SGR sequences and shared terminal palette (crossterm + clap).
//!
//! # Semantic roles
//!
//! | Name | Use |
//! |------|-----|
//! | [`ACCENT`] | Bold labels, prompts, command names in prose |
//! | [`BANNER`] | Large ASCII art / bright magenta blocks |
//! | [`ATTENTION`] | Secondary emphasis (e.g. “Broken” in sync summary) |
//! | [`SECONDARY`] | Light grey hints, metadata, example commands in help |
//! | [`INFO`] | Cyan emphasis |
//! | [`SUCCESS`] | OK / installed / positive outcomes |
//! | [`WARNING`] | Caution, “would remove”, removed counts |
//! | [`WARNING_EMPHASIS`] | Bold yellow for highlighted warnings |
//! | [`ERROR`] | Failures, destructive actions applied |
//! | [`CHIP_*`] | Status chip backgrounds |

use clap::builder::styling::{AnsiColor, Effects, Styles};

/// Crossterm colors aligned with [`super`] semantic SGR roles.
pub(crate) mod term {
    use crossterm::style::Color;

    /// [`super::ACCENT`] (magenta; add bold in crossterm where needed).
    pub(crate) const ACCENT: Color = Color::Magenta;
    /// [`super::BANNER`].
    pub(crate) const BANNER: Color = Color::AnsiValue(13);
    /// [`super::SECONDARY`] - standard grey, lighter than `DarkGrey` on typical terminals.
    pub(crate) const SECONDARY: Color = Color::Grey;
    /// [`super::INFO`].
    pub(crate) const INFO: Color = Color::Cyan;
    /// [`super::ERROR`].
    pub(crate) const ERROR: Color = Color::Red;
    /// Primary body text on colored TUI backgrounds.
    pub(crate) const TEXT: Color = Color::White;

    /// List browser when `NO_COLOR` is set: selection highlight (no palette hues).
    pub(crate) const MONO_SEL_BG: Color = Color::Grey;
    pub(crate) const MONO_SEL_FG: Color = Color::Black;
}

// --- clap: same roles as string constants above (clap uses its own `AnsiColor` API).
const CLAP_HEADER_USAGE: AnsiColor = AnsiColor::Magenta; // [`ACCENT`]
const CLAP_LITERAL: AnsiColor = AnsiColor::Yellow; // [`WARNING_EMPHASIS`]
const CLAP_PLACEHOLDER: AnsiColor = AnsiColor::Cyan; // [`INFO`]

/// Clap help styling - aligned with [`ACCENT`], [`WARNING_EMPHASIS`], [`INFO`].
pub(crate) fn clap_styles() -> Styles {
    Styles::styled()
        .header(CLAP_HEADER_USAGE.on_default().effects(Effects::BOLD))
        .usage(CLAP_HEADER_USAGE.on_default().effects(Effects::BOLD))
        .literal(CLAP_LITERAL.on_default().effects(Effects::BOLD))
        .placeholder(CLAP_PLACEHOLDER.on_default())
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

/// Bold accent - labels, prompts, highlighted tokens (SGR bold magenta).
pub(crate) const ACCENT: &str = "\x1b[1;35m";
/// Bright magenta for banner / large art.
pub(crate) const BANNER: &str = "\x1b[95m";
/// Plain magenta for secondary emphasis (e.g. “Broken” in summaries).
pub(crate) const ATTENTION: &str = "\x1b[35m";

/// Light grey foreground - hints, borders (`256-color` 248; lighter than bright-black `90m`).
pub(crate) const SECONDARY: &str = "\x1b[38;5;248m";

pub(crate) const SUCCESS: &str = "\x1b[32m";
pub(crate) const ERROR: &str = "\x1b[31m";
pub(crate) const WARNING: &str = "\x1b[33m";
pub(crate) const WARNING_EMPHASIS: &str = "\x1b[1;33m";
pub(crate) const INFO: &str = "\x1b[36m";

/// Carriage return + clear to end of line.
pub(crate) const CLEAR_LINE: &str = "\r\x1b[2K";

/// Status chips: black on colored background.
pub(crate) const CHIP_SUCCESS: &str = "\x1b[30;42m";
pub(crate) const CHIP_NEUTRAL: &str = "\x1b[30;47m";
pub(crate) const CHIP_WARNING: &str = "\x1b[30;43m";
pub(crate) const CHIP_ERROR: &str = "\x1b[30;41m";

/// Clap `after_help`: accent “Examples:” header and secondary example lines (compile-time only).
#[macro_export]
macro_rules! cli_examples {
    ($($line:literal),* $(,)?) => {
        concat!(
            "\x1b[1;35mExamples:\x1b[0m\n",
            $(
                // Must match [`ACCENT`] + [`RESET`] / [`SECONDARY`]; `concat!` rejects `const` refs - see `palette_tests::cli_examples_literals_match_acc_and_secondary`.
                concat!("  \x1b[38;5;248m", $line, "\x1b[0m\n"),
            )*
        )
    };
}

#[cfg(test)]
mod palette_tests {
    use super::{ACCENT, SECONDARY};

    /// `cli_examples!` must use literals inside `concat!`; keep them identical to these.
    #[test]
    fn cli_examples_literals_match_acc_and_secondary() {
        assert_eq!(ACCENT, "\x1b[1;35m");
        assert_eq!(SECONDARY, "\x1b[38;5;248m");
    }
}
