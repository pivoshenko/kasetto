use std::cmp::{max, min};
use std::io::{Stdout, Write};
use std::time::Duration;

use crossterm::cursor::{position, MoveTo};
use crossterm::queue;
use crossterm::style::{
    Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::size;
use crossterm::terminal::{Clear, ClearType};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::banner::{banner_lines, banner_width};
use crate::colors::term;
use crate::error::Result;
use crate::model::{InstalledSkill, Scope};
use crate::tui::draw_stars;

use super::session::{ListState, PaneRect};
use super::tab::Tab;
use super::types::{AssetEntry, BrowseInput};

/// Trait for items that can be displayed in a list pane.
trait ListItem {
    fn display_name(&self) -> &str;
}

impl ListItem for InstalledSkill {
    fn display_name(&self) -> &str {
        &self.name
    }
}

impl ListItem for AssetEntry {
    fn display_name(&self) -> &str {
        &self.name
    }
}

pub(super) fn draw(
    stdout: &mut Stdout,
    input: &BrowseInput,
    state: &mut ListState,
    tabs: &[Tab],
    active_tab: usize,
    elapsed: Duration,
    plain: bool,
) -> Result<()> {
    let (width, height) = size()?;
    let width = width as usize;
    let panel_height = height as usize;
    let colors = Colors::active(plain);

    clear_panel(stdout)?;

    if width < 72 || panel_height < 8 {
        draw_small_terminal(stdout, width, panel_height, &colors)?;
        stdout.flush()?;
        return Ok(());
    }

    let mut row = 0usize;
    let show_ascii_banner = width >= banner_width() && panel_height >= 16;
    if show_ascii_banner {
        move_to(stdout, 0, 0)?;
        stdout.flush()?;
        let (_, banner_origin_y) = position()?;
        let banner_h = draw_banner(stdout, width, 0, &colors)?;
        stdout.flush()?;
        if panel_height >= 22 {
            draw_stars(stdout, elapsed, banner_origin_y)?;
        }
        row += banner_h;
    } else {
        row += draw_compact_banner(stdout, width, row, &colors)?;
    }

    // Draw tab bar
    if tabs.len() > 1 {
        row = draw_tab_bar(stdout, width, row, tabs, active_tab, &colors)?;
    }

    let current_tab = tabs[active_tab];
    let item_count = match current_tab {
        Tab::Skills => input.skills.len(),
        Tab::Mcps => input.mcps.len(),
    };

    row = draw_header(stdout, width, row, item_count, current_tab.label(), &colors)?;

    let footer_height = 1usize;
    let content_top = row;
    let content_height = panel_height.saturating_sub(content_top + footer_height);
    if content_height < 4 {
        clear_panel(stdout)?;
        draw_small_terminal(stdout, width, panel_height, &colors)?;
        stdout.flush()?;
        return Ok(());
    }

    match current_tab {
        Tab::Skills => {
            let two_pane = content_height >= 8;
            let side_by_side = two_pane && width >= 80;
            if two_pane && !side_by_side {
                let list_height = max(5, content_height / 2);
                state.keep_visible(list_height.saturating_sub(2), input.skills.len());
                draw_list_pane(
                    stdout,
                    PaneRect {
                        left: 0,
                        top: content_top,
                        width,
                        height: list_height,
                    },
                    &input.skills,
                    state,
                    "Installed Skills",
                    &colors,
                )?;
                draw_skill_detail_pane(
                    stdout,
                    0,
                    content_top + list_height,
                    width,
                    content_height.saturating_sub(list_height),
                    input.skills.get(state.selected),
                    &colors,
                )?;
            } else if side_by_side {
                let list_width = (width / 3).clamp(34, 46);
                let detail_width = width.saturating_sub(list_width + 1);
                state.keep_visible(content_height.saturating_sub(2), input.skills.len());
                draw_list_pane(
                    stdout,
                    PaneRect {
                        left: 0,
                        top: content_top,
                        width: list_width,
                        height: content_height,
                    },
                    &input.skills,
                    state,
                    "Installed Skills",
                    &colors,
                )?;
                draw_skill_detail_pane(
                    stdout,
                    list_width + 1,
                    content_top,
                    detail_width,
                    content_height,
                    input.skills.get(state.selected),
                    &colors,
                )?;
            } else {
                state.keep_visible(content_height.saturating_sub(2), input.skills.len());
                draw_list_pane(
                    stdout,
                    PaneRect {
                        left: 0,
                        top: content_top,
                        width,
                        height: content_height,
                    },
                    &input.skills,
                    state,
                    "Installed Skills",
                    &colors,
                )?;
            }
        }
        Tab::Mcps => {
            let two_pane = content_height >= 8;
            let side_by_side = two_pane && width >= 80;
            if two_pane && !side_by_side {
                let list_height = max(5, content_height / 2);
                state.keep_visible(list_height.saturating_sub(2), input.mcps.len());
                draw_list_pane(
                    stdout,
                    PaneRect {
                        left: 0,
                        top: content_top,
                        width,
                        height: list_height,
                    },
                    &input.mcps,
                    state,
                    "MCP Servers",
                    &colors,
                )?;
                draw_mcp_detail_pane(
                    stdout,
                    0,
                    content_top + list_height,
                    width,
                    content_height.saturating_sub(list_height),
                    input.mcps.get(state.selected),
                    &colors,
                )?;
            } else if side_by_side {
                let list_width = (width / 3).clamp(34, 46);
                let detail_width = width.saturating_sub(list_width + 1);
                state.keep_visible(content_height.saturating_sub(2), input.mcps.len());
                draw_list_pane(
                    stdout,
                    PaneRect {
                        left: 0,
                        top: content_top,
                        width: list_width,
                        height: content_height,
                    },
                    &input.mcps,
                    state,
                    "MCP Servers",
                    &colors,
                )?;
                draw_mcp_detail_pane(
                    stdout,
                    list_width + 1,
                    content_top,
                    detail_width,
                    content_height,
                    input.mcps.get(state.selected),
                    &colors,
                )?;
            } else {
                state.keep_visible(content_height.saturating_sub(2), input.mcps.len());
                draw_list_pane(
                    stdout,
                    PaneRect {
                        left: 0,
                        top: content_top,
                        width,
                        height: content_height,
                    },
                    &input.mcps,
                    state,
                    "MCP Servers",
                    &colors,
                )?;
            }
        }
    }

    let tab_hint = if tabs.len() > 1 {
        "h/l or Tab switch tabs   "
    } else {
        ""
    };
    draw_footer(
        stdout,
        width,
        panel_height.saturating_sub(1),
        tab_hint,
        &colors,
    )?;
    stdout.flush()?;
    Ok(())
}

fn move_to(stdout: &mut Stdout, left: usize, top: usize) -> Result<()> {
    queue!(stdout, MoveTo(left as u16, top as u16))?;
    Ok(())
}

fn clear_panel(stdout: &mut Stdout) -> Result<()> {
    queue!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
    Ok(())
}

fn draw_small_terminal(
    stdout: &mut Stdout,
    width: usize,
    height: usize,
    colors: &Colors,
) -> Result<()> {
    let lines = [
        "kasetto list",
        "",
        "Terminal too small for the browser.",
        "Resize the window to at least 72x20.",
        "q quit",
    ];
    for (index, line) in lines.iter().enumerate() {
        if index >= height {
            break;
        }
        write_line(stdout, 0, index, width, line, colors, Style::Title)?;
    }
    Ok(())
}

fn draw_banner(stdout: &mut Stdout, width: usize, top: usize, colors: &Colors) -> Result<usize> {
    let lines = banner_lines();
    if width >= banner_width() {
        for (offset, line) in lines.iter().enumerate() {
            move_to(stdout, 0, top + offset)?;
            queue!(
                stdout,
                SetForegroundColor(colors.banner),
                Print(line),
                ResetColor
            )?;
        }
        Ok(lines.len() + 1)
    } else {
        write_line(stdout, 0, top, width, "kasetto", colors, Style::Title)?;
        Ok(2)
    }
}

fn draw_compact_banner(
    stdout: &mut Stdout,
    width: usize,
    top: usize,
    colors: &Colors,
) -> Result<usize> {
    write_line(
        stdout,
        0,
        top,
        width,
        "kasetto | skill browser",
        colors,
        Style::Title,
    )?;
    Ok(1)
}

fn draw_tab_bar(
    stdout: &mut Stdout,
    _width: usize,
    top: usize,
    tabs: &[Tab],
    active: usize,
    colors: &Colors,
) -> Result<usize> {
    move_to(stdout, 0, top)?;
    for (i, tab) in tabs.iter().enumerate() {
        if i == active {
            queue!(
                stdout,
                SetForegroundColor(colors.accent),
                SetAttribute(Attribute::Bold),
                Print(format!(" {} ", tab.label())),
                SetAttribute(Attribute::Reset),
                ResetColor
            )?;
        } else {
            queue!(
                stdout,
                SetForegroundColor(colors.secondary),
                Print(format!(" {} ", tab.label())),
                ResetColor
            )?;
        }
        if i < tabs.len() - 1 {
            queue!(
                stdout,
                SetForegroundColor(colors.border),
                Print("│"),
                ResetColor
            )?;
        }
    }
    Ok(top + 1)
}

fn draw_header(
    stdout: &mut Stdout,
    width: usize,
    top: usize,
    count: usize,
    label: &str,
    colors: &Colors,
) -> Result<usize> {
    let summary = format!("{count} {label}");
    write_line(stdout, 0, top, width, &summary, colors, Style::Secondary)?;
    Ok(top + 1)
}

fn draw_list_pane<T: ListItem>(
    stdout: &mut Stdout,
    rect: PaneRect,
    items: &[T],
    state: &ListState,
    title: &str,
    colors: &Colors,
) -> Result<()> {
    let PaneRect {
        left,
        top,
        width,
        height,
    } = rect;
    if width < 10 || height < 4 {
        return Ok(());
    }

    draw_box(stdout, left, top, width, height, title, colors)?;
    let inner_width = width.saturating_sub(2);
    let visible_rows = height.saturating_sub(2);
    let start = state.scroll;
    let end = min(start + visible_rows, items.len());

    for row in 0..visible_rows {
        let y = top + 1 + row;
        let item_index = start + row;
        if item_index >= end {
            write_fill(stdout, left + 1, y, inner_width, colors.background)?;
            continue;
        }

        let item = &items[item_index];
        let is_selected = item_index == state.selected;
        let pointer = if is_selected { "› " } else { "  " };
        let label_width = inner_width.saturating_sub(2);
        let label = truncate_width(item.display_name(), label_width);

        move_to(stdout, left + 1, y)?;
        if is_selected {
            queue!(
                stdout,
                SetForegroundColor(colors.accent),
                SetAttribute(Attribute::Bold),
                Print(pointer),
                SetForegroundColor(colors.text),
                Print(pad_width(&label, label_width)),
                SetAttribute(Attribute::Reset),
                ResetColor
            )?;
        } else {
            queue!(
                stdout,
                SetForegroundColor(colors.secondary),
                Print(pointer),
                SetForegroundColor(colors.text),
                Print(pad_width(&label, label_width)),
                ResetColor
            )?;
        }
    }

    Ok(())
}

fn draw_skill_detail_pane(
    stdout: &mut Stdout,
    left: usize,
    top: usize,
    width: usize,
    height: usize,
    item: Option<&InstalledSkill>,
    colors: &Colors,
) -> Result<()> {
    if width < 10 || height < 4 {
        return Ok(());
    }

    draw_box(stdout, left, top, width, height, "Details", colors)?;
    let inner_left = left + 1;
    let inner_top = top + 1;
    let inner_width = width.saturating_sub(2);
    let inner_height = height.saturating_sub(2);

    let Some(item) = item else {
        return Ok(());
    };

    let mut lines = Vec::new();
    let description = if item.description.is_empty() {
        "No description."
    } else {
        item.description.as_str()
    };
    lines.push(Line::label_value("Name", &item.name));
    lines.push(Line::label_value("Scope", scope_label(item.scope)));
    lines.push(Line::label_value(
        "Updated",
        &format!("{} ({})", item.updated_ago, item.updated_at),
    ));
    lines.push(Line::label_value("Description", description));

    let wrapped = wrap_lines(&lines, inner_width);
    for row in 0..inner_height {
        let y = inner_top + row;
        if let Some(line) = wrapped.get(row) {
            write_styled_line(stdout, inner_left, y, inner_width, line, colors)?;
        } else {
            write_fill(stdout, inner_left, y, inner_width, colors.background)?;
        }
    }

    Ok(())
}

fn draw_mcp_detail_pane(
    stdout: &mut Stdout,
    left: usize,
    top: usize,
    width: usize,
    height: usize,
    item: Option<&AssetEntry>,
    colors: &Colors,
) -> Result<()> {
    if width < 10 || height < 4 {
        return Ok(());
    }

    draw_box(stdout, left, top, width, height, "Details", colors)?;
    let inner_left = left + 1;
    let inner_top = top + 1;
    let inner_width = width.saturating_sub(2);
    let inner_height = height.saturating_sub(2);

    let Some(item) = item else {
        return Ok(());
    };

    let pack = if item.pack_file.is_empty() {
        "—"
    } else {
        item.pack_file.as_str()
    };
    let source = if item.source.is_empty() {
        "—"
    } else {
        item.source.as_str()
    };

    let lines = vec![
        Line::label_value("Server", &item.name),
        Line::label_value("Scope", scope_label(item.scope)),
        Line::label_value("Pack file", pack),
        Line::label_value("Source", source),
    ];

    let wrapped = wrap_lines(&lines, inner_width);
    for row in 0..inner_height {
        let y = inner_top + row;
        if let Some(line) = wrapped.get(row) {
            write_styled_line(stdout, inner_left, y, inner_width, line, colors)?;
        } else {
            write_fill(stdout, inner_left, y, inner_width, colors.background)?;
        }
    }

    Ok(())
}

fn scope_label(scope: Scope) -> &'static str {
    match scope {
        Scope::Global => "global",
        Scope::Project => "project",
    }
}

