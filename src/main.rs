mod app;
mod data;
mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{env, io, time::Duration};

struct CliConfig {
    preview: bool,
    width: Option<usize>,
    reserved: Option<usize>,
    fill: Option<bool>,
}

fn main() -> Result<()> {
    let config = parse_args();
    if let Some(width) = config.width {
        env::set_var("CC_STATUSLINE_WIDTH", width.to_string());
    }
    if let Some(reserved) = config.reserved {
        env::set_var("CC_STATUSLINE_RESERVED", reserved.to_string());
    }

    if let Some(fill) = config.fill {
        env::set_var("CC_STATUSLINE_FILL", if fill { "1" } else { "0" });
    }

    if config.preview {
        let mut terminal = setup_terminal()?;
        let result = run_preview(&mut terminal);
        restore_terminal(&mut terminal)?;
        result
    } else {
        run_statusline()
    }
}

fn parse_args() -> CliConfig {
    let mut preview = false;
    let mut width = None;
    let mut reserved = None;
    let mut fill = None;

    for arg in env::args().skip(1) {
        if arg == "--preview" || arg == "-p" {
            preview = true;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--width=") {
            if let Ok(parsed) = value.parse::<usize>() {
                width = Some(parsed);
            }
            continue;
        }

        if let Some(value) = arg.strip_prefix("--reserved=") {
            if let Ok(parsed) = value.parse::<usize>() {
                reserved = Some(parsed);
            }
            continue;
        }

        if arg == "--fill" {
            fill = Some(true);
            continue;
        }

        if arg == "--no-fill" {
            fill = Some(false);
            continue;
        }
    }

    CliConfig {
        preview,
        width,
        reserved,
        fill,
    }
}

fn run_preview(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = app::App::new_preview();
    let tick_rate = Duration::from_millis(1000);

    loop {
        terminal.draw(|frame| ui::render(frame, &app.snapshot))?;

        let timeout = tick_rate.saturating_sub(app.last_tick_elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                    return Ok(());
                }
            }
        }

        if app.last_tick_elapsed() >= tick_rate {
            app.tick();
        }
    }
}

fn run_statusline() -> Result<()> {
    let input = data::read_stdin_json();
    let snapshot = data::collect_from_input(input.as_ref());
    let output = ui::format_output(&snapshot);
    print!("{}", output);
    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
