//! `libuji` — the embeddable uji harness API.
//!
//! Everything the `uji` binary can do is available here: session storage,
//! message history, action dispatch, the Lua config + event loop runtime, and
//! the default TUI. Embedders depend on this single crate (or on `uji-core`
//! directly if they want no UI).
//!
//! ```no_run
//! use libuji::core::session::store::SessionStorage;
//! use libuji::core::storage::interface::StorageInterface;
//! use libuji::core::storage::sqlite::{SqliteStorage, default_db_path};
//! use libuji::runtime::Runtime;
//!
//! let mut storage: Box<dyn SessionStorage> =
//!     Box::new(SqliteStorage::open(default_db_path().unwrap()).unwrap());
//! let session = storage.create_session("my session").unwrap();
//! let runtime = Runtime::boot().unwrap();
//! runtime.run(session, storage).unwrap();
//! ```

pub use uji_api as api;
pub mod config;
pub mod lua;
pub mod runtime;

pub use tui;
pub use uji_core as core;

pub use runtime::Runtime;
pub use tui::model::UiModel;
pub use tui::state::UiState;
pub use uji_core::session::model::{Message, Session, StoredMessage};
pub use uji_core::session::store::SessionStorage;
pub use uji_core::storage::sqlite::{SqliteStorage, default_db_path};
