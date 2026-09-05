use std::rc::Rc;

use mlua::{Function, Lua as LuaState};

use crate::runtime::Inner;

pub(crate) fn schedule(lua: &LuaState, inner: &Rc<Inner>) -> mlua::Result<Function> {
    let inner = inner.clone();
    lua.create_function(move |_, callback: Function| {
        inner.scheduled.push(callback);
        Ok(())
    })
}
