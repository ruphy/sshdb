mod app;
mod config;
mod model;
mod ssh;
mod ui;

use std::io;
use std::time::Duration;

use anyhow::Result;
use app::{App, AppAction, StatusKind, StatusLine};
use config::ConfigStore;
use crossterm::event;
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

fn main() {
    if let Err(e) = start() {
        eprintln!("sshdb error: {e:?}");
        std::process::exit(1);
    }
}

fn start() -> Result<()> {
    let mut terminal = setup_terminal()?;
    let res = run_loop(&mut terminal);
    restore_terminal(&mut terminal)?;
    res
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new(ConfigStore::new()?)?;
    loop {
        terminal.draw(|f| ui::render(f, &app))?;
        if event::poll(Duration::from_millis(80))? {
            let evt = event::read()?;
            if let Some(action) = app.on_event(evt)? {
                match action {
                    AppAction::Quit => break,
                    AppAction::RunSsh(cmd) => {
                        run_ssh(terminal, &mut app, cmd)?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn run_ssh(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    cmd: std::process::Command,
) -> Result<()> {
    restore_terminal(terminal)?;
    let result = ssh::run_command(cmd);
    *terminal = setup_terminal()?;

    match result {
        Ok(_) => {
            app.status = Some(StatusLine {
                text: "ssh session ended".into(),
                kind: StatusKind::Info,
            });
        }
        Err(err) => {
            app.status = Some(StatusLine {
                text: format!("ssh failed: {err}"),
                kind: StatusKind::Error,
            });
        }
    }
    Ok(())
}
