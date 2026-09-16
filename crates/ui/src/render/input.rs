use crate::model::WindowSpec;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::Mode;
use crate::render::Context;
use crate::render::Render;
use crate::render::Surface;
use crate::render::modal::confirm;
use crate::render::style::{block_for, color_of};
use crate::render::wrap;

const CURSOR: char = '█';

pub(crate) struct Input<'a> {
    pub(crate) window: &'a WindowSpec,
}

impl Render for Input<'_> {
    fn render(&self, ctx: &Context<'_>, surface: &mut Surface<'_>) {
        match ctx.app.mode() {
            Mode::Confirm { title, body, allow } => {
                let block = block_for(self.window, ctx.palette);
                let inner = block
                    .as_ref()
                    .map_or(surface.area(), |block| block.inner(surface.area()));
                let lines = confirm::lines(ctx, title, body, confirm::choice(*allow), inner.width);
                let paragraph = Paragraph::new(lines);
                match block {
                    Some(block) => surface.render_widget(paragraph.block(block)),
                    None => surface.render_widget(paragraph),
                }
            }
            _ => self.render_input(ctx, surface),
        }
    }
}

impl Input<'_> {
    fn render_input(&self, ctx: &Context<'_>, surface: &mut Surface<'_>) {
        let block = block_for(self.window, ctx.palette);
        let inner = block
            .as_ref()
            .map_or(surface.area(), |block| block.inner(surface.area()));
        let height = usize::from(inner.height).max(1);

        let text_style = ctx.state.opts().input_color.map_or_else(
            || Style::default().fg(ctx.palette.text),
            |color| Style::default().fg(color_of(color)),
        );
        let mut cursor_style = Style::default().fg(ctx.palette.cursor);
        if ctx.state.opts().cursor_blink {
            cursor_style = cursor_style.add_modifier(Modifier::SLOW_BLINK);
        }

        let input = ctx.app.input();
        let cursor = input[..ctx.app.cursor_offset()].chars().count();
        let display = with_cursor(input, cursor);
        let rows = wrap::ranges(&display, usize::from(inner.width));
        let cursor_row = rows
            .iter()
            .position(|(start, end)| (*start..*end).contains(&cursor))
            .unwrap_or(0);

        let lines: Vec<Line<'static>> = rows
            .iter()
            .skip(cursor_row.saturating_sub(height - 1))
            .take(height)
            .map(|&(start, end)| {
                let row = &display[start..end];
                if !(start..end).contains(&cursor) {
                    return Line::from(Span::styled(collect(row), text_style));
                }
                let split = cursor - start;
                Line::from(vec![
                    Span::styled(collect(&row[..split]), text_style),
                    Span::styled(String::from(CURSOR), cursor_style),
                    Span::styled(collect(&row[split + 1..]), text_style),
                ])
            })
            .collect();

        let paragraph = Paragraph::new(lines);
        let paragraph = match block {
            Some(block) => paragraph.block(block),
            None => paragraph,
        };
        surface.render_widget(paragraph);
    }
}

pub(crate) fn rows_needed(input: &str, width: usize) -> usize {
    let display = with_cursor(input, input.chars().count());
    wrap::ranges(&display, width).len()
}

fn with_cursor(input: &str, at: usize) -> Vec<char> {
    let mut display: Vec<char> = input.chars().collect();
    display.insert(at.min(display.len()), CURSOR);
    display
}

fn collect(chars: &[char]) -> String {
    chars.iter().collect()
}
