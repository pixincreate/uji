/// Errors surfaced by the storage layer.
#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("connection: {0}")]
    Connection(#[from] diesel::ConnectionError),

    #[error("query: {0}")]
    Query(#[from] diesel::result::Error),

    #[error("decode: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("invalid uuid: {0}")]
    Uuid(#[from] uuid::Error),

    #[error("migration: {0}")]
    Migration(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[error("not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;
