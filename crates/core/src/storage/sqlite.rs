//! SQLite-backed storage via diesel.

use std::path::{Path, PathBuf};

use diesel::Connection;
use diesel::connection::SimpleConnection;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use super::error::{Result, StorageError};
use super::interface::StorageInterface;

/// Embedded diesel migrations (`crates/core/migrations`).
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// SQLite-backed storage via diesel.
///
/// Owns a single connection; wrap it in a `Mutex` when the harness grows
/// threads.
pub struct SqliteStorage {
    conn: SqliteConnection,
}

impl StorageInterface for SqliteStorage {
    type Connection = SqliteConnection;

    fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let url = path
            .to_str()
            .ok_or_else(|| StorageError::NotFound("non-utf8 database path".into()))?
            .to_owned();
        let mut conn = SqliteConnection::establish(&url)?;
        conn.batch_execute("PRAGMA foreign_keys = ON;")?;
        conn.run_pending_migrations(MIGRATIONS)?;
        Ok(Self { conn })
    }

    fn get_connection(&mut self) -> &mut SqliteConnection {
        &mut self.conn
    }
}

/// Default database location: `$UJI_DB` override, else
/// `~/.local/share/uji/uji.db`.
pub fn default_db_path() -> Result<PathBuf> {
    let Some(path) = std::env::var("UJI_DB").ok().filter(|p| !p.is_empty()) else {
        let home =
            std::env::var("HOME").map_err(|_| StorageError::NotFound("$HOME is not set".into()))?;
        return Ok(PathBuf::from(home).join(".local/share/uji/uji.db"));
    };
    Ok(PathBuf::from(path))
}
