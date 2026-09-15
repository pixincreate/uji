use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line as TLine;
use ratatui::widgets::{Paragraph, Widget};
use uji_screen::model::{Builtin, Size, WindowSpec};
use uji_screen::state::UiState;

use crate::app::App;

pub mod buffer;
pub mod input;
pub mod layout;
pub mod markdown;
pub mod messages;
pub mod modal;
pub mod style;
pub mod transcript;
pub mod wrap;

pub struct Context<'a> {
    pub app: &'a App,
    pub state: &'a UiState,
    pub palette: style::Palette,
}

pub struct Surface<'a> {
    area: Rect,
    buf: &'a mut Buffer,
}

impl<'a> Surface<'a> {
    pub fn new(area: Rect, buf: &'a mut Buffer) -> Self {
        Self { area, buf }
    }

    pub fn area(&self) -> Rect {
        self.area
    }

    pub fn buf(&mut self) -> &mut Buffer {
        &mut *self.buf
    }

    pub fn render_widget(&mut self, widget: impl Widget) {
        widget.render(self.area, self.buf);
    }

    pub fn render_at(&mut self, area: Rect, widget: impl Widget) {
        widget.render(area, self.buf);
    }
}

pub trait Render {
    fn render(&self, ctx: &Context<'_>, surface: &mut Surface<'_>);
}

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let state = app.state();
    let fitted = {
        let s = state.borrow();
        fits(frame.area(), app, &s)
    };
    {
        let mut s = state.borrow_mut();
        for (id, rows) in fitted {
            s.set_window_fitted(id, rows);
        }
    }
    let rects = {
        let s = state.borrow();
        layout::layout(frame.area(), s.windows())
    };
    let s = state.borrow();
    let rect_of = |builtin: Builtin| {
        s.windows()
            .iter()
            .zip(rects.iter().copied())
            .find(|(window, _)| window.builtin == Some(builtin))
            .map(|(_, rect)| rect)
    };
    let modal_rect = rect_of(Builtin::Modal);
    let input_rect = rect_of(Builtin::Input);
    let palette = style::Palette::of(&s.opts().theme);
    let ctx = Context {
        app,
        state: &s,
        palette,
    };
    for (window, rect) in s.windows().iter().zip(rects.iter().copied()) {
        let mut surface = Surface::new(rect, frame.buffer_mut());
        match window.builtin {
            Some(Builtin::Messages) => {
                messages::Messages { window }.render(&ctx, &mut surface);
            }
            Some(Builtin::Input) => {
                input::Input { window }.render(&ctx, &mut surface);
            }
            Some(Builtin::Modal) => {}
            None => blit(&mut surface, window, palette),
        }
    }
    let area = modal_rect.unwrap_or_else(|| above(frame.area(), input_rect));
    let mut surface = Surface::new(area, frame.buffer_mut());
    modal::Modal.render(&ctx, &mut surface);
}

const MAX_INPUT_ROWS: u16 = 10;

fn fits(area: Rect, app: &App, state: &UiState) -> Vec<(u32, u16)> {
    let rects = layout::layout(area, state.windows());
    let palette = style::Palette::of(&state.opts().theme);
    let ctx = Context {
        app,
        state,
        palette,
    };
    let mut fitted = Vec::new();
    for (window, rect) in state.windows().iter().zip(rects) {
        if window.opts.size != Size::Auto {
            continue;
        }
        let chrome = style::vertical_chrome(window);
        let content = match window.opts.border {
            uji_screen::model::Border::Plain | uji_screen::model::Border::Rounded => {
                rect.width.saturating_sub(2)
            }
            _ => rect.width,
        };
        if rect.width == 0 {
            continue;
        }
        let rows = match window.builtin {
            Some(Builtin::Input) => modal::takeover_rows(&ctx, content).unwrap_or_else(|| {
                let rows = input::rows_needed(app.input(), usize::from(content));
                u16::try_from(rows)
                    .unwrap_or(MAX_INPUT_ROWS)
                    .clamp(1, MAX_INPUT_ROWS)
            }),
            Some(Builtin::Modal) => modal::rows(&ctx, content).unwrap_or(0),
            Some(Builtin::Messages) => continue,
            None => u16::try_from(window.buffer.len()).unwrap_or(u16::MAX),
        };
        let total = if rows == 0 {
            0
        } else {
            rows.saturating_add(chrome)
        };
        fitted.push((window.id, total));
    }
    fitted
}

fn above(area: Rect, input: Option<Rect>) -> Rect {
    match input {
        Some(input) if input.y > area.y => Rect {
            height: input.y.saturating_sub(area.y),
            ..area
        },
        _ => area,
    }
}

fn blit(surface: &mut Surface<'_>, window: &WindowSpec, palette: style::Palette) {
    let block = style::block_for(window, palette);
    let inner = block
        .as_ref()
        .map_or(surface.area(), |block| block.inner(surface.area()));
    let width = usize::from(inner.width);
    let lines: Vec<TLine<'static>> = window
        .buffer
        .iter()
        .flat_map(|line| {
            if window.opts.wrap {
                buffer::wrap_line(line, width)
                    .into_iter()
                    .map(|wrapped| buffer::line_to_ratatui(&wrapped, width))
                    .collect::<Vec<_>>()
            } else {
                vec![buffer::line_to_ratatui(line, width)]
            }
        })
        .collect();
    let paragraph = Paragraph::new(lines);
    match block {
        Some(block) => surface.render_widget(paragraph.block(block)),
        None => surface.render_widget(paragraph),
    }
}
