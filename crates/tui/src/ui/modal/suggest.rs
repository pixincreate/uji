use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use crate::app::SuggestItem;
use crate::ui::Context;
use crate::ui::Render;
use crate::ui::Surface;

pub(crate) struct Suggest<'a> {
    pub(crate) items: &'a [SuggestItem],
    pub(crate) cursor: usize,
}

impl Render for Suggest<'_> {
    fn render(&self, ctx: &Context<'_>, surface: &mut Surface<'_>) {
        let area = surface.area();
        if self.items.is_empty() || area.height == 0 {
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

        let height = u16::try_from(window.len()).unwrap_or(1).min(area.height);
        let popup = Rect {
            x: area.x,
            y: area.y + area.height.saturating_sub(height),
            width: area.width,
            height,
        };
        surface.render_at(popup, Clear);

        let lines: Vec<Line<'_>> = window
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let selected = i == cursor_in_window;
                let row = if selected {
                    Style::default().bg(ctx.palette.selected_bg)
                } else {
                    Style::default()
                };
                let name = Span::styled(
                    format!("  {:<12}", item.name),
                    if selected {
                        Style::default().fg(ctx.palette.accent)
                    } else {
                        Style::default().fg(ctx.palette.text)
                    },
                );
                let desc = Span::styled(item.desc.clone(), Style::default().fg(ctx.palette.muted));
                Line::from(vec![name, desc]).style(row)
            })
            .collect();
        surface.render_at(popup, Paragraph::new(lines));
    }
}
