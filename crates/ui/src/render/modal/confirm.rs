use crate::model::Color;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::render::Context;
use crate::render::style::{Palette, color_of};
use crate::render::wrap::text as wrap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Choice {
    Allow,
    Deny,
}

pub(crate) fn choice(allow: bool) -> Choice {
    if allow { Choice::Allow } else { Choice::Deny }
}

fn styled(color: Option<Color>, fallback: Style) -> Style {
    color.map_or(fallback, |color| Style::default().fg(color_of(color)))
}

struct Sections {
    head: Vec<Line<'static>>,
    body: Vec<Line<'static>>,
    foot: Vec<Line<'static>>,
}

pub(crate) fn rows(ctx: &Context<'_>, title: &str, body: &str, width: u16) -> u16 {
    let Sections { head, body, foot } = sections(ctx, title, body, Choice::Allow, width);
    let count = head
        .len()
        .saturating_add(body.len())
        .saturating_add(foot.len());
    u16::try_from(count).unwrap_or(u16::MAX)
}

pub(crate) fn lines(
    ctx: &Context<'_>,
    title: &str,
    body: &str,
    choice: Choice,
    width: u16,
    height: u16,
) -> Vec<Line<'static>> {
    let Sections {
        head,
        body,
        mut foot,
    } = sections(ctx, title, body, choice, width);
    let room = usize::from(height)
        .saturating_sub(head.len().saturating_add(foot.len()))
        .max(1);
    let hidden = body.len().saturating_sub(room);
    let total = body.len();
    let shown: Vec<Line<'static>> = if hidden == 0 {
        body
    } else {
        let offset = ctx.app.confirm_scroll().resolve(hidden, room);
        if let Some(gap) = foot.first_mut() {
            *gap = position(offset, room, total, ctx.palette);
        }
        body.into_iter().skip(offset).take(room).collect()
    };
    head.into_iter().chain(shown).chain(foot).collect()
}

fn position(offset: usize, room: usize, total: usize, palette: Palette) -> Line<'static> {
    Line::from(Span::styled(
        format!(
            "  lines {}-{} of {total}, scroll for more",
            offset.saturating_add(1),
            offset.saturating_add(room)
        ),
        Style::default()
            .fg(palette.muted)
            .add_modifier(Modifier::DIM),
    ))
}

fn sections(ctx: &Context<'_>, title: &str, body: &str, choice: Choice, width: u16) -> Sections {
    let opts = ctx.state.opts();
    let confirm = &opts.confirm;
    let palette = ctx.palette;

    let selected = styled(confirm.selected, palette.accent_style());
    let unselected = styled(confirm.unselected, Style::default().fg(palette.muted));
    let title_style =
        styled(confirm.title_color, Style::default().fg(palette.text)).add_modifier(Modifier::BOLD);
    let body_style = styled(confirm.body_color, Style::default().fg(palette.text));

    let inner = usize::from(width).saturating_sub(2).max(1);
    let mut head: Vec<Line<'static>> = vec![Line::from("")];
    head.extend(
        wrap(title, inner)
            .into_iter()
            .map(|chunk| Line::from(Span::styled(format!("  {chunk}"), title_style))),
    );
    head.push(Line::from(""));
    let body = wrap(body, inner)
        .into_iter()
        .map(|chunk| Line::from(Span::styled(format!("  {chunk}"), body_style)))
        .collect();
    let foot = vec![
        Line::from(""),
        option_line(
            1,
            &format!("{}, proceed", confirm.yes),
            "y",
            choice == Choice::Allow,
            selected,
            unselected,
            palette,
        ),
        option_line(
            2,
            &format!("{}, and tell uji what to do differently", confirm.no),
            "esc",
            choice == Choice::Deny,
            selected,
            unselected,
            palette,
        ),
        Line::from(""),
    ];
    Sections { head, body, foot }
}

fn option_line(
    index: usize,
    label: &str,
    key: &str,
    active: bool,
    selected: Style,
    unselected: Style,
    palette: Palette,
) -> Line<'static> {
    let marker = if active { "\u{203a} " } else { "  " };
    let style = if active { selected } else { unselected };
    Line::from(vec![
        Span::styled(marker.to_string(), style),
        Span::styled(format!("{index}. {label}"), style),
        Span::styled(
            format!(" ({key})"),
            Style::default()
                .fg(palette.muted)
                .add_modifier(Modifier::DIM),
        ),
    ])
}
