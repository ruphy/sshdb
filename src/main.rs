// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2024 Riccardo Iaconelli <riccardo@kde.org>

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
use crossterm::event::{
    self, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

fn main() {
    if let Err(e) = start() {
        eprintln!("sshdb error: {e:?}");
        std::process::exit(1);
    }
}

fn start() -> Result<()> {
    let mut guard = TerminalGuard::new()?;
    let res = run_loop(guard.terminal());
    guard.restore()?;
    res
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        // Keep kitty keyboard protocol scoped to the TUI session.
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        // Pop before leaving the alternate screen to avoid leaking CSI u sequences.
        PopKeyboardEnhancementFlags,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    Ok(())
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    restored: bool,
}

impl TerminalGuard {
    fn new() -> Result<Self> {
        Ok(Self {
            terminal: setup_terminal()?,
            restored: false,
        })
    }

    fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }

    fn restore(&mut self) -> Result<()> {
        if !self.restored {
            restore_terminal(&mut self.terminal)?;
            self.restored = true;
        }
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
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
