use std::fmt::Display;

use mlua::serde::SerializeOptions;
use mlua::{IntoLua, IntoLuaMulti, Lua, LuaSerdeExt, MultiValue, Value};
use serde::Serialize;

pub(crate) struct Reply {
    pub(crate) id: u64,
    outcome: Box<dyn Outcome>,
}

trait Outcome: Send {
    fn into_args(self: Box<Self>, lua: &Lua) -> mlua::Result<MultiValue>;
}

impl<T: IntoLua + Send, E: Display + Send> Outcome for Result<T, E> {
    fn into_args(self: Box<Self>, lua: &Lua) -> mlua::Result<MultiValue> {
        match *self {
            Ok(value) => value.into_lua_multi(lua),
            Err(err) => (Value::Nil, err.to_string()).into_lua_multi(lua),
        }
    }
}

impl Reply {
    pub(crate) fn new<T, E>(id: u64, result: Result<T, E>) -> Self
    where
        T: IntoLua + Send + 'static,
        E: Display + Send + 'static,
    {
        Self {
            id,
            outcome: Box::new(result),
        }
    }

    pub(crate) fn into_args(self, lua: &Lua) -> mlua::Result<MultiValue> {
        self.outcome.into_args(lua)
    }
}

pub(crate) struct Json<T>(pub(crate) T);

impl<T: Serialize> IntoLua for Json<T> {
    fn into_lua(self, lua: &Lua) -> mlua::Result<Value> {
        let options = SerializeOptions::new()
            .serialize_none_to_null(false)
            .serialize_unit_to_null(false);
        lua.to_value_with(&self.0, options)
    }
}
