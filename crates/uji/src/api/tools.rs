use std::path::PathBuf;
use std::rc::Rc;

use mlua::{Function, Lua, Table, Value as LuaValue};
use uji_agent::fs::Files;
use uji_agent::tools::policy::Action;

use super::Api;
use crate::api::bind::bind;
use crate::api::request::Request;

/// Where file tools may reach: the working directory plus any granted roots,
/// and whether that set is enforced at all.
#[derive(Default)]
pub struct Access {
    roots: Vec<PathBuf>,
    confined: bool,
    disabled: std::collections::BTreeSet<String>,
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

    pub fn disabled(&self) -> &std::collections::BTreeSet<String> {
        &self.disabled
    }

    pub fn disable(&mut self, names: Vec<String>) {
        self.disabled.extend(names);
    }

    pub fn enable(&mut self, names: &[String]) {
        self.disabled.retain(|name| !names.contains(name));
    }

    pub fn files(&self, cwd: PathBuf) -> Files {
        Files::new(cwd, self.roots.clone(), self.confined)
    }
}

#[derive(Debug, Clone)]
pub enum Subject {
    Name,
    Label(String),
    Of(Function),
}

#[derive(Debug, Clone)]
pub struct Tool {
    pub description: String,
    pub parameters: LuaValue,
    pub subject: Subject,
    pub policy: Option<Action>,
    pub defer: bool,
    pub run: Function,
}

impl Tool {
    pub fn subject(&self, name: &str, args: &Table) -> mlua::Result<String> {
        match &self.subject {
            Subject::Name => Ok(name.to_string()),
            Subject::Label(label) => Ok(label.clone()),
            Subject::Of(of) => of
                .call::<Option<String>>(args.clone())
                .map(Option::unwrap_or_default),
        }
    }
}

fn subject(value: LuaValue) -> mlua::Result<Subject> {
    match value {
        LuaValue::Nil => Ok(Subject::Name),
        LuaValue::String(label) => Ok(Subject::Label(label.to_str()?.to_owned())),
        LuaValue::Function(of) => Ok(Subject::Of(of)),
        _ => Err(mlua::Error::runtime(
            "subject must be a string or a function",
        )),
    }
}

fn policy(value: Option<String>) -> mlua::Result<Option<Action>> {
    value
        .map(|value| {
            Action::parse(&value).ok_or_else(|| {
                mlua::Error::runtime(format!("policy `{value}` is not allow, ask or deny"))
            })
        })
        .transpose()
}

pub(crate) fn done(lua: &Lua, api: &Rc<Api>, call: u64) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, text: String| {
        api.request(Request::ToolResult { call, text });
        Ok(())
    })
}

pub(crate) fn progress(lua: &Lua, api: &Rc<Api>, call: u64) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, line: String| {
        api.request(Request::ToolProgress { call, line });
        Ok(())
    })
}

pub fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, (name, opts): (String, Table)| {
        let description = opts
            .get::<Option<String>>("description")?
            .unwrap_or_default();
        let parameters = opts.get::<LuaValue>("parameters")?;
        let subject = subject(opts.get("subject")?)?;
        let policy = policy(opts.get("policy")?)?;
        let defer = opts.get::<Option<bool>>("defer")?.unwrap_or(false);
        let run: Function = opts.get("run")?;
        api.tools().borrow_mut().insert(
            name,
            Tool {
                description,
                parameters,
                subject,
                policy,
                defer,
                run,
            },
        );
        Ok(())
    })
}

pub fn unregister(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, name: String| {
        api.tools().borrow_mut().remove(&name);
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

pub fn disable(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, names: Vec<String>| {
        api.access().borrow_mut().disable(names);
        Ok(())
    })
}

pub fn enable(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, names: Vec<String>| {
        api.access().borrow_mut().enable(&names);
        Ok(())
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
