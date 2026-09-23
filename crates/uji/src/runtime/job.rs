use std::path::PathBuf;
use std::time::Duration;

use mlua::{IntoLuaMulti, Lua, MultiValue};
use tokio::sync::mpsc::UnboundedReceiver;
use uji_agent::llm::CancelToken;
use uji_agent::process::{self, Exit, Spec, Stream};

pub(crate) enum JobEvent {
    Stdout { id: u64, line: String },
    Stderr { id: u64, line: String },
    Exit { id: u64, exit: Exit },
}

impl JobEvent {
    pub(crate) fn id(&self) -> u64 {
        match self {
            Self::Stdout { id, .. } | Self::Stderr { id, .. } | Self::Exit { id, .. } => *id,
        }
    }

    pub(crate) fn args(&self, lua: &Lua) -> mlua::Result<MultiValue> {
        match self {
            Self::Stdout { line, .. } | Self::Stderr { line, .. } => {
                line.as_str().into_lua_multi(lua)
            }
            Self::Exit {
                exit: Exit::Code(code),
                ..
            } => code.into_lua_multi(lua),
            Self::Exit {
                exit: Exit::TimedOut,
                ..
            } => (-1, "timeout").into_lua_multi(lua),
            Self::Exit {
                exit: Exit::Cancelled,
                ..
            } => (-1, "stopped").into_lua_multi(lua),
        }
    }
}

pub(crate) type Write = Option<String>;

pub(crate) async fn run(
    id: u64,
    command: Vec<String>,
    cwd: Option<PathBuf>,
    timeout: Option<Duration>,
    cancel: CancelToken,
    writes: UnboundedReceiver<Write>,
    send: impl Fn(JobEvent) + Send + 'static,
) {
    let mut spec = Spec::argv(&command).writing(writes);
    if let Some(cwd) = cwd.as_deref() {
        spec = spec.in_dir(cwd);
    }
    if let Some(timeout) = timeout {
        spec = spec.within(timeout);
    }
    let exit = process::stream(spec, &cancel, |stream, line| {
        send(match stream {
            Stream::Out => JobEvent::Stdout { id, line },
            Stream::Err => JobEvent::Stderr { id, line },
        });
    })
    .await;
    let exit = exit.unwrap_or_else(|err| {
        send(JobEvent::Stderr {
            id,
            line: format!("spawn: {err}"),
        });
        Exit::Code(-1)
    });
    send(JobEvent::Exit { id, exit });
}
