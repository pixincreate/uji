use std::rc::Rc;

use mlua::{Function, Lua as LuaState, Table};

use crate::runtime::Inner;

pub(crate) fn on(lua: &LuaState, inner: &Rc<Inner>) -> mlua::Result<Function> {
    let inner = inner.clone();
    lua.create_function(move |_, (event, handler): (String, Function)| {
        inner.handlers.borrow_mut().add(event, handler);
        Ok(())
    })
}

pub(crate) fn emit(lua: &LuaState, inner: &Rc<Inner>) -> mlua::Result<Function> {
    let inner = inner.clone();
    lua.create_function(move |_, (event, ctx): (String, Table)| {
        inner.dispatch(&event, &ctx);
        Ok(())
    })
}

pub(crate) fn notify(lua: &LuaState, _inner: &Rc<Inner>) -> mlua::Result<Function> {
    lua.create_function(move |_, message: String| {
        eprintln!("uji: {message}");
        Ok(())
    })
}
