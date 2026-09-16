//! Everything terminal-facing: what the UI is, and how it is drawn.
//!
//! [`model`], [`state`], [`keymap`] and [`config`] declare the UI — windows,
//! theme, bindings, and the schema plugins deserialise into. [`app`] holds the
//! live state the runtime drives: the composer, key handling, selection and
//! modes. [`render`] turns all of that into frames with ratatui.

pub mod app;
pub mod clipboard;
pub mod config;
pub mod keymap;
pub mod model;
pub mod render;
pub mod state;
pub mod terminal;
