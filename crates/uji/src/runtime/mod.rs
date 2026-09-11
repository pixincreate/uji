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
use std::sync::atomic::AtomicBool;

use calloop::{EventLoop, LoopHandle};
use crossterm::event::Event as TermEvent;
use uji_screen::state::UiState;

use uji_core::llm::StreamEvent;
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
        let app = App::new(session, conversation, inner.state());

        let (sender, channel) = calloop::channel::channel::<TermEvent>();
        let (llm_sender, llm_channel) = calloop::channel::channel::<StreamEvent>();
        let (job_sender, job_channel) = calloop::channel::channel::<job::JobEvent>();
        let (background_sender, background_channel) =
            calloop::channel::channel::<background::Background>();
        let reader_paused = Arc::new(AtomicBool::new(false));

        let mut frontend: Box<dyn Frontend> = Box::new(frontend);
        frontend.start(sender, Arc::clone(&reader_paused))?;

        let mut data = LoopData {
            inner,
            app,
            storage,
            frontend,
            dirty: false,
            control: Control::Run,
            llm_tx: llm_sender,
            active: None,
            action_done: false,
            pending_tool: None,
            queued: VecDeque::new(),
            cancel: None,
            config_dir,
            job_tx: job_sender,
            background_tx: background_sender,
            jobs: job::Running::default(),
            reader_paused,
            runtime: tokio::runtime::Runtime::new()?,
        };

        data.inner.resolve_llm(&mut *data.storage);
        data.inner.emit(events::STATUS_CHANGED, &[]);
        data.refresh_suggestions();

        install_sources(
            &event_loop.handle(),
            channel,
            llm_channel,
            job_channel,
            background_channel,
        )?;
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
                .dispatch(None, &mut data)
                .map_err(io::Error::other)?;
            data.pump(&loop_handle)?;
        }

        data.inner.emit(events::QUIT, &[]);
        data.frontend.stop()
    }
}

fn install_sources(
    handle: &LoopHandle<'static, LoopData>,
    term: calloop::channel::Channel<TermEvent>,
    llm: calloop::channel::Channel<StreamEvent>,
    jobs: calloop::channel::Channel<job::JobEvent>,
    background: calloop::channel::Channel<background::Background>,
) -> io::Result<()> {
    handle
        .insert_source(term, |event, _meta, data: &mut LoopData| match event {
            calloop::channel::Event::Msg(event) => data.on_term_event(&event),
            calloop::channel::Event::Closed => data.control = Control::Quit,
        })
        .map_err(|err| io::Error::other(format!("register input source: {err}")))?;
    handle
        .insert_source(llm, |event, _meta, data: &mut LoopData| {
            if let calloop::channel::Event::Msg(event) = event {
                data.on_llm_event(event);
            }
        })
        .map_err(|err| io::Error::other(format!("register llm source: {err}")))?;
    handle
        .insert_source(jobs, |event, _meta, data: &mut LoopData| {
            if let calloop::channel::Event::Msg(event) = event {
                data.on_job_event(&event);
            }
        })
        .map_err(|err| io::Error::other(format!("register job source: {err}")))?;
    handle
        .insert_source(background, |event, _meta, data: &mut LoopData| {
            if let calloop::channel::Event::Msg(event) = event {
                data.on_background(event);
            }
        })
        .map_err(|err| io::Error::other(format!("register background source: {err}")))?;
    Ok(())
}
