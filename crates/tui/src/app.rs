use std::cell::{Cell, Ref, RefCell};
use std::io::{self, Stdout};
use std::rc::Rc;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use uji_view::keymap;
use uji_view::state::UiState;

use crate::ui;
use uji_core::session::conversation::{Conversation, Shared};
use uji_core::session::model::Session;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    None,
    Quit,
    Submit(String),
    Command(String),
    Selected(String),
    Prompted(String),
    Cancel,
    Interrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
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
    ModalUp,
    ModalDown,
    ModalAccept,
    ModalCancel,
    SuggestComplete,
    ConfirmAllow,
    ConfirmDeny,
    ConfirmToggle,
}

const ACTIONS: &[(&str, Action)] = &[
    ("nothing", Action::Nothing),
    ("noop", Action::Nothing),
    ("quit", Action::Quit),
    ("interrupt", Action::Interrupt),
    ("submit", Action::Submit),
    ("clear_input", Action::ClearInput),
    ("backspace", Action::Backspace),
    ("cursor_left", Action::CursorLeft),
    ("cursor_right", Action::CursorRight),
    ("cursor_start", Action::CursorStart),
    ("cursor_end", Action::CursorEnd),
    ("scroll_up", Action::ScrollUp),
    ("scroll_down", Action::ScrollDown),
    ("page_up", Action::PageUp),
    ("page_down", Action::PageDown),
    ("scroll_top", Action::ScrollTop),
    ("scroll_bottom", Action::ScrollBottom),
    ("modal_up", Action::ModalUp),
    ("modal_down", Action::ModalDown),
    ("modal_accept", Action::ModalAccept),
    ("modal_cancel", Action::ModalCancel),
    ("suggest_complete", Action::SuggestComplete),
    ("confirm_allow", Action::ConfirmAllow),
    ("confirm_deny", Action::ConfirmDeny),
    ("confirm_toggle", Action::ConfirmToggle),
];

impl Action {
    pub fn parse(name: &str) -> Option<Self> {
        ACTIONS
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, action)| *action)
    }

    pub fn names() -> impl Iterator<Item = &'static str> {
        ACTIONS.iter().map(|(name, _)| *name)
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestItem {
    pub name: String,
    pub desc: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Select {
        title: String,
        items: Vec<String>,
        query: String,
        cursor: usize,
    },
    Prompt {
        title: String,
        value: String,
        secret: bool,
    },
    Suggest {
        items: Vec<SuggestItem>,
        cursor: usize,
    },
    Confirm {
        title: String,
        body: String,
        allow: bool,
    },
}

pub struct App {
    session: Session,
    conversation: Shared,
    state: Rc<RefCell<UiState>>,
    input: String,
    cursor: usize,
    running: bool,
    pending: Option<String>,
    mode: Mode,
    suggest_pool: Vec<SuggestItem>,
    scroll: Cell<usize>,
    viewport: Cell<usize>,
    last_max: Cell<usize>,
    notices: Vec<String>,
}

impl App {
    pub fn new(session: Session, conversation: Shared, state: Rc<RefCell<UiState>>) -> Self {
        Self {
            session,
            conversation,
            state,
            input: String::new(),
            cursor: 0,
            running: true,
            pending: None,
            mode: Mode::Normal,
            suggest_pool: Vec::new(),
            scroll: Cell::new(usize::MAX),
            viewport: Cell::new(0),
            last_max: Cell::new(0),
            notices: Vec::new(),
        }
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn conversation(&self) -> &Shared {
        &self.conversation
    }

    pub fn messages(&self) -> Ref<'_, Conversation> {
        self.conversation.borrow()
    }

