//! The `uji.schedule` queue: callbacks awaiting a safe point on the loop.

use std::cell::RefCell;

use mlua::Function;

/// `uji.schedule` callbacks awaiting a safe point on the loop.
#[derive(Default)]
pub(crate) struct Scheduled {
    pending: RefCell<Vec<Function>>,
}

impl Scheduled {
    /// Queue a callback to run at the next loop drain.
    pub(crate) fn push(&self, function: Function) {
        self.pending.borrow_mut().push(function);
    }

    /// Take all queued callbacks.
    pub(crate) fn take(&self) -> Vec<Function> {
        std::mem::take(&mut *self.pending.borrow_mut())
    }
}
