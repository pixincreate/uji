use std::rc::Rc;

use mlua::{Function, Lua, Table};

use super::Api;

pub fn on(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = api.clone();
    lua.create_function(move |_, (event, handler): (String, Function)| {
        api.handlers().borrow_mut().add(event, handler);
        Ok(())
    })
}

pub fn emit(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = api.clone();
    lua.create_function(move |_, (event, ctx): (String, Table)| {
        api.dispatch(&event, &ctx);
        Ok(())
    })
}

pub fn notify(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = api.clone();
    lua.create_function(move |_, message: String| {
        api.notify(message);
        Ok(())
    })
}
