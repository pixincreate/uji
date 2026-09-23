use std::path::PathBuf;
use std::time::Duration;

use mlua::{Function, Table, Value};
use uji_agent::session::model::Message;

use crate::api::agent::Report;
use crate::api::fs::FsOp;
use crate::api::http::Fetch;
use crate::api::modal;

/// Something a plugin asked the runtime to do, to be carried out on the next
/// loop tick.
///
/// One ordered queue rather than a slot per kind, so requests run in the order
/// the plugin made them.
pub enum Request {
    Submit(String),
    SetTitle(String),
    Exec(Vec<String>),
    Interrupt,
    Modal(Box<modal::ModalRequest>),
    Answer(modal::Answer),
    ToolResult {
        call: u64,
        text: String,
    },
    Report {
        event: Report,
        on_applied: Option<Function>,
    },
    Steer(u64),
    Approve {
        id: u64,
        name: String,
        arguments: Table,
    },
    RunTool {
        id: u64,
        name: String,
        arguments: Value,
    },
    Stop(u64),
    Compact {
        id: u64,
        messages: Vec<Message>,
    },
    Route(u64),
    Defer {
        id: u64,
        after: Duration,
    },
    ToolProgress {
        call: u64,
        line: String,
    },
    JobStart {
        id: u64,
        command: Vec<String>,
        cwd: Option<PathBuf>,
        timeout: Option<Duration>,
    },
    JobStop(u64),
    JobWrite {
        id: u64,
        data: Option<String>,
    },
    Fetch {
        id: u64,
        fetch: Box<Fetch>,
    },
    Fs {
        id: u64,
        op: FsOp,
    },
    /// Fresh candidates for a live picker, tagged with the query they answer.
    PickItems {
        items: Vec<String>,
        token: u64,
    },
}
