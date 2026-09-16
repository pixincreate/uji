use std::rc::Rc;

use mlua::{Function, Lua};

use super::Api;
use crate::api::bind::bind;

pub fn schedule(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, callback: Function| {
        api.scheduled().push(callback);
        Ok(())
    })
}
