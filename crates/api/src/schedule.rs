use std::rc::Rc;

use mlua::{Function, Lua};

use super::Api;

pub fn schedule(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |_, callback: Function| {
        api.scheduled().push(callback);
        Ok(())
    })
}
