//! Storage backends and the connection-source trait.

pub mod error;
pub mod interface;
// Diesel `table!` macro output; not hand-written API.
#[allow(missing_docs)]
pub mod schema;
pub mod sqlite;

pub use error::{Result, StorageError};
pub use interface::StorageInterface;
pub use sqlite::SqliteStorage;
