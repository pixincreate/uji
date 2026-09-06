use std::rc::Rc;

use mlua::{Function, Lua as LuaState, Table, Value as LuaValue};
use tui::model::{WinOpts, WindowKind};

use crate::api;
use crate::lua::convert::FromLuaValue;
use crate::runtime::Inner;

pub(crate) fn open_win(lua: &LuaState, inner: &Rc<Inner>) -> mlua::Result<Function> {
    let state = inner.state.clone();
    lua.create_function(move |_, opts: Option<Table>| {
        let (kind, lines, win_opts) = match opts {
            Some(table) => {
                let kind = match table.get::<Option<String>>("view")? {
                    Some(view) => view
                        .parse::<WindowKind>()
                        .map_err(|err| mlua::Error::runtime(err.to_string()))?,
                    None => WindowKind::Text,
                };
                let lines = table
                    .get::<Option<Vec<String>>>("lines")?
                    .unwrap_or_default();
                let win_opts = WinOpts::from_lua_value(&LuaValue::Table(table))?;
                (kind, lines, win_opts)
            }
            None => (WindowKind::Text, Vec::new(), WinOpts::default()),
        };
        let mut state = state.borrow_mut();
        Ok(api::window::open(&mut state, kind, lines, win_opts))
    })
}

pub(crate) fn close_win(lua: &LuaState, inner: &Rc<Inner>) -> mlua::Result<Function> {
    let state = inner.state.clone();
    lua.create_function(move |_, id: u32| {
        let mut state = state.borrow_mut();
        Ok(api::window::close(&mut state, id))
    })
}
