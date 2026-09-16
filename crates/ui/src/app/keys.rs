use crossterm::event::{KeyCode, KeyEvent};

use crate::model::Builtin;

use super::action::{Action, KeyAction, default_action, rank_items};
use super::{App, Mode, sent};
use std::rc::Rc;

const SELECT_PAGE: isize = 10;

impl App {
    /// Keys go to the focused window; the modal window then picks the handler
    /// for whatever it is showing.
    pub fn handle_key(&mut self, key: KeyEvent) -> KeyAction {
        if self.focus() == Builtin::Input {
            return self.handle_normal_key(key);
        }
        match self.mode {
            Mode::Select { .. } | Mode::Pick { .. } => self.handle_select_key(key),
            Mode::Prompt { .. } => self.handle_prompt_key(key),
            Mode::Suggest { .. } => self.handle_suggest_key(key),
            Mode::Confirm { .. } => self.handle_confirm_key(key),
            Mode::Normal => self.handle_normal_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Esc if self.input().is_empty() => KeyAction::Interrupt,
            KeyCode::Esc => self.apply(Action::ClearInput),
            KeyCode::Char(c) => self.insert_char(c),
            code => match default_action(code) {
                Some(action) => self.apply(action),
                None => KeyAction::None,
            },
        }
    }

    fn handle_select_key(&mut self, key: KeyEvent) -> KeyAction {
        if let Some(action) = modal_key(key.code).or_else(|| list_key(key.code)) {
            return self.apply(action);
        }
        match key.code {
            KeyCode::PageUp => {
                self.select_move(-SELECT_PAGE);
                KeyAction::None
            }
            KeyCode::PageDown => {
                self.select_move(SELECT_PAGE);
                KeyAction::None
            }
            KeyCode::Char(c) => {
                if let Mode::Select { query, cursor, .. } | Mode::Pick { query, cursor, .. } =
                    &mut self.mode
                {
                    query.push(c);
                    *cursor = 0;
                }
                self.rerank();
                KeyAction::None
            }
            KeyCode::Backspace => {
                if let Mode::Select { query, cursor, .. } | Mode::Pick { query, cursor, .. } =
                    &mut self.mode
                {
                    query.pop();
                    *cursor = 0;
                }
                self.rerank();
                KeyAction::None
            }
            _ => KeyAction::None,
        }
    }

