use crate::model::Line;
use uji_agent::session::model::StoredMessage;

#[derive(Clone, Copy)]
pub enum Block<'a> {
    Notice(&'a str),
    Message(&'a StoredMessage),
    Pending(&'a str),
    Queued(&'a str),
    Thinking(&'a str),
}

pub trait BlockRenderer {
    fn render(&self, block: Block<'_>) -> Option<Vec<Line>>;
}
