mod agent;
mod approval;
mod background;
mod commands;
mod compact;
mod drains;
pub(crate) mod events;
mod input;
mod model;
mod mouse;
mod persist;
mod picker;
mod prompt;

pub(crate) use picker::LiveQuery;
mod queue;
mod replies;
pub(crate) mod shell;
pub(crate) mod wires;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use uji_agent::session::store::SessionStorage;
use uji_ui::app::App;

use crate::cmd::{Action, LuaAction};

use super::Inner;
use super::frontend::Frontend;
use super::signal::Signal;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Control {
    Run,
    Reload,
    Quit,
}

enum ModalInput {
    Select(String),
    Prompt(String),
    Cancel,
}

pub(crate) enum Awaiting {
    Approval { id: u64, arguments: mlua::Table },
    Result(Running),
}

impl Awaiting {
    fn id(&self) -> u64 {
        match self {
            Self::Approval { id, .. } => *id,
            Self::Result(running) => running.call,
        }
    }
}

pub(crate) struct Running {
    call: u64,
    name: String,
    cancel: Option<mlua::Function>,
}

enum ToolApproval {
    Allow,
    Deny { reason: String },
    Ask { title: Option<String> },
}

pub(crate) struct LoopData {
    pub(crate) inner: Rc<Inner>,
    pub(crate) app: App,
    pub(crate) storage: Box<dyn SessionStorage>,
    pub(crate) frontend: Box<dyn Frontend>,
    pub(crate) dirty: bool,
    pub(crate) control: Control,
    pub(crate) signals: calloop::channel::Sender<Signal>,
    pub(crate) runtime: tokio::runtime::Runtime,
    pub(crate) active: Option<Box<dyn Action>>,
    pub(crate) modal: Option<LuaAction>,
    pub(crate) action_done: bool,
    pub(crate) awaiting: Option<Awaiting>,
    pub(crate) turn: Option<mlua::Function>,
    pub(crate) calls: Vec<wires::Live>,
    pub(crate) queued: VecDeque<String>,
    pub(crate) live_query: LiveQuery,
    pub(crate) shell: Option<shell::Running>,
    pub(crate) deferred: VecDeque<events::Reported>,
    pub(crate) last_reveal: Instant,
    pub(crate) config_dir: Option<PathBuf>,
}
