use std::error::Error;

use clap::{Parser, Subcommand};
use tui::{app, storage::session::SessionManager};

/// Embeddable harness — barebones TUI.
#[derive(Parser, Debug)]
#[command(name = "uji", version, about = "Embeddable harness — barebones TUI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Create a new session and open the TUI.
    New,
    /// Resume a session (by id, or the most recent) and open the TUI.
    Resume {
        /// Session id to resume. Defaults to the most recent session.
        #[arg(long)]
        id: Option<String>,
    },
    /// List all sessions.
    List,
    /// Delete a session by id.
    Delete {
        /// Session id to delete.
        id: String,
    },
}

fn main() {
    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Command::New);
    let mut store = SessionManager::new();

    let result = match command {
        Command::New => handle_new(&mut store),
        Command::Resume { id } => handle_resume(&mut store, id),
        Command::List => handle_list(&store),
        Command::Delete { id } => handle_delete(&mut store, &id),
    };

    if let Err(err) = result {
        eprintln!("uji: error: {err}");
        std::process::exit(1);
    }
}

fn handle_new(store: &mut SessionManager) -> Result<(), Box<dyn Error>> {
    let session = store.new_session("new");
    app::run(session)?;
    Ok(())
}

fn handle_resume(store: &mut SessionManager, id: Option<String>) -> Result<(), Box<dyn Error>> {
    let session = match id {
        Some(id) => store
            .resume_session(&id)
            .ok_or_else(|| format!("no session with id: {id}"))?,
        None => store
            .resume_latest()
            .unwrap_or_else(|| store.new_session("resumed")),
    };
    app::run(session)?;
    Ok(())
}

fn handle_list(store: &SessionManager) -> Result<(), Box<dyn Error>> {
    let sessions = store.list_sessions();
    if sessions.is_empty() {
        println!("no sessions");
    } else {
        for session in sessions {
            println!("{}  {}  {}", session.id, session.title, session.created_at);
        }
    }
    Ok(())
}

fn handle_delete(store: &mut SessionManager, id: &str) -> Result<(), Box<dyn Error>> {
    if store.delete_session(id) {
        println!("deleted session: {id}");
    } else {
        println!("no session with id: {id}");
    }
    Ok(())
}
