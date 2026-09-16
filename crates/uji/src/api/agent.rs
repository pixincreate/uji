use std::rc::Rc;

use mlua::{Function, Lua, Table, Value};

use crate::Api;
use crate::bind::bind;
use crate::registry::Entry;

pub(crate) fn context(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(
        lua,
        api,
        move |api, _, (name, call, opts): (String, Function, Option<Table>)| {
            let priority = opts
                .map(|opts| opts.get::<Option<i64>>("priority"))
                .transpose()?
                .flatten()
                .unwrap_or(50);
            api.agent_context().add(Entry {
                name,
                priority,
                call,
            });
            Ok(())
        },
    )
}

pub(crate) fn clear_context(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, name: String| {
        api.agent_context().remove(&name);
        Ok(())
    })
}

pub(crate) fn contexts(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, lua, ()| {
        let out = lua.create_table()?;
        for (name, call) in api.agent_context().calls() {
            match call.call::<Value>(()) {
                Ok(Value::Nil) => {}
                Ok(value) => out.push(value)?,
                Err(err) => api.notify(format!("agent context {name}: {err}")),
            }
        }
        Ok(out)
    })
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let agent = lua.create_table()?;
    agent.set("context", context(lua, api)?)?;
    agent.set("clear_context", clear_context(lua, api)?)?;
    agent.set("contexts", contexts(lua, api)?)?;
    Ok(agent)
}
