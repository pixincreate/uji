use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

use mlua::{Function, Lua, Table, Value as LuaValue};
use uji_core::config;
use uji_core::fs::Files;
use uji_core::tools::policy::Action;

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

    fn set_roots(&mut self, roots: Vec<PathBuf>) {
        self.roots = roots;
    }

    pub fn confined(&self) -> bool {
        self.confined
    }

    fn set_confined(&mut self, confined: bool) {
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

#[derive(Default)]
pub struct Rules {
    tools: BTreeMap<String, LuaValue>,
    changed: bool,
}

impl Rules {
    fn merge(&mut self, rules: &Table) -> mlua::Result<()> {
        for pair in rules.pairs::<String, LuaValue>() {
            let (name, value) = pair?;
            self.tools.insert(name, value);
        }
        self.changed = true;
        Ok(())
    }

    pub fn entries(&self) -> &BTreeMap<String, LuaValue> {
        &self.tools
    }

    pub fn changed(&self) -> bool {
        self.changed
    }

    pub fn settle(&mut self) {
        self.changed = false;
    }
}

#[derive(Debug, Clone)]
pub enum Subject {
    Name,
    Label(String),
    Of(Function),
}

#[derive(Debug, Clone, Default)]
pub struct Display {
    pub verb: Option<String>,
    pub question: Option<String>,
}

#[derive(Debug)]
pub struct Tool {
    pub description: String,
    pub parameters: LuaValue,
    pub subject: Subject,
    pub policy: Option<Action>,
    pub display: Display,
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

    pub fn detail(&self, args: &Table) -> mlua::Result<Option<String>> {
        match &self.subject {
            Subject::Name => Ok(None),
            Subject::Label(label) => Ok(Some(label.clone())),
            Subject::Of(of) => of.call::<Option<String>>(args.clone()),
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

fn display(value: Option<Table>) -> mlua::Result<Display> {
    let Some(table) = value else {
        return Ok(Display::default());
    };
    Ok(Display {
        verb: table.get("verb")?,
        question: table.get("question")?,
    })
}

pub(crate) fn context(lua: &Lua, api: &Rc<Api>, call: u64) -> mlua::Result<Table> {
    let ctx = lua.create_table()?;
    ctx.set(
        "done",
        bind(lua, api, move |api, _, text: String| {
            api.request(Request::ToolResult { call, text });
            Ok(())
        })?,
    )?;
    ctx.set(
        "progress",
        bind(lua, api, move |api, _, line: String| {
            api.request(Request::ToolProgress { call, line });
            Ok(())
        })?,
    )?;
    Ok(ctx)
}

pub fn add(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, (name, opts): (String, Table)| {
        let description = opts
            .get::<Option<String>>("description")?
            .unwrap_or_default();
        let parameters = opts.get::<LuaValue>("parameters")?;
        let subject = subject(opts.get("subject")?)?;
        let policy = policy(opts.get("policy")?)?;
        let display = display(opts.get("display")?)?;
        let run: Function = opts.get("run")?;
        api.tools().borrow_mut().insert(
            name,
            Rc::new(Tool {
                description,
                parameters,
                subject,
                policy,
                display,
                run,
            }),
        );
        Ok(())
    })
}

pub fn remove(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, name: String| {
        Ok(api.tools().borrow_mut().remove(&name).is_some())
    })
}

pub fn list(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, ()| {
        Ok(api.tools().borrow().keys().cloned().collect::<Vec<_>>())
    })
}

pub fn roots(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, paths: Option<Vec<String>>| {
        if let Some(paths) = paths {
            let expanded = paths.iter().map(|path| config::expand_home(path)).collect();
            api.access().borrow_mut().set_roots(expanded);
        }
        Ok(api
            .access()
            .borrow()
            .roots()
            .iter()
            .map(|root| root.display().to_string())
            .collect::<Vec<_>>())
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
        if let Some(enabled) = enabled {
            api.access().borrow_mut().set_confined(enabled);
        }
        Ok(api.access().borrow().confined())
    })
}

pub fn policy_rules(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, rules: Table| {
        api.rules().borrow_mut().merge(&rules)
    })
}
