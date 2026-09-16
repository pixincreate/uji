use std::rc::Rc;

use mlua::{FromLuaMulti, Function, IntoLuaMulti, Lua};

use crate::api::Api;

/// Wrap a handler as a Lua function with the [`Api`] captured for it.
///
/// Every `uji.*` binding has the same shape — clone the handle, create a
/// function, use the handle inside — so it lives here once instead of at each
/// call site.
pub(crate) fn bind<A, R, F>(lua: &Lua, api: &Rc<Api>, call: F) -> mlua::Result<Function>
where
    A: FromLuaMulti + 'static,
    R: IntoLuaMulti + 'static,
    F: Fn(&Rc<Api>, &Lua, A) -> mlua::Result<R> + 'static,
{
    let api = Rc::clone(api);
    lua.create_function(move |lua, args: A| call(&api, lua, args))
}
