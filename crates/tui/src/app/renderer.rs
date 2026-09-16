use uji_core::session::model::StoredMessage;
use uji_screen::model::Line;

#[derive(Clone, Copy)]
pub enum Block<'a> {
    Notice(&'a str),
    Message(&'a StoredMessage),
    Pending(&'a str),
    Queued(&'a str),
}

pub trait BlockRenderer {
    fn render(&self, block: Block<'_>) -> Option<Vec<Line>>;
}
