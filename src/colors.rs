//! ANSI SGR sequences and clap help styling.
//!
//! Brand DNA is anchored to the Kasetto site (`site/app/globals.css`):
//! Side A = mauve `#b89cdc`, Side B = amber `#d4b070` (split-complementary to mauve;
//! equal chroma, opposite hue), warm-neutral surfaces and cream text.
//! All semantic roles below resolve to 24-bit truecolor SGR — modern terminals support it,
//! and `NO_COLOR` / `--plain` already gate color emission upstream.
//!
//! # Semantic roles
//!
//! | Name | Use |
//! |------|-----|
//! | [`ACCENT`] | Bold mauve labels, prompts, command names in prose |
//! | [`BANNER`] | Mauve ASCII art and large brand blocks |
//! | [`ACCENT_WARM`] | Amber — Side B emphasis, spinner glyph, CTA-style highlights |
//! | [`ATTENTION`] | Amber — secondary emphasis (e.g. "Broken" in sync summary) |
//! | [`SECONDARY`] | Warm subtle grey — hints, metadata, example commands in help |
//! | [`INFO`] | Muted sky |
//! | [`SUCCESS`] | Muted green |
//! | [`WARNING`] | Tape tan — muted, distinct from amber Side B |
//! | [`WARNING_EMPHASIS`] | Bold tape tan |
//! | [`ERROR`] | Warm red |
//! | [`CHIP_*`] | Status chip backgrounds (warm fills, base-toned fg) |

use clap::builder::styling::{Color as ClapColor, Effects, RgbColor, Style, Styles};

// ─── Brand RGB anchors (mirror `site/app/globals.css:7-103`) ──────────────────
//
// `dead_code` is allowed: these are palette anchors, kept exhaustive so the TUI
// matches the site DNA even when a given tone isn't yet wired into any role.

#[allow(dead_code)]
pub(crate) const RGB_MAUVE: (u8, u8, u8) = (0xb8, 0x9c, 0xdc);
#[allow(dead_code)]
pub(crate) const RGB_LAVENDER: (u8, u8, u8) = (0xa8, 0xa3, 0xd8);
#[allow(dead_code)]
pub(crate) const RGB_RUST: (u8, u8, u8) = (0xd9, 0x77, 0x57);
#[allow(dead_code)]
pub(crate) const RGB_SKY: (u8, u8, u8) = (0x8a, 0xaa, 0xb8);
#[allow(dead_code)]
pub(crate) const RGB_GREEN: (u8, u8, u8) = (0x9a, 0xb2, 0x8a);
#[allow(dead_code)]
pub(crate) const RGB_AMBER: (u8, u8, u8) = (0xd4, 0xb0, 0x70);
#[allow(dead_code)]
pub(crate) const RGB_TAPE: (u8, u8, u8) = (0xc4, 0xad, 0x88);
#[allow(dead_code)]
pub(crate) const RGB_CREAM: (u8, u8, u8) = (0xeb, 0xe8, 0xe2);
#[allow(dead_code)]
pub(crate) const RGB_SUBTLE: (u8, u8, u8) = (0x8e, 0x8b, 0x86);
#[allow(dead_code)]
pub(crate) const RGB_BASE: (u8, u8, u8) = (0x0a, 0x09, 0x08);
/// Warm red for ERROR — derived from rust, shifted toward red to stay distinct from [`ACCENT_WARM`].
#[allow(dead_code)]
pub(crate) const RGB_RED: (u8, u8, u8) = (0xc2, 0x54, 0x50);

// ─── clap help styling ────────────────────────────────────────────────────────

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

