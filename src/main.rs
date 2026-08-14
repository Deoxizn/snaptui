mod app;
mod fmt;
mod snapper;
mod theme;
mod ui;

use std::io::{self, Stdout, Write};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::cursor;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::app::{App, Command as AppCommand};
use crate::fmt::{format_date, human_size, snapshot_type};
use crate::snapper::{friendly_error, SnapperDb};
use crate::theme::Theme;

type Term = Terminal<CrosstermBackend<Stdout>>;

fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
        original(info);
    }));
}

fn run_suspended(term: &mut Term, cmd: &str, args: &[String]) -> bool {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
    let _ = io::stdout().flush();
    let status = Command::new(cmd)
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let _ = execute!(io::stdout(), EnterAlternateScreen, cursor::Hide);
    let _ = enable_raw_mode();
    let _ = term.clear();
    status
}

fn run() -> Result<()> {
    install_panic_hook();

    let (theme, source) = match theme::load_theme_from_any() {
        Ok(Some((t, s))) => (t, Some(s)),
        _ => (Theme::default(), None),
    };

    let db = SnapperDb::connect()
        .context("cannot connect to snapperd over D-Bus — is the system bus available?")?;

    enable_raw_mode().context("raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide).context("alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend).context("terminal")?;

    let mut app = App::new(db, theme, source);
    let mut last_refresh = Instant::now();
    let result = loop {
        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.handle_key(key) {
                        AppCommand::RunSudo(args) => {
                            let ok = run_suspended(&mut term, "sudo", &args);
                            app.after_sudo(ok);
                        }
                        AppCommand::None => {}
                    }
                    if app.quit {
                        break Ok(());
                    }
                }
            }
        }

        if let Some((t, s)) = theme::reload_if_changed(&app.theme, &app.theme_source)? {
            app.theme = t;
            app.theme_source = Some(s);
        }

        if last_refresh.elapsed() >= Duration::from_secs(15) {
            app.refresh_all();
            last_refresh = Instant::now();
        }

        ui::draw(&mut term, &mut app);
    };

    disable_raw_mode().context("disable raw mode")?;
    execute!(io::stdout(), LeaveAlternateScreen, cursor::Show).context("leave alternate screen")?;
    result
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--check") {
        match run_check() {
            Ok(()) => {}
            Err(e) => {
                eprintln!("snaptui: {e:#}");
                std::process::exit(1);
            }
        }
        return;
    }
    if let Err(e) = run() {
        eprintln!("snaptui: {e:#}");
        std::process::exit(1);
    }
}

fn run_check() -> Result<()> {
    let db = SnapperDb::connect().context("cannot connect to snapperd")?;
    let configs = db.list_configs().context("ListConfigs failed")?;
    if configs.is_empty() {
        println!("no snapper configs found");
        return Ok(());
    }
    for config in &configs {
        println!("config: {}  subvol: {}", config.name, config.subvolume);
        for key in ["ALLOW_USERS", "ALLOW_GROUPS", "NUMBER_LIMIT", "TIMELINE_CREATE"] {
            if let Some(v) = config.values.get(key) {
                println!("  {key}: {v}");
            }
        }
        match db.list_snapshots(&config.name) {
            Ok(snaps) => {
                let total: u64 = snaps
                    .iter()
                    .map(|s| db.get_used_space(&config.name, s.id).unwrap_or(0))
                    .sum();
                println!("  snapshots: {} (used {})", snaps.len(), human_size(total));
                for s in &snaps {
                    println!(
                        "    #{:<5} {:7} {} {:<30} cleanup={}",
                        s.id,
                        snapshot_type(s.kind),
                        format_date(s.date),
                        s.description,
                        if s.cleanup.is_empty() { "-" } else { &s.cleanup }
                    );
                }
            }
            Err(e) => println!("  snapshots: error — {}", friendly_error(&e)),
        }
    }
    Ok(())
}
