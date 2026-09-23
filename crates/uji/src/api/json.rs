use std::rc::Rc;

use mlua::serde::SerializeOptions;
use mlua::{Function, Lua, LuaSerdeExt, Table, Value};

use crate::api::Api;
use crate::api::bind::bind;

pub(crate) fn encode(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |_, _, value: Value| {
        serde_json::to_string(&value).map_err(mlua::Error::external)
    })
}

pub(crate) fn decode(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(
        lua,
        api,
        move |_, lua, (text, opts): (String, Option<Table>)| {
            let value: serde_json::Value =
                serde_json::from_str(&text).map_err(mlua::Error::external)?;
            let nulls = opts
                .map(|opts| opts.get::<Option<bool>>("nulls"))
                .transpose()?
                .flatten()
                .unwrap_or(true);
            let options = SerializeOptions::new()
                .serialize_none_to_null(nulls)
                .serialize_unit_to_null(nulls);
            lua.to_value_with(&value, options)
        },
    )
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
