use uji_core::session::model::Message;

use super::LoopData;
use crate::runtime::events;

impl LoopData {
    pub(super) fn enqueue(&mut self, text: &str) {
        self.turn.queued.push_back(text.to_string());
        self.sync_queue();
        self.dirty = true;
    }

    pub(super) fn steer(&mut self, id: u64) {
        let next = self.turn.queued.pop_front();
        if let Some(text) = next.clone() {
            self.app.take_pending();
            self.append(Message::User { text });
            self.sync_queue();
        }
        self.answer(id, next);
    }

    pub(super) fn send_queued(&mut self) {
        let Some(text) = self.turn.queued.pop_front() else {
            return;
        };
        self.sync_queue();
        self.submit(&text);
    }

    pub(super) fn sync_queue(&mut self) {
        let queued: Vec<String> = self.turn.queued.iter().cloned().collect();
        let count = queued.len();
        self.inner.state().borrow_mut().set_queued(queued);
        self.inner.emit(&events::QueueChanged { count });
    }
}
