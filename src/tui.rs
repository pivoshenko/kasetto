use std::io::{stdout, Stdout};
use std::time::Duration;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::execute;
use crossterm::style::{Print, ResetColor, SetForegroundColor};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};

use crate::banner::{banner_lines, banner_width, BANNER_STAR_CELLS};
use crate::colors::term;
use crate::error::Result;

/// RAII guard that enters the alternate screen and restores on drop.
pub(crate) struct TuiGuard {
    pub stdout: Stdout,
}

impl TuiGuard {
    pub(crate) fn enter() -> Result<Self> {
        let mut stdout = stdout();
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen, Hide)?;
        Ok(Self { stdout })
    }
}

impl Drop for TuiGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.stdout, Show, LeaveAlternateScreen);
    }
}

/// Draw the ASCII banner starting at `top`. Returns the number of rows consumed (including spacing).
pub(crate) fn draw_banner(stdout: &mut Stdout, top: usize) -> Result<usize> {
    let lines = banner_lines();
    let subtitle_idx = lines.len().saturating_sub(1);
    for (offset, line) in lines.iter().enumerate() {
        let color = if offset == subtitle_idx {
            term::SECONDARY
        } else {
            term::BANNER
        };
        execute!(
            stdout,
            MoveTo(0, (top + offset) as u16),
            SetForegroundColor(color),
            Print(line),
            ResetColor
        )?;
    }
    Ok(lines.len() + 1)
}

/// Twinkling stars on the banner's empty + subtitle rows. `banner_origin_y` is the terminal row
/// where banner line 0 starts (use `0` on the home alternate screen).
pub(crate) fn draw_stars(
    stdout: &mut Stdout,
    elapsed: Duration,
    banner_origin_y: u16,
) -> Result<()> {
    let star_chars = [' ', '·', '✧', '✦', '✧', '·'];
    let tick = (elapsed.as_millis() / 200) as u16;
    for &(col, row, phase) in &BANNER_STAR_CELLS {
        let idx = ((tick + phase) as usize) % star_chars.len();
        let ch = star_chars[idx];
        if ch == ' ' {
            continue;
        }
        let y = banner_origin_y.saturating_add(row);
        execute!(
            stdout,
            MoveTo(col, y),
            SetForegroundColor(term::BANNER),
            Print(ch),
            ResetColor
        )?;
    }
    Ok(())
}

/// Draw the ASCII banner if the terminal is wide/tall enough, otherwise a compact title.
/// Returns the next row below the banner.
pub(crate) fn draw_banner_or_fallback(
    stdout: &mut Stdout,
    title: &str,
    width: usize,
    height: usize,
    top: u16,
) -> Result<u16> {
    if width >= banner_width() && height >= 18 {
        let row = draw_banner(stdout, top as usize)? as u16;
        if height >= 22 {
            draw_stars(stdout, std::time::Duration::ZERO, top)?;
        }
        Ok(row)
    } else {
        execute!(
            stdout,
            MoveTo(0, top),
            SetForegroundColor(term::ACCENT),
            Print(title),
            ResetColor
        )?;
        Ok(top.saturating_add(2))
    }
}
