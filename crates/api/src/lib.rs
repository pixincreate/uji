pub mod command;
mod convert;
pub mod event;
pub mod handlers;
pub mod llm;
pub mod model;
pub mod schedule;
pub mod scheduled;
pub mod state;
pub mod window;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use mlua::{Function, Lua, Table};

use crate::state::UiState;

use self::handlers::Handlers;
use self::scheduled::Scheduled;

pub struct Api {
    state: Rc<RefCell<UiState>>,
    scheduled: Scheduled,
    handlers: RefCell<Handlers>,
    commands: RefCell<HashMap<String, Function>>,
}

impl Api {
    pub fn new(state: Rc<RefCell<UiState>>) -> Rc<Self> {
        Rc::new(Self {
            state,
            scheduled: Scheduled::default(),
            handlers: RefCell::default(),
            commands: RefCell::default(),
        })
    }

    pub fn state(&self) -> Rc<RefCell<UiState>> {
        self.state.clone()
    }

    pub fn scheduled(&self) -> &Scheduled {
        &self.scheduled
    }

    pub fn commands(&self) -> &RefCell<HashMap<String, Function>> {
        &self.commands
    }

    pub(crate) fn handlers(&self) -> &RefCell<Handlers> {
        &self.handlers
    }

    pub fn dispatch(&self, event: &str, ctx: &Table) {
        for handler in self.handlers.borrow().get(event) {
            if let Err(err) = handler.call::<()>((event, ctx.clone())) {
                eprintln!("uji: handler error for {event}: {err}");
            }
        }
    }
}

pub fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let uji = lua.create_table()?;

    let ui = lua.create_table()?;
    ui.set("open_win", window::open_win(lua, api)?)?;
    ui.set("close_win", window::close_win(lua, api)?)?;
    uji.set("ui", ui)?;

    let llm = lua.create_table()?;
    llm.set("current_provider", llm::current_provider(lua, api)?)?;
    llm.set("current_model", llm::current_model(lua, api)?)?;
    uji.set("llm", llm)?;

    uji.set("schedule", schedule::schedule(lua, api)?)?;
    uji.set("on", event::on(lua, api)?)?;
    uji.set("emit", event::emit(lua, api)?)?;
    uji.set("notify", event::notify(lua)?)?;
    uji.set("command", command::command(lua, api)?)?;

    Ok(uji)
}
