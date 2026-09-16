use crate::model::Line;
use uji_agent::session::model::StoredMessage;

#[derive(Clone, Copy)]
pub enum Block<'a> {
    Notice(&'a str),
    Message(&'a StoredMessage),
    /// Part of the message still streaming in. `continuing` marks a chunk
    /// that follows one already rendered, so the blank line between blocks
    /// survives the join.
    Pending {
        text: &'a str,
        continuing: bool,
    },
    Queued(&'a str),
    Thinking(&'a str),
}

pub trait BlockRenderer {
    fn render(&self, block: Block<'_>) -> Option<Vec<Line>>;

    /// Whether anything is installed to override a block at all.
    ///
    /// A block handed to an override has to arrive whole, which rules out
    /// rendering the streaming message a piece at a time.
    fn overrides(&self) -> bool;
}
