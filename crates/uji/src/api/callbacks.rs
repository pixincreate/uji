use std::collections::HashMap;
use std::rc::Rc;

use mlua::{Function, Lua};

use crate::api::Api;
use crate::api::bind::bind;
use crate::api::request::Request;

#[derive(Default)]
pub struct Callbacks {
    waiting: HashMap<u64, Function>,
    next: u64,
}

impl Callbacks {
    pub fn open(&mut self, callback: Function) -> u64 {
        self.next = self.next.saturating_add(1);
        self.waiting.insert(self.next, callback);
        self.next
    }

    pub fn take(&mut self, id: u64) -> Option<Function> {
        self.waiting.remove(&id)
    }
}

pub(crate) fn canceller(lua: &Lua, api: &Rc<Api>, id: u64) -> mlua::Result<Function> {
    bind(lua, api, move |api, _, ()| {
        if api.callbacks().borrow_mut().take(id).is_some() {
            api.request(Request::Stop(id));
        }
        Ok(())
    })
}
