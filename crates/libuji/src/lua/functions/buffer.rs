use std::rc::Rc;

use mlua::{Function, Lua as LuaState, Table};
use tui::model::BufferKind;

use crate::api;
use crate::runtime::Inner;

pub(crate) fn create_buf(lua: &LuaState, inner: &Rc<Inner>) -> mlua::Result<Function> {
    let state = inner.state.clone();
    lua.create_function(move |_, (name, opts): (String, Option<Table>)| {
        let kind = match opts.and_then(|t| t.get::<Option<String>>("kind").ok().flatten()) {
            Some(kind) => kind
                .parse::<BufferKind>()
                .map_err(|err| mlua::Error::runtime(err.to_string()))?,
            None => name
                .parse::<BufferKind>()
                .map_err(|err| mlua::Error::runtime(err.to_string()))?,
        };
        let mut state = state.borrow_mut();
        api::buffer::create(&mut state, &name, kind);
        Ok(name)
    })
}
