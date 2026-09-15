use std::rc::Rc;

use mlua::{Function, Lua, Table};

use crate::handlers::DEFAULT_PRIORITY;

use super::Api;

pub fn on(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(
        move |_, (event, handler, opts): (String, Function, Option<Table>)| {
            let priority = opts
                .map(|opts| opts.get::<Option<i64>>("priority"))
                .transpose()?
                .flatten()
                .unwrap_or(DEFAULT_PRIORITY);
            api.handlers().borrow_mut().add(event, handler, priority);
            Ok(())
        },
    )
}

pub fn emit(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |_, (event, ctx): (String, Table)| {
        api.dispatch(&event, &ctx);
        Ok(())
    })
}

pub fn notify(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |_, message: String| {
        api.notify(message);
        Ok(())
    })
}
