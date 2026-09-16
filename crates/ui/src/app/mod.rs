pub mod action;
pub mod composer;
pub mod keys;
pub mod mode;
pub mod overlay;
pub mod paste;
pub mod renderer;
pub mod scroll;
pub mod selection;
pub mod stream;

pub use action::{Action, KeyAction, rank_items};
pub use composer::Composer;
pub use mode::{Echo, Mode, SuggestItem};
pub use scroll::Scroll;
pub use stream::Stream;

use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use crate::keymap;
use crate::state::UiState;

use uji_engine::session::conversation::{Conversation, Shared};
use uji_engine::session::model::{Message, Session};

pub struct App {
    session: Session,
    conversation: Shared,
    state: Rc<RefCell<UiState>>,
    composer: Composer,
    stream: Stream,
    scroll: Scroll,
    mode: Mode,
    suggest_pool: Vec<SuggestItem>,
    overlay: overlay::Overlay,
    transcript: RefCell<crate::render::transcript::Transcript>,
    renderer: Option<Rc<dyn renderer::BlockRenderer>>,
}

impl App {
    pub fn new(session: Session, conversation: Shared, state: Rc<RefCell<UiState>>) -> Self {
        Self {
            session,
            conversation,
            state,
            composer: Composer::default(),
            stream: Stream::default(),
            scroll: Scroll::default(),
            mode: Mode::Normal,
            suggest_pool: Vec::new(),
            overlay: overlay::Overlay::default(),
            transcript: RefCell::default(),
            renderer: None,
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

    pub fn transcript(&self) -> RefMut<'_, crate::render::transcript::Transcript> {
        self.transcript.borrow_mut()
    }

    pub fn set_renderer(&mut self, renderer: Rc<dyn renderer::BlockRenderer>) {
        self.renderer = Some(renderer);
    }

    pub fn renderer(&self) -> Option<&Rc<dyn renderer::BlockRenderer>> {
        self.renderer.as_ref()
    }

    pub fn state(&self) -> &Rc<RefCell<UiState>> {
        &self.state
    }

    pub fn input(&self) -> &str {
        self.composer.text()
    }

    pub fn cursor_offset(&self) -> usize {
        self.composer.cursor()
    }

    pub fn toggle_thinking(&mut self) -> KeyAction {
        let shown = self.state.borrow_mut().toggle_thinking();
        self.overlay_mut().push_notices(vec![String::from(if shown {
            "thinking: shown"
        } else {
            "thinking: hidden"
        })]);
        KeyAction::None
    }

    pub fn paste(&mut self, text: &str) {
        self.composer.paste(text);
        self.mode = Mode::Normal;
    }

    pub fn set_input(&mut self, text: String) {
        self.composer.set(text);
        self.after_input_change();
    }

    pub fn pending(&self) -> Option<&str> {
        Some(self.stream.visible()).filter(|text| !text.is_empty())
    }

    pub fn append_pending(&mut self, delta: &str) {
        self.stream.push(delta);
    }

    pub fn take_pending(&mut self) {
        self.stream.clear();
    }

    pub fn revealing(&self) -> bool {
        self.stream.revealing()
    }

    pub fn reveal_all(&mut self) {
        self.stream.reveal_all();
    }

    pub fn reveal_step(&mut self) -> bool {
        self.stream.reveal_step()
    }

    pub fn resolve_scroll(&self, max: usize, viewport: usize) -> usize {
        self.scroll.resolve(max, viewport)
    }

    pub fn viewport(&self) -> usize {
        self.scroll.viewport()
    }

    pub fn reset_scroll(&self) {
        self.scroll.follow();
    }

    pub fn scroll_up(&self, lines: usize) {
        self.scroll.up(lines);
    }

    pub fn scroll_down(&self, lines: usize) {
        self.scroll.down(lines);
    }

    pub fn jump_top(&self) {
        self.scroll.top();
    }

    pub fn jump_bottom(&self) {
        self.scroll.follow();
    }

    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    pub fn overlay(&self) -> &overlay::Overlay {
        &self.overlay
    }

    pub fn overlay_mut(&mut self) -> &mut overlay::Overlay {
        &mut self.overlay
    }

    pub fn open_select(&mut self, title: String, items: Vec<String>) {
        let matches = (0..items.len()).collect();
        self.mode = Mode::Select {
            title,
            items,
            query: String::new(),
            cursor: 0,
            matches,
        };
    }

    pub fn open_prompt(&mut self, title: String, value: String, echo: Echo) {
        self.mode = Mode::Prompt { title, value, echo };
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
        let input = self.composer.text();
        if input.starts_with('/')
            && !input.contains(' ')
            && self.state.borrow().opts().suggest_enabled
        {
            self.refresh_suggest();
        } else {
            self.mode = Mode::Normal;
        }
    }

    fn refresh_suggest(&mut self) {
        let query = self.composer.text().trim_start_matches('/').to_lowercase();
        let items: Vec<SuggestItem> = self
            .suggest_pool
            .iter()
            .filter(|item| item.name.to_lowercase().starts_with(&query))
            .cloned()
            .collect();
        self.mode = Mode::Suggest { items, cursor: 0 };
    }
}

fn sent(conversation: &Conversation, back: usize) -> Option<String> {
    conversation
        .messages()
        .iter()
        .rev()
        .filter_map(|stored| match &stored.message {
            Message::User { text } => Some(text),
            _ => None,
        })
        .nth(back)
        .cloned()
}
