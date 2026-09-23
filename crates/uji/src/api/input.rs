use std::rc::Rc;

use mlua::{Function, Lua, Table};

use crate::api::Api;
use crate::api::bind::bind;

#[derive(Default)]
pub struct Composer {
    text: String,
    written_by_lua: bool,
}

impl Composer {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set(&mut self, text: String) {
        self.text = text;
        self.written_by_lua = true;
    }

    pub fn observe(&mut self, text: &str) {
        self.text.clear();
        self.text.push_str(text);
    }

    pub fn take_written(&mut self) -> Option<String> {
        std::mem::take(&mut self.written_by_lua).then(|| self.text.clone())
    }
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let input = lua.create_table()?;

    input.set(
        "get",
        bind(lua, api, move |api, _, ()| {
            Ok(api.composer().borrow().text().to_string())
        })?,
    )?;

    input.set(
        "set",
        bind(lua, api, move |api, _, text: String| {
            api.composer().borrow_mut().set(text);
            Ok(())
        })?,
    )?;

    input.set(
        "append",
        bind(lua, api, move |api, _, text: String| {
            let mut composer = api.composer().borrow_mut();
            let next = format!("{}{text}", composer.text());
            composer.set(next);
            Ok(())
        })?,
    )?;

    input.set(
        "clear",
        bind(lua, api, move |api, _, ()| {
            api.composer().borrow_mut().set(String::new());
            Ok(())
        })?,
    )?;

    input.set(
        "capture",
        bind(lua, api, move |api, _, handler: Function| {
            *api.capture().borrow_mut() = Some(handler);
            Ok(())
        })?,
    )?;

    input.set(
        "release",
        bind(lua, api, move |api, _, ()| {
            api.capture().borrow_mut().take();
            Ok(())
        })?,
    )?;

    Ok(input)
}
