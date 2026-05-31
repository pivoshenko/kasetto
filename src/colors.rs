//! ANSI SGR sequences and clap help styling.
//!
//! Brand DNA is sourced from `pivoshenko.theme/themes/palettes/popil.json`
//! (the warm-ash flavor active on `kasetto.dev`). Side A is lavender
//! `#a89bb5` (muted, warm, low-chroma), Side B is peach `#d97757`
//! (split-complementary). All semantic roles below resolve to 24-bit
//! truecolor SGR; `NO_COLOR` / `--plain` gate emission upstream.
//!
//! # Semantic roles
//!
//! | Name | Use |
//! |------|-----|
//! | [`ACCENT`] | Bold lavender labels, prompts, command names in prose |
//! | [`BANNER`] | Lavender ASCII art and large brand blocks |
//! | [`ACCENT_WARM`] | Peach — Side B emphasis, spinner glyph, CTA highlights |
//! | [`ATTENTION`] | Popil yellow — secondary emphasis (e.g. "Broken" in sync summary) |
//! | [`SECONDARY`] | Warm subtle grey — hints, metadata, example commands in help |
//! | [`INFO`] | Popil sky |
//! | [`SUCCESS`] | Popil green |
//! | [`WARNING`] | Tape tan — muted, distinct from peach Side B |
//! | [`WARNING_EMPHASIS`] | Bold tape tan |
//! | [`ERROR`] | Popil red — warm, less alarming than primary red |

use clap::builder::styling::{Color as ClapColor, Effects, RgbColor, Style, Styles};

// Brand RGB anchors mirror `pivoshenko.theme/themes/palettes/popil.json`.
// Kept exhaustive (with `dead_code` allowed) so roles can reference any
// anchor without having to thread a new constant through every consumer.

#[allow(dead_code)]
pub(crate) const RGB_LAVENDER: (u8, u8, u8) = (0xa8, 0x9b, 0xb5);
#[allow(dead_code)]
pub(crate) const RGB_PEACH: (u8, u8, u8) = (0xd9, 0x77, 0x57);
#[allow(dead_code)]
pub(crate) const RGB_YELLOW: (u8, u8, u8) = (0xd4, 0xa8, 0x5a);
#[allow(dead_code)]
pub(crate) const RGB_GREEN: (u8, u8, u8) = (0x8a, 0x9d, 0x68);
#[allow(dead_code)]
pub(crate) const RGB_RED: (u8, u8, u8) = (0xc8, 0x7a, 0x72);
#[allow(dead_code)]
pub(crate) const RGB_SKY: (u8, u8, u8) = (0x7b, 0xa0, 0xc4);
#[allow(dead_code)]
pub(crate) const RGB_TAPE: (u8, u8, u8) = (0xc4, 0xad, 0x88);
#[allow(dead_code)]
pub(crate) const RGB_TEXT: (u8, u8, u8) = (0xe4, 0xe2, 0xde);
#[allow(dead_code)]
pub(crate) const RGB_SUBTLE: (u8, u8, u8) = (0x9b, 0x95, 0x8a);
#[allow(dead_code)]
pub(crate) const RGB_BASE: (u8, u8, u8) = (0x1f, 0x1f, 0x1e);

fn clap_rgb(t: (u8, u8, u8)) -> ClapColor {
    RgbColor(t.0, t.1, t.2).into()
}

fn clap_style(t: (u8, u8, u8), bold: bool) -> Style {
    let s = Style::new().fg_color(Some(clap_rgb(t)));
    if bold {
        s.effects(Effects::BOLD)
    } else {
        s
    }
}

