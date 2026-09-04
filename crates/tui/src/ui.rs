use ratatui::prelude::*;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;

/// Render the TUI: a single input box anchored to the bottom of the screen.
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    let block = Block::default().borders(Borders::ALL);
    let paragraph = Paragraph::new(render_input(app)).block(block);
    frame.render_widget(paragraph, chunks[1]);
}

/// Render the text buffer with a blinking cursor at the cursor position.
fn render_input(app: &App) -> Line<'static> {
    let before = &app.input[..app.cursor];
    let after = &app.input[app.cursor..];

    let cursor = Span::styled("█", Style::default().add_modifier(Modifier::SLOW_BLINK));

    Line::from(vec![
        Span::raw(before.to_owned()),
        cursor,
        Span::raw(after.to_owned()),
    ])
}
