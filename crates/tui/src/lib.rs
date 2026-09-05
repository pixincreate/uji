//! `tui` — the default terminal UI components.
//!
//! Pure UI: it knows how to *draw* buffer kinds and run an input loop, and
//! nothing about storage beyond the [`uji_core::session::SessionStorage`]
//! trait it renders from.

pub mod app;
pub mod model;
pub mod state;
pub mod ui;
