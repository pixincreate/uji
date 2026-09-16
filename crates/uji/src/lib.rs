//! The application: wires the agent and the terminal UI together and
//! exposes them to plugins.
//!
//! [`runtime`] owns the event loop, [`api`] is the `uji.*` Lua surface,
//! [`cmd`] holds the built-in slash commands, and [`pack`] loads plugins.
//! The `uji` binary lives here too.

pub mod api;
pub mod cmd;
pub mod list;
pub mod pack;
pub mod runtime;

pub use uji_agent as engine;
pub use uji_ui as ui;

pub use runtime::{Runtime, events};
pub use uji_agent::config;
pub use uji_agent::session;
pub use uji_agent::storage;
pub use uji_agent::{
    Message, Session, SessionStorage, SqliteStorage, StoredMessage, default_db_path,
};
pub use uji_ui::state::UiState;
