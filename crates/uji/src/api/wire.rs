use std::rc::Rc;

use mlua::{Function, Lua, Table};

use super::Api;
use crate::api::bind::bind;

pub fn add(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, (name, spec): (String, Table)| {
        let stream = spec.get::<Function>("stream")?;
        api.wires().borrow_mut().insert(name, stream);
        Ok(())
    })
}

pub fn remove(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, name: String| {
        Ok(api.wires().borrow_mut().remove(&name).is_some())
    })
}

pub fn list(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, ()| {
        Ok(api.wires().borrow().keys().cloned().collect::<Vec<_>>())
    })
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let wire = lua.create_table()?;
    wire.set("add", add(lua, api)?)?;
    wire.set("remove", remove(lua, api)?)?;
    wire.set("list", list(lua, api)?)?;
    Ok(wire)
}
