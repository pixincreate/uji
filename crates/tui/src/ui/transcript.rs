use ratatui::text::Line;
use uji_core::session::conversation::Conversation;
use uji_core::session::model::{Message, StoredMessage};

#[derive(Debug, Default, Clone, Copy)]
struct Grouping {
    started: bool,
    open_group: bool,
}

impl Grouping {
    fn separates(&mut self, stored: &StoredMessage) -> bool {
        let continues = self.open_group && matches!(stored.message, Message::Tool { .. });
        let separates = self.started && !continues;
        self.started = true;
        self.open_group = opens_tool_group(stored);
        separates
    }
}

fn opens_tool_group(stored: &StoredMessage) -> bool {
    match &stored.message {
        Message::Assistant { tool_calls, .. } => !tool_calls.is_empty(),
        Message::Tool { .. } => true,
        _ => false,
    }
}

#[derive(Default)]
struct Committed {
    text: String,
    lines: Vec<Line<'static>>,
}

impl Committed {
    fn clear(&mut self) {
        self.text.clear();
        self.lines.clear();
    }
}

#[derive(Default)]
pub struct Transcript {
    width: usize,
    folded: usize,
    last_seq: i64,
    grouping: Grouping,
    lines: Vec<Line<'static>>,
    committed: Committed,
}

impl Transcript {
    pub(crate) fn frame(
        &mut self,
        conversation: &Conversation,
        committed: &str,
        width: usize,
        message: impl Fn(&mut Vec<Line<'static>>, &StoredMessage, usize),
        markdown: impl Fn(&mut Vec<Line<'static>>, &str, usize),
    ) -> (&[Line<'static>], &[Line<'static>]) {
        if self.width != width {
            self.width = width;
            self.reset();
            self.committed.clear();
        }
        self.fold(conversation, width, message);
        self.commit(committed, width, markdown);
        (&self.lines, &self.committed.lines)
    }

    fn fold(
        &mut self,
        conversation: &Conversation,
        width: usize,
        render: impl Fn(&mut Vec<Line<'static>>, &StoredMessage, usize),
    ) {
        let messages = conversation.messages();
        if !self.continues(messages) {
            self.reset();
        }
        for stored in &messages[self.folded..] {
            if self.grouping.separates(stored) {
                self.lines.push(Line::from(""));
            }
            render(&mut self.lines, stored, width);
            self.last_seq = stored.seq;
        }
        self.folded = messages.len();
    }

    fn commit(
        &mut self,
        text: &str,
        width: usize,
        render: impl Fn(&mut Vec<Line<'static>>, &str, usize),
    ) {
        if self.committed.text == text {
            return;
        }
        self.committed.clear();
        self.committed.text.push_str(text);
        if !text.is_empty() {
            render(&mut self.committed.lines, text, width);
        }
    }

    fn reset(&mut self) {
        self.folded = 0;
        self.last_seq = 0;
        self.grouping = Grouping::default();
        self.lines.clear();
    }

    fn continues(&self, messages: &[StoredMessage]) -> bool {
        if self.folded > messages.len() {
            return false;
        }
        match self.folded.checked_sub(1) {
            None => true,
            Some(at) => messages[at].seq == self.last_seq,
        }
    }
}
