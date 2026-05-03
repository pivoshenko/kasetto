use std::io::{Stdout, Write};

use clap::Parser;
use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::style::{Attribute, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};

use crate::cli::{Cli, Commands, SyncArgs};
use crate::colors::term;
use crate::error::Result;
use crate::model::{resolve_scope, Scope};
use crate::tui::draw_banner_or_fallback;

pub(super) fn prompt_sync_args(
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

    let title = format!("{program_name} | カセット");
    let mut row = draw_banner_or_fallback(stdout, &title, width, height, 0)?;

    execute!(
        stdout,
        MoveTo(0, row),
        SetForegroundColor(term::INFO),
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
        SetForegroundColor(term::SECONDARY),
        Print(format!(
            "Example: {} --config https://example.com/kasetto.yaml --dry-run",
            program_name
        )),
        ResetColor
    )?;
    row = row.saturating_add(1);

    execute!(
        stdout,
        MoveTo(0, row),
        SetForegroundColor(term::SECONDARY),
        Print(format!(
            "Shorthand: {} \"/path/to/kasetto.yaml\" --verbose",
            program_name
        )),
        ResetColor
    )?;
    row = row.saturating_add(1);

    let cfg_scope = resolve_scope(None, None);
    execute!(
        stdout,
        MoveTo(0, row),
        SetForegroundColor(term::SECONDARY),
        Print(format!(
            "Default scope from config: {} — append --global or --project to override.",
            scope_label(cfg_scope)
        )),
        ResetColor
    )?;
    row = row.saturating_add(2);

    execute!(
        stdout,
        MoveTo(0, row),
        SetForegroundColor(term::ACCENT),
        Print("sync> "),
        ResetColor
    )?;

    if input.is_empty() {
        execute!(
            stdout,
            SetForegroundColor(term::SECONDARY),
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
            SetForegroundColor(term::ERROR),
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
        SetForegroundColor(term::SECONDARY),
        Print("Enter run   Esc cancel   Ctrl-U clear"),
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

fn scope_label(scope: Scope) -> &'static str {
    match scope {
        Scope::Global => "global",
        Scope::Project => "project",
    }
}

#[cfg(test)]
mod tests {
    use super::parse_sync_args;

    #[test]
    fn parse_sync_args_accepts_shorthand_config_path() {
        let sync = parse_sync_args("kasetto", "kasetto.yaml --dry-run").expect("sync args");
        assert_eq!(sync.config.as_deref(), Some("kasetto.yaml"));
        assert!(sync.dry_run);
    }

    #[test]
    fn parse_sync_args_accepts_explicit_sync_command() {
        let sync =
            parse_sync_args("kasetto", "sync --config remote.yaml --verbose").expect("sync args");
        assert_eq!(sync.config.as_deref(), Some("remote.yaml"));
        assert!(sync.verbose);
    }

    #[test]
    fn parse_sync_args_accepts_scope_flags() {
        let sync =
            parse_sync_args("kasetto", "--config foo.yaml --project --dry-run").expect("sync");
        assert_eq!(sync.config.as_deref(), Some("foo.yaml"));
        assert!(sync.dry_run);
        assert!(sync.scope.project);
        assert!(!sync.scope.global);
    }
}
