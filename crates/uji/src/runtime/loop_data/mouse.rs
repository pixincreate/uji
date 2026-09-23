use std::time::Instant;

use uji_ui::app::selection::Point;
use uji_ui::clipboard::{self, Copied};
use uji_ui::input::MouseKind;

use super::LoopData;

const SCROLL_LINES: usize = 3;

impl LoopData {
    pub(super) fn on_mouse(&mut self, point: Point, kind: MouseKind) {
        match kind {
            MouseKind::ScrollUp => self.app.scroll_up(SCROLL_LINES),
            MouseKind::ScrollDown => self.app.scroll_down(SCROLL_LINES),
            MouseKind::Press => {
                self.app
                    .overlay()
                    .screen()
                    .borrow_mut()
                    .press(point, Instant::now());
            }
            MouseKind::Drag => {
                self.app.overlay().screen().borrow_mut().drag(point);
            }
            MouseKind::Release => {
                let text = self.app.overlay().screen().borrow_mut().release();
                if let Some(text) = text {
                    self.copy(&text);
                }
            }
        }
        self.dirty = true;
    }

    pub(super) fn clear_selection(&mut self) {
        if self.app.overlay().screen().borrow_mut().clear() {
            self.dirty = true;
        }
    }

    fn copy(&mut self, text: &str) {
        let lines = text.lines().count();
        self.inner.notify(match clipboard::write(text) {
            Copied::Native => format!("copied {lines} line(s)"),
            Copied::Osc52 => format!("copied {lines} line(s) via the terminal"),
        });
        self.drain_notices();
    }
}
