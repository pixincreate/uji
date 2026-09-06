use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use uji_api::model::WindowKind;
use uji_api::state::UiState;

use crate::app::App;

pub(crate) mod input;
pub(crate) mod layout;
pub(crate) mod messages;
pub(crate) mod modal;
pub(crate) mod status;
pub(crate) mod style;
pub(crate) mod text;

pub struct Context<'a> {
    pub app: &'a App,
    pub state: &'a UiState,
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
    let state = app.state().borrow();
    let ctx = Context { app, state: &state };
    let rects = layout::layout(frame.area(), state.windows());
    let input_rect = state
        .windows()
        .iter()
        .zip(rects.iter().copied())
        .find(|(window, _)| window.kind == WindowKind::Input)
        .map(|(_, rect)| rect);
    for (window, rect) in state.windows().iter().zip(rects.iter().copied()) {
        let mut surface = Surface::new(rect, frame.buffer_mut());
        match window.kind {
            WindowKind::Messages => {
                messages::Messages { window }.render(&ctx, &mut surface);
            }
            WindowKind::Input => {
                input::Input.render(&ctx, &mut surface);
            }
            WindowKind::Status => {
                status::Status.render(&ctx, &mut surface);
            }
            WindowKind::Text => {
                text::Text { window }.render(&ctx, &mut surface);
            }
        }
    }
    let mut surface = Surface::new(frame.area(), frame.buffer_mut());
    modal::Modal { input_rect }.render(&ctx, &mut surface);
}
