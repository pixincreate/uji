use mlua::serde::SerializeOptions;
use mlua::{Lua, LuaSerdeExt, Table, Value};
use uji_macros::{FromLua, function, register};

#[function(json)]
fn encode(value: &Value) -> mlua::Result<String> {
    serde_json::to_string(value).map_err(mlua::Error::external)
}

#[derive(FromLua)]
struct DecodeOptions {
    #[lua(default = true)]
    nulls: bool,
}

#[function(json)]
fn decode(lua: &Lua, text: &mlua::LuaString, opts: &DecodeOptions) -> mlua::Result<Value> {
    let value: serde_json::Value =
        serde_json::from_slice(&text.as_bytes()).map_err(mlua::Error::external)?;
    let options = SerializeOptions::new()
        .serialize_none_to_null(opts.nulls)
        .serialize_unit_to_null(opts.nulls);
    lua.to_value_with(&value, options)
}

#[function(json)]
fn array(lua: &Lua, table: Table) -> mlua::Result<Table> {
    table.set_metatable(Some(lua.array_metatable()))?;
    Ok(table)
}

#[register(json)]
fn null(lua: &Lua) -> Value {
    lua.null()
}
