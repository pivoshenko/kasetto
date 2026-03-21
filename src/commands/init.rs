use std::io::{stdin, stdout, IsTerminal, Stdout, Write};
use std::path::Path;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{
    self, disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
    LeaveAlternateScreen,
};

use crate::error::{err, Result};

const CONFIG_FILE: &str = "skills.config.yaml";

/// All supported agents — popular ones first, then alphabetical.
const AGENTS: &[&str] = &[
    "claude-code",
    "cursor",
    "windsurf",
    "codex",
    "amp",
    "cline",
    "github-copilot",
    "opencode",
    "openhands",
    "goose",
    "gemini-cli",
    "adal",
    "antigravity",
    "augment",
    "codebuddy",
    "command-code",
    "continue",
    "cortex",
    "crush",
    "deepagents",
    "droid",
    "iflow-cli",
    "junie",
    "kilo",
    "kimi-cli",
    "kiro-cli",
    "kode",
    "mcpjam",
    "mistral-vibe",
    "mux",
    "neovate",
    "openclaw",
    "pi",
    "pochi",
    "qoder",
    "qwen-code",
    "replit",
    "roo",
    "trae",
    "trae-cn",
    "universal",
    "warp",
    "zencoder",
];

pub fn run(force: bool) -> Result<()> {
    if !stdin().is_terminal() {
        return Err(err("kasetto init requires an interactive terminal"));
    }
    if Path::new(CONFIG_FILE).exists() && !force {
        return Err(err(format!(
            "{CONFIG_FILE} already exists. Use --force to overwrite."
        )));
    }

    println!();

    let agent = match select_agent()? {
        Some(a) => a,
        None => {
            println!("  Cancelled.");
            return Ok(());
        }
    };

    println!();

    let mut sources: Vec<(String, String)> = Vec::new();

    loop {
        println!("    A GitHub repo or local directory containing skill subdirectories.");
        println!("    Leave blank to skip and add sources manually later.");
        println!();
        let url = match prompt_text("  Source:", "")? {
            None => {
                println!("  Cancelled.");
                return Ok(());
            }
            Some(s) if s.trim().is_empty() => break,
            Some(s) => s.trim().to_string(),
        };

        let skills = match prompt_text("  Skills (* for all, or comma-separated names):", "*")? {
            None => {
                println!("  Cancelled.");
                return Ok(());
            }
            Some(s) if s.trim().is_empty() => "*".to_string(),
            Some(s) => s.trim().to_string(),
        };

        sources.push((url, skills));
        println!();

        match prompt_yn("  Add another source?")? {
            None => {
                println!("  Cancelled.");
                return Ok(());
            }
            Some(true) => {
                println!();
                continue;
            }
            Some(false) => break,
        }
    }

    write_config(&agent, &sources)?;

    let mut out = stdout();
    println!();
    execute!(
        out,
        Print("  "),
        SetForegroundColor(Color::Green),
        SetAttribute(Attribute::Bold),
        Print("✓"),
        SetAttribute(Attribute::Reset),
        ResetColor,
        Print(format!(" Created {CONFIG_FILE}\n")),
    )?;
    if sources.is_empty() {
        println!("    Edit {CONFIG_FILE} to add a source, then run kasetto sync.");
    } else {
        println!("    Run kasetto sync to install your skills.");
    }
    println!();

    Ok(())
}

fn select_agent() -> Result<Option<String>> {
    let mut stdout = stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, Hide)?;

    let result = run_agent_selector(&mut stdout);

    let _ = disable_raw_mode();
    let _ = execute!(stdout, Show, LeaveAlternateScreen);

    result
}

fn run_agent_selector(stdout: &mut Stdout) -> Result<Option<String>> {
    let mut selected = 0usize;
    let mut scroll = 0usize;

    loop {
        let (_, height) = terminal::size()?;
        let visible = (height as usize).saturating_sub(7);

        if selected < scroll {
            scroll = selected;
        } else if visible > 0 && selected >= scroll + visible {
            scroll = selected + 1 - visible;
        }

        draw_agent_selector(stdout, selected, scroll, visible)?;

        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if selected + 1 < AGENTS.len() {
                        selected += 1;
                    }
                }
                KeyCode::Char('g') => {
                    selected = 0;
                    scroll = 0;
                }
                KeyCode::Char('G') => {
                    selected = AGENTS.len() - 1;
                }
                KeyCode::PageUp => {
                    selected = selected.saturating_sub(visible.max(1));
                }
                KeyCode::PageDown => {
                    selected = (selected + visible.max(1)).min(AGENTS.len() - 1);
                }
                KeyCode::Enter => {
                    return Ok(Some(AGENTS[selected].to_string()));
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    return Ok(None);
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(None);
                }
                _ => {}
            },
            Event::Resize(_, _) => {}
            _ => {}
        }
    }
}

