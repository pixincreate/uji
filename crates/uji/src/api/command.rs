use std::rc::Rc;

use mlua::{Function, Lua, Table, Value};

use super::Api;
use crate::api::bind::bind;

#[derive(Clone)]
pub struct LuaCommand {
    pub handler: Function,
    pub desc: String,
    pub force: bool,
}

pub fn add(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, (name, spec): (String, Value)| {
        let command = parse(spec)?;
        api.commands().borrow_mut().insert(name, command);
        Ok(())
    })
}

pub fn remove(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, name: String| {
        Ok(api.commands().borrow_mut().remove(&name).is_some())
    })
}

pub fn list(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, ()| {
        let mut names: Vec<String> = api.commands().borrow().keys().cloned().collect();
        names.sort();
        Ok(names)
    })
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let command = lua.create_table()?;
    command.set("add", add(lua, api)?)?;
    command.set("remove", remove(lua, api)?)?;
    command.set("list", list(lua, api)?)?;
    Ok(command)
}

fn parse(spec: Value) -> mlua::Result<LuaCommand> {
    match spec {
        Value::Function(handler) => Ok(LuaCommand {
            handler,
            desc: String::new(),
            force: false,
        }),
        Value::Table(opts) => Ok(LuaCommand {
            handler: opts.get("handler")?,
            desc: opts.get::<Option<String>>("desc")?.unwrap_or_default(),
            force: opts.get::<Option<bool>>("force")?.unwrap_or(false),
        }),
        _ => Err(mlua::Error::runtime(
            "uji.command.add needs a function or a table with a handler",
        )),
    }
}
