use std::str::FromStr;

use crossterm::event::KeyCode;
use strum::{EnumString, VariantNames};

use super::App;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    None,
    Quit,
    Submit(String),
    Command(String),
    Selected(String),
    Prompted(String),
    Confirmed(bool),
    Cancel,
    Interrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, VariantNames)]
#[strum(serialize_all = "snake_case")]
pub enum Action {
    #[strum(to_string = "nothing", serialize = "noop")]
    Nothing,
    Quit,
    Interrupt,
    Submit,
    ClearInput,
    Backspace,
    CursorLeft,
    CursorRight,
    CursorStart,
    CursorEnd,
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
    ScrollTop,
    ScrollBottom,
    HistoryPrev,
    HistoryNext,
    ModalUp,
    ModalDown,
    ModalAccept,
    ModalCancel,
    SuggestComplete,
    ConfirmAllow,
    ConfirmDeny,
    ConfirmToggle,
}

impl Action {
    pub fn parse(name: &str) -> Option<Self> {
        Self::from_str(name).ok()
    }

    pub fn names() -> impl Iterator<Item = &'static str> {
        Self::VARIANTS.iter().copied()
    }
}

impl App {
    pub fn apply(&mut self, action: Action) -> KeyAction {
        match action {
            Action::Nothing => KeyAction::None,
            Action::Interrupt => KeyAction::Interrupt,
            Action::Quit => KeyAction::Quit,
            Action::Submit => self.take_submit(),
            Action::ClearInput => self.clear_input(),
            Action::Backspace => self.backspace(),
            Action::CursorLeft => self.cursor_left(),
            Action::CursorRight => self.cursor_right(),
            Action::CursorStart => self.cursor_start(),
            Action::CursorEnd => self.cursor_end(),
            Action::ScrollUp => self.line_up(),
            Action::ScrollDown => self.line_down(),
            Action::PageUp => self.page_up(),
            Action::PageDown => self.page_down(),
            Action::ScrollTop => self.top(),
            Action::ScrollBottom => self.bottom(),
            Action::HistoryPrev => self.history_prev(),
            Action::HistoryNext => self.history_next(),
            Action::ModalUp => self.modal_up(),
            Action::ModalDown => self.modal_down(),
            Action::ModalAccept => self.modal_accept(),
            Action::ModalCancel => self.modal_cancel(),
            Action::SuggestComplete => self.suggest_complete(),
            Action::ConfirmAllow => self.confirm_allow(),
            Action::ConfirmDeny => self.confirm_deny(),
            Action::ConfirmToggle => self.confirm_toggle(),
        }
    }
}

pub(super) fn default_action(code: KeyCode) -> Option<Action> {
    Some(match code {
        KeyCode::Backspace => Action::Backspace,
        KeyCode::Left => Action::CursorLeft,
        KeyCode::Right => Action::CursorRight,
        KeyCode::Up => Action::HistoryPrev,
        KeyCode::Down => Action::HistoryNext,
        KeyCode::PageUp => Action::PageUp,
        KeyCode::PageDown => Action::PageDown,
        KeyCode::Home => Action::ScrollTop,
        KeyCode::End => Action::ScrollBottom,
        KeyCode::Enter => Action::Submit,
        _ => return None,
    })
}

pub fn filter_items<'a>(items: &'a [String], query: &str) -> Vec<&'a String> {
    if query.is_empty() {
        return items.iter().collect();
    }
    let needle = query.to_lowercase();
    items
        .iter()
        .filter(|item| item.to_lowercase().contains(&needle))
        .collect()
}