    pub fn state(&self) -> &Rc<RefCell<UiState>> {
        &self.state
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn set_input(&mut self, text: String) {
        self.cursor = text.len();
        self.input = text;
        self.after_input_change();
    }

    pub fn cursor_offset(&self) -> usize {
        self.cursor
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn pending(&self) -> Option<&str> {
        self.pending.as_deref()
    }

    pub fn scroll(&self) -> usize {
        self.scroll.get()
    }

    pub fn set_scroll(&self, offset: usize) {
        self.scroll.set(offset);
    }

    pub fn viewport(&self) -> usize {
        self.viewport.get()
    }

    pub fn set_viewport(&self, height: usize) {
        self.viewport.set(height);
    }

    pub fn last_max(&self) -> usize {
        self.last_max.get()
    }

    pub fn set_last_max(&self, max_scroll: usize) {
        self.last_max.set(max_scroll);
    }

    pub fn reset_scroll(&self) {
        self.scroll.set(usize::MAX);
    }

    pub fn scroll_up(&self, lines: usize) {
        self.scroll.set(self.scroll.get().saturating_sub(lines));
    }

    pub fn scroll_down(&self, lines: usize) {
        self.scroll.set(self.scroll.get().saturating_add(lines));
    }

    pub fn jump_top(&self) {
        self.scroll.set(0);
    }

    pub fn jump_bottom(&self) {
        self.scroll.set(usize::MAX);
    }

    pub fn append_pending(&mut self, delta: &str) {
        self.pending.get_or_insert_with(String::new).push_str(delta);
    }

    pub fn take_pending(&mut self) -> Option<String> {
        self.pending.take()
    }

    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    pub fn notices(&self) -> &[String] {
        &self.notices
    }

    pub fn push_notices(&mut self, notices: Vec<String>) {
        self.notices.extend(notices);
    }

    pub fn clear_notices(&mut self) {
        self.notices.clear();
    }

    pub fn open_select(&mut self, title: String, items: Vec<String>) {
        self.mode = Mode::Select {
            title,
            items,
            query: String::new(),
            cursor: 0,
        };
    }

    pub fn open_prompt(&mut self, title: String, value: String, secret: bool) {
        self.mode = Mode::Prompt {
            title,
            value,
            secret,
        };
    }

    pub fn open_confirm(&mut self, title: String, body: String) {
        self.mode = Mode::Confirm {
            title,
            body,
            allow: true,
        };
    }

    pub fn close_modal(&mut self) {
        self.mode = Mode::Normal;
    }

    pub fn set_suggestions(&mut self, items: Vec<SuggestItem>) {
        self.suggest_pool = items;
    }

    pub fn keymap_mode(&self) -> keymap::Mode {
        match self.mode {
            Mode::Normal => keymap::Mode::Normal,
            Mode::Select { .. } => keymap::Mode::Select,
            Mode::Prompt { .. } => keymap::Mode::Prompt,
            Mode::Suggest { .. } => keymap::Mode::Suggest,
            Mode::Confirm { .. } => keymap::Mode::Confirm,
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn apply(&mut self, action: Action) -> KeyAction {
        match action {
            Action::Quit => self.quit(),
            Action::Interrupt => KeyAction::Interrupt,
            Action::Nothing => KeyAction::None,
            Action::Submit => self.take_submit(),
            Action::ClearInput => {
                self.input.clear();
                self.cursor = 0;
                self.after_input_change();
                KeyAction::None
            }
            Action::Backspace => {
                if self.cursor > 0 {
                    let prev = prev_char_boundary(&self.input, self.cursor);
                    self.input.remove(prev);
                    self.cursor = prev;
                }
                self.after_input_change();
                KeyAction::None
            }
            Action::CursorLeft => {
                self.cursor = prev_char_boundary(&self.input, self.cursor);
                KeyAction::None
            }
            Action::CursorRight => {
                self.cursor = next_char_boundary(&self.input, self.cursor);
                KeyAction::None
            }
            Action::CursorStart => {
                self.cursor = 0;
                KeyAction::None
            }
            Action::CursorEnd => {
                self.cursor = self.input.len();
                KeyAction::None
            }
            Action::ScrollUp => {
                self.scroll_up(1);
                KeyAction::None
            }
            Action::ScrollDown => {
                self.scroll_down(1);
                KeyAction::None
            }
            Action::PageUp => {
                self.scroll_up(self.viewport().max(1));
                KeyAction::None
            }
            Action::PageDown => {
                self.scroll_down(self.viewport().max(1));
                KeyAction::None
            }
            Action::ScrollTop => {
                self.jump_top();
                KeyAction::None
            }
            Action::ScrollBottom => {
                self.jump_bottom();
                KeyAction::None
            }
            Action::ModalUp => {
                match &mut self.mode {
                    Mode::Select { cursor, .. } | Mode::Suggest { cursor, .. } => {
                        *cursor = cursor.saturating_sub(1);
                    }
                    Mode::Confirm { allow, .. } => *allow = true,
                    _ => {}
                }
                KeyAction::None
            }
            Action::ModalDown => {
                match &mut self.mode {
                    Mode::Select { items, cursor, .. } => {
                        *cursor = cursor.saturating_add(1).min(items.len().saturating_sub(1));
                    }
                    Mode::Suggest { items, cursor } => {
                        *cursor = cursor.saturating_add(1).min(items.len().saturating_sub(1));
                    }
                    Mode::Confirm { allow, .. } => *allow = false,
                    _ => {}
                }
                KeyAction::None
            }
            Action::ModalAccept => self.modal_accept(),
            Action::ModalCancel => self.modal_cancel(),
            Action::SuggestComplete => {
                if let Some(name) = self.highlighted_suggest() {
                    self.input = format!("/{name} ");
                    self.cursor = self.input.len();
                    self.mode = Mode::Normal;
                }
                KeyAction::None
            }
            Action::ConfirmAllow => {
                self.mode = Mode::Normal;
                KeyAction::Selected("allow".into())
            }
            Action::ConfirmDeny => {
                self.mode = Mode::Normal;
                KeyAction::Selected("deny".into())
            }
            Action::ConfirmToggle => {
                if let Mode::Confirm { allow, .. } = &mut self.mode {
                    *allow = !*allow;
                }
                KeyAction::None
            }
        }
    }

    fn modal_accept(&mut self) -> KeyAction {
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
                KeyAction::Selected(if allow { "allow".into() } else { "deny".into() })
            }
            Mode::Normal => self.take_submit(),
        }
    }

    fn modal_cancel(&mut self) -> KeyAction {
        match self.mode {
            Mode::Suggest { .. } => {
                self.input.clear();
                self.cursor = 0;
                self.mode = Mode::Normal;
                KeyAction::None
            }
            Mode::Confirm { .. } => KeyAction::Cancel,
            Mode::Normal => KeyAction::Interrupt,
            _ => {
                self.mode = Mode::Normal;
                KeyAction::Cancel
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> KeyAction {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return self.quit();
        }
        if matches!(self.mode, Mode::Select { .. }) {
            return self.handle_select_key(key);
        }
        if matches!(self.mode, Mode::Prompt { .. }) {
            return self.handle_prompt_key(key);
        }
        if matches!(self.mode, Mode::Suggest { .. }) {
            return self.handle_suggest_key(key);
        }
        if matches!(self.mode, Mode::Confirm { .. }) {
            return self.handle_confirm_key(key);
        }
        self.handle_normal_key(key)
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Esc => {
                if self.input.is_empty() {
                    KeyAction::Interrupt
                } else {
                    self.input.clear();
                    self.cursor = 0;
                    self.after_input_change();
                    KeyAction::None
                }
            }
            KeyCode::Char(c) => {
                self.input.insert(self.cursor, c);
                self.cursor += c.len_utf8();
                self.after_input_change();
                KeyAction::None
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    let prev = prev_char_boundary(&self.input, self.cursor);
                    self.input.remove(prev);
                    self.cursor = prev;
                }
                self.after_input_change();
                KeyAction::None
            }
            KeyCode::Left => {
                self.cursor = prev_char_boundary(&self.input, self.cursor);
                KeyAction::None
            }
            KeyCode::Right => {
                self.cursor = next_char_boundary(&self.input, self.cursor);
                KeyAction::None
            }
            KeyCode::Up => {
                self.scroll_up(1);
                KeyAction::None
            }
            KeyCode::Down => {
                self.scroll_down(1);
                KeyAction::None
            }
            KeyCode::PageUp => {
                self.scroll_up(self.viewport().max(1));
                KeyAction::None
            }
            KeyCode::PageDown => {
                self.scroll_down(self.viewport().max(1));
                KeyAction::None
            }
            KeyCode::Home => {
                self.jump_top();
                KeyAction::None
            }
            KeyCode::End => {
                self.jump_bottom();
                KeyAction::None
            }
            KeyCode::Enter => self.take_submit(),
            _ => KeyAction::None,
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
            let last = len.saturating_sub(1);
            *cursor = if delta < 0 {
                cursor.saturating_sub(delta.unsigned_abs())
            } else {
                cursor.saturating_add(delta.unsigned_abs()).min(last)
            };
        }
    }

    fn handle_select_key(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                KeyAction::Cancel
            }
            KeyCode::Up => {
                self.select_move(-1);
                KeyAction::None
            }
            KeyCode::Down => {
                self.select_move(1);
                KeyAction::None
            }
            KeyCode::PageUp => {
                self.select_move(-10);
                KeyAction::None
            }
            KeyCode::PageDown => {
                self.select_move(10);
                KeyAction::None
            }
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
            KeyCode::Enter => {
                let cursor = match &self.mode {
                    Mode::Select { cursor, .. } => *cursor,
                    _ => 0,
                };
                let item = self
                    .select_matches()
                    .get(cursor)
                    .map(|item| (*item).clone());
                self.mode = Mode::Normal;
                item.map_or(KeyAction::None, KeyAction::Selected)
            }
            _ => KeyAction::None,
        }
    }

