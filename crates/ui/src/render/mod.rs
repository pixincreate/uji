use crate::model::{Builtin, Size, WindowSpec};
use crate::state::UiState;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line as TLine;
use ratatui::widgets::Widget;

use crate::app::App;

pub mod buffer;
pub mod highlight;
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

pub(crate) fn write_lines<'a>(
    surface: &mut Surface<'_>,
    area: Rect,
    lines: impl Iterator<Item = &'a TLine<'static>>,
) {
    let buf = surface.buf();
    for (at, line) in lines.enumerate() {
        let Ok(at) = u16::try_from(at) else {
            break;
        };
        if at >= area.height {
            break;
        }
        buf.set_line(area.x, area.y.saturating_add(at), line, area.width);
    }
}

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let state = app.state();
    let fitted = fits(frame.area(), app, &state.borrow());
    for (id, rows) in fitted {
        state.borrow_mut().set_window_fitted(id, rows);
    }
    let rects = layout::layout(frame.area(), state.borrow().windows());
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
    // A picker needs room; if no float was declared for the modal window, give
    // it a centred slice of the frame rather than the usual strip.
    let area = match modal_rect {
        Some(rect) if !wants_float(&ctx) || rect.height >= MIN_PICK_ROWS => rect,
        _ if wants_float(&ctx) => centered(frame.area()),
        Some(rect) => rect,
        None => above(frame.area(), input_rect),
    };
    let mut surface = Surface::new(area, frame.buffer_mut());
    modal::Modal.render(&ctx, &mut surface);

    highlight::overlay(frame.buffer_mut(), app);
}

const MAX_INPUT_ROWS: u16 = 10;

const MIN_PICK_ROWS: u16 = 10;

fn wants_float(ctx: &Context<'_>) -> bool {
    matches!(ctx.app.mode(), crate::app::Mode::Pick { .. })
}

/// The default picker area when the config did not declare a float.
fn centered(area: Rect) -> Rect {
    let width = area.width.saturating_mul(90) / 100;
    let height = area.height.saturating_mul(80) / 100;
    Rect {
        x: area.x.saturating_add(area.width.saturating_sub(width) / 2),
        y: area
            .y
            .saturating_add(area.height.saturating_sub(height) / 2),
        width,
        height,
    }
}

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
            crate::model::Border::Plain | crate::model::Border::Rounded => {
                rect.width.saturating_sub(2)
            }
            _ => rect.width,
        };
        if rect.width == 0 {
            continue;
        }
        let rows = match window.builtin {
            Some(Builtin::Input) => modal::takeover_rows(&ctx, content).unwrap_or_else(|| {
                let rows = input::rows_needed(app, usize::from(content));
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
    if let Some(block) = block {
        surface.render_widget(block);
    }
    write_lines(surface, inner, lines.iter());
}
