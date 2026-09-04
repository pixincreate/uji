use ratatui::prelude::*;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use uji_core::session::model::{Message, StoredMessage};
use uji_core::session::store::SessionStorage;

use crate::app::App;

/// Render the TUI: message history on top, input box at the bottom.
pub fn render<S: SessionStorage>(frame: &mut Frame, app: &App<'_, S>) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    // Top: message history.
    let messages: Vec<_> = app
        .messages
        .iter()
        .map(|stored| Line::from(render_message(stored)))
        .collect();
    frame.render_widget(Paragraph::new(messages), chunks[0]);

    // Bottom: input box.
    let block = Block::default().borders(Borders::ALL);
    let paragraph = Paragraph::new(render_input(app)).block(block);
    frame.render_widget(paragraph, chunks[1]);
}

fn render_message(stored: &StoredMessage) -> String {
    match &stored.message {
        Message::User { text } => format!("> {text}"),
        Message::Assistant { text } => format!("  {text}"),
        Message::System { text } => format!("[{text}]"),
    }
}

/// Render the text buffer with a blinking cursor at the cursor position.
fn render_input<S: SessionStorage>(app: &App<'_, S>) -> Line<'static> {
    let before = &app.input[..app.cursor];
    let after = app.input[app.cursor..].to_owned();

    let cursor = Span::styled("█", Style::default().add_modifier(Modifier::SLOW_BLINK));

    Line::from(vec![Span::raw(before.to_owned()), cursor, Span::raw(after)])
}
