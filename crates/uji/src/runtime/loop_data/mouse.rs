use std::time::Instant;

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use uji_tui::app::selection::Point;
use uji_tui::clipboard::{self, Copied};

use super::LoopData;

const SCROLL_LINES: usize = 3;

impl LoopData {
    pub(super) fn on_mouse(&mut self, mouse: MouseEvent) {
        let point = Point::new(mouse.column, mouse.row);
        match mouse.kind {
            MouseEventKind::ScrollUp => self.app.scroll_up(SCROLL_LINES),
            MouseEventKind::ScrollDown => self.app.scroll_down(SCROLL_LINES),
            MouseEventKind::Down(MouseButton::Left) => {
                self.app
                    .overlay()
                    .screen()
                    .borrow_mut()
                    .press(point, Instant::now());
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.app.overlay().screen().borrow_mut().drag(point);
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let text = self.app.overlay().screen().borrow_mut().release();
                if let Some(text) = text {
                    self.copy(&text);
                }
            }
            _ => return,
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
        self.inner.report(match clipboard::write(text) {
            Copied::Native => format!("copied {lines} line(s)"),
            Copied::Osc52 => format!("copied {lines} line(s) via the terminal"),
        });
        self.drain_diagnostics();
    }
}
