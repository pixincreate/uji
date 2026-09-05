//! `uji.schedule(fn)` — defer a Lua callback to the next safe point on the
//! loop (nvim's `vim.schedule`). Lua-only: the payload is a Lua function.

use std::rc::Rc;

use mlua::{Function, Lua as LuaState};

use crate::runtime::Inner;

/// `uji.schedule(fn)`
pub(crate) fn schedule(lua: &LuaState, inner: &Rc<Inner>) -> mlua::Result<Function> {
    let inner = inner.clone();
    lua.create_function(move |_, callback: Function| {
        inner.scheduled.push(callback);
        Ok(())
    })
}
