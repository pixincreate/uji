//! libuji — the embeddable uji harness API.
//!
//! Everything the `uji` binary can do is available here: session storage,
//! message history, action dispatch, the Lua UI config, and the default TUI.
//! Embedders depend on this single crate (or on `uji-core` directly if they
//! want no UI).
//!
//! ```no_run
//! use libuji::core::session::store::SessionStorage;
//! use libuji::core::storage::interface::StorageInterface;
//! use libuji::core::storage::sqlite::{SqliteStorage, default_db_path};
//!
//! let mut storage = SqliteStorage::open(default_db_path().unwrap()).unwrap();
//! let session = storage.create_session("my session").unwrap();
//! // Load the UI from config, or build a UiModel programmatically instead.
//! let model = libuji::config::load();
//! libuji::tui::app::run(session, &mut storage, model).unwrap();
//! ```

pub mod config;

pub use tui;
pub use uji_core as core;

pub use tui::model::UiModel;
pub use uji_core::session::model::{Message, Session, StoredMessage};
pub use uji_core::session::store::SessionStorage;
pub use uji_core::storage::sqlite::{SqliteStorage, default_db_path};
