use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use crate::app::SuggestItem;
use crate::ui::Context;
use crate::ui::Render;
use crate::ui::Surface;
use crate::ui::style::{MUTED, SELECTED_BG, TEXT};

pub(crate) struct Suggest<'a> {
    pub(crate) items: &'a [SuggestItem],
    pub(crate) cursor: usize,
    pub(crate) input_rect: Option<Rect>,
}

impl Render for Suggest<'_> {
    fn render(&self, ctx: &Context<'_>, surface: &mut Surface<'_>) {
        let Some(input_rect) = self.input_rect else {
            return;
        };
        if self.items.is_empty() {
            return;
        }
        let max = usize::from(ctx.state.opts().suggest_max_height).max(1);
        let visible = self.items.len().min(max);
        let start = if self.cursor >= visible {
            self.cursor.saturating_sub(visible - 1)
        } else {
            0
        };
        let window = &self.items[start..(start + visible).min(self.items.len())];
        let cursor_in_window = self.cursor - start;

        let height = u16::try_from(window.len()).unwrap_or(1);
        let popup = Rect {
            x: input_rect.x,
            y: input_rect.y.saturating_sub(height),
            width: input_rect.width,
            height,
        };
        surface.render_at(popup, Clear);

        let lines: Vec<Line<'_>> = window
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let selected = i == cursor_in_window;
                let row = if selected {
                    Style::default().bg(SELECTED_BG)
                } else {
                    Style::default()
                };
                let name = Span::styled(
                    format!(" {:<12}", item.name),
                    if selected {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default().fg(TEXT)
                    },
                );
                let desc = Span::styled(item.desc.clone(), Style::default().fg(MUTED));
                Line::from(vec![name, desc]).style(row)
            })
            .collect();
        surface.render_at(popup, Paragraph::new(lines));
    }
}
