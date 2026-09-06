use std::path::Path;

use super::error::Result;

pub trait StorageInterface: Send {
    type Connection;

    fn open(path: impl AsRef<Path>) -> Result<Self>
    where
        Self: Sized;

    fn get_connection(&mut self) -> &mut Self::Connection;

    fn backend(&self) -> &'static str {
        "sqlite"
    }
}
