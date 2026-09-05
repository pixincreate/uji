pub mod error;
pub mod interface;
#[allow(missing_docs)]
pub mod schema;
pub mod sqlite;

pub use error::{Result, StorageError};
pub use interface::StorageInterface;
pub use sqlite::SqliteStorage;
