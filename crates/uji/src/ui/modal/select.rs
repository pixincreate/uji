use ratatui::text::Line;
use ratatui::widgets::Clear;

use crate::ui::Context;
use crate::ui::Render;
use crate::ui::Surface;
use crate::ui::layout::centered_rect;
use crate::ui::modal::menu::Menu;
use crate::ui::style::accent_style;

pub(crate) struct Select<'a> {
    pub(crate) title: &'a str,
    pub(crate) items: &'a [String],
    pub(crate) cursor: usize,
}

impl Render for Select<'_> {
    fn render(&self, ctx: &Context<'_>, surface: &mut Surface<'_>) {
        let rows = u16::try_from(self.items.len()).unwrap_or(1);
        let popup = centered_rect(surface.area(), 64, rows + 5);
        surface.render_at(popup, Clear);

        let lines: Vec<Line<'static>> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                if i == self.cursor {
                    Line::styled(item.clone(), accent_style())
                } else {
                    Line::raw(item.clone())
                }
            })
            .collect();

        Menu {
            area: popup,
            title: self.title,
            rows: lines,
        }
        .render(ctx, surface);
    }
}
