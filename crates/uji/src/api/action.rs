use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use mlua::{Function, Lua, Table};

use crate::api::Api;
use crate::api::bind::bind;

#[derive(Default)]
pub struct Actions {
    reserved: HashSet<String>,
    registered: HashMap<String, Function>,
}

impl Actions {
    pub fn reserve(&mut self, names: impl IntoIterator<Item = String>) {
        self.reserved.extend(names);
    }

    fn is_reserved(&self, name: &str) -> bool {
        self.reserved.contains(name)
    }

    pub fn add(&mut self, name: String, handler: Function) {
        self.registered.insert(name, handler);
    }

    fn remove(&mut self, name: &str) -> bool {
        self.registered.remove(name).is_some()
    }

    pub fn get(&self, name: &str) -> Option<Function> {
        self.registered.get(name).cloned()
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.reserved.iter().cloned().collect();
        names.extend(self.registered.keys().cloned());
        names.sort();
        names.dedup();
        names
    }
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let action = lua.create_table()?;

    action.set(
        "add",
        bind(
            lua,
            api,
            move |api, _, (name, handler): (String, Function)| {
                let mut actions = api.actions().borrow_mut();
                if actions.is_reserved(&name) {
                    return Err(mlua::Error::runtime(format!(
                        "action {name} is built in and cannot be replaced"
                    )));
                }
                actions.add(name, handler);
                Ok(())
            },
        )?,
    )?;

    action.set(
        "remove",
        bind(lua, api, move |api, _, name: String| {
            Ok(api.actions().borrow_mut().remove(&name))
        })?,
    )?;

    action.set(
        "list",
        bind(lua, api, move |api, _, ()| {
            Ok(api.actions().borrow().names())
        })?,
    )?;

    Ok(action)
}
