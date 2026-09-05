use std::rc::Rc;

use crossterm::event::Event as TermEvent;
use tui::app::{self, App, KeyAction};
use uji_core::action;
use uji_core::session::model::Message;
use uji_core::session::store::SessionStorage;

use super::Inner;
use super::events;

pub(crate) struct LoopData {
    pub(crate) inner: Rc<Inner>,
    pub(crate) app: App,
    pub(crate) storage: Box<dyn SessionStorage>,
    pub(crate) terminal: app::Term,
    pub(crate) dirty: bool,
    pub(crate) running: bool,
}

impl LoopData {
    pub(crate) fn on_term_event(&mut self, event: &TermEvent) {
        match event {
            TermEvent::Key(key) => {
                match self.app.handle_key(*key) {
                    KeyAction::Quit => self.running = false,
                    KeyAction::Submit(text) => self.submit(&text),
                    KeyAction::None => {}
                }
                self.dirty = true;
            }
            TermEvent::Resize(..) => self.dirty = true,
            _ => {}
        }
    }

    pub(crate) fn submit(&mut self, text: &str) {
        self.inner
            .emit(events::MESSAGE_SUBMITTED, &[("text", text.to_string())]);

        let user = Message::User {
            text: text.to_string(),
        };
        match self.storage.append_message(&self.app.session().id, user) {
            Ok(stored) => {
                self.app.push_message(stored);
                self.inner.emit(
                    events::MESSAGE_APPENDED,
                    &[("type", "user".into()), ("text", text.to_string())],
                );
            }
            Err(err) => {
                eprintln!("uji: failed to persist message: {err}");
            }
        }

        if let Some(response) = action::run(text) {
            let assistant = Message::Assistant {
                text: response.clone(),
            };
            match self
                .storage
                .append_message(&self.app.session().id, assistant)
            {
                Ok(stored) => {
                    self.app.push_message(stored);
                    self.inner.emit(
                        events::MESSAGE_APPENDED,
                        &[("type", "assistant".into()), ("text", response)],
                    );
                }
                Err(err) => {
                    eprintln!("uji: failed to persist response: {err}");
                }
            }
        }
    }

    pub(crate) fn reload(&mut self) {
        self.inner.reload();
        self.dirty = true;
    }
}
