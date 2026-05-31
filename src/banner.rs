use unicode_width::UnicodeWidthStr;

use crate::colors::{ATTENTION, BRAND, RESET};
use crate::ui::color_stdout_enabled;

/// Frame + logo: brand violet (`BRAND`) — the ceremonial brand mark surface.
const BANNER_FG: &str = BRAND;
/// Japanese subtitle: amber (`ATTENTION`) — the brand-adjacent lead hue.
const SUBTITLE_FG: &str = ATTENTION;

const LOGO_LINES: [&str; 6] = [
    "██╗  ██╗ █████╗ ███████╗███████╗████████╗████████╗ ██████╗ ",
    "██║ ██╔╝██╔══██╗██╔════╝██╔════╝╚══██╔══╝╚══██╔══╝██╔═══██╗",
    "█████╔╝ ███████║███████╗█████╗     ██║      ██║   ██║   ██║",
    "██╔═██╗ ██╔══██║╚════██║██╔══╝     ██║      ██║   ██║   ██║",
    "██║  ██╗██║  ██║███████║███████╗   ██║      ██║   ╚██████╔╝",
    "╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝╚══════╝   ╚═╝      ╚═╝    ╚═════╝ ",
];
const JAPANESE_SUBTITLE: &str = "スキル・パッケージ・マネージャー";
const LOGO_WIDTH: usize = 59;
/// Inner content width inside the frame (logo + 2-char gutter each side).
const INNER_WIDTH: usize = LOGO_WIDTH + 4;

fn frame_top(use_color: bool) -> String {
    let bar = "═".repeat(INNER_WIDTH);
    if use_color {
        format!("{BANNER_FG}╔{bar}╗{RESET}")
    } else {
        format!("╔{bar}╗")
    }
}

fn frame_bottom(use_color: bool) -> String {
    let bar = "═".repeat(INNER_WIDTH);
    if use_color {
        format!("{BANNER_FG}╚{bar}╝{RESET}")
    } else {
        format!("╚{bar}╝")
    }
}

fn frame_line(colored_content: &str, visible_width: usize, use_color: bool) -> String {
    let total_pad = INNER_WIDTH.saturating_sub(visible_width);
    let left = total_pad / 2;
    let right = total_pad - left;
    if use_color {
        format!(
            "{BANNER_FG}║{RESET}{lp}{colored_content}{rp}{BANNER_FG}║{RESET}",
            lp = " ".repeat(left),
            rp = " ".repeat(right),
        )
    } else {
        format!(
            "║{lp}{colored_content}{rp}║",
            lp = " ".repeat(left),
            rp = " ".repeat(right),
        )
    }
}

fn frame_blank(use_color: bool) -> String {
    frame_line("", 0, use_color)
}

fn logo_line(line: &str, use_color: bool) -> String {
    let colored = if use_color {
        format!("{BANNER_FG}{line}{RESET}")
    } else {
        line.to_string()
    };
    frame_line(&colored, UnicodeWidthStr::width(line), use_color)
}

fn tagline_inside(use_color: bool) -> String {
    let visible = UnicodeWidthStr::width(JAPANESE_SUBTITLE);
    let colored = if use_color {
        format!("{SUBTITLE_FG}{JAPANESE_SUBTITLE}{RESET}")
    } else {
        JAPANESE_SUBTITLE.to_string()
    };
    frame_line(&colored, visible, use_color)
}

pub(crate) fn banner_string(use_color: bool) -> String {
    let mut out = String::with_capacity(1024);
    out.push_str(&frame_top(use_color));
    out.push('\n');
    for line in LOGO_LINES {
        out.push_str(&logo_line(line, use_color));
        out.push('\n');
    }
    out.push_str(&frame_blank(use_color));
    out.push('\n');
    out.push_str(&tagline_inside(use_color));
    out.push('\n');
    out.push_str(&frame_bottom(use_color));
    out.push('\n');
    out
}

pub(crate) fn print_banner() {
    if !color_stdout_enabled() {
        return;
    }
    print!("{}", banner_string(true));
}
