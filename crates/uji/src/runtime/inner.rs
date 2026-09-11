use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use mlua::Lua as LuaState;
use uji_api::Api;
use uji_core::session::conversation::Shared;
use uji_screen::state::UiState;

use super::policy;
use crate::pack;
use uji_core::config::{self, DEFAULT_LUA};
use uji_core::llm::{Llm, NotConfigured};
use uji_core::session::store::SessionStorage;
use uji_core::tools::policy::ToolPolicy;

use super::loader;

pub(crate) struct Inner {
    pub(crate) lua: LuaState,
    pub(crate) api: Rc<Api>,
    pub(crate) llm: RefCell<Arc<dyn Llm>>,
    pub(crate) llm_model: RefCell<String>,
    pub(crate) client: Arc<reqwest::Client>,
    pub(crate) policy: RefCell<ToolPolicy>,
}

impl Inner {
    pub(crate) fn boot(
        state: Rc<RefCell<UiState>>,
        conversation: Shared,
        client: Arc<reqwest::Client>,
        config_dir: Option<PathBuf>,
    ) -> Rc<Self> {
        let inner = Rc::new(Self {
            lua: LuaState::new(),
            api: Api::new(state, conversation),
            llm: RefCell::new(Arc::new(NotConfigured)),
            llm_model: RefCell::default(),
            client,
            policy: RefCell::new(ToolPolicy::default()),
        });

        let dir = config_dir.or_else(config::config_dir);
        if let Some(dir) = dir {
            inner.api.packs().borrow_mut().push(dir);
        }

        match uji_api::register(&inner.lua, &inner.api) {
            Ok(uji) => {
                match pack::register(&inner.lua, &inner.api) {
                    Ok(table) => {
                        if let Err(err) = uji.set("pack", table) {
                            inner.report(format!("cannot expose uji.pack: {err}"));
                        }
                    }
                    Err(err) => inner.report(format!("cannot build uji.pack: {err}")),
                }
                if let Err(err) = inner.lua.globals().set("uji", uji) {
                    inner.report(format!("cannot expose the uji table: {err}"));
                }
            }
            Err(err) => inner.report(format!("cannot build the uji table: {err}")),
        }
        if let Err(err) = loader::install(&inner.lua, &inner.api) {
            inner.report(format!("cannot install the module loader: {err}"));
        }

        inner
            .api
            .actions()
            .reserve(uji_tui::app::Action::names().map(ToString::to_string));
        inner.run_init();
        inner.source_plugins();
        inner.compile_policy();
        inner
    }

    pub(crate) fn report(&self, message: String) {
        self.api.notify(message);
    }

    pub(crate) fn take_diagnostics(&self) -> Vec<String> {
        self.api.take_notices()
    }

    pub(crate) fn compile_policy(&self) {
        let (policy, notices) = policy::compile(&self.lua);
        *self.policy.borrow_mut() = policy;
        for notice in notices {
            self.report(notice);
        }
    }

    pub(crate) fn state(&self) -> Rc<RefCell<UiState>> {
        self.api.state()
    }

    pub(crate) fn emit(&self, event: &str, fields: &[(&str, String)]) {
        let Ok(ctx) = self.lua.create_table() else {
            return;
        };
        for (key, value) in fields {
            let _ = ctx.set(*key, value.clone());
        }
        self.api.dispatch(event, &ctx);
    }

    fn run_init(&self) {
        let path = self
            .api
            .packs()
            .borrow()
            .first()
            .and_then(|dir| config::init_path(dir));
        let (source, name) = match path {
            Some(path) => match std::fs::read_to_string(&path) {
                Ok(source) => (source, path.display().to_string()),
                Err(err) => {
                    self.report(format!("cannot read {}: {err}", path.display()));
                    return;
                }
            },
            None => (DEFAULT_LUA.to_owned(), String::from("default.lua")),
        };
        if let Err(err) = self.lua.load(&source).set_name(&name).exec() {
            self.report(format!("{name}: {err}"));
        }
    }

    fn source_plugins(&self) {
        let roots = self.api.packs().borrow().clone();
        for root in roots {
            for path in plugin_files(&root.join(config::PLUGIN_DIR)) {
                match std::fs::read_to_string(&path) {
                    Ok(source) => {
                        let name = path.display().to_string();
                        if let Err(err) = self.lua.load(&source).set_name(&name).exec() {
                            self.report(format!("{name}: {err}"));
                        }
                    }
                    Err(err) => self.report(format!("cannot read {}: {err}", path.display())),
                }
            }
        }
    }

    pub(crate) fn resolve_llm(&self, storage: &mut dyn SessionStorage) {
        let selection = uji_core::llm::resolve_from_storage(storage);
        *self.llm.borrow_mut() = selection.llm;
        self.llm_model.borrow_mut().clone_from(&selection.model);

        let state = self.state();
        let mut state = state.borrow_mut();
        state.set_current_provider(selection.provider);
        state.set_current_model(selection.model);
    }
}

fn plugin_files(dir: &std::path::Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "lua"))
        .collect();
    paths.sort();
    paths
}
