use super::LoopData;
use crate::runtime::events;

impl LoopData {
    pub(super) fn enqueue(&mut self, text: &str) {
        self.queued.push_back(text.to_string());
        self.sync_queue();
        self.dirty = true;
    }

    pub(super) fn sync_queue(&mut self) {
        let queued: Vec<String> = self.queued.iter().cloned().collect();
        self.inner.emit(
            events::QUEUE_CHANGED,
            &[("count", queued.len().to_string())],
        );
        self.inner.state().borrow_mut().set_queued(queued.clone());
        self.app.overlay_mut().set_queued(queued);
    }
}
