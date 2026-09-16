use std::cell::RefCell;

use super::selection::Screen;

#[derive(Debug, Default)]
pub struct Overlay {
    notices: Vec<String>,
    queued: Vec<String>,
    screen: RefCell<Screen>,
}

impl Overlay {
    pub fn notices(&self) -> &[String] {
        &self.notices
    }

    pub fn push_notices(&mut self, notices: Vec<String>) {
        self.notices.extend(notices);
    }

    pub fn clear_notices(&mut self) {
        self.notices.clear();
    }

    pub fn queued(&self) -> &[String] {
        &self.queued
    }

    pub fn set_queued(&mut self, queued: Vec<String>) {
        self.queued = queued;
    }

    pub fn screen(&self) -> &RefCell<Screen> {
        &self.screen
    }
}
