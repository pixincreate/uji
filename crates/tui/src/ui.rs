//! Renderer: projects the UI model into terminal widgets.

use ratatui::prelude::*;
use ratatui::style::Modifier;
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use uji_core::session::model::{Message, StoredMessage};
use uji_core::session::store::SessionStorage;

use crate::app::App;
use crate::model::{self, Border, BufferKind, WindowSpec};

// Pi dark-theme message colors.
const USER_BG: Color = Color::Rgb(0x34, 0x35, 0x41);
const TEXT: Color = Color::Rgb(0xd4, 0xd4, 0xd4);
const MUTED: Color = Color::Rgb(0x80, 0x80, 0x80);

/// Render the TUI purely from the UI model: walk the declared windows and
/// render whatever buffer each one views.
pub fn render<S: SessionStorage>(frame: &mut Frame<'_>, app: &App<'_, S>) {
    let rects = model::layout(frame.area(), &app.model().windows);
    for (win, area) in app.model().windows.iter().zip(rects) {
        render_window(frame, app, win, area);
    }
}

fn render_window<S: SessionStorage>(
    frame: &mut Frame<'_>,
    app: &App<'_, S>,
    win: &WindowSpec,
    area: Rect,
) {
    let block = block_for(win);
    let inner = block.as_ref().map_or(area, |b| b.inner(area));
    let kind = app
        .model()
        .buffer_kind(&win.buffer)
        .unwrap_or(BufferKind::Messages);

    let paragraph = match kind {
        BufferKind::Messages => Paragraph::new(render_messages(app.messages(), inner.width)),
        BufferKind::Input => Paragraph::new(render_input(app)),
    };
    let paragraph = match block {
        Some(block) => paragraph.block(block),
        None => paragraph,
    };
    frame.render_widget(paragraph, area);
}

fn block_for(win: &WindowSpec) -> Option<Block<'static>> {
    let mut block = match win.opts.border {
        Border::None => return None,
        Border::Plain => Block::default().borders(Borders::ALL),
        Border::Rounded => Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED),
    };
    if let Some(title) = &win.opts.title {
        block = block.title(title.clone());
    }
    Some(block)
}

/// Render messages the way pi does:
///
/// - user messages: full-width block on `userMessageBg`, text-colored, one
///   padded line above/below and one column left/right
/// - assistant messages: no background, one blank spacer line before, one
///   column left pad
/// - system messages: italic muted text, same spacing as assistant
fn render_messages(messages: &[StoredMessage], width: u16) -> Vec<Line<'static>> {
    // Full-width space fill; Paragraph clips lines at the area width, so
    // padding with `fill` guarantees the bg spans the whole row.
    let fill = " ".repeat(usize::from(width));
    let mut lines = Vec::new();
    for stored in messages {
        match &stored.message {
            Message::User { text } => {
                let block_style = Style::default().bg(USER_BG).fg(TEXT);
                lines.push(Line::from(fill.clone()).style(block_style));
                for line in text.lines() {
                    lines.push(Line::from(format!(" {line} {fill}")).style(block_style));
                }
                lines.push(Line::from(fill.clone()).style(block_style));
            }
            Message::Assistant { text } => {
                lines.push(Line::from(""));
                let text_style = Style::default().fg(TEXT);
                for line in text.lines() {
                    lines.push(Line::from(format!(" {line}")).style(text_style));
                }
            }
            Message::System { text } => {
                lines.push(Line::from(""));
                let muted = Style::default().fg(MUTED).add_modifier(Modifier::ITALIC);
                for line in text.lines() {
                    lines.push(Line::from(format!(" {line}")).style(muted));
                }
            }
        }
    }
    lines
}

/// Render the text buffer with a cursor (blinking per `uji.opt.cursor_blink`)
/// at the cursor position.
fn render_input<S: SessionStorage>(app: &App<'_, S>) -> Line<'static> {
    let cursor_offset = app.cursor_offset();
    let before = &app.input()[..cursor_offset];
    let after = app.input()[cursor_offset..].to_owned();

    let mut cursor_style = Style::default();
    if app.model().opts.cursor_blink {
        cursor_style = cursor_style.add_modifier(Modifier::SLOW_BLINK);
    }
    let cursor = Span::styled("█", cursor_style);

    Line::from(vec![Span::raw(before.to_owned()), cursor, Span::raw(after)])
}
