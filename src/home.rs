use clap::Parser;
use std::io::{stdout, IsTerminal, Stdout, Write};
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{
    self, disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
    LeaveAlternateScreen,
};

use crate::banner::{banner_lines, banner_width};
use crate::cli::{Cli, Commands, SyncArgs};
use crate::error::Result;

pub fn run(program_name: &str, default_config: &str) -> Result<()> {
    if !stdout().is_terminal() || std::env::var_os("NO_TUI").is_some() {
        print_sleeping_hint(program_name, default_config);
        return Ok(());
    }

    match browse(program_name, default_config)? {
        HomeAction::Sync(sync) => crate::commands::sync::run(
            &sync.config.unwrap_or_else(|| default_config.into()),
            sync.dry_run,
            sync.quiet,
            sync.json,
            sync.plain,
            sync.verbose,
        ),
        HomeAction::List => crate::commands::list::run(false),
        HomeAction::Doctor => crate::commands::doctor::run(false),
        HomeAction::Quit => Ok(()),
    }
}

enum HomeAction {
    Sync(SyncArgs),
    List,
    Doctor,
    Quit,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HomeItemAction {
    Sync,
    List,
    Doctor,
    Quit,
}

#[derive(Clone, Copy)]
struct HomeItem {
    title: &'static str,
    command: &'static str,
    detail: &'static str,
    action: HomeItemAction,
}

const HOME_ITEMS: [HomeItem; 4] = [
    HomeItem {
        title: "sync",
        command: "--config <path-or-url> [--dry-run] [--verbose]",
        detail: "Prompt for sync args, then run sync immediately.",
        action: HomeItemAction::Sync,
    },
    HomeItem {
        title: "list",
        command: "kasetto list",
        detail: "Browse installed skills from the local manifest.",
        action: HomeItemAction::List,
    },
    HomeItem {
        title: "doctor",
        command: "kasetto doctor",
        detail: "Inspect version, paths, and the latest sync state.",
        action: HomeItemAction::Doctor,
    },
    HomeItem {
        title: "quit",
        command: "q",
        detail: "Exit without running a command.",
        action: HomeItemAction::Quit,
    },
];

fn browse(program_name: &str, default_config: &str) -> Result<HomeAction> {
    let mut guard = HomeGuard::enter()?;
    let started = Instant::now();
    let mut selected = 1usize;

    loop {
        draw(
            &mut guard.stdout,
            selected,
            started.elapsed(),
            program_name,
            default_config,
        )?;
        if event::poll(Duration::from_millis(120))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        selected = selected.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        selected = (selected + 1).min(HOME_ITEMS.len().saturating_sub(1));
                    }
                    KeyCode::Tab => {
                        selected = (selected + 1) % HOME_ITEMS.len();
                    }
                    KeyCode::Char('s') => {
                        if let Some(sync) =
                            prompt_sync_args(&mut guard.stdout, program_name, default_config)?
                        {
                            return Ok(HomeAction::Sync(sync));
                        }
                    }
                    KeyCode::Char('l') => return Ok(HomeAction::List),
                    KeyCode::Char('d') => return Ok(HomeAction::Doctor),
                    KeyCode::Enter => match HOME_ITEMS[selected].action {
                        HomeItemAction::Sync => {
                            if let Some(sync) =
                                prompt_sync_args(&mut guard.stdout, program_name, default_config)?
                            {
                                return Ok(HomeAction::Sync(sync));
                            }
                        }
                        HomeItemAction::List => return Ok(HomeAction::List),
                        HomeItemAction::Doctor => return Ok(HomeAction::Doctor),
                        HomeItemAction::Quit => return Ok(HomeAction::Quit),
                    },
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(HomeAction::Quit),
                    _ => {}
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }
}

struct HomeGuard {
    stdout: Stdout,
}

impl HomeGuard {
    fn enter() -> Result<Self> {
        let mut stdout = stdout();
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen, Hide)?;
        Ok(Self { stdout })
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.stdout, Show, LeaveAlternateScreen);
    }
}

