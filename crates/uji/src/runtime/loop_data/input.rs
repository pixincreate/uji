use crossterm::event::{Event as TermEvent, KeyCode, KeyEvent, KeyModifiers};
use uji_ui::app::{Action, KeyAction};
use uji_ui::keymap::{Binding, Chord, Key, describe};

use super::{Control, LoopData, ModalInput};

impl LoopData {
    pub(crate) fn on_term_event(&mut self, event: &TermEvent) {
        match event {
            TermEvent::Key(key) => self.on_key(*key),
            TermEvent::Mouse(mouse) => self.on_mouse(*mouse),
            TermEvent::Paste(text) => {
                self.app.paste(text);
                self.dirty = true;
            }
            TermEvent::Resize(..) => self.dirty = true,
            _ => {}
        }
    }

    fn on_key(&mut self, key: KeyEvent) {
        self.clear_selection();
        let Some(action) = self.dispatch_key(key) else {
            self.dirty = true;
            return;
        };
        match action {
            KeyAction::Quit => self.control = Control::Quit,
            KeyAction::Submit(text) => self.submit(&text),
            KeyAction::Command(command) => self.on_command(&command),
            KeyAction::Confirmed(allow) => self.resolve_tool_confirmation(allow),
            KeyAction::Selected(item) => self.on_modal(ModalInput::Select(item)),
            KeyAction::Prompted(value) => self.on_modal(ModalInput::Prompt(value)),
            KeyAction::Cancel => self.on_modal(ModalInput::Cancel),
            KeyAction::Interrupt => {
                self.interrupt();
            }
            KeyAction::InterruptOrQuit => {
                if !self.interrupt() {
                    self.control = Control::Quit;
                }
            }
            KeyAction::None => {}
        }
        self.dirty = true;
    }

    fn dispatch_key(&mut self, key: KeyEvent) -> Option<KeyAction> {
        if self.inner.api.capture().is_active() {
            self.dispatch_capture(key);
            return None;
        }
        let mode = self.app.keymap_mode();
        let binding = chord_of(key)
            .and_then(|chord| self.inner.api.keymap().borrow().get(mode, chord).cloned());
        match binding {
            Some(Binding::Unbound) => None,
            Some(Binding::Command(command)) => {
                self.on_command(&command);
                None
            }
            Some(Binding::Action(name)) => {
                if let Some(action) = Action::parse(&name) {
                    return Some(self.app.apply(action));
                }
                match self.inner.api.actions().get(&name) {
                    Some(handler) => {
                        if let Err(err) = handler.call::<()>(()) {
                            self.inner.report(format!("action {name}: {err}"));
                        }
                        self.dirty = true;
                    }
                    None => self.inner.report(format!("unknown keymap action: {name}")),
                }
                None
            }
            None => Some(self.app.handle_key(key)),
        }
    }

    fn dispatch_capture(&mut self, key: KeyEvent) {
        let Some(handler) = self.inner.api.capture().handler() else {
            return;
        };
        let Some(chord) = chord_of(key) else {
            return;
        };
        let Ok(event) = self.inner.lua.create_table() else {
            return;
        };
        let _ = event.set("key", describe(chord));
        if let Key::Char(c) = chord.key {
            let _ = event.set("char", c.to_string());
        }
        let _ = event.set("ctrl", chord.ctrl);
        let _ = event.set("alt", chord.alt);
        let _ = event.set("shift", chord.shift);
        if let Err(err) = handler.call::<()>((event,)) {
            self.inner.report(format!("capture handler: {err}"));
            self.inner.api.capture().clear();
        }
        self.dirty = true;
    }
}

fn chord_of(key: KeyEvent) -> Option<Chord> {
    let mapped = match key.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Escape,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::Tab => Key::Tab,
        KeyCode::BackTab => Key::BackTab,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Insert => Key::Insert,
        KeyCode::F(number) => Key::F(number),
        _ => return None,
    };
    Some(Chord::new(
        mapped,
        key.modifiers.contains(KeyModifiers::CONTROL),
        key.modifiers.contains(KeyModifiers::ALT),
        key.modifiers.contains(KeyModifiers::SHIFT),
    ))
}
