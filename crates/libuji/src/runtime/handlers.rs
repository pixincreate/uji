use mlua::Function;

#[derive(Default)]
pub(crate) struct Handlers {
    by_event: Vec<(String, Vec<Function>)>,
}

impl Handlers {
    pub(crate) fn add(&mut self, event: String, handler: Function) {
        if let Some((_, list)) = self.by_event.iter_mut().find(|(name, _)| *name == event) {
            list.push(handler);
        } else {
            self.by_event.push((event, vec![handler]));
        }
    }

    pub(crate) fn get(&self, event: &str) -> Vec<Function> {
        self.by_event
            .iter()
            .find(|(name, _)| name == event)
            .map(|(_, list)| list.clone())
            .unwrap_or_default()
    }
}
