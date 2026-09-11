use ratatui::text::Line;
use ratatui::widgets::Clear;

use crate::ui::Context;
use crate::ui::Render;
use crate::ui::Surface;
use crate::ui::layout::centered_rect;
use crate::ui::modal::menu::Menu;

pub(crate) struct Prompt<'a> {
    pub(crate) title: &'a str,
    pub(crate) value: &'a str,
    pub(crate) secret: bool,
}

impl Render for Prompt<'_> {
    fn render(&self, ctx: &Context<'_>, surface: &mut Surface<'_>) {
        let popup = centered_rect(surface.area(), 64, 6);
        surface.render_at(popup, Clear);

        let shown = if self.secret {
            "•".repeat(self.value.chars().count())
        } else {
            self.value.to_string()
        };

        Menu {
            area: popup,
            title: self.title,
            rows: vec![Line::raw(format!("{shown} █"))],
        }
        .render(ctx, surface);
    }
}
