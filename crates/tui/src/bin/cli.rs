use clap::Parser;
use tui::{app, storage::session::SessionManager};

/// Embeddable harness — barebones TUI.
#[derive(Parser, Debug)]
#[command(name = "uji", version, about = "Embeddable harness — barebones TUI")]
struct Cli {
    /// Resume the most recent session instead of creating a new one.
    #[arg(long)]
    resume: bool,
}

fn main() {
    let cli = Cli::parse();
    let mut store = SessionManager::new();

    let session = if cli.resume {
        store
            .resume_latest()
            .unwrap_or_else(|| store.new_session("resumed"))
    } else {
        store.new_session("new")
    };

    if let Err(err) = app::run(session) {
        eprintln!("uji: error: {err}");
        std::process::exit(1);
    }
}
