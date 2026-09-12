pub mod action;
pub mod keys;
pub mod mode;

pub use action::{Action, KeyAction, filter_items};
pub use mode::{Mode, SuggestItem};

use std::cell::{Cell, Ref, RefCell};
use std::rc::Rc;

use uji_screen::keymap;
use uji_screen::state::UiState;

use uji_core::session::conversation::{Conversation, Shared};
use uji_core::session::model::Session;

const REVEAL_EASE: usize = 6;
const REVEAL_MIN: usize = 3;
const REVEAL_MAX: usize = 120;
const REVEAL_BURST: usize = 4096;

pub struct App {
    session: Session,
    conversation: Shared,
    state: Rc<RefCell<UiState>>,
    input: String,
    cursor: usize,
    pending: Option<String>,
    revealed: usize,
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
            pending: None,
            revealed: 0,
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

    pub fn set_title(&mut self, title: String) {
        self.session.title = title;
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

    pub fn pending(&self) -> Option<&str> {
        let text = self.pending.as_deref()?;
        Some(&text[..self.revealed.min(text.len())])
    }

    pub fn revealing(&self) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|text| self.revealed < text.len())
    }

    pub fn reveal_all(&mut self) {
        self.revealed = self.pending.as_ref().map_or(0, String::len);
    }

    pub fn reveal_step(&mut self) -> bool {
        let Some(text) = self.pending.as_deref() else {
            return false;
        };
        let backlog = text.len().saturating_sub(self.revealed);
        if backlog == 0 {
            return false;
        }
        let step = if backlog > REVEAL_BURST {
            backlog
        } else {
            backlog.div_ceil(REVEAL_EASE).clamp(REVEAL_MIN, REVEAL_MAX)
        };
        let mut at = self.revealed.saturating_add(step).min(text.len());
        while !text.is_char_boundary(at) {
            at = at.saturating_add(1);
        }
        self.revealed = at;
        true
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
        self.revealed = 0;
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
}
