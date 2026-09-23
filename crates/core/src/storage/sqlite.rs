use std::path::{Path, PathBuf};

use diesel::Connection;
use diesel::connection::SimpleConnection;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use crate::config;

use super::error::{Result, StorageError};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub struct SqliteStorage {
    conn: SqliteConnection,
}

impl SqliteStorage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let url = path
            .to_str()
            .ok_or_else(|| StorageError::Path("not valid utf-8".into()))?
            .to_owned();
        let mut conn = SqliteConnection::establish(&url)?;
        conn.batch_execute(
            "PRAGMA busy_timeout = 5000;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;
        conn.run_pending_migrations(MIGRATIONS)?;
        Ok(Self { conn })
    }

    pub(crate) fn connection(&mut self) -> &mut SqliteConnection {
        &mut self.conn
    }
}

pub fn default_db_path() -> Result<PathBuf> {
    let Some(path) = std::env::var("UJI_DB").ok().filter(|p| !p.is_empty()) else {
        let dir =
            config::data_dir().ok_or_else(|| StorageError::Path("$HOME is not set".into()))?;
        return Ok(dir.join("uji.db"));
    };
    Ok(PathBuf::from(path))
}
