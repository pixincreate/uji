use std::rc::Rc;
use std::time::Duration;

use mlua::{Function, Lua};

use super::Api;
use crate::api::bind::bind;
use crate::api::callbacks::canceller;
use crate::api::request::Request;

pub fn schedule(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, callback: Function| {
        api.scheduled().borrow_mut().push(callback);
        Ok(())
    })
}

pub fn defer(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(
        lua,
        api,
        |api, lua, (seconds, callback): (f64, Function)| {
            let after = Duration::try_from_secs_f64(seconds)
                .map_err(|_| mlua::Error::runtime("defer needs a number of seconds"))?;
            let id = api.callbacks().borrow_mut().open(callback);
            api.request(Request::Defer { id, after });
            canceller(lua, api, id)
        },
    )
}
