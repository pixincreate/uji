#[derive(thiserror::Error, Debug)]
pub enum RuntimeError {
    #[error("lua: {0}")]
    Lua(#[from] mlua::Error),
    #[error("loop: {0}")]
    Loop(#[from] calloop::Error),
}
