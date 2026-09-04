use std::error::Error;

use clap::{Parser, Subcommand};
use libuji::core::session::id::SessionId;
use libuji::core::session::store::SessionStorage;
use libuji::core::storage::interface::StorageInterface;
use libuji::core::storage::sqlite::{SqliteStorage, default_db_path};
use libuji::tui::app;

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

    let mut storage = match default_db_path().and_then(SqliteStorage::open) {
        Ok(storage) => storage,
        Err(err) => {
            eprintln!("uji: storage error: {err}");
            std::process::exit(1);
        }
    };

    let result = match command {
        Command::New => handle_new(&mut storage),
        Command::Resume { id } => handle_resume(&mut storage, id),
        Command::List => handle_list(&mut storage),
        Command::Delete { id } => handle_delete(&mut storage, &id),
    };

    if let Err(err) = result {
        eprintln!("uji: error: {err}");
        std::process::exit(1);
    }
}

fn handle_new(storage: &mut SqliteStorage) -> Result<(), Box<dyn Error>> {
    let session = storage.create_session("new")?;
    let model = libuji::config::load();
    app::run(session, storage, model)?;
    Ok(())
}

fn handle_resume(storage: &mut SqliteStorage, id: Option<String>) -> Result<(), Box<dyn Error>> {
    let session = match id {
        Some(id) => {
            let session_id: SessionId = id
                .parse()
                .map_err(|_| format!("invalid session id: {id}"))?;
            storage
                .get_session(&session_id)?
                .ok_or_else(|| format!("no session with id: {id}"))?
        }
        None => match storage.latest_session()? {
            Some(session) => session,
            None => storage.create_session("resumed")?,
        },
    };
    let model = libuji::config::load();
    app::run(session, storage, model)?;
    Ok(())
}

fn handle_list(storage: &mut SqliteStorage) -> Result<(), Box<dyn Error>> {
    let sessions = storage.list_sessions()?;
    if sessions.is_empty() {
        println!("no sessions");
    } else {
        for session in sessions {
            println!("{}  {}  {}", session.id, session.title, session.time.updated);
        }
    }
    Ok(())
}

fn handle_delete(storage: &mut SqliteStorage, id: &str) -> Result<(), Box<dyn Error>> {
    let session_id: SessionId = id
        .parse()
        .map_err(|_| format!("invalid session id: {id}"))?;
    if storage.delete_session(&session_id)? {
        println!("deleted session: {id}");
    } else {
        println!("no session with id: {id}");
    }
    Ok(())
}
