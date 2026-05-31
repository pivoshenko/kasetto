//! Cassette CLI palette — semantic SGR aliases.
//!
//! Seven semantic roles cover every colored surface. Call sites use the
//! semantic names (`SUCCESS`, `ERROR`, `ATTENTION`, `INFO`, `SECONDARY`,
//! `INFRA`, `BRAND`); only this file holds hex values. Body text inherits
//! from the user's terminal — there's deliberately no "foreground" constant.
//!
//! Color is gated on `color_stdout_enabled()` — set `NO_COLOR=1` or pipe
//! stdout to drop it entirely.
//!
//! # When each role is used
//!
//! | Token       | Hex       | Use                                                                                    |
//! |-------------|-----------|----------------------------------------------------------------------------------------|
//! | `ACCENT`    | bold      | emphasis (no color) — prompts, labels, lead verbs. Compose with a hue for bold-colored. |
//! | `ATTENTION` | `#e8a94d` | help headers/literals, section headers, `Updated`/`Would …` verbs, `warning:`, banner subtitle, spinner glyph |
//! | `SUCCESS`   | `#84c578` | `Installed` / `Created` / `Audited` verbs, `+` and `✓` glyphs, `✓ healthy` badge        |
//! | `ERROR`     | `#e87e6c` | `error:` prefix, `−` and `✗` glyphs, `✗ issues` badge, failure rows                     |
//! | `INFO`      | `#6cbfd3` | `tip:` / `note:` prefixes, source repo labels                                          |
//! | `BRAND`     | `#b6a6ef` | **brand mark only** — banner frame + wordmark, `◆` farewell on `self uninstall`        |
//! | `SECONDARY` | `#a8a195` | muted content — paths, timing tails, hints, `[y/N]`, clap placeholders, example lines  |
//! | `INFRA`     | `#6e6759` | structure only — tree branches, bullet glyphs, strikethrough overlay (never content)   |
//!
//! Non-color SGR helpers: `RESET`, `STRIKE` / `STRIKE_RESET`, `CLEAR_LINE`.

use clap::builder::styling::{Effects, RgbColor, Style, Styles};

/// Bold (no foreground color). Prompts, labels, lead verbs. Compose with a
/// hue for bold-colored emphasis (e.g. `{ACCENT}{ATTENTION}…`).
pub(crate) const ACCENT: &str = "\x1b[1m";

/// Amber `#e8a94d`. Help headers/literals, section headers, `Updated` /
/// `Would …` verbs, `warning:` prefix, banner subtitle, spinner glyph,
/// `kasetto` lead.
pub(crate) const ATTENTION: &str = "\x1b[38;2;232;169;77m";

/// Green `#84c578`. `Installed` / `Created` / `Audited` verbs, `+` add glyph,
/// `✓` success glyph, `✓ healthy` badge.
pub(crate) const SUCCESS: &str = "\x1b[38;2;132;197;120m";

/// Red `#e87e6c`. `error:` prefix, `−` remove glyph, `✗` failure glyph,
/// `✗ issues` badge.
pub(crate) const ERROR: &str = "\x1b[38;2;232;126;108m";

/// Cyan `#6cbfd3`. `tip:` / `note:` prefixes, source repo labels.
pub(crate) const INFO: &str = "\x1b[38;2;108;191;211m";

/// Brand violet `#b6a6ef`. **Brand mark only** — banner frame + wordmark,
/// `◆` farewell on `self uninstall`. Reserved for ceremonial brand surfaces;
/// not for operational status.
pub(crate) const BRAND: &str = "\x1b[38;2;182;166;239m";

/// Muted grey `#a8a195`. Paths, timing tails, hints, `[y/N]`, soft state
/// (`Cancelled.`, `Nothing to sync`), clap placeholders, example lines.
pub(crate) const SECONDARY: &str = "\x1b[38;2;168;161;149m";

/// Structural `#6e6759`. Tree branches `├─` / `└─`, bullet glyphs (`●` /
/// `•`), strikethrough overlay on removed entries. Never for content.
pub(crate) const INFRA: &str = "\x1b[38;2;110;103;89m";

/// Reset all attributes (`SGR 0`).
pub(crate) const RESET: &str = "\x1b[0m";

/// Strikethrough on (`SGR 9`) — removed entries in trees.
pub(crate) const STRIKE: &str = "\x1b[9m";
/// Strikethrough off (`SGR 29`).
pub(crate) const STRIKE_RESET: &str = "\x1b[29m";

/// Carriage return + clear to end of line.
pub(crate) const CLEAR_LINE: &str = "\r\x1b[2K";

/// Clap help styling: amber `Usage:` / `Commands:` headers + literals,
/// `SECONDARY`-grey `<COMMAND>` / `<ARG>` placeholders.
pub(crate) fn clap_styles() -> Styles {
    let amber = Style::new().fg_color(Some(RgbColor(232, 169, 77).into())) | Effects::BOLD;
    let secondary = Style::new().fg_color(Some(RgbColor(168, 161, 149).into()));
    Styles::styled()
        .header(amber)
        .usage(amber)
        .literal(amber)
        .placeholder(secondary)
}

/// Clap `after_help`: amber `Examples:` header, `SECONDARY` example lines.
#[macro_export]
macro_rules! cli_examples {
    ($($line:literal),* $(,)?) => {
        concat!(
            "\x1b[1m\x1b[38;2;232;169;77mExamples:\x1b[0m\n",
            $(
                concat!("  \x1b[38;2;168;161;149m", $line, "\x1b[0m\n"),
            )*
        )
    };
}
