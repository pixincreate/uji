//! `uji-api` — the canonical, Lua-free API (nvim's `src/nvim/api`).
//!
//! One set of operations, many surfaces: Rust embedders call these directly,
//! libuji's Lua bindings wrap them, and a future RPC layer can expose the
//! same functions. Only buffer/window/option operations live here; the
//! Lua-specific surfaces (`schedule`, `on`, `emit`, `notify`) stay in libuji
//! because their payloads are Lua values.

pub mod buffer;
pub mod opts;
pub mod window;

/// Errors from the canonical API.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// A buffer name had no built-in kind to infer.
    #[error("unknown buffer kind: {0}")]
    UnknownBufferKind(String),
}
