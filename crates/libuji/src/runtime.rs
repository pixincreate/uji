//! The persistent harness runtime — nvim's main loop, executor and editor
//! state rolled into one.
//!
//! `Runtime` owns the Lua state for the whole session (config is never a
//! frozen snapshot), the live [`UiState`], the event handlers, and a calloop
//! event loop. Mapping to nvim:
//!
//! - `uji.schedule` → calloop `insert_idle` (deferred, non-re-entrant)
//! - event dispatch (`uji.on`/`uji.emit`) → nvim autocmds, run synchronously
//! - input reader thread + channel source → the multiqueue wakeup
//! - post-batch redraw → nvim flushing before it blocks again

use std::cell::RefCell;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use calloop::{EventLoop, LoopHandle};
use crossterm::event::Event as TermEvent;
use mlua::{Function, Lua as LuaState, Table};
use tui::app::{self, App, KeyAction};
use tui::state::UiState;
use uji_core::action;
use uji_core::session::model::{Message, Session};
use uji_core::session::store::SessionStorage;

use crate::config::{self, DEFAULT_LUA};
use crate::lua::functions::api_table;

/// How long the input reader waits between polls (ms).
const INPUT_POLL: Duration = Duration::from_millis(100);

/// Event names emitted by the harness (`PascalCase`, `nvim`-autocmd flavored).
pub mod events {
    /// A new session was created. Fields: `session_id`.
    pub const SESSION_CREATED: &str = "SessionCreated";
    /// An existing session was resumed. Fields: `session_id`.
    pub const SESSION_RESUMED: &str = "SessionResumed";
    /// Input was submitted. Fields: `text`.
    pub const MESSAGE_SUBMITTED: &str = "MessageSubmitted";
    /// A message was persisted. Fields: `type`, `text`.
    pub const MESSAGE_APPENDED: &str = "MessageAppended";
    /// The loop is about to exit.
    pub const QUIT: &str = "Quit";
}

