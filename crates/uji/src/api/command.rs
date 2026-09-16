use std::rc::Rc;

use mlua::{Function, Lua, Value};

use super::Api;
use crate::api::bind::bind;

#[derive(Clone)]
pub struct LuaCommand {
    pub handler: Function,
    pub desc: String,
    pub force: bool,
}

pub fn command(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, (name, spec): (String, Value)| {
        let command = parse(spec)?;
        api.commands().borrow_mut().insert(name, command);
        Ok(())
    })
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
            "uji.command needs a function or a table with a handler",
        )),
    }
}
