//! Errors from booting the runtime.

/// Errors from booting the runtime.
#[derive(thiserror::Error, Debug)]
pub enum RuntimeError {
    /// Building or running Lua failed.
    #[error("lua: {0}")]
    Lua(#[from] mlua::Error),
    /// Creating the event loop failed.
    #[error("loop: {0}")]
    Loop(#[from] calloop::Error),
}
