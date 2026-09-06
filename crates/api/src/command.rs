use std::rc::Rc;

use mlua::{Function, Lua};

use super::Api;

pub fn command(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = api.clone();
    lua.create_function(move |_, (name, handler): (String, Function)| {
        api.commands().borrow_mut().insert(name, handler);
        Ok(())
    })
}
