//! Full-screen menu when no default config exists (`kasetto` with no args).

mod prompt;

use std::io::{stdout, IsTerminal, Stdout, Write};
use std::time::{Duration, Instant};

use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::style::{Attribute, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};

use crate::banner::banner_width;
use crate::cli::SyncArgs;
use crate::colors::term;
use crate::error::Result;
use crate::tui::{draw_banner_or_fallback, draw_stars, TuiGuard};
use crate::ui::SPINNER_FRAMES;

use prompt::prompt_sync_args;

pub(crate) fn run(program_name: &str, default_config: &str) -> Result<()> {
    if !stdout().is_terminal() || std::env::var_os("NO_TUI").is_some() {
        print_sleeping_hint(program_name, default_config);
        return Ok(());
    }

    match browse(program_name, default_config)? {
        HomeAction::Sync(sync) => {
            let config = sync.config.unwrap_or_else(|| default_config.into());
            crate::commands::sync::run(&crate::commands::sync::SyncOptions {
                config_path: &config,
                dry_run: sync.dry_run,
                quiet: sync.quiet,
                as_json: sync.json,
                plain: sync.plain,
                verbose: sync.verbose,
                scope_override: sync.scope.scope_override(),
                show_banner: true,
            })
        }
        HomeAction::Init => crate::commands::init::run(false),
        HomeAction::List => crate::commands::list::run(false, None),
        HomeAction::Doctor => crate::commands::doctor::run(false, None, program_name),
        HomeAction::SelfUpdate => crate::commands::self_update::run(false),
        HomeAction::Clean => crate::commands::clean::run(false, false, false, None),
        HomeAction::Quit => Ok(()),
    }
}

enum HomeAction {
    Sync(SyncArgs),
    Init,
    List,
    Doctor,
    SelfUpdate,
    Clean,
    Quit,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HomeItemAction {
    Sync,
    Init,
    List,
    Doctor,
    SelfUpdate,
    Clean,
    Quit,
}

impl HomeItemAction {
    fn home_action(self) -> Option<HomeAction> {
        match self {
            HomeItemAction::Sync => None,
            HomeItemAction::Init => Some(HomeAction::Init),
            HomeItemAction::List => Some(HomeAction::List),
            HomeItemAction::Doctor => Some(HomeAction::Doctor),
            HomeItemAction::SelfUpdate => Some(HomeAction::SelfUpdate),
            HomeItemAction::Clean => Some(HomeAction::Clean),
            HomeItemAction::Quit => Some(HomeAction::Quit),
        }
    }
}

#[derive(Clone, Copy)]
struct HomeItem {
    title: &'static str,
    command: &'static str,
    action: HomeItemAction,
}

const HOME_ITEMS: [HomeItem; 7] = [
    HomeItem {
        title: "init",
        command: "kasetto init [--force]",
        action: HomeItemAction::Init,
    },
    HomeItem {
        title: "sync",
        command: "kasetto sync --config <path-or-url> [--dry-run] [--verbose]",
        action: HomeItemAction::Sync,
    },
    HomeItem {
        title: "list",
        command: "kasetto list",
        action: HomeItemAction::List,
    },
    HomeItem {
        title: "doctor",
        command: "kasetto doctor",
        action: HomeItemAction::Doctor,
    },
    HomeItem {
        title: "clean",
        command: "kasetto clean",
        action: HomeItemAction::Clean,
    },
    HomeItem {
        title: "self update",
        command: "kasetto self update",
        action: HomeItemAction::SelfUpdate,
    },
    HomeItem {
        title: "quit",
        command: "q",
        action: HomeItemAction::Quit,
    },
];

fn browse(program_name: &str, default_config: &str) -> Result<HomeAction> {
    let mut guard = TuiGuard::enter()?;
    let started = Instant::now();
    let mut selected = 0usize;

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

                    KeyCode::Char('i') => return Ok(HomeAction::Init),
                    KeyCode::Char('s') => {
                        if let Some(sync) =
                            prompt_sync_args(&mut guard.stdout, program_name, default_config)?
                        {
                            return Ok(HomeAction::Sync(sync));
                        }
                    }
                    KeyCode::Char('l') => return Ok(HomeAction::List),
                    KeyCode::Char('d') => return Ok(HomeAction::Doctor),
                    KeyCode::Char('c') => return Ok(HomeAction::Clean),
                    KeyCode::Char('u') => return Ok(HomeAction::SelfUpdate),
                    KeyCode::Enter => {
                        let action = HOME_ITEMS[selected].action;
                        if let Some(ha) = action.home_action() {
                            return Ok(ha);
                        }
                        if let Some(sync) =
                            prompt_sync_args(&mut guard.stdout, program_name, default_config)?
                        {
                            return Ok(HomeAction::Sync(sync));
                        }
                    }
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(HomeAction::Quit),
                    _ => {}
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }
}

fn draw(
    stdout: &mut Stdout,
    selected: usize,
    elapsed: Duration,
    program_name: &str,
    _default_config: &str,
) -> Result<()> {
    let (width, height) = terminal::size()?;
    let width = width as usize;
    let height = height as usize;
    let frame = SPINNER_FRAMES[((elapsed.as_millis() / 80) as usize) % SPINNER_FRAMES.len()];

    execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;

    let title = format!("{program_name} | カセット");
    let mut row = draw_banner_or_fallback(stdout, &title, width, height, 0)?;
    if width >= banner_width() && height >= 22 {
        draw_stars(stdout, elapsed, 0)?;
    }

    execute!(
        stdout,
        MoveTo(0, row),
        SetForegroundColor(term::SECONDARY),
        Print(frame),
        ResetColor,
        Print(" "),
        SetForegroundColor(term::INFO),
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
        if row as usize >= height.saturating_sub(3) {
            break;
        }
        execute!(stdout, MoveTo(0, row))?;
        if index == selected {
            execute!(
                stdout,
                SetForegroundColor(term::INFO),
                SetAttribute(Attribute::Bold),
                Print("› "),
                Print(format!("{:<12}", item.title)),
                SetAttribute(Attribute::Reset),
                ResetColor
            )?;
        } else {
            execute!(
                stdout,
                SetForegroundColor(term::SECONDARY),
                Print("  "),
                ResetColor,
                SetForegroundColor(term::ACCENT),
                Print(format!("{:<12}", item.title)),
                ResetColor
            )?;
        }

        execute!(
            stdout,
            Print(" "),
            SetForegroundColor(if index == selected {
                term::TEXT
            } else {
                term::SECONDARY
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
    }

    let footer_row = height.saturating_sub(2) as u16;
    execute!(
        stdout,
        MoveTo(0, footer_row),
        SetForegroundColor(term::SECONDARY),
        Print("q quit   ↑/↓ or j/k move   Enter run   i/s/l/d/c/u shortcuts"),
        ResetColor
    )?;

    stdout.flush()?;
    Ok(())
}

fn print_sleeping_hint(program_name: &str, default_config: &str) {
    println!("Sleeping");
    println!("No config provided.");
    println!();
    println!("Try one of these next:");
    println!("  {} init", program_name);
    println!("  {} sync --config {}", program_name, default_config);
    println!("  {} list", program_name);
    println!("  {} doctor", program_name);
    println!("  {} clean", program_name);
    println!("  {} self update", program_name);
}

fn command_text(program_name: &str, item: &HomeItem) -> String {
    item.command.replace("kasetto", program_name)
}
