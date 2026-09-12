use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::ui::Context;
use crate::ui::style::{MUTED, TEXT, accent_style, color_of};
use crate::ui::wrap::text as wrap;

pub(crate) fn rows(ctx: &Context<'_>, title: &str, body: &str, width: u16) -> u16 {
    let count = lines(ctx, title, body, true, width).len();
    u16::try_from(count).unwrap_or(u16::MAX)
}

pub(crate) fn lines(
    ctx: &Context<'_>,
    title: &str,
    body: &str,
    allow: bool,
    width: u16,
) -> Vec<Line<'static>> {
    let opts = ctx.state.opts();
    let confirm = &opts.confirm;

    let selected = confirm
        .selected
        .map_or_else(accent_style, |color| Style::default().fg(color_of(color)));
    let unselected = confirm.unselected.map_or_else(
        || Style::default().fg(MUTED),
        |color| Style::default().fg(color_of(color)),
    );
    let title_style = confirm.title_color.map_or_else(
        || Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        |color| {
            Style::default()
                .fg(color_of(color))
                .add_modifier(Modifier::BOLD)
        },
    );
    let body_style = confirm.body_color.map_or_else(
        || Style::default().fg(TEXT),
        |color| Style::default().fg(color_of(color)),
    );

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
        allow,
        selected,
        unselected,
    ));
    lines.push(option_line(
        2,
        &format!("{}, and tell uji what to do differently", confirm.no),
        "esc",
        !allow,
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