/// Errors from booting the runtime.
#[derive(thiserror::Error, Debug)]
pub enum RuntimeError {
    /// Building or running Lua failed.
    #[error("lua: {0}")]
    Lua(#[from] mlua::Error),
    /// Creating the event loop failed.
    #[error("loop: {0}")]
    Loop(#[from] calloop::Error),
}

/// `uji.on` handlers, grouped by event name.
#[derive(Default)]
pub(crate) struct Handlers {
    by_event: Vec<(String, Vec<Function>)>,
}

impl Handlers {
    /// Register a handler for `event`.
    pub(crate) fn add(&mut self, event: String, handler: Function) {
        if let Some((_, list)) = self.by_event.iter_mut().find(|(name, _)| *name == event) {
            list.push(handler);
        } else {
            self.by_event.push((event, vec![handler]));
        }
    }

    fn get(&self, event: &str) -> Vec<Function> {
        self.by_event
            .iter()
            .find(|(name, _)| name == event)
            .map(|(_, list)| list.clone())
            .unwrap_or_default()
    }
}

/// `uji.schedule` callbacks awaiting a safe point on the loop.
#[derive(Default)]
pub(crate) struct Scheduled {
    pending: RefCell<Vec<Function>>,
}

impl Scheduled {
    /// Queue a callback to run at the next loop drain.
    pub(crate) fn push(&self, function: Function) {
        self.pending.borrow_mut().push(function);
    }

    /// Take all queued callbacks.
    pub(crate) fn take(&self) -> Vec<Function> {
        std::mem::take(&mut *self.pending.borrow_mut())
    }
}

/// Loop-independent runtime guts, shared with Lua closures.
pub(crate) struct Inner {
    pub(crate) lua: LuaState,
    pub(crate) state: Rc<RefCell<UiState>>,
    pub(crate) handlers: RefCell<Handlers>,
    pub(crate) scheduled: Scheduled,
}

impl Inner {
    /// Create a fresh inner runtime.
    pub(crate) fn new(lua: LuaState) -> Rc<Self> {
        Rc::new(Self {
            lua,
            state: Rc::new(RefCell::new(UiState::new())),
            handlers: RefCell::default(),
            scheduled: Scheduled::default(),
        })
    }

    /// Emit a harness event from Rust: build a ctx table from fields and
    /// dispatch to handlers.
    pub(crate) fn emit(&self, event: &str, fields: &[(&str, String)]) {
        let Ok(ctx) = self.lua.create_table() else {
            return;
        };
        for (key, value) in fields {
            let _ = ctx.set(*key, value.clone());
        }
        self.dispatch(event, &ctx);
    }

    /// Dispatch an event + ctx table to registered handlers (synchronously,
    /// `nvim`-autocmd style).
    pub(crate) fn dispatch(&self, event: &str, ctx: &Table) {
        for handler in self.handlers.borrow().get(event) {
            if let Err(err) = handler.call::<()>((event, ctx.clone())) {
                // ast-grep-ignore: no-print-in-lib
                eprintln!("uji: handler error for {event}: {err}");
            }
        }
    }
}

/// Per-loop state (owned by the calloop loop).
struct LoopData {
    inner: Rc<Inner>,
    app: App,
    storage: Box<dyn SessionStorage>,
    terminal: app::Term,
    dirty: bool,
    running: bool,
}

impl LoopData {
    fn on_term_event(&mut self, event: &TermEvent) {
        match event {
            TermEvent::Key(key) => {
                match self.app.handle_key(*key) {
                    KeyAction::Quit => self.running = false,
                    KeyAction::Submit(text) => self.submit(&text),
                    KeyAction::None => {}
                }
                self.dirty = true;
            }
            TermEvent::Resize(..) => self.dirty = true,
            _ => {}
        }
    }

    fn submit(&mut self, text: &str) {
        self.inner
            .emit(events::MESSAGE_SUBMITTED, &[("text", text.to_string())]);

        let user = Message::User {
            text: text.to_string(),
        };
        match self.storage.append_message(&self.app.session().id, user) {
            Ok(stored) => {
                self.app.push_message(stored);
                self.inner.emit(
                    events::MESSAGE_APPENDED,
                    &[("type", "user".into()), ("text", text.to_string())],
                );
            }
            Err(err) => {
                // ast-grep-ignore: no-print-in-lib
                eprintln!("uji: failed to persist message: {err}");
            }
        }

        if let Some(response) = action::run(text) {
            let assistant = Message::Assistant {
                text: response.clone(),
            };
            match self
                .storage
                .append_message(&self.app.session().id, assistant)
            {
                Ok(stored) => {
                    self.app.push_message(stored);
                    self.inner.emit(
                        events::MESSAGE_APPENDED,
                        &[("type", "assistant".into()), ("text", response)],
                    );
                }
                Err(err) => {
                    // ast-grep-ignore: no-print-in-lib
                    eprintln!("uji: failed to persist response: {err}");
                }
            }
        }
    }
}

/// The persistent harness runtime.
pub struct Runtime {
    inner: Rc<Inner>,
    loop_handle: LoopHandle<'static, LoopData>,
    event_loop: EventLoop<'static, LoopData>,
}

impl Runtime {
    /// Boot the runtime: build the `uji` API, run init.lua, load plugins, and
    /// fall back to the default layout if nothing was declared.
    pub fn boot() -> Result<Self, RuntimeError> {
        Self::boot_in(None, None)
    }

    /// Boot with explicit config file and plugin dir (embedder/test hook).
    pub fn boot_in(
        config_path: Option<PathBuf>,
        plugin_dir: Option<PathBuf>,
    ) -> Result<Self, RuntimeError> {
        let inner = Inner::new(LuaState::new());
        let uji = api_table(&inner.lua, &inner)?;
        if let Ok(opt) = inner.lua.create_table() {
            let _ = opt.set("cursor_blink", true);
            let _ = uji.set("opt", opt);
        }
        let _ = inner.lua.globals().set("uji", uji);

        let event_loop = EventLoop::try_new()?;
        let loop_handle = event_loop.handle();

        let runtime = Self {
            inner,
            loop_handle,
            event_loop,
        };
        runtime.run_init(config_path);
        runtime.load_plugins(plugin_dir);
        runtime.ensure_default_layout();
        runtime.sync_opts();
        Ok(runtime)
    }

    /// Emit a harness event (usable before and after [`Runtime::run`]).
    pub fn emit(&self, event: &str, fields: &[(&str, String)]) {
        self.inner.emit(event, fields);
    }

    /// The live UI state, shared with the renderer.
    pub fn state(&self) -> Rc<RefCell<UiState>> {
        self.inner.state.clone()
    }

    /// Evaluate a Lua chunk against the live runtime (embedder/test hook).
    pub fn eval(&self, chunk: &str) -> mlua::Result<()> {
        self.inner.lua.load(chunk).exec()
    }

    /// Run the harness loop until the session exits.
    pub fn run(self, session: Session, mut storage: Box<dyn SessionStorage>) -> io::Result<()> {
        let Self {
            inner,
            loop_handle,
            mut event_loop,
        } = self;

        let messages = storage.messages(&session.id).map_err(io::Error::other)?;
        let app = App::new(session, messages, inner.state.clone());

        let terminal = app::setup()?;
        let (sender, channel) = calloop::channel::channel::<TermEvent>();
        let reader_running = Arc::new(AtomicBool::new(true));
        spawn_input_reader(sender, reader_running.clone());

        let mut data = LoopData {
            inner,
            app,
            storage,
            terminal,
            dirty: false,
            running: true,
        };

        event_loop
            .handle()
            .insert_source(channel, |event, _meta, data: &mut LoopData| match event {
                calloop::channel::Event::Msg(event) => data.on_term_event(&event),
                calloop::channel::Event::Closed => data.running = false,
            })
            .map_err(|err| io::Error::other(format!("register input source: {err}")))?;

        app::draw(&mut data.terminal, &data.app)?;
        data.dirty = false;

        while data.running {
            event_loop
                .dispatch(None, &mut data)
                .map_err(io::Error::other)?;

            // Defer `uji.schedule` callbacks into idles for the next pass.
            for callback in data.inner.scheduled.take() {
                let _ = loop_handle.insert_idle(move |data: &mut LoopData| {
                    if let Err(err) = callback.call::<()>(()) {
                        // ast-grep-ignore: no-print-in-lib
                        eprintln!("uji: scheduled callback error: {err}");
                    }
                    data.dirty = true;
                });
            }

            if data.dirty {
                app::draw(&mut data.terminal, &data.app)?;
                data.dirty = false;
            }
        }

        reader_running.store(false, Ordering::Relaxed);
        data.inner.emit(events::QUIT, &[]);
        app::restore(&mut data.terminal)
    }

    fn run_init(&self, config_path: Option<PathBuf>) {
        let path = config_path.or_else(config::config_path);
        let (source, name) = match path {
            Some(path) => match std::fs::read_to_string(&path) {
                Ok(source) => (source, path.display().to_string()),
                Err(err) => {
                    // ast-grep-ignore: no-print-in-lib
                    eprintln!("uji: cannot read config {}: {err}", path.display());
                    return;
                }
            },
            None => (DEFAULT_LUA.to_owned(), "default.lua".to_owned()),
        };
        if let Err(err) = self.inner.lua.load(&source).set_name(&name).exec() {
            // ast-grep-ignore: no-print-in-lib
            eprintln!("uji: config error in {name}: {err}");
        }
    }

    fn load_plugins(&self, plugin_dir: Option<PathBuf>) {
        let Some(dir) = plugin_dir.or_else(config::plugin_dir) else {
            return;
        };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "lua"))
            .collect();
        paths.sort();
        for path in paths {
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            let name = path.display().to_string();
            if let Err(err) = self.inner.lua.load(&source).set_name(&name).exec() {
                // ast-grep-ignore: no-print-in-lib
                eprintln!("uji: plugin error in {name}: {err}");
            }
        }
    }

