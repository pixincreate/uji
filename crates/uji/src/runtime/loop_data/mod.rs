mod agent;
mod background;
mod commands;
mod drains;
mod input;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use uji_core::llm::{CancelToken, StreamEvent, ToolDecision};
use uji_core::session::store::SessionStorage;
use uji_tui::app::App;

use super::Inner;
use super::builtin::Builtin;
use super::frontend::Frontend;
use super::job::Running;
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
    pub(crate) active: Option<Builtin>,
    pub(crate) action_done: bool,
    pub(crate) pending_tool: Option<(String, tokio::sync::oneshot::Sender<ToolDecision>)>,
    pub(crate) queued: VecDeque<String>,
    pub(crate) cancel: Option<CancelToken>,
    pub(crate) deferred: VecDeque<StreamEvent>,
    pub(crate) last_reveal: Instant,
    pub(crate) config_dir: Option<PathBuf>,
    pub(crate) jobs: Running,
}