    fn handle_prompt_key(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                KeyAction::Cancel
            }
            KeyCode::Enter => {
                let value = match &self.mode {
                    Mode::Prompt { value, .. } => value.clone(),
                    _ => String::new(),
                };
                self.mode = Mode::Normal;
                KeyAction::Prompted(value)
            }
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
            KeyCode::Esc => {
                self.input.clear();
                self.cursor = 0;
                self.mode = Mode::Normal;
                KeyAction::None
            }
            KeyCode::Up => {
                if let Mode::Suggest { cursor, .. } = &mut self.mode {
                    *cursor = cursor.saturating_sub(1);
                }
                KeyAction::None
            }
            KeyCode::Down => {
                if let Mode::Suggest { items, cursor, .. } = &mut self.mode {
                    *cursor = cursor.saturating_add(1).min(items.len().saturating_sub(1));
                }
                KeyAction::None
            }
            KeyCode::Tab => {
                if let Some(name) = self.highlighted_suggest() {
                    self.input = format!("/{name} ");
                    self.cursor = self.input.len();
                    self.mode = Mode::Normal;
                }
                KeyAction::None
            }
            KeyCode::Enter => {
                if let Some(name) = self.highlighted_suggest() {
                    self.input = format!("/{name}");
                    self.cursor = self.input.len();
                }
                self.mode = Mode::Normal;
                self.take_submit()
            }
            KeyCode::Char(c) => {
                self.input.insert(self.cursor, c);
                self.cursor += c.len_utf8();
                self.after_input_change();
                KeyAction::None
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    let prev = prev_char_boundary(&self.input, self.cursor);
                    self.input.remove(prev);
                    self.cursor = prev;
                }
                self.after_input_change();
                KeyAction::None
            }
            _ => KeyAction::None,
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Esc => KeyAction::Cancel,
            KeyCode::Char('y' | 'Y' | '1') => KeyAction::Selected("allow".into()),
            KeyCode::Char('n' | 'N' | '2') => KeyAction::Selected("deny".into()),
            KeyCode::Enter => {
                let allow = matches!(self.mode, Mode::Confirm { allow, .. } if allow);
                KeyAction::Selected(if allow { "allow".into() } else { "deny".into() })
            }
            KeyCode::Up => {
                if let Mode::Confirm { allow, .. } = &mut self.mode {
                    *allow = true;
                }
                KeyAction::None
            }
            KeyCode::Down => {
                if let Mode::Confirm { allow, .. } = &mut self.mode {
                    *allow = false;
                }
                KeyAction::None
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
                if let Mode::Confirm { allow, .. } = &mut self.mode {
                    *allow = !*allow;
                }
                KeyAction::None
            }
            _ => KeyAction::None,
        }
    }

    fn highlighted_suggest(&self) -> Option<String> {
        match &self.mode {
            Mode::Suggest { items, cursor } => items.get(*cursor).map(|item| item.name.clone()),
            _ => None,
        }
    }

    fn after_input_change(&mut self) {
        if self.input.starts_with('/')
            && !self.input.contains(' ')
            && self.state.borrow().opts().suggest_enabled
        {
            self.refresh_suggest();
        } else {
            self.mode = Mode::Normal;
        }
    }

    fn refresh_suggest(&mut self) {
        let query = self.input.trim_start_matches('/').to_lowercase();
        let items: Vec<SuggestItem> = self
            .suggest_pool
            .iter()
            .filter(|item| item.name.to_lowercase().starts_with(&query))
            .cloned()
            .collect();
        self.mode = Mode::Suggest { items, cursor: 0 };
    }

    fn quit(&mut self) -> KeyAction {
        self.running = false;
        KeyAction::Quit
    }

    fn take_submit(&mut self) -> KeyAction {
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
}

pub type Term = Terminal<CrosstermBackend<Stdout>>;

pub fn setup() -> io::Result<Term> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

pub fn restore(terminal: &mut Term) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()
}

pub fn resume(terminal: &mut Term) -> io::Result<()> {
    enable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        EnterAlternateScreen,
        EnableMouseCapture
    )?;
    terminal.clear()?;
    terminal.hide_cursor()
}

pub fn draw(terminal: &mut Term, app: &App) -> io::Result<()> {
    terminal.draw(|frame| ui::render(frame, app)).map(|_| ())
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