fn draw(
    stdout: &mut Stdout,
    selected: usize,
    elapsed: Duration,
    program_name: &str,
    default_config: &str,
) -> Result<()> {
    let (width, height) = terminal::size()?;
    let width = width as usize;
    let height = height as usize;
    let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let frame = frames[((elapsed.as_millis() / 80) as usize) % frames.len()];

    execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;

    let mut row = 0u16;
    if width >= banner_width() && height >= 18 {
        for line in banner_lines() {
            execute!(
                stdout,
                MoveTo(0, row),
                SetForegroundColor(Color::Magenta),
                Print(line),
                ResetColor
            )?;
            row = row.saturating_add(1);
        }
        row = row.saturating_add(1);
    } else {
        execute!(
            stdout,
            MoveTo(0, row),
            SetForegroundColor(Color::Magenta),
            SetAttribute(Attribute::Bold),
            Print(format!("{} | カセット", program_name)),
            SetAttribute(Attribute::Reset),
            ResetColor
        )?;
        row = row.saturating_add(2);
    }

    execute!(
        stdout,
        MoveTo(0, row),
        SetForegroundColor(Color::DarkGrey),
        Print(frame),
        ResetColor,
        Print(" "),
        SetForegroundColor(Color::Cyan),
        SetAttribute(Attribute::Bold),
        Print("Sleeping"),
        SetAttribute(Attribute::Reset),
        ResetColor
    )?;
    row = row.saturating_add(1);

    execute!(
        stdout,
        MoveTo(0, row),
        Print("No config provided. Pick a command to continue.")
    )?;
    row = row.saturating_add(2);

    for (index, item) in HOME_ITEMS.iter().enumerate() {
        execute!(stdout, MoveTo(0, row))?;
        if index == selected {
            execute!(
                stdout,
                SetForegroundColor(Color::Cyan),
                SetAttribute(Attribute::Bold),
                Print("› "),
                Print(format!("{:<8}", item.title)),
                SetAttribute(Attribute::Reset),
                ResetColor
            )?;
        } else {
            execute!(
                stdout,
                SetForegroundColor(Color::DarkGrey),
                Print("  "),
                ResetColor,
                SetForegroundColor(Color::Magenta),
                Print(format!("{:<8}", item.title)),
                ResetColor
            )?;
        }

        execute!(
            stdout,
            Print(" "),
            SetForegroundColor(if index == selected {
                Color::White
            } else {
                Color::Grey
            }),
            SetAttribute(if index == selected {
                Attribute::Underlined
            } else {
                Attribute::NoUnderline
            }),
            Print(command_text(program_name, item)),
            SetAttribute(Attribute::NoUnderline),
            ResetColor
        )?;
        row = row.saturating_add(1);

        let detail = if item.action == HomeItemAction::Sync {
            format!("{} Example default name: {}.", item.detail, default_config)
        } else {
            item.detail.to_string()
        };
        execute!(
            stdout,
            MoveTo(2, row),
            SetForegroundColor(if index == selected {
                Color::Grey
            } else {
                Color::DarkGrey
            }),
            Print(detail),
            ResetColor
        )?;
        row = row.saturating_add(2);
    }

    let footer_row = height.saturating_sub(2) as u16;
    execute!(
        stdout,
        MoveTo(0, footer_row),
        SetForegroundColor(Color::DarkGrey),
        Print("Use ↑/↓ or j/k to move, Enter to run, s/l/d for shortcuts, q to quit."),
        ResetColor
    )?;

    stdout.flush()?;
    Ok(())
}

fn prompt_sync_args(
    stdout: &mut Stdout,
    program_name: &str,
    default_config: &str,
) -> Result<Option<SyncArgs>> {
    let mut input = String::new();
    let mut error = None::<String>;

    loop {
        draw_sync_prompt(
            stdout,
            program_name,
            default_config,
            &input,
            error.as_deref(),
        )?;
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                KeyCode::Enter => match parse_sync_args(program_name, &input) {
                    Ok(sync) => return Ok(Some(sync)),
                    Err(message) => error = Some(message),
                },
                KeyCode::Esc => return Ok(None),
                KeyCode::Backspace => {
                    input.pop();
                    error = None;
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    input.clear();
                    error = None;
                }
                KeyCode::Char(ch) => {
                    input.push(ch);
                    error = None;
                }
                _ => {}
            },
            Event::Paste(text) => {
                input.push_str(&text);
                error = None;
            }
            Event::Resize(_, _) => {}
            _ => {}
        }
    }
}

