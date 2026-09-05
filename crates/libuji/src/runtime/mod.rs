//! The persistent harness runtime — nvim's main loop, executor and editor
//! state rolled into one.
//!
//! Mapping to nvim: `uji.schedule` → calloop `insert_idle`; `uji.on`/`emit`
//! → synchronous autocmds; input reader + channel → the multiqueue wakeup;
//! post-batch redraw → flushing before it blocks again. A file watcher
//! re-sources config on change (hot reload).

mod error;
pub mod events;
mod handlers;
mod inner;
mod input;
mod loop_data;
mod scheduled;
mod watcher;

pub use error::RuntimeError;
pub(crate) use inner::Inner;
pub(crate) use loop_data::LoopData;

use std::cell::RefCell;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use calloop::{EventLoop, LoopHandle};
use crossterm::event::Event as TermEvent;
use mlua::Lua as LuaState;
use tui::app::{self, App};
use tui::state::UiState;
use uji_core::session::model::Session;
use uji_core::session::store::SessionStorage;

use crate::config::{self};
use crate::lua::functions::register_all;

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
        let uji = register_all(&inner.lua, &inner)?;
        if let Ok(opt) = inner.lua.create_table() {
            let _ = opt.set("cursor_blink", true);
            let _ = uji.set("opt", opt);
        }
        let _ = inner.lua.globals().set("uji", uji);

        let event_loop = EventLoop::try_new()?;
        let loop_handle = event_loop.handle();

        inner.run_init(config_path);
        inner.load_plugins(plugin_dir);
        inner.ensure_default_layout();
        inner.sync_opts();

        Ok(Self {
            inner,
            loop_handle,
            event_loop,
        })
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
        input::spawn(sender, reader_running.clone());

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

        // Hot reload: watch config/plugins and re-source on change. The
        // watcher is dropped at the end of this function, which stops it.
        let _watcher = {
            let paths = config::watch_paths();
            if paths.is_empty() {
                None
            } else {
                let (sender, channel) = calloop::channel::channel::<watcher::ConfigEvent>();
                match watcher::watch(paths, sender) {
                    Ok(watcher) => {
                        event_loop
                            .handle()
                            .insert_source(channel, |event, _meta, data: &mut LoopData| {
                                if let calloop::channel::Event::Msg(watcher::ConfigEvent::Reload) =
                                    event
                                {
                                    data.reload();
                                }
                            })
                            .map_err(|err| {
                                io::Error::other(format!("register config watcher: {err}"))
                            })?;
                        Some(watcher)
                    }
                    Err(err) => {
                        // ast-grep-ignore: no-print-in-lib
                        eprintln!("uji: config watcher failed: {err}");
                        None
                    }
                }
            }
        };

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
