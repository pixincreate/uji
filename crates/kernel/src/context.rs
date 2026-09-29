use std::cell::RefCell;
use std::time::Instant;

use tokio::runtime::Handle;
use uji_native::abi::{self, Work};

use crate::kernel::Restart;
use crate::queue::{Channel, Queue};
use crate::tty::Tty;
use crate::vm::Sources;

pub(crate) struct Context {
    pub(crate) io: Handle,
    pub(crate) layers: Vec<Sources>,
    pub(crate) queue: Queue,
    pub(crate) terminal: Option<Tty>,
    pub(crate) clipboard: Option<arboard::Clipboard>,
    pub(crate) started: Instant,
    pub(crate) exit: Option<u8>,
    pub(crate) restart: Option<Restart>,
    pub(crate) errors: Vec<String>,
}

impl Context {
    pub(crate) fn new(
        io: Handle,
        layers: Vec<Sources>,
        terminal: Option<Tty>,
        channel: Channel,
    ) -> Self {
        Self {
            io,
            layers,
            queue: Queue::new(channel),
            terminal,
            clipboard: None,
            started: Instant::now(),
            exit: None,
            restart: None,
            errors: Vec::new(),
        }
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Context>> = const { RefCell::new(None) };
}

pub(crate) fn enter(context: Context) {
    CURRENT.set(Some(context));
}

pub(crate) fn leave() -> Option<Context> {
    CURRENT.take()
}

pub(crate) fn with<R>(run: impl FnOnce(&mut Context) -> R) -> Option<R> {
    CURRENT.with_borrow_mut(|current| current.as_mut().map(run))
}

pub(crate) fn start(work: Work) -> u64 {
    with(|context| context.queue.start(&context.io, work)).unwrap_or_else(|| {
        abi::fail(&"the kernel is not running");
        0
    })
}
