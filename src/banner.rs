use std::io::IsTerminal;
use unicode_width::UnicodeWidthStr;

use crate::colors::{ansi_cursor_column_1based, ansi_cursor_up, BANNER, RESET, SECONDARY};

const BANNER_TOP: &str = "╔═══════════════════════════════════════════════════════════════╗";
const BANNER_BOTTOM: &str = "╚═══════════════════════════════════════════════════════════════╝";
const BANNER_INNER_WIDTH: usize = 63;
const LOGO_LINES: [&str; 6] = [
    "  ██╗  ██╗ █████╗ ███████╗███████╗████████╗████████╗ ██████╗   ",
    "  ██║ ██╔╝██╔══██╗██╔════╝██╔════╝╚══██╔══╝╚══██╔══╝██╔═══██╗  ",
    "  █████╔╝ ███████║███████╗█████╗     ██║      ██║   ██║   ██║  ",
    "  ██╔═██╗ ██╔══██║╚════██║██╔══╝     ██║      ██║   ██║   ██║  ",
    "  ██║  ██╗██║  ██║███████║███████╗   ██║      ██║   ╚██████╔╝  ",
    "  ╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝╚══════╝   ╚═╝      ╚═╝    ╚═════╝   ",
];
const JAPANESE_SUBTITLE: &str = "スキル・パッケージ・マネージャー";

/// Total banner lines: top border + LOGO_LINES + empty + subtitle + bottom border.
const BANNER_LINE_COUNT: usize = 1 + LOGO_LINES.len() + 1 + 1 + 1;

/// Screen column (0-based), row index in [`banner_lines`] (0 = top border), twinkle phase.
pub(crate) const BANNER_STAR_CELLS: [(u16, u16, u16); 10] = [
    (3, 7, 0),
    (8, 7, 2),
    (13, 7, 4),
    // Right margin: use phases that render as stars in the CLI overlay (not middle dots).
    (50, 7, 2),
    (55, 7, 3),
    (60, 7, 5),
    (2, 8, 6),
    (10, 8, 8),
    (52, 8, 0),
    (58, 8, 9),
];

fn color_stdout_enabled() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

fn empty_banner_line() -> String {
    format!("║{}║", " ".repeat(BANNER_INNER_WIDTH))
}

fn left_boxed_line(content: &str) -> String {
    let width = UnicodeWidthStr::width(content);
    let right_pad = BANNER_INNER_WIDTH.saturating_sub(width);
    format!("║{}{}║", content, " ".repeat(right_pad))
}

fn centered_boxed_line(content: &str) -> String {
    let width = UnicodeWidthStr::width(content);
    let total_pad = BANNER_INNER_WIDTH.saturating_sub(width);
    let left_pad = total_pad / 2;
    let right_pad = total_pad - left_pad;
    format!(
        "║{}{}{}║",
        " ".repeat(left_pad),
        content,
        " ".repeat(right_pad)
    )
}

fn colorize_content(line: &str, content: &str, color: &str, base: &str) -> String {
    line.replacen(content, &format!("{color}{content}{base}"), 1)
}

pub(crate) fn banner_lines() -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(BANNER_TOP.to_string());
    for logo in LOGO_LINES {
        lines.push(left_boxed_line(logo));
    }
    lines.push(empty_banner_line());
    lines.push(centered_boxed_line(JAPANESE_SUBTITLE));
    lines.push(BANNER_BOTTOM.to_string());
    lines
}

pub(crate) fn banner_width() -> usize {
    UnicodeWidthStr::width(BANNER_TOP)
}

pub(crate) fn banner_string(use_color: bool) -> String {
    let mut lines = banner_lines();
    if use_color {
        if let Some(subtitle) = lines.get_mut(LOGO_LINES.len() + 2) {
            *subtitle = colorize_content(subtitle, JAPANESE_SUBTITLE, SECONDARY, BANNER);
        }
        format!("{}{}{RESET}\n", BANNER, lines.join("\n"))
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

/// Glyph for one static “star” cell (filled, outline, or dot) from animation phase.
fn cli_static_star_glyph(phase: u16) -> &'static str {
    match phase % 6 {
        0 | 5 => "✦",
        1 | 4 => "·",
        2 => "✧",
        _ => "•",
    }
}

/// One static frame of banner stars using cursor motion (stdout must already show the banner).
fn cli_static_star_overlay() -> String {
    let up = BANNER_LINE_COUNT.saturating_sub(7);
    let mut s = String::with_capacity(256);
    s.push_str(RESET);
    s.push_str(&ansi_cursor_up(up as u16));

    let mut row7: Vec<(u16, u16)> = BANNER_STAR_CELLS
        .iter()
        .filter(|(_, r, _)| *r == 7)
        .map(|(c, _, ph)| (*c, *ph))
        .collect();
    row7.sort_unstable_by_key(|(c, _)| *c);
    for (col, ph) in row7 {
        let g = cli_static_star_glyph(ph);
        s.push_str(&ansi_cursor_column_1based(col.saturating_add(1)));
        s.push_str(BANNER);
        s.push_str(g);
        s.push_str(RESET);
    }
    s.push_str("\r\n");

    let mut row8: Vec<(u16, u16)> = BANNER_STAR_CELLS
        .iter()
        .filter(|(_, r, _)| *r == 8)
        .map(|(c, _, ph)| (*c, *ph))
        .collect();
    row8.sort_unstable_by_key(|(c, _)| *c);
    for (col, ph) in row8 {
        let g = cli_static_star_glyph(ph);
        s.push_str(&ansi_cursor_column_1based(col.saturating_add(1)));
        s.push_str(BANNER);
        s.push_str(g);
        s.push_str(RESET);
    }
    // Past bottom border; align with cursor after `banner_string`’s trailing newline.
    s.push_str("\r\n\r\n");
    s
}

pub(crate) fn print_banner() {
    let color = color_stdout_enabled();
    print!("{}", banner_string(color));
    if color && std::io::stdout().is_terminal() {
        print!("{}", cli_static_star_overlay());
    }
}

/// Print the full banner, or fall back to a one-line plain title when `use_plain` is true.
pub(crate) fn print_banner_or_plain(use_plain: bool) {
    if use_plain {
        println!("kasetto | カセット");
    } else {
        print_banner();
    }
}
