use std::path::PathBuf;

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
    ToolResult(String),
    JobStart {
        id: u64,
        command: Vec<String>,
        cwd: Option<PathBuf>,
    },
    JobStop(u64),
    JobWrite {
        id: u64,
        data: Option<String>,
    },
    /// Fresh candidates for a live picker, tagged with the query they answer.
    PickItems {
        items: Vec<String>,
        token: u64,
    },
}
