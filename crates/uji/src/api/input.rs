use std::cell::{Cell, RefCell};
use std::rc::Rc;

use mlua::{Function, Lua, Table};

use crate::Api;
use crate::bind::bind;

#[derive(Default)]
pub struct Capture {
    handler: RefCell<Option<Function>>,
}

impl Capture {
    pub fn set(&self, handler: Function) {
        *self.handler.borrow_mut() = Some(handler);
    }

    pub fn clear(&self) {
        self.handler.borrow_mut().take();
    }

    pub fn handler(&self) -> Option<Function> {
        self.handler.borrow().clone()
    }

    pub fn is_active(&self) -> bool {
        self.handler.borrow().is_some()
    }
}

#[derive(Default)]
pub struct Composer {
    text: RefCell<String>,
    written_by_lua: Cell<bool>,
}

impl Composer {
    pub fn text(&self) -> String {
        self.text.borrow().clone()
    }

    pub fn set(&self, text: String) {
        *self.text.borrow_mut() = text;
        self.written_by_lua.set(true);
    }

    pub fn observe(&self, text: &str) {
        let mut current = self.text.borrow_mut();
        current.clear();
        current.push_str(text);
    }

    pub fn take_written(&self) -> Option<String> {
        self.written_by_lua
            .replace(false)
            .then(|| self.text.borrow().clone())
    }
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let input = lua.create_table()?;

    input.set(
        "get",
        bind(lua, api, move |api, _, ()| Ok(api.composer().text()))?,
    )?;

    input.set(
        "set",
        bind(lua, api, move |api, _, text: String| {
            api.composer().set(text);
            Ok(())
        })?,
    )?;

    input.set(
        "append",
        bind(lua, api, move |api, _, text: String| {
            let composer = api.composer();
            let mut next = composer.text();
            next.push_str(&text);
            composer.set(next);
            Ok(())
        })?,
    )?;

    input.set(
        "clear",
        bind(lua, api, move |api, _, ()| {
            api.composer().set(String::new());
            Ok(())
        })?,
    )?;

    input.set(
        "capture",
        bind(lua, api, move |api, _, handler: Function| {
            api.capture().set(handler);
            Ok(())
        })?,
    )?;

    input.set(
        "release",
        bind(lua, api, move |api, _, ()| {
            api.capture().clear();
            Ok(())
        })?,
    )?;

    Ok(input)
}
