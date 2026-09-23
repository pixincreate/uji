use uji_agent::session::model::Message;

use super::LoopData;
use crate::runtime::events;

impl LoopData {
    pub(super) fn enqueue(&mut self, text: &str) {
        self.queued.push_back(text.to_string());
        self.sync_queue();
        self.dirty = true;
    }

    pub(super) fn steer(&mut self, id: u64) {
        let next = self.queued.pop_front();
        if let Some(text) = next.clone() {
            self.app.take_pending();
            self.append(Message::User { text });
            self.sync_queue();
        }
        self.answer(id, next);
    }

    pub(super) fn sync_queue(&mut self) {
        let queued: Vec<String> = self.queued.iter().cloned().collect();
        self.inner.emit(
            events::Event::QueueChanged.name(),
            &[("count", queued.len().to_string())],
        );
        self.inner.state().borrow_mut().set_queued(queued.clone());
        self.app.overlay_mut().set_queued(queued);
    }
}
