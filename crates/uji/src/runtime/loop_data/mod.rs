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
use std::rc::Rc;
use std::time::{Duration, Instant};

use mlua::Function;
use uji_agent::session::store::SessionStorage;
use uji_ui::app::App;

use crate::cmd::{Action, LuaAction};

use super::Inner;
use super::frontend::Frontend;
use super::work::Work;

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
    cancel: Option<Function>,
}

enum ToolApproval {
    Allow,
    Deny { reason: String },
    Ask { title: Option<String> },
}

#[derive(Default)]
pub(crate) struct Turn {
    cancel: Option<Function>,
    awaiting: Option<Awaiting>,
    queued: VecDeque<String>,
}

pub(crate) struct Reveal {
    deferred: VecDeque<events::Reported>,
    last: Instant,
}

const FRAME: Duration = Duration::from_millis(16);

impl Default for Reveal {
    fn default() -> Self {
        Self {
            deferred: VecDeque::new(),
            last: Instant::now(),
        }
    }
}

impl Reveal {
    fn holding(&self) -> bool {
        !self.deferred.is_empty()
    }

    fn hold(&mut self, reported: events::Reported) {
        self.deferred.push_back(reported);
    }

    fn release(&mut self, revealing: bool) -> Option<events::Reported> {
        if revealing && !self.deferred.front()?.event.is_delta() {
            return None;
        }
        self.deferred.pop_front()
    }

    fn due(&self) -> bool {
        self.last.elapsed() >= FRAME
    }

    fn stepped(&mut self) {
        self.last = Instant::now();
    }
}

#[derive(Default)]
pub(crate) struct Command {
    active: Option<Box<dyn Action>>,
    modal: Option<LuaAction>,
    done: bool,
}

pub(crate) struct LoopData {
    pub(crate) inner: Rc<Inner>,
    pub(crate) app: App,
    pub(crate) storage: Box<dyn SessionStorage>,
    pub(crate) frontend: Box<dyn Frontend>,
    pub(crate) work: Work,
    pub(crate) control: Control,
    pub(crate) dirty: bool,
    turn: Turn,
    reveal: Reveal,
    command: Command,
    calls: Vec<wires::Live>,
    live_query: LiveQuery,
    shell: Option<shell::Running>,
}

impl LoopData {
    pub(crate) fn new(
        inner: Rc<Inner>,
        app: App,
        storage: Box<dyn SessionStorage>,
        frontend: Box<dyn Frontend>,
        work: Work,
    ) -> Self {
        Self {
            inner,
            app,
            storage,
            frontend,
            work,
            control: Control::Run,
            dirty: false,
            turn: Turn::default(),
            reveal: Reveal::default(),
            command: Command::default(),
            calls: Vec::new(),
            live_query: LiveQuery::default(),
            shell: None,
        }
    }
}
