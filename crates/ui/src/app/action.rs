use std::str::FromStr;

use crate::keymap::Key;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};
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
    InterruptOrQuit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, VariantNames)]
#[strum(serialize_all = "snake_case")]
pub enum Action {
    #[strum(to_string = "nothing", serialize = "noop")]
    Nothing,
    Quit,
    Interrupt,
    ToggleThinking,
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
            Action::ToggleThinking => self.toggle_thinking(),
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

pub(super) fn default_action(code: Key) -> Option<Action> {
    Some(match code {
        Key::Backspace => Action::Backspace,
        Key::Left => Action::CursorLeft,
        Key::Right => Action::CursorRight,
        Key::Up => Action::HistoryPrev,
        Key::Down => Action::HistoryNext,
        Key::PageUp => Action::PageUp,
        Key::PageDown => Action::PageDown,
        Key::Home => Action::ScrollTop,
        Key::End => Action::ScrollBottom,
        Key::Enter => Action::Submit,
        _ => return None,
    })
}

struct Candidate<'a> {
    at: usize,
    text: &'a str,
}

impl AsRef<str> for Candidate<'_> {
    fn as_ref(&self) -> &str {
        self.text
    }
}

pub fn rank_items(items: &[String], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..items.len()).collect();
    }
    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    let candidates = items.iter().enumerate().map(|(at, item)| Candidate {
        at,
        text: item.as_str(),
    });
    Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart)
        .match_list(candidates, &mut matcher)
        .into_iter()
        .map(|(candidate, _)| candidate.at)
        .collect()
}
