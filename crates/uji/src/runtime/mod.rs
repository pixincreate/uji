mod auth;
mod background;
mod builtin;
mod error;
pub mod events;
pub mod frontend;
mod inner;
mod input;
mod job;
mod loader;
mod loop_data;
mod policy;
mod renderer;
mod signal;

pub use error::RuntimeError;
pub use frontend::{Frontend, Terminal};
pub(crate) use inner::Inner;
pub(crate) use loop_data::{Control, LoopData};

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use calloop::{EventLoop, LoopHandle};
use crossterm::event::Event as TermEvent;
use uji_screen::state::UiState;

use signal::Signal;
use uji_core::session::conversation::{Conversation, Shared};
use uji_core::session::model::Session;
use uji_core::session::store::SessionStorage;
use uji_tui::app::App;

pub struct Runtime {
    inner: Rc<Inner>,
    loop_handle: LoopHandle<'static, LoopData>,
    event_loop: EventLoop<'static, LoopData>,
    config_dir: Option<PathBuf>,
    conversation: Shared,
}

impl Runtime {
    pub fn boot() -> Result<Self, RuntimeError> {
        Self::boot_in(None)
    }

    pub fn boot_in(config_dir: Option<PathBuf>) -> Result<Self, RuntimeError> {
        let state = Rc::new(RefCell::new(UiState::new()));
        let conversation = Conversation::shared();
        let client = Arc::new(reqwest::Client::new());
        let inner = Inner::boot(state, conversation.clone(), client, config_dir.clone());

        let event_loop = EventLoop::try_new()?;
        let loop_handle = event_loop.handle();

        Ok(Self {
            inner,
            loop_handle,
            event_loop,
            config_dir,
            conversation,
        })
    }

    pub fn emit(&self, event: &str, fields: &[(&str, String)]) {
        self.inner.emit(event, fields);
    }

    pub fn state(&self) -> Rc<RefCell<UiState>> {
        self.inner.state()
    }

    pub fn diagnostics(&self) -> Vec<String> {
        self.inner.take_diagnostics()
    }

    pub fn eval(&self, chunk: &str) -> mlua::Result<()> {
        self.inner.lua.load(chunk).exec()
    }

    pub fn run(self, session: Session, storage: Box<dyn SessionStorage>) -> io::Result<()> {
        self.run_with(session, storage, Terminal::new())
    }

    pub fn run_with(
        self,
        session: Session,
        mut storage: Box<dyn SessionStorage>,
        frontend: impl Frontend + 'static,
    ) -> io::Result<()> {
        let Self {
            inner,
            loop_handle,
            mut event_loop,
            config_dir,
            conversation,
        } = self;

        let messages = storage.messages(&session.id).map_err(io::Error::other)?;
        conversation.borrow_mut().attach(&session, messages);
        let mut app = App::new(session, conversation, inner.state());
        app.set_renderer(Rc::new(renderer::LuaRenderer::new(Rc::clone(&inner))));

        let (keys, key_channel) = calloop::channel::channel::<TermEvent>();
        let (signals, signal_channel) = calloop::channel::channel::<Signal>();
        let mut frontend: Box<dyn Frontend> = Box::new(frontend);
        frontend.start(keys)?;

        let mut data = LoopData {
            inner,
            app,
            storage,
            frontend,
            dirty: false,
            control: Control::Run,
            signals,
            active: None,
            action_done: false,
            pending_tool: None,
            queued: VecDeque::new(),
            cancel: None,
            deferred: VecDeque::new(),
            last_reveal: std::time::Instant::now(),
            config_dir,
            jobs: job::Running::default(),
            runtime: tokio::runtime::Runtime::new()?,
        };

        data.bootstrap()?;

        install_sources(&event_loop.handle(), key_channel, signal_channel)?;
        let timer = calloop::timer::Timer::from_duration(data.timer_interval());
        event_loop
            .handle()
            .insert_source(timer, |_event, _meta, data: &mut LoopData| {
                data.on_timer();
                calloop::timer::TimeoutAction::ToDuration(data.timer_interval())
            })
            .map_err(|err| io::Error::other(format!("register timer source: {err}")))?;

        data.frontend.draw(&data.app)?;
        data.dirty = false;

        while data.control != Control::Quit {
            event_loop
                .dispatch(data.frame_timeout(), &mut data)
                .map_err(io::Error::other)?;
            data.pump(&loop_handle)?;
        }

        data.inner.emit(events::QUIT, &[]);
        data.frontend.stop()
    }
}

fn install_sources(
    handle: &LoopHandle<'static, LoopData>,
    keys: calloop::channel::Channel<TermEvent>,
    signals: calloop::channel::Channel<Signal>,
) -> io::Result<()> {
    handle
        .insert_source(keys, |event, _meta, data: &mut LoopData| match event {
            calloop::channel::Event::Msg(event) => data.on_term_event(&event),
            calloop::channel::Event::Closed => data.control = Control::Quit,
        })
        .map_err(|err| io::Error::other(format!("register input source: {err}")))?;
    handle
        .insert_source(signals, |event, _meta, data: &mut LoopData| {
            if let calloop::channel::Event::Msg(signal) = event {
                data.on_signal(signal);
            }
        })
        .map_err(|err| io::Error::other(format!("register signal source: {err}")))?;
    Ok(())
}
