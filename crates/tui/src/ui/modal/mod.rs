use ratatui::layout::Rect;

use crate::app::Mode;
use crate::ui::Context;
use crate::ui::Render;
use crate::ui::Surface;

pub(crate) mod confirm;
mod menu;
mod prompt;
mod select;
mod suggest;

pub(crate) struct Modal {
    pub(crate) input_rect: Option<Rect>,
}

/// Rows the input window must grow to when a view takes it over, so the
/// windows above reflow instead of being covered.
pub(crate) fn takeover_rows(ctx: &Context<'_>, width: u16) -> Option<u16> {
    match ctx.app.mode() {
        Mode::Confirm { title, body, .. } => Some(confirm::rows(ctx, title, body, width)),
        _ => None,
    }
}

impl Render for Modal {
    fn render(&self, ctx: &Context<'_>, surface: &mut Surface<'_>) {
        match ctx.app.mode() {
            Mode::Normal | Mode::Confirm { .. } => {}
            Mode::Select {
                title,
                items,
                query,
                cursor,
            } => {
                select::Select {
                    title,
                    items,
                    query,
                    cursor: *cursor,
                }
                .render(ctx, surface);
            }
            Mode::Prompt {
                title,
                value,
                secret,
            } => {
                prompt::Prompt {
                    title,
                    value,
                    secret: *secret,
                }
                .render(ctx, surface);
            }
            Mode::Suggest { items, cursor } => {
                suggest::Suggest {
                    items,
                    cursor: *cursor,
                    input_rect: self.input_rect,
                }
                .render(ctx, surface);
            }
        }
    }
}
