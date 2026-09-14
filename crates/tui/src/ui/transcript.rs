use ratatui::text::Line;
use uji_core::session::conversation::Conversation;
use uji_core::session::model::{Message, StoredMessage};

use crate::app::renderer::Block;
use crate::ui::style::Palette;

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
struct Cached {
    key: String,
    lines: Vec<Line<'static>>,
}

impl Cached {
    fn clear(&mut self) {
        self.key.clear();
        self.lines.clear();
    }

    fn get(&mut self, key: &str, render: impl FnOnce(&mut Vec<Line<'static>>)) -> &[Line<'static>] {
        if self.key != key {
            self.clear();
            self.key.push_str(key);
            if !key.is_empty() {
                render(&mut self.lines);
            }
        }
        &self.lines
    }
}

#[derive(Default)]
struct CachedList {
    key: Vec<String>,
    lines: Vec<Line<'static>>,
}

impl CachedList {
    fn clear(&mut self) {
        self.key.clear();
        self.lines.clear();
    }

    fn get(
        &mut self,
        key: &[String],
        render: impl FnOnce(&mut Vec<Line<'static>>),
    ) -> &[Line<'static>] {
        if self.key != key {
            self.clear();
            self.key.extend_from_slice(key);
            render(&mut self.lines);
        }
        &self.lines
    }
}

pub struct Input<'a> {
    pub conversation: &'a Conversation,
    pub notices: &'a [String],
    pub pending: &'a str,
    pub width: usize,
    pub palette: Palette,
}

pub struct Rendered<'a> {
    pub notices: &'a [Line<'static>],
    pub folded: &'a [Line<'static>],
    pub pending: &'a [Line<'static>],
}

#[derive(Default)]
pub struct Transcript {
    width: usize,
    palette: Palette,
    folded: usize,
    last_seq: i64,
    grouping: Grouping,
    lines: Vec<Line<'static>>,
    notices: CachedList,
    pending: Cached,
}

impl Transcript {
    pub(crate) fn frame(
        &mut self,
        input: &Input<'_>,
        render: impl Fn(&mut Vec<Line<'static>>, Block<'_>, usize),
    ) -> Rendered<'_> {
        if self.width != input.width || self.palette != input.palette {
            self.width = input.width;
            self.palette = input.palette;
            self.reset();
            self.notices.clear();
            self.pending.clear();
        }
        let width = input.width;
        self.fold(input.conversation, width, &render);
        self.notices.get(input.notices, |lines| {
            for notice in input.notices {
                render(lines, Block::Notice(notice), width);
            }
        });
        self.pending.get(input.pending, |lines| {
            render(lines, Block::Pending(input.pending), width);
        });
        Rendered {
            notices: &self.notices.lines,
            folded: &self.lines,
            pending: &self.pending.lines,
        }
    }

    fn fold(
        &mut self,
        conversation: &Conversation,
        width: usize,
        render: &impl Fn(&mut Vec<Line<'static>>, Block<'_>, usize),
    ) {
        let messages = conversation.messages();
        if !self.continues(messages) {
            self.reset();
        }
        for stored in &messages[self.folded..] {
            if self.grouping.separates(stored) {
                self.lines.push(Line::from(""));
            }
            render(&mut self.lines, Block::Message(stored), width);
            self.last_seq = stored.seq;
        }
        self.folded = messages.len();
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
