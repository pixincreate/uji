use crate::app::Mode;
use crate::ui::Context;
use crate::ui::Render;
use crate::ui::Surface;

pub(crate) mod confirm;
mod prompt;
mod select;
mod suggest;

pub(crate) struct Modal;

pub(crate) fn rows(ctx: &Context<'_>, _width: u16) -> Option<u16> {
    match ctx.app.mode() {
        Mode::Normal | Mode::Confirm { .. } => None,
        Mode::Select { matches, .. } => {
            let matches = matches.len();
            let visible = matches.min(select::MAX_ROWS);
            let overflow = u16::from(matches > visible);
            let rows = u16::try_from(visible).unwrap_or(u16::MAX);
            Some(rows.saturating_add(4).saturating_add(overflow))
        }
        Mode::Prompt { .. } => Some(prompt::ROWS),
        Mode::Suggest { items, .. } => {
            let max = usize::from(ctx.state.opts().suggest_max_height).max(1);
            let visible = items.len().min(max);
            u16::try_from(visible).ok().filter(|rows| *rows > 0)
        }
    }
}

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
                matches,
            } => {
                select::Select {
                    title,
                    items,
                    query,
                    cursor: *cursor,
                    matches,
                }
                .render(ctx, surface);
            }
            Mode::Prompt { title, value, echo } => {
                prompt::Prompt {
                    title,
                    value,
                    echo: *echo,
                }
                .render(ctx, surface);
            }
            Mode::Suggest { items, cursor } => {
                suggest::Suggest {
                    items,
                    cursor: *cursor,
                }
                .render(ctx, surface);
            }
        }
    }
}
