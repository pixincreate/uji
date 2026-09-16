//! The agent engine: talking to models, running tools, and persisting sessions.
//!
//! Knows nothing about terminals or Lua, so it can be embedded on its own.
//! Holds the provider protocols ([`llm`]), the tool registry ([`tools`]),
//! session storage ([`session`], [`storage`]), and credentials ([`auth`]).

pub mod auth;
pub mod config;
pub mod credential;
pub mod llm;
pub mod session;
pub mod storage;
pub mod tools;

pub use session::model::{Message, Session, StoredMessage};
pub use session::store::SessionStorage;
pub use storage::sqlite::{SqliteStorage, default_db_path};