fn draw_footer(
    stdout: &mut Stdout,
    width: usize,
    top: usize,
    tab_hint: &str,
    colors: &Colors,
) -> Result<()> {
    let hint = format!(
        "q quit   ↑/↓ or j/k move   PgUp/PgDn page   g/G jump   {}",
        tab_hint
    );
    write_line(stdout, 0, top, width, &hint, colors, Style::Secondary)?;
    Ok(())
}

fn draw_box(
    stdout: &mut Stdout,
    left: usize,
    top: usize,
    width: usize,
    height: usize,
    title: &str,
    colors: &Colors,
) -> Result<()> {
    move_to(stdout, left, top)?;
    let horizontal = "─".repeat(width.saturating_sub(2));
    queue!(
        stdout,
        SetForegroundColor(colors.border),
        Print("┌"),
        Print(horizontal.clone()),
        Print("┐"),
        ResetColor
    )?;

    for offset in 1..height.saturating_sub(1) {
        move_to(stdout, left, top + offset)?;
        queue!(
            stdout,
            SetForegroundColor(colors.border),
            Print("│"),
            SetBackgroundColor(colors.background),
            Print(" ".repeat(width.saturating_sub(2))),
            ResetColor,
            SetForegroundColor(colors.border),
            Print("│"),
            ResetColor
        )?;
    }

    if height >= 2 {
        move_to(stdout, left, top + height - 1)?;
        queue!(
            stdout,
            SetForegroundColor(colors.border),
            Print("└"),
            Print("─".repeat(width.saturating_sub(2))),
            Print("┘"),
            ResetColor
        )?;
    }

    let title_text = truncate_width(&format!(" {} ", title), width.saturating_sub(4));
    move_to(stdout, left + 2, top)?;
    queue!(
        stdout,
        SetForegroundColor(colors.accent),
        Print(title_text),
        ResetColor
    )?;
    Ok(())
}

