use std::cell::{Cell, RefCell};
use std::rc::Rc;

use mlua::{Function, Lua, Table};

use crate::Api;

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

    let get_api = Rc::clone(api);
    input.set(
        "get",
        lua.create_function(move |_, ()| Ok(get_api.composer().text()))?,
    )?;

    let set_api = Rc::clone(api);
    input.set(
        "set",
        lua.create_function(move |_, text: String| {
            set_api.composer().set(text);
            Ok(())
        })?,
    )?;

    let append_api = Rc::clone(api);
    input.set(
        "append",
        lua.create_function(move |_, text: String| {
            let composer = append_api.composer();
            let mut next = composer.text();
            next.push_str(&text);
            composer.set(next);
            Ok(())
        })?,
    )?;

    let clear_api = Rc::clone(api);
    input.set(
        "clear",
        lua.create_function(move |_, ()| {
            clear_api.composer().set(String::new());
            Ok(())
        })?,
    )?;

    let capture_api = Rc::clone(api);
    input.set(
        "capture",
        lua.create_function(move |_, handler: Function| {
            capture_api.capture().set(handler);
            Ok(())
        })?,
    )?;

    let release_api = Rc::clone(api);
    input.set(
        "release",
        lua.create_function(move |_, ()| {
            release_api.capture().clear();
            Ok(())
        })?,
    )?;

    Ok(input)
}
