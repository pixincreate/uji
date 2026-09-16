use std::cell::RefCell;

use mlua::Function;

pub struct Entry {
    pub name: String,
    pub priority: i64,
    pub call: Function,
}

#[derive(Default)]
pub struct Registry {
    entries: RefCell<Vec<Entry>>,
}

impl Registry {
    pub fn add(&self, entry: Entry) {
        let mut entries = self.entries.borrow_mut();
        entries.retain(|existing| existing.name != entry.name);
        entries.push(entry);
        entries.sort_by_key(|entry| entry.priority);
    }

    pub fn remove(&self, name: &str) {
        self.entries.borrow_mut().retain(|entry| entry.name != name);
    }

    pub fn calls(&self) -> Vec<(String, Function)> {
        self.entries
            .borrow()
            .iter()
            .map(|entry| (entry.name.clone(), entry.call.clone()))
            .collect()
    }
}
