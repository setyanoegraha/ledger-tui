//! `lgr` — local double-entry bookkeeping TUI.

mod app;
mod db;
mod money;
mod report;
mod ui;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};

use crate::app::App;
use crate::db::Db;

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--version" | "-V") => {
            println!("lgr {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Some("--help" | "-h") => {
            print_help();
            return;
        }
        Some(other) => {
            eprintln!(
                "[!] unknown argument '{other}' — run 'lgr' with no arguments to open the dashboard"
            );
            std::process::exit(1);
        }
        None => {}
    }

    if let Err(error) = run() {
        eprintln!("[!] {error:#}");
        std::process::exit(1);
    }
}

/// `LGR_DB` overrides the default `~/.lgr/ledger.db`.
fn db_path() -> PathBuf {
    if let Some(path) = std::env::var_os("LGR_DB") {
        return PathBuf::from(path);
    }
    home::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".lgr")
        .join("ledger.db")
}

fn run() -> Result<()> {
    let mut db = Db::open(&db_path())?;
    let mut app = App::new(&db)?;
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut app, &mut db);
    ratatui::restore();
    result
}

fn event_loop(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    db: &mut Db,
) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key, db);
                }
            }
        }

        if app.quit {
            return Ok(());
        }
    }
}

fn print_help() {
    println!(
        "lgr v{} — local double-entry bookkeeping TUI",
        env!("CARGO_PKG_VERSION")
    );
    println!();
    println!("  lgr            open the dashboard");
    println!("  lgr --version  print the version");
    println!("  lgr --help     show this help");
    println!();
    println!("Ledger: {}", db_path().display());
    println!("Override with the LGR_DB environment variable.");
    println!();
    println!("Keys:");
    println!("  1-4            switch tab (Dashboard, Accounts, Register, Reports)");
    println!("  j / k          move selection");
    println!("  Enter          open the register for an account");
    println!("  [ / ]          previous / next month");
    println!("  n              new transaction");
    println!("  e / d          edit / delete the selected transaction");
    println!("  ?              show all keys");
    println!("  q              quit");
}
