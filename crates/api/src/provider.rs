use std::rc::Rc;

use mlua::{Lua, LuaSerdeExt, Table, Value as LuaValue};
use uji_core::llm::Provider;

use super::Api;

pub fn add(lua: &Lua, api: &Rc<Api>) -> mlua::Result<mlua::Function> {
    let api = api.clone();
    lua.create_function(move |lua, table: Table| {
        let provider: Provider = lua.from_value(LuaValue::Table(table))?;
        api.providers().borrow_mut().add(provider);
        Ok(())
    })
}

pub fn remove(lua: &Lua, api: &Rc<Api>) -> mlua::Result<mlua::Function> {
    let api = api.clone();
    lua.create_function(move |_, id: String| {
        api.providers().borrow_mut().remove(&id);
        Ok(())
    })
}

pub fn list(lua: &Lua, api: &Rc<Api>) -> mlua::Result<mlua::Function> {
    let api = api.clone();
    lua.create_function(move |lua, ()| {
        let catalog = api.providers();
        let catalog = catalog.borrow();
        let out = lua.create_table()?;
        for (index, provider) in catalog.all().iter().enumerate() {
            let entry = lua.create_table()?;
            entry.set("id", provider.id.clone())?;
            entry.set("name", provider.name.clone())?;
            entry.set("base_url", provider.base_url.clone())?;
            let models = lua.create_table()?;
            for (at, model) in provider.models.iter().enumerate() {
                let row = lua.create_table()?;
                row.set("id", model.id.clone())?;
                row.set("context", model.context)?;
                row.set("output", model.output)?;
                models.set(at + 1, row)?;
            }
            entry.set("models", models)?;
            out.set(index + 1, entry)?;
        }
        Ok(out)
    })
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let provider = lua.create_table()?;
    provider.set("add", add(lua, api)?)?;
    provider.set("remove", remove(lua, api)?)?;
    provider.set("list", list(lua, api)?)?;
    Ok(provider)
}
