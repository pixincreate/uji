use std::time::Instant;

use super::gesture::Gesture;
use super::{Point, Selection};

#[derive(Debug, Default)]
pub struct Screen {
    lines: Vec<String>,
    gesture: Gesture,
    pending: Option<(Point, Instant)>,
}

impl Screen {
    pub fn sync(&mut self, height: usize, fill: impl FnOnce(&mut [String])) {
        if self.pending.is_none() && self.gesture.selection().is_none() {
            return;
        }
        self.lines.resize_with(height, String::new);
        fill(&mut self.lines);
        self.resolve();
    }

    fn resolve(&mut self) {
        if let Some((point, now)) = self.pending.take() {
            self.gesture.press(point, &self.lines, now);
        }
    }

    pub fn selection(&self) -> Option<Selection> {
        self.gesture.selection()
    }

    pub fn press(&mut self, point: Point, now: Instant) {
        self.pending = Some((point, now));
    }

    pub fn drag(&mut self, point: Point) {
        self.resolve();
        self.gesture.drag(point, &self.lines);
    }

    pub fn release(&mut self) -> Option<String> {
        self.resolve();
        let text = self.gesture.selection()?.text(&self.lines);
        (!text.trim().is_empty()).then_some(text)
    }

    pub fn clear(&mut self) -> bool {
        self.pending = None;
        self.gesture.clear()
    }
}
