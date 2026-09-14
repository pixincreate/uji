use crossterm::event::{KeyCode, KeyEvent};

use super::action::{Action, KeyAction, default_action, filter_items};
use super::{App, Mode};

const SELECT_PAGE: isize = 10;

impl App {
    pub fn handle_key(&mut self, key: KeyEvent) -> KeyAction {
        match self.mode {
            Mode::Select { .. } => self.handle_select_key(key),
            Mode::Prompt { .. } => self.handle_prompt_key(key),
            Mode::Suggest { .. } => self.handle_suggest_key(key),
            Mode::Confirm { .. } => self.handle_confirm_key(key),
            Mode::Normal => self.handle_normal_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Esc if self.input.is_empty() => KeyAction::Interrupt,
            KeyCode::Esc => self.apply(Action::ClearInput),
            KeyCode::Char(c) => self.insert_char(c),
            code => match default_action(code) {
                Some(action) => self.apply(action),
                None => KeyAction::None,
            },
        }
    }

    fn handle_select_key(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Esc => self.apply(Action::ModalCancel),
            KeyCode::Up => self.apply(Action::ModalUp),
            KeyCode::Down => self.apply(Action::ModalDown),
            KeyCode::PageUp => {
                self.select_move(-SELECT_PAGE);
                KeyAction::None
            }
            KeyCode::PageDown => {
                self.select_move(SELECT_PAGE);
                KeyAction::None
            }
            KeyCode::Enter => self.apply(Action::ModalAccept),
            KeyCode::Char(c) => {
                if let Mode::Select { query, cursor, .. } = &mut self.mode {
                    query.push(c);
                    *cursor = 0;
                }
                KeyAction::None
            }
            KeyCode::Backspace => {
                if let Mode::Select { query, cursor, .. } = &mut self.mode {
                    query.pop();
                    *cursor = 0;
                }
                KeyAction::None
            }
            _ => KeyAction::None,
        }
    }

    fn handle_prompt_key(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Esc => self.apply(Action::ModalCancel),
            KeyCode::Enter => self.apply(Action::ModalAccept),
            KeyCode::Backspace => {
                if let Mode::Prompt { value, .. } = &mut self.mode {
                    value.pop();
                }
                KeyAction::None
            }
            KeyCode::Char(c) => {
                if let Mode::Prompt { value, .. } = &mut self.mode {
                    value.push(c);
                }
                KeyAction::None
            }
            _ => KeyAction::None,
        }
    }

    fn handle_suggest_key(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Esc => self.apply(Action::ModalCancel),
            KeyCode::Up => self.apply(Action::ModalUp),
            KeyCode::Down => self.apply(Action::ModalDown),
            KeyCode::Tab => self.apply(Action::SuggestComplete),
            KeyCode::Enter => self.apply(Action::ModalAccept),
            KeyCode::Char(c) => self.insert_char(c),
            KeyCode::Backspace => self.backspace(),
            _ => KeyAction::None,
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Esc => self.apply(Action::ModalCancel),
            KeyCode::Char('y' | 'Y' | '1') => self.apply(Action::ConfirmAllow),
            KeyCode::Char('n' | 'N' | '2') => self.apply(Action::ConfirmDeny),
            KeyCode::Enter => self.apply(Action::ModalAccept),
            KeyCode::Up => self.apply(Action::ModalUp),
            KeyCode::Down => self.apply(Action::ModalDown),
            KeyCode::Left | KeyCode::Right | KeyCode::Tab => self.apply(Action::ConfirmToggle),
            _ => KeyAction::None,
        }
    }

    pub(super) fn take_submit(&mut self) -> KeyAction {
        self.set_browsing(None);
        let text = self.input.trim().to_string();
        self.input.clear();
        self.cursor = 0;
        if text.is_empty() {
            KeyAction::None
        } else if let Some(command) = text.strip_prefix('/') {
            KeyAction::Command(command.to_string())
        } else {
            KeyAction::Submit(text)
        }
    }

    /// Walk back through previously submitted messages. The first step stashes
    /// whatever was being typed so `history_next` can restore it.
    pub(super) fn history_prev(&mut self) -> KeyAction {
        let next = if let Some(at) = self.browsing() {
            at.saturating_add(1)
        } else {
            self.stash_draft();
            0
        };
        let Some(text) = self.submitted(next) else {
            return KeyAction::None;
        };
        self.set_browsing(Some(next));
        self.replace_input(text);
        KeyAction::None
    }

    pub(super) fn history_next(&mut self) -> KeyAction {
        let Some(at) = self.browsing() else {
            return KeyAction::None;
        };
        let text = at
            .checked_sub(1)
            .and_then(|newer| self.submitted(newer).map(|text| (newer, text)));
        if let Some((newer, text)) = text {
            self.set_browsing(Some(newer));
            self.replace_input(text);
        } else {
            self.set_browsing(None);
            let draft = self.take_draft();
            self.replace_input(draft);
        }
        KeyAction::None
    }

    pub(super) fn clear_input(&mut self) -> KeyAction {
        self.set_browsing(None);
        self.input.clear();
        self.cursor = 0;
        self.after_input_change();
        KeyAction::None
    }

    fn insert_char(&mut self, c: char) -> KeyAction {
        self.set_browsing(None);
        self.input.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.after_input_change();
        KeyAction::None
    }

    pub(super) fn backspace(&mut self) -> KeyAction {
        self.set_browsing(None);
        if self.cursor > 0 {
            let prev = prev_char_boundary(&self.input, self.cursor);
            self.input.remove(prev);
            self.cursor = prev;
        }
        self.after_input_change();
        KeyAction::None
    }

    pub(super) fn cursor_left(&mut self) -> KeyAction {
        self.cursor = prev_char_boundary(&self.input, self.cursor);
        KeyAction::None
    }

    pub(super) fn cursor_right(&mut self) -> KeyAction {
        self.cursor = next_char_boundary(&self.input, self.cursor);
        KeyAction::None
    }

    pub(super) fn cursor_start(&mut self) -> KeyAction {
        self.cursor = 0;
        KeyAction::None
    }

    pub(super) fn cursor_end(&mut self) -> KeyAction {
        self.cursor = self.input.len();
        KeyAction::None
    }

    pub(super) fn line_up(&mut self) -> KeyAction {
        self.scroll_up(1);
        KeyAction::None
    }

    pub(super) fn line_down(&mut self) -> KeyAction {
        self.scroll_down(1);
        KeyAction::None
    }

    pub(super) fn page_up(&mut self) -> KeyAction {
        self.scroll_up(self.viewport().max(1));
        KeyAction::None
    }

    pub(super) fn page_down(&mut self) -> KeyAction {
        self.scroll_down(self.viewport().max(1));
        KeyAction::None
    }

    pub(super) fn top(&mut self) -> KeyAction {
        self.jump_top();
        KeyAction::None
    }

    pub(super) fn bottom(&mut self) -> KeyAction {
        self.jump_bottom();
        KeyAction::None
    }

    pub(super) fn modal_up(&mut self) -> KeyAction {
        self.modal_move(-1);
        KeyAction::None
    }

    pub(super) fn modal_down(&mut self) -> KeyAction {
        self.modal_move(1);
        KeyAction::None
    }

    pub(super) fn modal_accept(&mut self) -> KeyAction {
        match &self.mode {
            Mode::Select { cursor, .. } => {
                let cursor = *cursor;
                let item = self
                    .select_matches()
                    .get(cursor)
                    .map(|item| (*item).clone());
                self.mode = Mode::Normal;
                item.map_or(KeyAction::None, KeyAction::Selected)
            }
            Mode::Prompt { value, .. } => {
                let value = value.clone();
                self.mode = Mode::Normal;
                KeyAction::Prompted(value)
            }
            Mode::Suggest { .. } => {
                if let Some(name) = self.highlighted_suggest() {
                    self.input = format!("/{name}");
                    self.cursor = self.input.len();
                }
                self.mode = Mode::Normal;
                self.take_submit()
            }
            Mode::Confirm { allow, .. } => {
                let allow = *allow;
                self.mode = Mode::Normal;
                KeyAction::Confirmed(allow)
            }
            Mode::Normal => self.take_submit(),
        }
    }

    pub(super) fn modal_cancel(&mut self) -> KeyAction {
        match self.mode {
            Mode::Suggest { .. } => {
                self.input.clear();
                self.cursor = 0;
                self.mode = Mode::Normal;
                KeyAction::None
            }
            Mode::Confirm { .. } => KeyAction::Confirmed(false),
            Mode::Normal => KeyAction::Interrupt,
            _ => {
                self.mode = Mode::Normal;
                KeyAction::Cancel
            }
        }
    }

    pub(super) fn suggest_complete(&mut self) -> KeyAction {
        if let Some(name) = self.highlighted_suggest() {
            self.input = format!("/{name} ");
            self.cursor = self.input.len();
            self.mode = Mode::Normal;
        }
        KeyAction::None
    }

    pub(super) fn confirm_allow(&mut self) -> KeyAction {
        self.mode = Mode::Normal;
        KeyAction::Confirmed(true)
    }

    pub(super) fn confirm_deny(&mut self) -> KeyAction {
        self.mode = Mode::Normal;
        KeyAction::Confirmed(false)
    }

    pub(super) fn confirm_toggle(&mut self) -> KeyAction {
        if let Mode::Confirm { allow, .. } = &mut self.mode {
            *allow = !*allow;
        }
        KeyAction::None
    }

    fn modal_move(&mut self, delta: isize) {
        match &mut self.mode {
            Mode::Select { .. } => self.select_move(delta),
            Mode::Suggest { items, cursor } => {
                *cursor = step(*cursor, delta, items.len());
            }
            Mode::Confirm { allow, .. } => *allow = delta < 0,
            Mode::Normal | Mode::Prompt { .. } => {}
        }
    }

    fn select_matches(&self) -> Vec<&String> {
        match &self.mode {
            Mode::Select { items, query, .. } => filter_items(items, query),
            _ => Vec::new(),
        }
    }

    fn select_move(&mut self, delta: isize) {
        let len = self.select_matches().len();
        if let Mode::Select { cursor, .. } = &mut self.mode {
            *cursor = step(*cursor, delta, len);
        }
    }

    fn highlighted_suggest(&self) -> Option<String> {
        match &self.mode {
            Mode::Suggest { items, cursor } => items.get(*cursor).map(|item| item.name.clone()),
            _ => None,
        }
    }
}

fn step(cursor: usize, delta: isize, len: usize) -> usize {
    let last = len.saturating_sub(1);
    if delta < 0 {
        cursor.saturating_sub(delta.unsigned_abs())
    } else {
        cursor.saturating_add(delta.unsigned_abs()).min(last)
    }
}

fn prev_char_boundary(s: &str, index: usize) -> usize {
    if index == 0 {
        return 0;
    }
    let mut i = index - 1;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn next_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    let mut i = index + 1;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}
