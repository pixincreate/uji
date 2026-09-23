use std::rc::Rc;

use mlua::{Function, Lua};

use super::Api;
use crate::api::bind::bind;
use crate::api::callbacks::canceller;
use crate::api::request::Request;

pub fn route(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, |api, lua, on_done: Function| {
        let id = api.callbacks().borrow_mut().open(on_done);
        api.request(Request::Route(id));
        canceller(lua, api, id)
    })
}

pub fn current_provider(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let state = api.state();
    lua.create_function(move |_, ()| Ok(state.borrow().current_provider().map(str::to_string)))
}

pub fn current_model(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let state = api.state();
    lua.create_function(move |_, ()| Ok(state.borrow().current_model().map(str::to_string)))
}
