//! Errors surfaced by the storage layer.

/// Errors surfaced by the storage layer.
#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    /// Filesystem failure (creating data directories, reading config).
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to establish a database connection.
    #[error("connection: {0}")]
    Connection(#[from] diesel::ConnectionError),

    /// A query failed.
    #[error("query: {0}")]
    Query(#[from] diesel::result::Error),

    /// A stored JSON payload failed to decode.
    #[error("decode: {0}")]
    Serde(#[from] serde_json::Error),

    /// A stored id failed to parse as a UUID.
    #[error("invalid uuid: {0}")]
    Uuid(#[from] uuid::Error),

    /// A migration failed to apply.
    #[error("migration: {0}")]
    Migration(#[from] Box<dyn std::error::Error + Send + Sync>),

    /// A required value was absent.
    #[error("not found: {0}")]
    NotFound(String),
}

/// Convenience alias for storage results.
pub type Result<T> = std::result::Result<T, StorageError>;