fn draw_agent_selector(
    stdout: &mut Stdout,
    selected: usize,
    scroll: usize,
    visible: usize,
) -> Result<()> {
    let (width, height) = terminal::size()?;
    let width = width as usize;
    let height = height as usize;

    execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;

    execute!(
        stdout,
        MoveTo(0, 0),
        SetForegroundColor(Color::Magenta),
        SetAttribute(Attribute::Bold),
        Print("kasetto init"),
        SetAttribute(Attribute::Reset),
        ResetColor,
    )?;
    execute!(
        stdout,
        MoveTo(0, 1),
        SetForegroundColor(Color::DarkGrey),
        Print("Choose your agent"),
        ResetColor,
    )?;
    execute!(
        stdout,
        MoveTo(0, 2),
        SetForegroundColor(Color::DarkGrey),
        Print("─".repeat(width.min(50))),
        ResetColor,
    )?;

    for i in 0..visible {
        let idx = scroll + i;
        if idx >= AGENTS.len() {
            break;
        }
        execute!(stdout, MoveTo(0, (3 + i) as u16))?;
        if idx == selected {
            execute!(
                stdout,
                SetForegroundColor(Color::Cyan),
                SetAttribute(Attribute::Bold),
                Print(format!("  › {}", AGENTS[idx])),
                SetAttribute(Attribute::Reset),
                ResetColor,
            )?;
        } else {
            execute!(
                stdout,
                SetForegroundColor(Color::DarkGrey),
                Print("    "),
                ResetColor,
                Print(AGENTS[idx]),
            )?;
        }
    }

    if scroll > 0 {
        execute!(
            stdout,
            MoveTo(0, 3),
            SetForegroundColor(Color::DarkGrey),
            Print("  ↑ more above"),
            ResetColor,
        )?;
    }
    if scroll + visible < AGENTS.len() {
        execute!(
            stdout,
            MoveTo(0, (3 + visible.saturating_sub(1)) as u16),
            SetForegroundColor(Color::DarkGrey),
            Print("  ↓ more below"),
            ResetColor,
        )?;
    }

    execute!(
        stdout,
        MoveTo(0, height.saturating_sub(3) as u16),
        SetForegroundColor(Color::DarkGrey),
        Print("─".repeat(width.min(50))),
        ResetColor,
    )?;
    execute!(
        stdout,
        MoveTo(0, height.saturating_sub(2) as u16),
        SetForegroundColor(Color::DarkGrey),
        Print(format!("{}/{} agents", selected + 1, AGENTS.len())),
        ResetColor,
    )?;
    execute!(
        stdout,
        MoveTo(0, height.saturating_sub(1) as u16),
        SetForegroundColor(Color::DarkGrey),
        Print("j/k  ↑/↓  Enter to select  gg/G to jump  Esc to cancel"),
        ResetColor,
    )?;

    stdout.flush()?;
    Ok(())
}

fn prompt_text(label: &str, default: &str) -> Result<Option<String>> {
    let mut stdout = stdout();
    let mut input = String::new();

    enable_raw_mode()?;
    execute!(stdout, Show)?;

    loop {
        execute!(
            stdout,
            Print("\r"),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::Cyan),
            SetAttribute(Attribute::Bold),
            Print(label),
            SetAttribute(Attribute::Reset),
            ResetColor,
            Print(" "),
        )?;

        if input.is_empty() && !default.is_empty() {
            execute!(
                stdout,
                SetForegroundColor(Color::DarkGrey),
                Print(default),
                ResetColor,
            )?;
        } else {
            execute!(stdout, Print(&input))?;
        }
        stdout.flush()?;

        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                KeyCode::Enter => {
                    execute!(stdout, Print("\r\n"))?;
                    let _ = disable_raw_mode();
                    return Ok(Some(input));
                }
                KeyCode::Esc => {
                    execute!(stdout, Print("\r\n"))?;
                    let _ = disable_raw_mode();
                    return Ok(None);
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    execute!(stdout, Print("\r\n"))?;
                    let _ = disable_raw_mode();
                    return Ok(None);
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    input.clear();
                }
                KeyCode::Char(ch) => {
                    input.push(ch);
                }
                _ => {}
            },
            Event::Paste(text) => {
                input.push_str(&text);
            }
            Event::Resize(_, _) => {}
            _ => {}
        }
    }
}

fn prompt_yn(label: &str) -> Result<Option<bool>> {
    let mut stdout = stdout();
    enable_raw_mode()?;
    execute!(stdout, Show)?;

    execute!(
        stdout,
        SetForegroundColor(Color::Cyan),
        SetAttribute(Attribute::Bold),
        Print(label),
        SetAttribute(Attribute::Reset),
        ResetColor,
        Print(" [y/N] "),
    )?;
    stdout.flush()?;

    let result = loop {
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    execute!(stdout, Print("y\r\n"))?;
                    break Ok(Some(true));
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Enter => {
                    execute!(stdout, Print("N\r\n"))?;
                    break Ok(Some(false));
                }
                KeyCode::Esc => {
                    execute!(stdout, Print("\r\n"))?;
                    break Ok(None);
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    execute!(stdout, Print("\r\n"))?;
                    break Ok(None);
                }
                _ => {}
            },
            _ => {}
        }
    };

    let _ = disable_raw_mode();
    result
}

fn write_config(agent: &str, sources: &[(String, String)]) -> Result<()> {
    let mut content = String::new();
    content.push_str(&format!("agent: {agent}\n\n"));
    content.push_str("skills:\n");

    if sources.is_empty() {
        content.push_str("  # Add a source to get started, for example:\n");
        content.push_str("  # - source: https://github.com/org/my-skills-repo\n");
        content.push_str("  #   skills: \"*\"\n");
    }

    for (source, skills) in sources {
        content.push_str(&format!("  - source: {source}\n"));
        if skills == "*" {
            content.push_str("    skills: \"*\"\n");
        } else {
            content.push_str("    skills:\n");
            for name in skills.split(',') {
                let name = name.trim();
                if !name.is_empty() {
                    content.push_str(&format!("      - {name}\n"));
                }
            }
        }
    }

    std::fs::write(CONFIG_FILE, content)
        .map_err(|e| err(format!("failed to write {CONFIG_FILE}: {e}")))?;
    Ok(())
}
