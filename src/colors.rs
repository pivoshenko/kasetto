//! Module that contains the Lime Ice CLI palette: semantic SGR aliases.
//!
//! Seven semantic roles cover every colored surface. Call sites use the
//! semantic names (`SUCCESS`, `ERROR`, `ATTENTION`, `INFO`, `SECONDARY`,
//! `INFRA`, `BRAND`); only this file holds hex values. Body text inherits
//! from the user's terminal; there's deliberately no "foreground" constant.
//!
//! Color is gated on `color_stdout_enabled()`. Set `NO_COLOR=1` or pipe
//! stdout to drop it entirely.
//!
//! # When each role is used
//!
//! | Token       | Hex       | Use                                                                                    |
//! |-------------|-----------|----------------------------------------------------------------------------------------|
//! | `ACCENT`    | bold      | emphasis (no color): prompts, labels, lead verbs. Compose with a hue for bold-colored.  |
//! | `ATTENTION` | `#c6ff3d` | help headers/literals, section headers, `Updated`/`Would ...` verbs, `warning:`, banner subtitle, spinner glyph |
//! | `SUCCESS`   | `#5fe3a1` | `Installed` / `Created` / `Audited` verbs, `+` and `✓` glyphs, `✓ healthy` badge        |
//! | `ERROR`     | `#ff6b6b` | `error:` prefix, `−` and `✗` glyphs, `✗ issues` badge, failure rows                     |
//! | `INFO`      | `#3fd7ff` | `tip:` / `note:` prefixes, source repo labels                                          |
//! | `BRAND`     | `#c6ff3d` | **brand mark only**: banner frame + wordmark, `◆` farewell on `self uninstall`         |
//! | `SECONDARY` | `#8a919b` | muted content: paths, timing tails, hints, `[y/N]`, clap placeholders, example lines   |
//! | `INFRA`     | `#596069` | structure only: tree branches, bullet glyphs, strikethrough overlay (never content)    |
//!
//! Non-color SGR helpers: `RESET`, `STRIKE` / `STRIKE_RESET`, `CLEAR_LINE`.

use clap::builder::styling::{Effects, RgbColor, Style, Styles};

/// Bold (no foreground color). Prompts, labels, lead verbs. Compose with a
/// hue for bold-colored emphasis (e.g. `{ACCENT}{ATTENTION}...`).
pub(crate) const ACCENT: &str = "\x1b[1m";

/// Lime `#c6ff3d`. Help headers/literals, section headers, `Updated` /
/// `Would ...` verbs, `warning:` prefix, banner subtitle, spinner glyph,
/// `kasetto` lead.
pub(crate) const ATTENTION: &str = "\x1b[38;2;198;255;61m";

/// Mint `#5fe3a1`. `Installed` / `Created` / `Audited` verbs, `+` add glyph,
/// `✓` success glyph, `✓ healthy` badge.
pub(crate) const SUCCESS: &str = "\x1b[38;2;95;227;161m";

/// Coral red `#ff6b6b`. `error:` prefix, `−` remove glyph, `✗` failure glyph,
/// `✗ issues` badge.
pub(crate) const ERROR: &str = "\x1b[38;2;255;107;107m";

/// Cyan `#3fd7ff`. `tip:` / `note:` prefixes, source repo labels.
pub(crate) const INFO: &str = "\x1b[38;2;63;215;255m";

/// Brand lime `#c6ff3d`. **Brand mark only**: banner frame + wordmark,
/// `◆` farewell on `self uninstall`. Reserved for ceremonial brand surfaces;
/// not for operational status.
pub(crate) const BRAND: &str = "\x1b[38;2;198;255;61m";

/// Slate grey `#8a919b`. Paths, timing tails, hints, `[y/N]`, soft state
/// (`Cancelled.`, `Nothing to sync`), clap placeholders, example lines.
pub(crate) const SECONDARY: &str = "\x1b[38;2;138;145;155m";

/// Slate `#596069`. Tree branches `├─` / `└─`, bullet glyphs (`●` /
/// `•`), strikethrough overlay on removed entries. Never for content.
pub(crate) const INFRA: &str = "\x1b[38;2;89;96;105m";

/// Reset all attributes (`SGR 0`).
pub(crate) const RESET: &str = "\x1b[0m";

/// Strikethrough on (`SGR 9`): removed entries in trees.
pub(crate) const STRIKE: &str = "\x1b[9m";
/// Strikethrough off (`SGR 29`).
pub(crate) const STRIKE_RESET: &str = "\x1b[29m";

/// Carriage return + clear to end of line.
pub(crate) const CLEAR_LINE: &str = "\r\x1b[2K";

/// Clap help styling: lime `Usage:` / `Commands:` headers + literals,
/// `SECONDARY`-grey `<COMMAND>` / `<ARG>` placeholders.
///
/// The diagnostic roles are set too, so a clap arg error reads in the same
/// palette as a kasetto runtime error: `error:` in `ERROR`, clap's `tip:` and
/// its suggested value in `INFO`, the rejected token in `ATTENTION`. Left at
/// clap's defaults these fall back to basic 4-bit red/green/yellow, which is
/// the one place the CLI would speak a different color language than the rest
/// of its output.
pub(crate) fn clap_styles() -> Styles {
    let attention = Style::new().fg_color(Some(RgbColor(198, 255, 61).into())) | Effects::BOLD;
    let secondary = Style::new().fg_color(Some(RgbColor(138, 145, 155).into()));
    let error = Style::new().fg_color(Some(RgbColor(255, 107, 107).into())) | Effects::BOLD;
    let info = Style::new().fg_color(Some(RgbColor(63, 215, 255).into())) | Effects::BOLD;
    Styles::styled()
        .header(attention)
        .usage(attention)
        .literal(attention)
        .placeholder(secondary)
        .error(error)
        .valid(info)
        .invalid(attention)
}

/// Clap `after_help`: lime `Examples:` header, `SECONDARY` example lines.
#[macro_export]
macro_rules! cli_examples {
    ($($line:literal),* $(,)?) => {
        concat!(
            "\x1b[1m\x1b[38;2;198;255;61mExamples:\x1b[0m\n",
            $(
                concat!("  \x1b[38;2;138;145;155m", $line, "\x1b[0m\n"),
            )*
        )
    };
}