    fn ensure_default_layout(&self) {
        if self.inner.state.borrow().windows().is_empty() {
            let _ = self
                .inner
                .lua
                .load(DEFAULT_LUA)
                .set_name("default.lua")
                .exec();
        }
    }

    fn sync_opts(&self) {
        let cursor_blink = self
            .inner
            .lua
            .globals()
            .get::<Table>("uji")
            .and_then(|uji| uji.get::<Table>("opt"))
            .ok()
            .and_then(|opt| opt.get::<Option<bool>>("cursor_blink").ok().flatten())
            .unwrap_or(true);
        self.inner.state.borrow_mut().set_cursor_blink(cursor_blink);
    }
}

/// Reader thread: polls crossterm and forwards events to the loop channel.
fn spawn_input_reader(sender: calloop::channel::Sender<TermEvent>, running: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        while running.load(Ordering::Relaxed) {
            if crossterm::event::poll(INPUT_POLL).unwrap_or(false) {
                match crossterm::event::read() {
                    Ok(event) => {
                        if sender.send(event).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("uji-{name}-{nanos}"))
    }

    fn quiet_runtime() -> Runtime {
        let missing = tmp("missing.lua");
        let plugins = tmp("plugins");
        // ast-grep-ignore: no-expect-in-lib
        std::fs::create_dir_all(&plugins).expect("make plugin dir");
        // ast-grep-ignore: no-expect-in-lib
        Runtime::boot_in(Some(missing), Some(plugins)).expect("boot")
    }

    #[test]
    fn boot_falls_back_to_the_default_layout() {
        let runtime = quiet_runtime();
        assert_eq!(runtime.state().borrow().windows().len(), 2);
    }

    #[test]
    fn scheduled_callbacks_run_when_drained() {
        let runtime = quiet_runtime();
        // ast-grep-ignore: no-expect-in-lib
        runtime
            .eval("uji.schedule(function() uji.open_win(\"input\", { size = 5 }) end)")
            .expect("schedule");
        assert_eq!(runtime.state().borrow().windows().len(), 2);

        let callbacks = runtime.inner.scheduled.take();
        assert_eq!(callbacks.len(), 1);
        for callback in callbacks {
            // ast-grep-ignore: no-expect-in-lib
            callback.call::<()>(()).expect("callback runs");
        }
        assert_eq!(runtime.state().borrow().windows().len(), 3);
    }

    #[test]
    fn emit_reaches_registered_handlers() {
        let runtime = quiet_runtime();
        // ast-grep-ignore: no-expect-in-lib
        runtime
            .eval("uji.on(\"Ping\", function(name, ctx) GOT = ctx.x end)")
            .expect("on");
        // ast-grep-ignore: no-expect-in-lib
        runtime.eval("GOT = 0").expect("reset");
        runtime.emit("Ping", &[("x", "7".into())]);
        // ast-grep-ignore: no-expect-in-lib
        runtime
            .eval("assert(GOT == \"7\")")
            .expect("handler ran and set GOT");
    }
}