fn write_fill(stdout: &mut Stdout, left: usize, top: usize, width: usize, bg: Color) -> Result<()> {
    move_to(stdout, left, top)?;
    queue!(
        stdout,
        SetBackgroundColor(bg),
        Print(" ".repeat(width)),
        ResetColor
    )?;
    Ok(())
}

fn write_line(
    stdout: &mut Stdout,
    left: usize,
    top: usize,
    width: usize,
    text: &str,
    colors: &Colors,
    style: Style,
) -> Result<()> {
    let line = StyledLine::new(style, truncate_width(text, width));
    write_styled_line(stdout, left, top, width, &line, colors)
}

fn write_styled_line(
    stdout: &mut Stdout,
    left: usize,
    top: usize,
    width: usize,
    line: &StyledLine,
    colors: &Colors,
) -> Result<()> {
    move_to(stdout, left, top)?;
    match line.style {
        Style::Title => queue!(
            stdout,
            SetForegroundColor(colors.accent),
            SetAttribute(Attribute::Bold)
        )?,
        Style::Secondary => queue!(stdout, SetForegroundColor(colors.secondary))?,
        Style::Value => queue!(stdout, SetForegroundColor(colors.text))?,
    }
    queue!(
        stdout,
        SetBackgroundColor(colors.background),
        Print(pad_width(&line.text, width)),
        ResetColor,
        SetAttribute(Attribute::Reset)
    )?;
    Ok(())
}

