use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use crate::api::Api;
use mlua::{FromLuaMulti, Function, Lua as LuaState, LuaSerdeExt, Value as LuaValue};
use uji_agent::session::conversation::Shared;
use uji_ui::state::UiState;

use super::events;
use super::policy;
use crate::pack;
use std::collections::BTreeSet;
use uji_agent::config;
use uji_agent::llm::{Catalog, Dispatch, Llm, NotConfigured, Provider};
use uji_agent::session::store::SessionStorage;

use uji_agent::tools::policy::ToolPolicy;

use super::loader;

const DEFAULTS: &str = "uji.defaults";

const TOOLS: &str = "uji.tools";

const WIRES: &str = "uji.wires";

const PROVIDERS: &str = "uji.providers";

pub(crate) struct Inner {
    pub(crate) lua: LuaState,
    pub(crate) api: Rc<Api>,
    pub(crate) llm: RefCell<Arc<Llm>>,
    pub(crate) llm_model: RefCell<String>,
    pub(crate) llm_provider: RefCell<String>,
    pub(crate) llm_effort: RefCell<uji_agent::llm::Effort>,
    pub(crate) llm_cache: RefCell<uji_agent::llm::Retention>,
    pub(crate) client: Arc<reqwest::Client>,
    pub(crate) policy: RefCell<ToolPolicy>,
    pub(crate) dispatch: Dispatch,
}

impl Inner {
    pub(crate) fn boot(
        state: Rc<RefCell<UiState>>,
        conversation: Shared,
        client: Arc<reqwest::Client>,
        config_dir: Option<PathBuf>,
        dispatch: Dispatch,
    ) -> Rc<Self> {
        let inner = Rc::new(Self {
            lua: LuaState::new(),
            api: Api::new(state, conversation),
            llm: RefCell::new(Arc::new(Llm::NotConfigured(NotConfigured))),
            llm_model: RefCell::default(),
            llm_provider: RefCell::default(),
            llm_effort: RefCell::default(),
            llm_cache: RefCell::default(),
            client,
            policy: RefCell::new(ToolPolicy::default()),
            dispatch,
        });

        let dir = config_dir.or_else(config::config_dir);
        if let Some(dir) = dir {
            inner.api.packs().borrow_mut().push(dir);
        }

        match crate::api::register(&inner.lua, &inner.api) {
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
            .reserve(uji_ui::app::Action::names().map(ToString::to_string));
        inner.load(WIRES);
        inner.load_providers();
        inner.load(TOOLS);
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
        let known = self.tool_names();
        let (policy, notices) = policy::compile(&self.lua, &known);
        *self.policy.borrow_mut() = policy;
        for notice in notices {
            self.report(notice);
        }
    }

    fn tool_names(&self) -> BTreeSet<String> {
        self.api.tools().borrow().keys().cloned().collect()
    }

    pub(crate) fn state(&self) -> Rc<RefCell<UiState>> {
        self.api.state()
    }

    pub(crate) fn ask(&self, event: &str, fields: &[(&str, String)]) -> Option<mlua::Value> {
        let ctx = self.lua.create_table().ok()?;
        for (key, value) in fields {
            let _ = ctx.set(*key, value.clone());
        }
        self.api.ask(event, &ctx)
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
        match path {
            Some(path) => self.source(&path),
            None => self.load(DEFAULTS),
        }
    }

    pub(crate) fn require<T: FromLuaMulti>(&self, module: &str) -> mlua::Result<T> {
        self.lua.globals().get::<Function>("require")?.call(module)
    }

    fn load_providers(&self) {
        let builtin = self
            .require::<LuaValue>(PROVIDERS)
            .and_then(|value| self.lua.from_value::<Vec<Provider>>(value));
        match builtin {
            Ok(builtin) => *self.api.providers().borrow_mut() = Catalog::new(builtin),
            Err(err) => self.report(format!("{PROVIDERS}: {err}")),
        }
    }

    fn load(&self, module: &str) {
        if let Err(err) = self.require::<()>(module) {
            self.report(format!("{module}: {err}"));
        }
    }

    fn source_plugins(&self) {
        let roots = self.api.packs().borrow().clone();
        for root in roots {
            for path in plugin_files(&root.join(config::PLUGIN_DIR)) {
                self.source(&path);
            }
        }
    }

    fn source(&self, path: &Path) {
        let name = path.display().to_string();
        match std::fs::read_to_string(path) {
            Ok(source) => {
                if let Err(err) = self.lua.load(&source).set_name(&name).exec() {
                    self.report(format!("{name}: {err}"));
                }
            }
            Err(err) => self.report(format!("cannot read {name}: {err}")),
        }
    }

    pub(crate) fn resolve_llm(&self, storage: &mut dyn SessionStorage) {
        let selection = {
            let catalog = self.api.providers().borrow();
            uji_agent::llm::resolve_from_storage(storage, &catalog, &self.dispatch)
        };
        *self.llm.borrow_mut() = selection.llm;
        self.llm_model.borrow_mut().clone_from(&selection.model);
        self.llm_provider.borrow_mut().clone_from(&selection.id);
        *self.llm_effort.borrow_mut() = selection.effort;
        *self.llm_cache.borrow_mut() = selection.cache;

        let window = {
            let catalog = self.api.providers().borrow();
            catalog
                .get(&selection.id)
                .and_then(|provider| provider.budget(&selection.model))
                .map(|budget| budget.window)
        };
        let state = self.state();
        let mut state = state.borrow_mut();
        state.set_context_window(window);
        state.set_current_provider(selection.name);
        state.set_current_model(selection.model);
        state.set_current_effort(
            selection
                .effort
                .enabled()
                .then(|| selection.effort.to_string()),
        );
        drop(state);
        self.emit(
            events::Event::ModelChanged.name(),
            &[
                ("provider", self.llm_provider.borrow().clone()),
                ("model", self.llm_model.borrow().clone()),
            ],
        );
    }
}

fn plugin_files(dir: &Path) -> Vec<PathBuf> {
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
