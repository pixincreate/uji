use std::path::PathBuf;
use std::rc::Rc;

use mlua::{Function, Lua, Table, Value as LuaValue};

use super::Api;

#[derive(Debug, Clone)]
pub struct LuaTool {
    pub description: String,
    pub parameters: LuaValue,
    pub subject: Option<String>,
    pub run: Function,
}

pub fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |_, (name, opts): (String, Table)| {
        let description = opts
            .get::<Option<String>>("description")?
            .unwrap_or_default();
        let parameters = opts.get::<LuaValue>("parameters")?;
        let subject = opts.get::<Option<String>>("subject")?;
        let run: Function = opts.get("run")?;
        api.lua_tools().borrow_mut().insert(
            name,
            LuaTool {
                description,
                parameters,
                subject,
                run,
            },
        );
        Ok(())
    })
}

pub fn unregister(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |_, name: String| {
        api.lua_tools().borrow_mut().remove(&name);
        Ok(())
    })
}

pub fn roots(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |_, paths: Vec<String>| {
        let expanded = paths
            .iter()
            .map(|path| PathBuf::from(expand(path)))
            .collect();
        *api.tool_roots().borrow_mut() = expanded;
        Ok(())
    })
}

pub fn list_roots(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |lua, ()| {
        let out = lua.create_table()?;
        for (at, root) in api.tool_roots().borrow().iter().enumerate() {
            out.set(at + 1, root.display().to_string())?;
        }
        Ok(out)
    })
}

pub fn confine(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    let api = Rc::clone(api);
    lua.create_function(move |_, enabled: Option<bool>| {
        api.set_tool_confined(enabled.unwrap_or(true));
        Ok(api.tool_confined())
    })
}

fn expand(path: &str) -> String {
    match path.strip_prefix("~/") {
        Some(rest) => {
            std::env::var("HOME").map_or_else(|_| path.to_string(), |home| format!("{home}/{rest}"))
        }
        None => path.to_string(),
    }
}
