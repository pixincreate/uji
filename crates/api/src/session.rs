use std::cell::{Ref, RefCell};
use std::rc::Rc;

use mlua::{Lua, Table};

use crate::Api;

#[derive(Default, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub directory: String,
}

#[derive(Clone)]
pub struct MessageView {
    pub kind: String,
    pub text: String,
    pub name: Option<String>,
}

#[derive(Default)]
pub struct SessionState {
    info: RefCell<SessionInfo>,
    messages: RefCell<Vec<MessageView>>,
    submits: RefCell<Vec<String>>,
}

impl SessionState {
    pub fn set_info(&self, info: SessionInfo) {
        *self.info.borrow_mut() = info;
    }

    pub fn set_messages(&self, messages: Vec<MessageView>) {
        *self.messages.borrow_mut() = messages;
    }

    pub fn push_message(&self, message: MessageView) {
        self.messages.borrow_mut().push(message);
    }

    pub fn queue_submit(&self, text: String) {
        self.submits.borrow_mut().push(text);
    }

    pub fn info(&self) -> SessionInfo {
        self.info.borrow().clone()
    }

    pub fn messages(&self) -> Ref<'_, Vec<MessageView>> {
        self.messages.borrow()
    }

    pub fn take_submits(&self) -> Vec<String> {
        std::mem::take(&mut *self.submits.borrow_mut())
    }
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let session = lua.create_table()?;

    let info_api = Rc::clone(api);
    session.set(
        "info",
        lua.create_function(move |lua, ()| {
            let info = info_api.session().info();
            let out = lua.create_table()?;
            out.set("id", info.id)?;
            out.set("title", info.title)?;
            out.set("directory", info.directory)?;
            Ok(out)
        })?,
    )?;

    let messages_api = Rc::clone(api);
    session.set(
        "messages",
        lua.create_function(move |lua, ()| {
            let out = lua.create_table()?;
            for message in messages_api.session().messages().iter() {
                let row = lua.create_table()?;
                row.set("type", message.kind.clone())?;
                row.set("text", message.text.clone())?;
                if let Some(name) = &message.name {
                    row.set("name", name.clone())?;
                }
                out.push(row)?;
            }
            Ok(out)
        })?,
    )?;

    let submit_api = Rc::clone(api);
    session.set(
        "submit",
        lua.create_function(move |_, text: String| {
            if text.trim().is_empty() {
                return Err(mlua::Error::runtime("submit needs non-empty text"));
            }
            submit_api.session().queue_submit(text);
            Ok(())
        })?,
    )?;

    Ok(session)
}
