//! `uji.open_win` / `uji.close_win` — Lua bindings over [`crate::api::window`].

use std::rc::Rc;

use mlua::{Function, Lua as LuaState, Table, Value as LuaValue};
use tui::model::WinOpts;

use crate::api;
use crate::lua::convert::FromLuaValue;
use crate::runtime::Inner;

/// `uji.open_win(buffer [, opts])`
pub(crate) fn open_win(lua: &LuaState, inner: &Rc<Inner>) -> mlua::Result<Function> {
    let state = inner.state.clone();
    lua.create_function(move |_, (buffer, opts): (String, Option<Table>)| {
        let opts = match opts {
            Some(table) => WinOpts::from_lua_value(&LuaValue::Table(table))?,
            None => WinOpts::default(),
        };
        let mut state = state.borrow_mut();
        api::window::open(&mut state, &buffer, opts)
            .map_err(|err| mlua::Error::runtime(err.to_string()))?;
        Ok(())
    })
}

/// `uji.close_win(buffer) -> removed`
pub(crate) fn close_win(lua: &LuaState, inner: &Rc<Inner>) -> mlua::Result<Function> {
    let state = inner.state.clone();
    lua.create_function(move |_, buffer: String| {
        let mut state = state.borrow_mut();
        Ok(api::window::close(&mut state, &buffer))
    })
}
