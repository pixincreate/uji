//! `uji.on` / `uji.emit` / `uji.notify` — the event surface. Lua-only: the
//! callbacks and ctx tables are Lua values (nvim keeps autocmds Lua-side too).

use std::rc::Rc;

use mlua::{Function, Lua as LuaState, Table};

use crate::runtime::Inner;

/// `uji.on(event, handler)`
pub(crate) fn on(lua: &LuaState, inner: &Rc<Inner>) -> mlua::Result<Function> {
    let inner = inner.clone();
    lua.create_function(move |_, (event, handler): (String, Function)| {
        inner.handlers.borrow_mut().add(event, handler);
        Ok(())
    })
}

/// `uji.emit(event, ctx)`
pub(crate) fn emit(lua: &LuaState, inner: &Rc<Inner>) -> mlua::Result<Function> {
    let inner = inner.clone();
    lua.create_function(move |_, (event, ctx): (String, Table)| {
        inner.dispatch(&event, &ctx);
        Ok(())
    })
}

/// `uji.notify(message)`
pub(crate) fn notify(lua: &LuaState, _inner: &Rc<Inner>) -> mlua::Result<Function> {
    lua.create_function(move |_, message: String| {
        // ast-grep-ignore: no-print-in-lib
        eprintln!("uji: {message}");
        Ok(())
    })
}
