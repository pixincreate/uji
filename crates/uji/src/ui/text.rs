use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use uji_api::model::WindowSpec;

use crate::ui::Context;
use crate::ui::Render;
use crate::ui::Surface;
use crate::ui::style::block_for;

pub(crate) struct Text<'a> {
    pub(crate) window: &'a WindowSpec,
}

impl Render for Text<'_> {
    fn render(&self, _ctx: &Context<'_>, surface: &mut Surface<'_>) {
        let block = block_for(self.window);
        let lines = self
            .window
            .lines
            .iter()
            .map(|line| Line::raw(line.clone()))
            .collect::<Vec<_>>();
        let paragraph = Paragraph::new(lines);
        let paragraph = match block {
            Some(block) => paragraph.block(block),
            None => paragraph,
        };
        surface.render_widget(paragraph);
    }
}