/// Clap help styling — bold mauve headers, bold amber literals, sky placeholders.
pub(crate) fn clap_styles() -> Styles {
    Styles::styled()
        .header(clap_style(RGB_MAUVE, true))
        .usage(clap_style(RGB_MAUVE, true))
        .literal(clap_style(RGB_AMBER, true))
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

// ─── 24-bit foreground SGR literals ───────────────────────────────────────────
//
// `\x1b[38;2;R;G;Bm` (ECMA-48 truecolor). Keep values in lockstep with the `RGB_*`
// tuples above — the `palette_tests` module asserts the literals match.

/// Bold mauve — labels, prompts, highlighted tokens.
pub(crate) const ACCENT: &str = "\x1b[1;38;2;184;156;220m";
/// Plain mauve — banner / large art / non-bold accent fields.
pub(crate) const BANNER: &str = "\x1b[38;2;184;156;220m";
/// Bold amber — Side B emphasis (spinner, warm CTA highlights).
pub(crate) const ACCENT_WARM: &str = "\x1b[1;38;2;212;176;112m";
/// Plain amber — secondary emphasis (e.g. "Broken" in sync summaries).
pub(crate) const ATTENTION: &str = "\x1b[38;2;212;176;112m";

/// Warm subtle grey — hints, metadata, borders. Matches `--fg-subtle` on the site.
pub(crate) const SECONDARY: &str = "\x1b[38;2;142;139;134m";

pub(crate) const SUCCESS: &str = "\x1b[38;2;154;178;138m";
pub(crate) const ERROR: &str = "\x1b[38;2;194;84;80m";
pub(crate) const WARNING: &str = "\x1b[38;2;196;173;136m";
pub(crate) const WARNING_EMPHASIS: &str = "\x1b[1;38;2;196;173;136m";
pub(crate) const INFO: &str = "\x1b[38;2;138;170;184m";

/// Carriage return + clear to end of line.
pub(crate) const CLEAR_LINE: &str = "\r\x1b[2K";

// ─── Status chips: warm cassette-label fills, base-toned text ─────────────────
//
// `\x1b[38;2;R;G;Bm` fg + `\x1b[48;2;R;G;Bm` bg. Foreground is base `#0a0908`
// so chips read like printed labels on tinted tape rather than VT100 highlights.

/// Green fill, base fg.
pub(crate) const CHIP_SUCCESS: &str = "\x1b[38;2;10;9;8;48;2;154;178;138m";
/// Subtle warm-grey fill, base fg.
pub(crate) const CHIP_NEUTRAL: &str = "\x1b[38;2;10;9;8;48;2;142;139;134m";
/// Tape-tan fill, base fg (mirrors `WARNING` foreground).
pub(crate) const CHIP_WARNING: &str = "\x1b[38;2;10;9;8;48;2;196;173;136m";
/// Warm-red fill, base fg.
pub(crate) const CHIP_ERROR: &str = "\x1b[38;2;10;9;8;48;2;194;84;80m";

/// Clap `after_help`: accent "Examples:" header and secondary example lines (compile-time only).
#[macro_export]
macro_rules! cli_examples {
    ($($line:literal),* $(,)?) => {
        concat!(
            "\x1b[1;38;2;184;156;220mExamples:\x1b[0m\n",
            $(
                // Must match [`ACCENT`] + [`RESET`] / [`SECONDARY`]; `concat!` rejects `const` refs - see `palette_tests::cli_examples_literals_match_acc_and_secondary`.
                concat!("  \x1b[38;2;142;139;134m", $line, "\x1b[0m\n"),
            )*
        )
    };
}

#[cfg(test)]
mod palette_tests {
    use super::{
        ACCENT, ACCENT_WARM, ATTENTION, BANNER, ERROR, INFO, RGB_AMBER, RGB_GREEN, RGB_MAUVE,
        RGB_RED, RGB_SKY, RGB_SUBTLE, RGB_TAPE, SECONDARY, SUCCESS, WARNING,
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
        assert_eq!(ACCENT, bold_fg(RGB_MAUVE));
        assert_eq!(SECONDARY, fg(RGB_SUBTLE));
    }

    #[test]
    fn semantic_sgr_matches_brand_rgb() {
        assert_eq!(BANNER, fg(RGB_MAUVE));
        assert_eq!(ACCENT_WARM, bold_fg(RGB_AMBER));
        assert_eq!(ATTENTION, fg(RGB_AMBER));
        assert_eq!(SUCCESS, fg(RGB_GREEN));
        assert_eq!(WARNING, fg(RGB_TAPE));
        assert_eq!(INFO, fg(RGB_SKY));
        assert_eq!(ERROR, fg(RGB_RED));
    }
}
