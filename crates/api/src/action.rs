use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use mlua::{Function, Lua, Table};

use crate::Api;
use crate::bind::bind;

#[derive(Default)]
pub struct Actions {
    reserved: RefCell<HashSet<String>>,
    registered: RefCell<HashMap<String, Function>>,
}

impl Actions {
    pub fn reserve(&self, names: impl IntoIterator<Item = String>) {
        self.reserved.borrow_mut().extend(names);
    }

    pub fn is_reserved(&self, name: &str) -> bool {
        self.reserved.borrow().contains(name)
    }

    pub fn get(&self, name: &str) -> Option<Function> {
        self.registered.borrow().get(name).cloned()
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.reserved.borrow().iter().cloned().collect();
        names.extend(self.registered.borrow().keys().cloned());
        names.sort();
        names.dedup();
        names
    }
}

pub(crate) fn register(lua: &Lua, api: &Rc<Api>) -> mlua::Result<Table> {
    let action = lua.create_table()?;

    action.set(
        "set",
        bind(
            lua,
            api,
            move |api, _, (name, handler): (String, Function)| {
                if api.actions().is_reserved(&name) {
                    return Err(mlua::Error::runtime(format!(
                        "action {name} is built in and cannot be replaced"
                    )));
                }
                api.actions().registered.borrow_mut().insert(name, handler);
                Ok(())
            },
        )?,
    )?;

    action.set(
        "del",
        bind(lua, api, move |api, _, name: String| {
            api.actions().registered.borrow_mut().remove(&name);
            Ok(())
        })?,
    )?;

    action.set(
        "list",
        bind(lua, api, move |api, lua, ()| {
            let out = lua.create_table()?;
            for name in api.actions().names() {
                out.push(name)?;
            }
            Ok(out)
        })?,
    )?;

    Ok(action)
}
