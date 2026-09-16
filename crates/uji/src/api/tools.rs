use std::path::PathBuf;
use std::rc::Rc;

use mlua::{Function, Lua, Table, Value as LuaValue};

use super::Api;
use crate::api::bind::bind;

/// Where file tools may reach: the working directory plus any granted roots,
/// and whether that set is enforced at all.
#[derive(Default)]
pub struct Access {
    roots: Vec<PathBuf>,
    confined: bool,
}

impl Access {
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    pub fn set_roots(&mut self, roots: Vec<PathBuf>) {
        self.roots = roots;
    }

    pub fn confined(&self) -> bool {
        self.confined
    }

    pub fn set_confined(&mut self, confined: bool) {
        self.confined = confined;
    }
}

#[derive(Debug, Clone)]
pub struct LuaTool {
    pub description: String,
    pub parameters: LuaValue,
    pub subject: Option<String>,
    /// Whether `run` answers later through `done` instead of returning.
    pub defer: bool,
    pub run: Function,
}

/// The `done` a deferred tool is handed, to answer with once it has something.
///
/// It only posts the result; the loop matches it to the tool that is waiting.
pub(crate) fn completion(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, text: String| {
        api.request(crate::api::request::Request::ToolResult(text));
        Ok(())
    })
}

pub fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, (name, opts): (String, Table)| {
        let description = opts
            .get::<Option<String>>("description")?
            .unwrap_or_default();
        let parameters = opts.get::<LuaValue>("parameters")?;
        let subject = opts.get::<Option<String>>("subject")?;
        let defer = opts.get::<Option<bool>>("defer")?.unwrap_or(false);
        let run: Function = opts.get("run")?;
        api.lua_tools().borrow_mut().insert(
            name,
            LuaTool {
                description,
                parameters,
                subject,
                defer,
                run,
            },
        );
        Ok(())
    })
}

pub fn unregister(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, name: String| {
        api.lua_tools().borrow_mut().remove(&name);
        Ok(())
    })
}

pub fn roots(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, paths: Vec<String>| {
        let expanded = paths
            .iter()
            .map(|path| PathBuf::from(expand(path)))
            .collect();
        api.access().borrow_mut().set_roots(expanded);
        Ok(())
    })
}

pub fn list_roots(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, lua, ()| {
        let out = lua.create_table()?;
        for (at, root) in api.access().borrow().roots().iter().enumerate() {
            out.set(at + 1, root.display().to_string())?;
        }
        Ok(out)
    })
}

pub fn confine(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, enabled: Option<bool>| {
        api.access()
            .borrow_mut()
            .set_confined(enabled.unwrap_or(true));
        Ok(api.access().borrow().confined())
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
