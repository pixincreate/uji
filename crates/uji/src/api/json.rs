use std::rc::Rc;

use mlua::{Function, Lua, LuaSerdeExt, Table, Value};

use crate::api::Api;
use crate::api::bind::bind;

pub(crate) fn encode(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |_, lua, value: Value| {
        let value: serde_json::Value = lua.from_value(value)?;
        serde_json::to_string(&value).map_err(mlua::Error::external)
    })
}

pub(crate) fn decode(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |_, lua, text: String| {
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(mlua::Error::external)?;
        lua.to_value(&value)
    })
}

pub(crate) fn array(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |_, lua, table: Table| {
        table.set_metatable(Some(lua.array_metatable()))?;
        Ok(table)
    })
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let json = lua.create_table()?;
    json.set("encode", encode(lua, api)?)?;
    json.set("decode", decode(lua, api)?)?;
    json.set("array", array(lua, api)?)?;
    json.set("null", lua.null())?;
    Ok(json)
}
