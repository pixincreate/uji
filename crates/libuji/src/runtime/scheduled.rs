use std::cell::RefCell;

use mlua::Function;

#[derive(Default)]
pub(crate) struct Scheduled {
    pending: RefCell<Vec<Function>>,
}

impl Scheduled {
    pub(crate) fn push(&self, function: Function) {
        self.pending.borrow_mut().push(function);
    }

    pub(crate) fn take(&self) -> Vec<Function> {
        std::mem::take(&mut *self.pending.borrow_mut())
    }
}
