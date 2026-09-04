use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::error::{Result, StorageError};
use super::interface::StorageInterface;

/// SQLite-backed storage. Owns a single connection (wrap it in a Mutex when
/// the harness grows threads).
#[derive(Debug)]
pub struct SqliteStorage {
    conn: Connection,
}

impl StorageInterface for SqliteStorage {
    type Connection = Connection;

    fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Self::init_schema(&conn)?;
        Ok(Self { conn })
    }

    fn get_connection(&self) -> &Connection {
        &self.conn
    }
}

impl SqliteStorage {
    /// Create the opencode-shaped schema if it does not exist yet.
    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS session (
                id           TEXT PRIMARY KEY,
                parent_id    TEXT REFERENCES session(id) ON DELETE SET NULL,
                title        TEXT NOT NULL,
                directory    TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                time_updated INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS message (
                id           TEXT PRIMARY KEY,
                session_id   TEXT NOT NULL REFERENCES session(id) ON DELETE CASCADE,
                seq          INTEGER NOT NULL,
                type         TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                data         TEXT NOT NULL
            );

            CREATE UNIQUE INDEX IF NOT EXISTS message_session_seq_idx
                ON message(session_id, seq);
            CREATE INDEX IF NOT EXISTS message_session_time_idx
                ON message(session_id, time_created, id);
            ",
        )?;
        Ok(())
    }
}

/// Default database location: `$UJI_DB` override, else `~/.local/share/uji/uji.db`.
pub fn default_db_path() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("UJI_DB") {
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }
    let home = std::env::var("HOME").map_err(|_| StorageError::NotFound("$HOME is not set".into()))?;
    Ok(PathBuf::from(home).join(".local/share/uji/uji.db"))
}
