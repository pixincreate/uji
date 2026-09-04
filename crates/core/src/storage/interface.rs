use std::path::Path;

use super::error::Result;

/// Source of database connections — the seam embedders implement to swap
/// storage backends (sqlite today; postgres, in-memory for tests, … later).
///
/// Mirrors the split opencode uses: a connection source (their `Database`
/// service) that domain stores (`SessionStorage`) are layered on top of.
pub trait StorageInterface: Send {
    /// Backend-native connection handle (`rusqlite::Connection` for sqlite).
    type Connection;

    /// Open the backend at `path` (file path, URL, or `:memory:`).
    fn open(path: impl AsRef<Path>) -> Result<Self>
    where
        Self: Sized;

    /// Borrow the live connection used to run queries.
    fn get_connection(&self) -> &Self::Connection;

    /// Backend name, for status/debug output.
    fn backend(&self) -> &'static str {
        "sqlite"
    }
}
