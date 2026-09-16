use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use mlua::{Function, Lua, Table, Value};

use crate::api::Api;
use crate::api::bind::bind;

pub struct JobRequest {
    pub id: u64,
    pub command: Vec<String>,
    pub cwd: Option<PathBuf>,
}

#[derive(Default)]
#[allow(clippy::struct_field_names)]
pub struct JobHandlers {
    pub on_stdout: Option<Function>,
    pub on_stderr: Option<Function>,
    pub on_exit: Option<Function>,
}

#[derive(Default)]
pub struct Jobs {
    handlers: HashMap<u64, JobHandlers>,
    requests: Vec<JobRequest>,
    stops: Vec<u64>,
    next: u64,
}

impl Jobs {
    pub fn take_requests(&mut self) -> Vec<JobRequest> {
        std::mem::take(&mut self.requests)
    }

    pub fn take_stops(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.stops)
    }

    pub fn handlers(&self, id: u64) -> Option<&JobHandlers> {
        self.handlers.get(&id)
    }

    pub fn finish(&mut self, id: u64) {
        self.handlers.remove(&id);
    }
}

fn command_from(value: &Value) -> mlua::Result<Vec<String>> {
    match value {
        Value::String(text) => Ok(vec![
            String::from("sh"),
            String::from("-c"),
            text.to_string_lossy(),
        ]),
        Value::Table(table) => {
            let parts: Vec<String> = table
                .clone()
                .sequence_values::<String>()
                .collect::<mlua::Result<_>>()?;
            if parts.is_empty() {
                return Err(mlua::Error::runtime("cmd list must not be empty"));
            }
            Ok(parts)
        }
        _ => Err(mlua::Error::runtime(
            "cmd must be a string or a list of strings",
        )),
    }
}

pub(crate) fn start(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, opts: Table| {
        let command = command_from(&opts.get::<Value>("cmd")?)?;
        let cwd = opts.get::<Option<String>>("cwd")?.map(PathBuf::from);
        let handlers = JobHandlers {
            on_stdout: opts.get("on_stdout")?,
            on_stderr: opts.get("on_stderr")?,
            on_exit: opts.get("on_exit")?,
        };
        let jobs = api.jobs();
        let mut jobs = jobs.borrow_mut();
        jobs.next += 1;
        let id = jobs.next;
        jobs.handlers.insert(id, handlers);
        jobs.requests.push(JobRequest { id, command, cwd });
        Ok(id)
    })
}

pub(crate) fn stop(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, id: u64| {
        api.jobs().borrow_mut().stops.push(id);
        Ok(())
    })
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let job = lua.create_table()?;
    job.set("start", start(lua, api)?)?;
    job.set("stop", stop(lua, api)?)?;
    Ok(job)
}