    fn handle_prompt_key(&mut self, key: KeyEvent) -> KeyAction {
        if let Some(action) = modal_key(key.code) {
            return self.apply(action);
        }
        match key.code {
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
        if let Some(action) = modal_key(key.code).or_else(|| list_key(key.code)) {
            return self.apply(action);
        }
        match key.code {
            KeyCode::Tab => self.apply(Action::SuggestComplete),
            KeyCode::Char(c) => self.insert_char(c),
            KeyCode::Backspace => self.backspace(),
            _ => KeyAction::None,
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) -> KeyAction {
        if let Some(action) = modal_key(key.code).or_else(|| list_key(key.code)) {
            return self.apply(action);
        }
        match key.code {
            KeyCode::Char('y' | 'Y' | '1') => self.apply(Action::ConfirmAllow),
            KeyCode::Char('n' | 'N' | '2') => self.apply(Action::ConfirmDeny),
            KeyCode::Left | KeyCode::Right | KeyCode::Tab => self.apply(Action::ConfirmToggle),
            _ => KeyAction::None,
        }
    }

    pub(super) fn take_submit(&mut self) -> KeyAction {
        let text = self.composer.take().trim().to_string();
        if text.is_empty() {
            KeyAction::None
        } else if let Some(command) = text.strip_prefix('/') {
            KeyAction::Command(command.to_string())
        } else {
            KeyAction::Submit(text)
        }
    }

    pub(super) fn history_prev(&mut self) -> KeyAction {
        let conversation = Rc::clone(&self.conversation);
        self.composer
            .recall_prev(|back| sent(&conversation.borrow(), back));
        self.after_input_change();
        KeyAction::None
    }

    pub(super) fn history_next(&mut self) -> KeyAction {
        let conversation = Rc::clone(&self.conversation);
        self.composer
            .recall_next(|back| sent(&conversation.borrow(), back));
        self.after_input_change();
        KeyAction::None
    }

    pub(super) fn clear_input(&mut self) -> KeyAction {
        self.composer.clear();
        self.after_input_change();
        KeyAction::None
    }

    fn insert_char(&mut self, c: char) -> KeyAction {
        self.composer.insert(c);
        self.after_input_change();
        KeyAction::None
    }

    pub(super) fn backspace(&mut self) -> KeyAction {
        self.composer.backspace();
        self.after_input_change();
        KeyAction::None
    }

    pub(super) fn cursor_left(&mut self) -> KeyAction {
        self.composer.left();
        KeyAction::None
    }

    pub(super) fn cursor_right(&mut self) -> KeyAction {
        self.composer.right();
        KeyAction::None
    }

    pub(super) fn cursor_start(&mut self) -> KeyAction {
        self.composer.home();
        KeyAction::None
    }

    pub(super) fn cursor_end(&mut self) -> KeyAction {
        self.composer.end();
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
            Mode::Select { cursor, .. } | Mode::Pick { cursor, .. } => {
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
                    self.composer.set(format!("/{name}"));
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
                self.composer.clear();
                self.mode = Mode::Normal;
                KeyAction::None
            }
            Mode::Confirm { .. } => KeyAction::Confirmed(false),
            Mode::Normal => KeyAction::InterruptOrQuit,
            _ => {
                self.mode = Mode::Normal;
                KeyAction::Cancel
            }
        }
    }

    pub(super) fn suggest_complete(&mut self) -> KeyAction {
        if let Some(name) = self.highlighted_suggest() {
            self.composer.set(format!("/{name} "));
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
            Mode::Select { .. } | Mode::Pick { .. } => self.select_move(delta),
            Mode::Suggest { items, cursor } => {
                *cursor = step(*cursor, delta, items.len());
            }
            Mode::Confirm { allow, .. } => *allow = delta < 0,
            Mode::Normal | Mode::Prompt { .. } => {}
        }
    }

    fn rerank(&mut self) {
        let (Mode::Select {
            items,
            query,
            matches,
            ..
        }
        | Mode::Pick {
            items,
            query,
            matches,
            ..
        }) = &mut self.mode
        else {
            return;
        };
        *matches = rank_items(items, query);
    }

    fn select_matches(&self) -> Vec<&String> {
        match &self.mode {
            Mode::Select { items, matches, .. } | Mode::Pick { items, matches, .. } => {
                matches.iter().filter_map(|at| items.get(*at)).collect()
            }
            _ => Vec::new(),
        }
    }

    fn select_move(&mut self, delta: isize) {
        let len = self.select_matches().len();
        if let Mode::Select { cursor, .. } | Mode::Pick { cursor, .. } = &mut self.mode {
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

/// Move a modal cursor. Single steps cycle the list the way telescope does;
/// page jumps clamp, so paging never teleports across the ends.
fn step(cursor: usize, delta: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let last = len.saturating_sub(1);
    let distance = delta.unsigned_abs();
    match (distance, delta < 0) {
        (1, true) if cursor == 0 => last,
        (1, false) if cursor >= last => 0,
        (_, true) => cursor.saturating_sub(distance),
        (_, false) => cursor.saturating_add(distance).min(last),
    }
}

/// Keys every modal answers the same way.
fn modal_key(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Esc => Some(Action::ModalCancel),
        KeyCode::Enter => Some(Action::ModalAccept),
        _ => None,
    }
}

/// Navigation shared by the modals that show a list.
fn list_key(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Up => Some(Action::ModalUp),
        KeyCode::Down => Some(Action::ModalDown),
        _ => None,
    }
}
