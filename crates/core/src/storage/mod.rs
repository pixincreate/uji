pub mod error;
pub mod interface;
pub mod sqlite;

pub use error::{Result, StorageError};
pub use interface::StorageInterface;
pub use sqlite::SqliteStorage;
