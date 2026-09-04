//! Barebone session storage.
//!
//! Everything in here is a stub — these functions deliberately "do nothing"
//! real yet. They only define the shape of the session layer so a real
//! database (e.g. SQLite via `rusqlite`) can be dropped in behind them later.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// A single session record.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub created_at: u64,
}

/// In-memory session manager. Currently a no-op placeholder.
#[derive(Debug, Default)]
pub struct SessionManager;

impl SessionManager {
    pub fn new() -> Self {
        Self
    }

    /// Create a new session.
    // TODO: real DB insert query.
    pub fn new_session(&mut self, title: &str) -> Session {
        Session {
            id: next_id(),
            title: title.to_string(),
            created_at: now_secs(),
        }
    }

    /// Resume a session by id.
    // TODO: real DB select query.
    pub fn resume_session(&self, _id: &str) -> Option<Session> {
        None
    }

    /// Resume the most recent session.
    // TODO: real DB select query (ORDER BY created_at DESC LIMIT 1).
    pub fn resume_latest(&self) -> Option<Session> {
        None
    }

    /// List all sessions.
    // TODO: real DB select query.
    pub fn list_sessions(&self) -> Vec<Session> {
        Vec::new()
    }

    /// Delete a session by id.
    // TODO: real DB delete query.
    pub fn delete_session(&mut self, _id: &str) -> bool {
        false
    }
}

fn next_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("sess-{n}")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
