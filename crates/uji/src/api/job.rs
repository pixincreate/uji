use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use mlua::{Function, Lua, Table, Value};
use uji_agent::llm::CancelToken;

use crate::api::Api;
use crate::api::bind::bind;
use crate::api::request::Request;

#[derive(Default)]
#[allow(clippy::struct_field_names)]
pub struct JobHandlers {
    pub on_stdout: Option<Function>,
    pub on_stderr: Option<Function>,
    pub on_exit: Option<Function>,
}

/// A job a plugin started: the callbacks it gave, and the token that stops it
/// once the loop has actually spawned it.
struct Job {
    handlers: JobHandlers,
    cancel: Option<CancelToken>,
    stdin: Option<tokio::sync::mpsc::UnboundedSender<Option<String>>>,
}

/// Every job a plugin has running, in one place.
///
/// The callbacks and the cancel token used to live either side of the Lua
/// boundary, keyed by the same id, so ending a job meant finishing it twice.
#[derive(Default)]
pub struct Jobs {
    live: HashMap<u64, Job>,
    next: u64,
}

impl Jobs {
    pub fn handlers(&self, id: u64) -> Option<&JobHandlers> {
        self.live.get(&id).map(|job| &job.handlers)
    }

    /// Record a job Lua asked for. It is not running until the loop spawns it.
    fn open(&mut self, handlers: JobHandlers) -> u64 {
        self.next = self.next.saturating_add(1);
        self.live.insert(
            self.next,
            Job {
                handlers,
                cancel: None,
                stdin: None,
            },
        );
        self.next
    }

    /// Hand a spawned job the ways to reach it: the token that stops it, and
    /// the channel that feeds its stdin.
    pub fn attach(
        &mut self,
        id: u64,
        cancel: CancelToken,
        stdin: tokio::sync::mpsc::UnboundedSender<Option<String>>,
    ) {
        if let Some(job) = self.live.get_mut(&id) {
            job.cancel = Some(cancel);
            job.stdin = Some(stdin);
        }
    }

    /// Put `data` on a job's stdin, or close it when `data` is `None`.
    pub fn write(&self, id: u64, data: Option<String>) {
        if let Some(stdin) = self.live.get(&id).and_then(|job| job.stdin.as_ref()) {
            let _ = stdin.send(data);
        }
    }

    /// Ask a job to stop. Its callbacks stay until it reports that it exited.
    pub fn stop(&self, id: u64) {
        if let Some(cancel) = self.live.get(&id).and_then(|job| job.cancel.as_ref()) {
            cancel.cancel();
        }
    }

    pub fn finish(&mut self, id: u64) {
        self.live.remove(&id);
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
        let id = api.jobs().borrow_mut().open(handlers);
        api.request(Request::JobStart { id, command, cwd });
        Ok(id)
    })
}

pub(crate) fn stop(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, id: u64| {
        api.request(Request::JobStop(id));
        Ok(())
    })
}

/// Write to a job's stdin. A newline is added unless the data already ends in
/// one, since the protocols this carries are line-delimited.
pub(crate) fn send(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, (id, data): (u64, String)| {
        let data = if data.ends_with('\n') {
            data
        } else {
            format!("{data}\n")
        };
        api.request(Request::JobWrite {
            id,
            data: Some(data),
        });
        Ok(())
    })
}

/// Close a job's stdin, so the child sees EOF.
pub(crate) fn close(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, id: u64| {
        api.request(Request::JobWrite { id, data: None });
        Ok(())
    })
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let job = lua.create_table()?;
    job.set("start", start(lua, api)?)?;
    job.set("stop", stop(lua, api)?)?;
    job.set("send", send(lua, api)?)?;
    job.set("close", close(lua, api)?)?;
    Ok(job)
}
