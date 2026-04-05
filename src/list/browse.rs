use std::io::stdout;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};

use crate::error::Result;

use super::render;
use super::session::{ListState, TerminalGuard};
use super::tab::Tab;
use super::types::BrowseInput;

pub(crate) fn browse(input: &BrowseInput) -> Result<()> {
    let mut guard = TerminalGuard::enter()?;
    let mut stdout = stdout();
    let started = Instant::now();
    let mut state = ListState::default();

    let tabs: Vec<Tab> = {
        let mut t = vec![Tab::Skills];
        if !input.mcps.is_empty() {
            t.push(Tab::Mcps);
        }
        t
    };
    let mut active_tab = 0usize;

    loop {
        let current_len = match tabs[active_tab] {
            Tab::Skills => input.skills.len(),
            Tab::Mcps => input.mcps.len(),
        };
        render::draw(
            &mut stdout,
            input,
            &mut state,
            &mut guard,
            &tabs,
            active_tab,
            started.elapsed(),
        )?;
        if !event::poll(Duration::from_millis(120))? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Up | KeyCode::Char('k') => state.move_by(-1, current_len),
                KeyCode::Down | KeyCode::Char('j') => state.move_by(1, current_len),
                KeyCode::PageUp => state.page_up(current_len),
                KeyCode::PageDown => state.page_down(current_len),
                KeyCode::Home | KeyCode::Char('g') => state.jump_to(0, current_len),
                KeyCode::End | KeyCode::Char('G') => {
                    state.jump_to(current_len.saturating_sub(1), current_len)
                }
                KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') if tabs.len() > 1 => {
                    active_tab = (active_tab + 1) % tabs.len();
                    state.selected = 0;
                    state.scroll = 0;
                }
                KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') if tabs.len() > 1 => {
                    active_tab = if active_tab == 0 {
                        tabs.len() - 1
                    } else {
                        active_tab - 1
                    };
                    state.selected = 0;
                    state.scroll = 0;
                }
                _ => {}
            },
            Event::Resize(_, _) => {}
            _ => {}
        }
    }

    Ok(())
}