fn wrap_lines(lines: &[Line], width: usize) -> Vec<StyledLine> {
    let mut wrapped = Vec::new();
    for line in lines {
        match line {
            Line::LabelValue(label, value) => {
                wrapped.push(StyledLine::new(Style::Secondary, format!("{label}:")));
                wrapped.extend(wrap_text(value, width, Style::Value));
                wrapped.push(StyledLine::new(Style::Value, String::new()));
            }
        }
    }
    while matches!(wrapped.last(), Some(StyledLine { text, .. }) if text.is_empty()) {
        wrapped.pop();
    }
    wrapped
}

fn wrap_text(text: &str, width: usize, style: Style) -> Vec<StyledLine> {
    if width == 0 {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;

    for word in text.split_whitespace() {
        let word_width = UnicodeWidthStr::width(word);
        let spacer = usize::from(!current.is_empty());
        if current_width + spacer + word_width > width && !current.is_empty() {
            lines.push(StyledLine::new(style, current));
            current = word.to_string();
            current_width = word_width;
        } else {
            if !current.is_empty() {
                current.push(' ');
                current_width += 1;
            }
            current.push_str(word);
            current_width += word_width;
        }
    }

    if current.is_empty() {
        lines.push(StyledLine::new(style, String::new()));
    } else {
        lines.push(StyledLine::new(style, current));
    }
    lines
}

fn truncate_width(text: &str, max_width: usize) -> String {
    if UnicodeWidthStr::width(text) <= max_width {
        return text.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    if max_width == 1 {
        return "…".to_string();
    }

    let mut out = String::new();
    let mut width = 0usize;
    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width + 1 > max_width {
            break;
        }
        out.push(ch);
        width += ch_width;
    }
    out.push('…');
    out
}

fn pad_width(text: &str, width: usize) -> String {
    let actual = UnicodeWidthStr::width(text);
    if actual >= width {
        text.to_string()
    } else {
        format!("{text}{}", " ".repeat(width - actual))
    }
}

#[derive(Clone, Copy)]
enum Style {
    Title,
    Secondary,
    Value,
}

struct StyledLine {
    style: Style,
    text: String,
}

impl StyledLine {
    fn new(style: Style, text: String) -> Self {
        Self { style, text }
    }
}

enum Line {
    LabelValue(String, String),
}

impl Line {
    fn label_value(label: &str, value: &str) -> Self {
        Self::LabelValue(label.to_string(), value.to_string())
    }
}

struct Colors {
    banner: Color,
    accent: Color,
    border: Color,
    text: Color,
    secondary: Color,
    background: Color,
}

impl Colors {
    fn active(plain: bool) -> Self {
        if plain || std::env::var_os("NO_COLOR").is_some() {
            Self {
                banner: term::TEXT,
                accent: term::TEXT,
                border: term::TEXT,
                text: term::TEXT,
                secondary: term::TEXT,
                background: Color::Reset,
            }
        } else {
            Self {
                banner: term::BANNER,
                accent: term::ACCENT,
                border: term::SECONDARY,
                text: term::TEXT,
                secondary: term::SECONDARY,
                background: Color::Reset,
            }
        }
    }
}
