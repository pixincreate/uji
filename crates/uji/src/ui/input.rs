use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::ui::Context;
use crate::ui::Render;
use crate::ui::Surface;

pub(crate) struct Input;

impl Render for Input {
    fn render(&self, ctx: &Context<'_>, surface: &mut Surface<'_>) {
        let cursor_offset = ctx.app.cursor_offset();
        let before = &ctx.app.input()[..cursor_offset];
        let after = ctx.app.input()[cursor_offset..].to_owned();

        let mut cursor_style = Style::default().white();
        if ctx.state.opts().cursor_blink {
            cursor_style = cursor_style.add_modifier(Modifier::SLOW_BLINK);
        }
        let cursor = Span::styled("█", cursor_style);
        let line = Line::from(vec![Span::raw(before.to_owned()), cursor, Span::raw(after)]);

        let block = Block::default()
            .borders(Borders::TOP | Borders::BOTTOM)
            .style(Color::LightMagenta);
        surface.render_widget(Paragraph::new(line).block(block));
    }
}
