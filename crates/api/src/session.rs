use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, Table};
use uji_core::llm::Usage;
use uji_core::session::conversation::Shared;

use crate::Api;
use crate::bind::bind;

pub struct SessionState {
    conversation: Shared,
    submits: RefCell<Vec<String>>,
    titles: RefCell<Vec<String>>,
}

impl SessionState {
    pub fn new(conversation: Shared) -> Self {
        Self {
            conversation,
            submits: RefCell::default(),
            titles: RefCell::default(),
        }
    }

    pub fn conversation(&self) -> &Shared {
        &self.conversation
    }

    pub fn set_title(&self, title: String) {
        self.conversation.borrow_mut().set_title(title.clone());
        self.titles.borrow_mut().push(title);
    }

    pub fn take_titles(&self) -> Vec<String> {
        std::mem::take(&mut *self.titles.borrow_mut())
    }

    pub fn queue_submit(&self, text: String) {
        self.submits.borrow_mut().push(text);
    }

    pub fn add_usage(&self, usage: Usage) {
        self.conversation.borrow_mut().add_usage(usage);
    }

    pub fn take_submits(&self) -> Vec<String> {
        std::mem::take(&mut *self.submits.borrow_mut())
    }
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let session = lua.create_table()?;

    session.set(
        "info",
        bind(lua, api, move |api, lua, ()| {
            let conversation = api.session().conversation().borrow();
            let info = conversation.info();
            let out = lua.create_table()?;
            out.set("id", info.id.clone())?;
            out.set("title", info.title.clone())?;
            out.set("directory", info.directory.clone())?;
            Ok(out)
        })?,
    )?;

    session.set(
        "messages",
        bind(lua, api, move |api, lua, ()| {
            let out = lua.create_table()?;
            let conversation = api.session().conversation().borrow();
            for stored in conversation.messages() {
                let row = lua.create_table()?;
                row.set("type", stored.message.type_name())?;
                row.set("text", stored.message.text())?;
                if let uji_core::session::model::Message::Tool { name, .. } = &stored.message {
                    row.set("name", name.clone())?;
                }
                out.push(row)?;
            }
            Ok(out)
        })?,
    )?;

    session.set(
        "usage",
        bind(lua, api, move |api, lua, ()| {
            let tally = api.session().conversation().borrow().tally();
            let out = lua.create_table()?;
            out.set("input", tally.usage.input)?;
            out.set("output", tally.usage.output)?;
            out.set("cache_read", tally.usage.cache_read)?;
            out.set("cache_write", tally.usage.cache_write)?;
            out.set("total", tally.usage.total())?;
            out.set("requests", tally.turns)?;
            Ok(out)
        })?,
    )?;

    session.set(
        "set_title",
        bind(lua, api, move |api, _, title: String| {
            let title = title.trim().to_string();
            if title.is_empty() {
                return Err(mlua::Error::runtime("set_title needs non-empty text"));
            }
            api.session().set_title(title);
            Ok(())
        })?,
    )?;

    session.set(
        "submit",
        bind(lua, api, move |api, _, text: String| {
            if text.trim().is_empty() {
                return Err(mlua::Error::runtime("submit needs non-empty text"));
            }
            api.session().queue_submit(text);
            Ok(())
        })?,
    )?;

    session.set(
        "interrupt",
        bind(lua, api, move |api, _, ()| {
            api.queue_interrupt();
            Ok(())
        })?,
    )?;

    Ok(session)
}
