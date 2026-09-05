//! `uji-core` — the UI-free engine of the uji harness.
//!
//! Owns storage (diesel/sqlite), the session/message domain model, and the
//! action, ai and memory seams. Embedders wanting the default terminal UI
//! should depend on `libuji` instead.

pub mod action;
pub mod ai;
pub mod memory;
pub mod session;
pub mod storage;
