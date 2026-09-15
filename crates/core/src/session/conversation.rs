use std::cell::RefCell;
use std::rc::Rc;

use crate::llm::Usage;

use super::model::{Message, Session, StoredMessage};

pub type Shared = Rc<RefCell<Conversation>>;

#[derive(Default, Clone)]
pub struct Info {
    pub id: String,
    pub title: String,
    pub directory: String,
}

impl From<&Session> for Info {
    fn from(session: &Session) -> Self {
        Self {
            id: session.id.to_string(),
            title: session.title.clone(),
            directory: session.directory.clone(),
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct Tally {
    pub usage: Usage,
    pub turns: u64,
}

#[derive(Default)]
pub struct Conversation {
    info: Info,
    messages: Vec<StoredMessage>,
    tally: Tally,
    reported_input: u64,
    reported_seq: i64,
}

impl Conversation {
    pub fn shared() -> Shared {
        Rc::new(RefCell::new(Self::default()))
    }

    pub fn attach(&mut self, session: &Session, messages: Vec<StoredMessage>) {
        self.info = Info::from(session);
        self.messages = messages;
        self.forget_reported_input();
    }

    pub fn info(&self) -> &Info {
        &self.info
    }

    pub fn set_title(&mut self, title: String) {
        self.info.title = title;
    }

    pub fn messages(&self) -> &[StoredMessage] {
        &self.messages
    }

    pub fn push(&mut self, message: StoredMessage) {
        if matches!(message.message, Message::Compaction { .. }) {
            self.forget_reported_input();
        }
        self.messages.push(message);
    }

    pub fn reported_input(&self) -> Option<(u64, i64)> {
        (self.reported_input > 0).then_some((self.reported_input, self.reported_seq))
    }

    fn forget_reported_input(&mut self) {
        self.reported_input = 0;
        self.reported_seq = 0;
    }

    pub fn add_cost(&mut self, usage: Usage) {
        self.tally.usage.add(usage);
    }

    pub fn add_usage(&mut self, usage: Usage) {
        if usage.input > 0 {
            self.reported_input = usage.input;
            self.reported_seq = self.messages.last().map_or(0, |stored| stored.seq);
        }
        self.tally.usage.add(usage);
        self.tally.turns = self.tally.turns.saturating_add(1);
    }

    pub fn tally(&self) -> Tally {
        self.tally
    }
}
