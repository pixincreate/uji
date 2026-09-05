//! The `uji.on` handler registry.

use mlua::Function;

/// `uji.on` handlers, grouped by event name.
#[derive(Default)]
pub(crate) struct Handlers {
    by_event: Vec<(String, Vec<Function>)>,
}

impl Handlers {
    /// Register a handler for `event`.
    pub(crate) fn add(&mut self, event: String, handler: Function) {
        if let Some((_, list)) = self.by_event.iter_mut().find(|(name, _)| *name == event) {
            list.push(handler);
        } else {
            self.by_event.push((event, vec![handler]));
        }
    }

    /// Handlers registered for `event`, cloned out so the borrow is released
    /// before dispatch (handlers may re-enter).
    pub(crate) fn get(&self, event: &str) -> Vec<Function> {
        self.by_event
            .iter()
            .find(|(name, _)| name == event)
            .map(|(_, list)| list.clone())
            .unwrap_or_default()
    }
}
