//! Loop-independent runtime guts, shared with Lua closures.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use mlua::{Lua as LuaState, Table};
use tui::state::UiState;

use super::handlers::Handlers;
use super::scheduled::Scheduled;
use crate::config::{self, DEFAULT_LUA};

/// The loop-independent core: Lua state, live UI state, handlers and the
/// schedule queue. Shared as an `Rc` between the loop and the Lua closures.
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

    /// Re-source config: clear the live state, re-run init.lua + plugins,
    /// re-apply options. Called on file changes (hot reload).
    pub(crate) fn reload(&self) {
        self.state.borrow_mut().clear();
        self.run_init(None);
        self.load_plugins(None);
        self.ensure_default_layout();
        self.sync_opts();
    }

    /// Run the resolved init.lua (or the embedded default) against the live
    /// state.
    pub(crate) fn run_init(&self, config_path: Option<PathBuf>) {
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
        if let Err(err) = self.lua.load(&source).set_name(&name).exec() {
            // ast-grep-ignore: no-print-in-lib
            eprintln!("uji: config error in {name}: {err}");
        }
    }

    /// Load all `*.lua` files from the plugin directory, sorted by name.
    pub(crate) fn load_plugins(&self, plugin_dir: Option<PathBuf>) {
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
            if let Err(err) = self.lua.load(&source).set_name(&name).exec() {
                // ast-grep-ignore: no-print-in-lib
                eprintln!("uji: plugin error in {name}: {err}");
            }
        }
    }

    /// If no windows were declared, run the embedded default layout.
    pub(crate) fn ensure_default_layout(&self) {
        if self.state.borrow().windows().is_empty() {
            let _ = self.lua.load(DEFAULT_LUA).set_name("default.lua").exec();
        }
    }

    /// Copy the live `uji.opt` table into the renderer's option state.
    pub(crate) fn sync_opts(&self) {
        let cursor_blink = self
            .lua
            .globals()
            .get::<Table>("uji")
            .and_then(|uji| uji.get::<Table>("opt"))
            .ok()
            .and_then(|opt| opt.get::<Option<bool>>("cursor_blink").ok().flatten())
            .unwrap_or(true);
        self.state.borrow_mut().set_cursor_blink(cursor_blink);
    }
}
