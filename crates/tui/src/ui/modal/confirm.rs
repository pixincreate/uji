use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use uji_screen::model::Color;

use crate::ui::Context;
use crate::ui::style::{MUTED, TEXT, accent_style, color_of};
use crate::ui::wrap::text as wrap;

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

pub(crate) fn rows(ctx: &Context<'_>, title: &str, body: &str, width: u16) -> u16 {
    let count = lines(ctx, title, body, Choice::Allow, width).len();
    u16::try_from(count).unwrap_or(u16::MAX)
}

pub(crate) fn lines(
    ctx: &Context<'_>,
    title: &str,
    body: &str,
    choice: Choice,
    width: u16,
) -> Vec<Line<'static>> {
    let opts = ctx.state.opts();
    let confirm = &opts.confirm;

    let selected = styled(confirm.selected, accent_style());
    let unselected = styled(confirm.unselected, Style::default().fg(MUTED));
    let title_style =
        styled(confirm.title_color, Style::default().fg(TEXT)).add_modifier(Modifier::BOLD);
    let body_style = styled(confirm.body_color, Style::default().fg(TEXT));

    let inner = usize::from(width).saturating_sub(2).max(1);
    let mut lines: Vec<Line<'static>> = vec![Line::from("")];
    for chunk in wrap(title, inner) {
        lines.push(Line::from(Span::styled(format!("  {chunk}"), title_style)));
    }
    lines.push(Line::from(""));
    for chunk in wrap(body, inner) {
        lines.push(Line::from(Span::styled(format!("  {chunk}"), body_style)));
    }
    lines.push(Line::from(""));
    lines.push(option_line(
        1,
        &format!("{}, proceed", confirm.yes),
        "y",
        choice == Choice::Allow,
        selected,
        unselected,
    ));
    lines.push(option_line(
        2,
        &format!("{}, and tell uji what to do differently", confirm.no),
        "esc",
        choice == Choice::Deny,
        selected,
        unselected,
    ));
    lines.push(Line::from(""));
    lines
}

fn option_line(
    index: usize,
    label: &str,
    key: &str,
    active: bool,
    selected: Style,
    unselected: Style,
) -> Line<'static> {
    let marker = if active { "\u{203a} " } else { "  " };
    let style = if active { selected } else { unselected };
    Line::from(vec![
        Span::styled(marker.to_string(), style),
        Span::styled(format!("{index}. {label}"), style),
        Span::styled(
            format!(" ({key})"),
            Style::default().fg(MUTED).add_modifier(Modifier::DIM),
        ),
    ])
}
