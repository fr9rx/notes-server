//! notes-admin: terminal UI for managing a notes-server over HTTPS.
//!
//!     notes-admin [--url URL] [--ca-cert FILE] [--insecure]
//!
//! The admin token is read from NOTES_ADMIN_TOKEN or typed into the login
//! screen (never taken as an argument, so it stays out of shell history).

mod api;
mod app;
mod input;
mod ui;
#[cfg(test)]
mod tests;

use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind};
use ratatui::crossterm::execute;

use crate::app::App;

const USAGE: &str = "\
notes-admin - manage courses, chapters, notes and images on a notes-server

USAGE:
    notes-admin [--url URL] [--ca-cert FILE] [--insecure]

OPTIONS:
    --url URL        Server address, e.g. https://notes.example.com   [env: NOTES_URL]
    --ca-cert FILE   Trust this certificate (for a self-signed server) [env: NOTES_CA_CERT]
    --insecure       Skip TLS certificate verification (testing only)
    -h, --help       Show this help

The admin token comes from NOTES_ADMIN_TOKEN, or is asked for on start.
With NOTES_URL and NOTES_ADMIN_TOKEN set, it connects immediately.";

struct Args {
    url: String,
    ca_cert: Option<PathBuf>,
    insecure: bool,
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut args = Args {
        url: std::env::var("NOTES_URL").unwrap_or_default(),
        ca_cert: std::env::var_os("NOTES_CA_CERT").filter(|s| !s.is_empty()).map(PathBuf::from),
        insecure: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--url" => args.url = it.next().ok_or("--url needs a value")?,
            "--ca-cert" => args.ca_cert = Some(it.next().ok_or("--ca-cert needs a value")?.into()),
            "--insecure" => args.insecure = true,
            other => return Err(format!("unknown argument `{other}` (see --help)")),
        }
    }
    Ok(Some(args))
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(Some(args)) => args,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    notes_server::tls::install_crypto_provider();
    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("error: starting runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    let token = std::env::var("NOTES_ADMIN_TOKEN").unwrap_or_default();
    let mut app = App::new(rt, args.url, token, args.ca_cert, args.insecure);

    // ratatui::init enables raw mode + the alternate screen and installs a
    // panic hook that restores the terminal.
    let mut terminal = ratatui::init();
    let _ = execute!(io::stdout(), EnableBracketedPaste);
    let result = run(&mut terminal, &mut app);
    let _ = execute!(io::stdout(), DisableBracketedPaste);
    ratatui::restore();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;
        if app.quit {
            return Ok(());
        }
        // Draw the "busy" frame first, then do the network work.
        if app.run_pending() {
            continue;
        }
        match event::read()? {
            // Windows reports key releases too; act on presses only.
            Event::Key(key) if key.kind == KeyEventKind::Press => app.on_key(key),
            Event::Paste(text) => app.on_paste(&text),
            _ => {}
        }
    }
}