fn draw_sync_prompt(
    stdout: &mut Stdout,
    program_name: &str,
    default_config: &str,
    input: &str,
    error: Option<&str>,
) -> Result<()> {
    let (width, height) = terminal::size()?;
    let width = width as usize;
    let height = height as usize;

    execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;

    let mut row = 0u16;
    if width >= banner_width() && height >= 18 {
        for line in banner_lines() {
            execute!(
                stdout,
                MoveTo(0, row),
                SetForegroundColor(Color::Magenta),
                Print(line),
                ResetColor
            )?;
            row = row.saturating_add(1);
        }
        row = row.saturating_add(1);
    } else {
        execute!(
            stdout,
            MoveTo(0, row),
            SetForegroundColor(Color::Magenta),
            SetAttribute(Attribute::Bold),
            Print(format!("{} | カセット", program_name)),
            SetAttribute(Attribute::Reset),
            ResetColor
        )?;
        row = row.saturating_add(2);
    }

    execute!(
        stdout,
        MoveTo(0, row),
        SetForegroundColor(Color::Cyan),
        SetAttribute(Attribute::Bold),
        Print("Sync Args"),
        SetAttribute(Attribute::Reset),
        ResetColor
    )?;
    row = row.saturating_add(1);

    execute!(
        stdout,
        MoveTo(0, row),
        Print("Enter sync args exactly as you would after the binary name.")
    )?;
    row = row.saturating_add(1);

    execute!(
        stdout,
        MoveTo(0, row),
        SetForegroundColor(Color::DarkGrey),
        Print(format!(
            "Example: {} --config https://example.com/skills.config.yaml --dry-run",
            program_name
        )),
        ResetColor
    )?;
    row = row.saturating_add(1);

    execute!(
        stdout,
        MoveTo(0, row),
        SetForegroundColor(Color::DarkGrey),
        Print(format!(
            "Shorthand: {} \"/path/to/skills.config.yaml\" --verbose",
            program_name
        )),
        ResetColor
    )?;
    row = row.saturating_add(2);

    execute!(
        stdout,
        MoveTo(0, row),
        SetForegroundColor(Color::Magenta),
        Print("sync> "),
        ResetColor
    )?;

    if input.is_empty() {
        execute!(
            stdout,
            SetForegroundColor(Color::DarkGrey),
            Print(format!("--config {}", default_config)),
            ResetColor
        )?;
    } else {
        execute!(stdout, Print(input))?;
    }
    let input_row = row;
    row = row.saturating_add(2);

    if let Some(message) = error {
        execute!(
            stdout,
            MoveTo(0, row),
            SetForegroundColor(Color::Red),
            Print(message),
            ResetColor
        )?;
    }

    let footer_row = height.saturating_sub(2) as u16;
    let input_col = if input.is_empty() {
        6
    } else {
        6 + input.chars().count() as u16
    };
    execute!(
        stdout,
        MoveTo(0, footer_row),
        SetForegroundColor(Color::DarkGrey),
        Print("Enter to run, Esc to cancel, Ctrl-U to clear."),
        ResetColor,
        MoveTo(input_col, input_row)
    )?;

    stdout.flush()?;
    Ok(())
}

fn parse_sync_args(program_name: &str, input: &str) -> std::result::Result<SyncArgs, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Enter sync args or a config path to continue.".into());
    }

    let mut tokens = shlex::split(trimmed)
        .ok_or_else(|| "Could not parse sync args. Check quotes and escaping.".to_string())?;

    if matches!(tokens.first().map(String::as_str), Some("sync")) {
        tokens.remove(0);
    }

    if matches!(tokens.first().map(String::as_str), Some(first) if !first.starts_with('-')) {
        tokens.insert(0, "--config".into());
    }

    let argv = std::iter::once(program_name.to_string())
        .chain(std::iter::once("sync".to_string()))
        .chain(tokens)
        .collect::<Vec<_>>();

    let cli = Cli::try_parse_from(argv).map_err(|err| err.to_string())?;
    match cli.command {
        Some(Commands::Sync { sync }) => Ok(sync),
        _ => Err("Sync args did not resolve to the sync command.".into()),
    }
}

fn print_sleeping_hint(program_name: &str, default_config: &str) {
    println!("Sleeping");
    println!("No config provided.");
    println!();
    println!("Try one of these next:");
    println!("  {} sync --config {}", program_name, default_config);
    println!("  {} list", program_name);
    println!("  {} doctor", program_name);
}

fn command_text(program_name: &str, item: &HomeItem) -> String {
    item.command.replace("kasetto", program_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sync_args_accepts_shorthand_config_path() {
        let sync = parse_sync_args("kasetto", "skills.config.yaml --dry-run").expect("sync args");
        assert_eq!(sync.config.as_deref(), Some("skills.config.yaml"));
        assert!(sync.dry_run);
    }

    #[test]
    fn parse_sync_args_accepts_explicit_sync_command() {
        let sync =
            parse_sync_args("kasetto", "sync --config remote.yaml --verbose").expect("sync args");
        assert_eq!(sync.config.as_deref(), Some("remote.yaml"));
        assert!(sync.verbose);
    }
}
