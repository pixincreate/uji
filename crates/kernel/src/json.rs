use mlua::serde::SerializeOptions;
use mlua::{ExternalResult, Lua, LuaSerdeExt, Table, Value};
use uji_macros::{constant, function, options};

#[function(json, raise)]
fn encode(value: &Value) -> Result<String, serde_json::Error> {
    serde_json::to_string(value)
}

#[options]
struct DecodeOptions {
    nulls: Option<bool>,
}

#[function(json)]
fn decode(lua: &Lua, text: &mlua::LuaString, opts: &DecodeOptions) -> mlua::Result<Value> {
    let value: serde_json::Value = serde_json::from_slice(&text.as_bytes()).into_lua_err()?;
    let nulls = opts.nulls != Some(false);
    let options = SerializeOptions::new()
        .serialize_none_to_null(nulls)
        .serialize_unit_to_null(nulls);
    lua.to_value_with(&value, options)
}

#[function(json)]
fn array(lua: &Lua, table: Table) -> mlua::Result<Table> {
    table.set_metatable(Some(lua.array_metatable()))?;
    Ok(table)
}

#[constant(json)]
fn null(lua: &Lua) -> Value {
    lua.null()
}
