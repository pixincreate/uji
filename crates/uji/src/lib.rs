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
