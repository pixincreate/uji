use std::rc::Rc;

use mlua::{Function, Lua, Table};

use crate::api::handlers::DEFAULT_PRIORITY;

use super::Api;
use crate::api::bind::bind;

pub fn on(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(
        lua,
        api,
        move |api, _, (event, handler, opts): (String, Function, Option<Table>)| {
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
    bind(lua, api, move |api, _, (event, ctx): (String, Table)| {
        api.dispatch(&event, &ctx);
        Ok(())
    })
}

pub fn notify(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, message: String| {
        api.notify(message);
        Ok(())
    })
}
