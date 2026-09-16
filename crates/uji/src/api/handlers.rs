use mlua::Function;

pub const DEFAULT_PRIORITY: i64 = 50;

struct Handler {
    priority: i64,
    call: Function,
}

#[derive(Default)]
pub struct Handlers {
    by_event: Vec<(String, Vec<Handler>)>,
}

impl Handlers {
    pub fn add(&mut self, event: String, handler: Function, priority: i64) {
        let entry = Handler {
            priority,
            call: handler,
        };
        if let Some((_, list)) = self.by_event.iter_mut().find(|(name, _)| *name == event) {
            list.push(entry);
            list.sort_by_key(|handler| handler.priority);
        } else {
            self.by_event.push((event, vec![entry]));
        }
    }

    pub fn has(&self, event: &str) -> bool {
        self.by_event
            .iter()
            .any(|(name, list)| name == event && !list.is_empty())
    }

    pub fn get(&self, event: &str) -> Vec<Function> {
        self.by_event
            .iter()
            .find(|(name, _)| name == event)
            .map(|(_, list)| list.iter().map(|handler| handler.call.clone()).collect())
            .unwrap_or_default()
    }
}
