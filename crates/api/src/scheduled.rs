use std::cell::RefCell;

use mlua::Function;

#[derive(Default)]
pub struct Scheduled {
    pending: RefCell<Vec<Function>>,
}

impl Scheduled {
    pub fn push(&self, function: Function) {
        self.pending.borrow_mut().push(function);
    }

    pub fn take(&self) -> Vec<Function> {
        std::mem::take(&mut *self.pending.borrow_mut())
    }
}
