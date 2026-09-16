use std::time::Instant;

use super::gesture::Gesture;
use super::{Point, Selection};

#[derive(Debug, Default)]
pub struct Screen {
    lines: Vec<String>,
    gesture: Gesture,
}

impl Screen {
    pub fn rows(&mut self, height: usize) -> &mut [String] {
        self.lines.resize_with(height, String::new);
        &mut self.lines
    }

    pub fn selection(&self) -> Option<Selection> {
        self.gesture.selection()
    }

    pub fn press(&mut self, point: Point, now: Instant) {
        self.gesture.press(point, &self.lines, now);
    }

    pub fn drag(&mut self, point: Point) {
        self.gesture.drag(point, &self.lines);
    }

    pub fn release(&mut self) -> Option<String> {
        let text = self.gesture.selection()?.text(&self.lines);
        (!text.trim().is_empty()).then_some(text)
    }

    pub fn clear(&mut self) -> bool {
        self.gesture.clear()
    }
}