/// Clap help styling — bold lavender headers, bold peach literals, sky placeholders.
pub(crate) fn clap_styles() -> Styles {
    Styles::styled()
        .header(clap_style(RGB_LAVENDER, true))
        .usage(clap_style(RGB_LAVENDER, true))
        .literal(clap_style(RGB_PEACH, true))
        .placeholder(clap_style(RGB_SKY, false))
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

// 24-bit foreground SGR literals: `\x1b[38;2;R;G;Bm` (ECMA-48 truecolor).
// Keep in lockstep with the `RGB_*` tuples above — `palette_tests` asserts it.

/// Bold lavender — labels, prompts, highlighted tokens.
pub(crate) const ACCENT: &str = "\x1b[1;38;2;168;155;181m";
/// Plain lavender — banner / large art / non-bold accent fields.
pub(crate) const BANNER: &str = "\x1b[38;2;168;155;181m";
/// Bold peach — Side B emphasis (spinner, warm CTA highlights).
pub(crate) const ACCENT_WARM: &str = "\x1b[1;38;2;217;119;87m";
/// Plain popil yellow — secondary emphasis (e.g. "Broken" in sync summaries).
pub(crate) const ATTENTION: &str = "\x1b[38;2;212;168;90m";

/// Warm subtle grey — hints, metadata, borders. Matches popil `subtext0`.
pub(crate) const SECONDARY: &str = "\x1b[38;2;155;149;138m";

pub(crate) const SUCCESS: &str = "\x1b[38;2;138;157;104m";
pub(crate) const ERROR: &str = "\x1b[38;2;200;122;114m";
pub(crate) const WARNING: &str = "\x1b[38;2;196;173;136m";
pub(crate) const WARNING_EMPHASIS: &str = "\x1b[1;38;2;196;173;136m";
pub(crate) const INFO: &str = "\x1b[38;2;123;160;196m";

/// Carriage return + clear to end of line.
pub(crate) const CLEAR_LINE: &str = "\r\x1b[2K";

/// Clap `after_help`: accent "Examples:" header and secondary example lines (compile-time only).
#[macro_export]
macro_rules! cli_examples {
    ($($line:literal),* $(,)?) => {
        concat!(
            "\x1b[1;38;2;168;155;181mExamples:\x1b[0m\n",
            $(
                // Must match [`ACCENT`] + [`RESET`] / [`SECONDARY`]; `concat!` rejects `const` refs - see `palette_tests::cli_examples_literals_match_accent_and_secondary`.
                concat!("  \x1b[38;2;155;149;138m", $line, "\x1b[0m\n"),
            )*
        )
    };
}

#[cfg(test)]
mod palette_tests {
    use super::{
        ACCENT, ACCENT_WARM, ATTENTION, BANNER, ERROR, INFO, RGB_GREEN, RGB_LAVENDER, RGB_PEACH,
        RGB_RED, RGB_SKY, RGB_SUBTLE, RGB_TAPE, RGB_YELLOW, SECONDARY, SUCCESS, WARNING,
    };

    fn fg(t: (u8, u8, u8)) -> String {
        format!("\x1b[38;2;{};{};{}m", t.0, t.1, t.2)
    }
    fn bold_fg(t: (u8, u8, u8)) -> String {
        format!("\x1b[1;38;2;{};{};{}m", t.0, t.1, t.2)
    }

    /// `cli_examples!` must use literals inside `concat!`; keep them identical to these.
    #[test]
    fn cli_examples_literals_match_accent_and_secondary() {
        assert_eq!(ACCENT, bold_fg(RGB_LAVENDER));
        assert_eq!(SECONDARY, fg(RGB_SUBTLE));
    }

    #[test]
    fn semantic_sgr_matches_brand_rgb() {
        assert_eq!(BANNER, fg(RGB_LAVENDER));
        assert_eq!(ACCENT_WARM, bold_fg(RGB_PEACH));
        assert_eq!(ATTENTION, fg(RGB_YELLOW));
        assert_eq!(SUCCESS, fg(RGB_GREEN));
        assert_eq!(WARNING, fg(RGB_TAPE));
        assert_eq!(INFO, fg(RGB_SKY));
        assert_eq!(ERROR, fg(RGB_RED));
    }
}
