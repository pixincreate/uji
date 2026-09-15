use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use uji_screen::model::WindowSpec;

use crate::app::renderer::Block;
use crate::ui::Context;
use crate::ui::Render;
use crate::ui::Surface;
use crate::ui::style::{Palette, block_for};
use crate::ui::transcript;
use crate::ui::wrap::text as wrap_text;
use uji_core::session::model::{Message, StoredMessage};

pub(crate) struct Messages<'a> {
    pub(crate) window: &'a WindowSpec,
}

impl Render for Messages<'_> {
    fn render(&self, ctx: &Context<'_>, surface: &mut Surface<'_>) {
        let palette = ctx.palette;
        let block = block_for(self.window, palette);
        let inner = block
            .as_ref()
            .map_or(surface.area(), |b| b.inner(surface.area()));
        let width = usize::from(inner.width);
        let height = usize::from(inner.height);

        let renderer = ctx.app.renderer();
        let conversation = ctx.app.messages();
        let pending = ctx.app.pending().unwrap_or_default();
        let (committed, partial) = split_committed(pending);
        let mut transcript = ctx.app.transcript();
        let parts = transcript.frame(
            &transcript::Input {
                conversation: &conversation,
                notices: ctx.app.notices(),
                pending: committed,
                width,
                palette,
            },
            |lines, block, width| match renderer.and_then(|renderer| renderer.render(block)) {
                Some(custom) => push_custom(lines, &custom, width),
                None => push_builtin(lines, block, width, palette),
            },
        );

        let mut gap = Vec::new();
        if !parts.notices.is_empty() {
            gap.push(Line::from(""));
        }
        let above = parts
            .notices
            .len()
            .saturating_add(gap.len())
            .saturating_add(parts.folded.len());

        let mut lead = Vec::new();
        if !pending.is_empty() && above > 0 {
            lead.push(Line::from(""));
        }
        let mut tail = Vec::new();
        if !partial.is_empty() {
            push_wrapped(
                &mut tail,
                partial,
                width,
                Style::default().fg(palette.text),
                " ",
                Fill::Line,
            );
        }

        let total = above
            .saturating_add(lead.len())
            .saturating_add(parts.pending.len())
            .saturating_add(tail.len());
        let start = ctx.app.resolve_scroll(total.saturating_sub(height), height);
        let end = start.saturating_add(height).min(total);
        let window: Vec<Line<'static>> = parts
            .notices
            .iter()
            .chain(gap.iter())
            .chain(parts.folded.iter())
            .chain(lead.iter())
            .chain(parts.pending.iter())
            .chain(tail.iter())
            .skip(start)
            .take(end.saturating_sub(start))
            .cloned()
            .collect();
        let paragraph = Paragraph::new(window);
        let paragraph = match block {
            Some(block) => paragraph.block(block),
            None => paragraph,
        };
        surface.render_widget(paragraph);
    }
}

fn push_builtin(lines: &mut Vec<Line<'static>>, block: Block<'_>, width: usize, palette: Palette) {
    match block {
        Block::Notice(text) => push_wrapped(
            lines,
            text,
            width,
            Style::default().fg(palette.notice),
            " ! ",
            Fill::Line,
        ),
        Block::Message(stored) => push_message(lines, stored, width, palette),
        Block::Pending(text) => push_markdown(lines, text, width, palette),
    }
}

fn push_custom(lines: &mut Vec<Line<'static>>, custom: &[uji_screen::model::Line], width: usize) {
    lines.extend(custom.iter().flat_map(|line| {
        crate::ui::buffer::wrap_line(line, width)
            .into_iter()
            .map(|wrapped| crate::ui::buffer::line_to_ratatui(&wrapped, width))
    }));
}

pub(crate) fn push_message(
    lines: &mut Vec<Line<'static>>,
    stored: &StoredMessage,
    width: usize,
    palette: Palette,
) {
    match &stored.message {
        Message::User { text } => {
            let style = Style::default().bg(palette.user_bg).fg(palette.text);
            let fill = Line::from(" ".repeat(width)).style(style);
            lines.push(fill.clone());
            push_wrapped(lines, text, width, style, " ", Fill::Block);
            lines.push(fill);
        }
        Message::Assistant {
            text, tool_calls, ..
        } => {
            if !text.is_empty() {
                push_markdown(lines, text, width, palette);
            }
            for call in tool_calls {
                if !text.is_empty() {
                    lines.push(Line::from(""));
                }
                push_tool_header(lines, &call.name, &call.arguments, width, palette);
            }
        }
        Message::Tool { content, .. } => push_tool_output(lines, content, width, palette),
        Message::System { text } => {
            let style = Style::default()
                .fg(palette.muted)
                .add_modifier(Modifier::ITALIC);
            push_wrapped(lines, text, width, style, " ", Fill::Line);
        }
        Message::Error { text } => {
            push_wrapped(
                lines,
                text,
                width,
                Style::default().fg(palette.error),
                " ",
                Fill::Line,
            );
        }
        Message::Compaction { .. } => push_divider(lines, width, palette),
    }
}

fn push_divider(lines: &mut Vec<Line<'static>>, width: usize, palette: Palette) {
    let label = " compacted ";
    let rule = width.saturating_sub(label.chars().count() + 2) / 2;
    let bar = "─".repeat(rule);
    lines.push(Line::from(Span::styled(
        format!(" {bar}{label}{bar}"),
        Style::default()
            .fg(palette.muted)
            .add_modifier(Modifier::DIM),
    )));
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fill {
    Line,
    Block,
}

fn push_wrapped(
    lines: &mut Vec<Line<'static>>,
    text: &str,
    width: usize,
    style: Style,
    prefix: &str,
    fill: Fill,
) {
    let trailing = usize::from(fill == Fill::Block);
    let inner = width.saturating_sub(prefix.chars().count() + trailing);
    for chunk in wrap_text(text, inner) {
        let line = match fill {
            Fill::Line => format!("{prefix}{chunk}"),
            Fill::Block => {
                let pad = " ".repeat(inner.saturating_sub(chunk.chars().count()));
                format!("{prefix}{chunk} {pad}")
            }
        };
        lines.push(Line::from(line).style(style));
    }
}

const MAX_TOOL_PREVIEW_LINES: usize = 8;

fn tool_verb(name: &str) -> &'static str {
    match name {
        "read_file" => "Read",
        "edit_file" => "Edited",
        "write_file" => "Wrote",
        "list_dir" => "Listed",
        "grep" => "Searched",
        "run_command" => "Ran",
        _ => "Called",
    }
}

fn tool_detail(name: &str, arguments: &str) -> String {
    let args = serde_json::from_str::<serde_json::Value>(arguments).unwrap_or_default();
    let field = |key: &str| {
        args.get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    };
    let detail = match name {
        "run_command" => field("command"),
        "grep" => field("pattern"),
        _ => field("path"),
    };
    detail.unwrap_or_else(|| arguments.chars().take(200).collect())
}

fn push_tool_header(
    lines: &mut Vec<Line<'static>>,
    name: &str,
    arguments: &str,
    width: usize,
    palette: Palette,
) {
    let verb = tool_verb(name);
    let detail = tool_detail(name, arguments);
    let detail = detail.replace('\n', " ");
    let head = if verb == "Called" {
        format!("{verb} {name} {detail}")
    } else {
        format!("{verb} {detail}")
    };
    let available = width.saturating_sub(4).max(1);
    let mut chunks = wrap_text(&head, available).into_iter();
    let first = chunks.next().unwrap_or_default();
    lines.push(Line::from(vec![
        Span::styled(" • ", Style::default().fg(palette.muted)),
        Span::styled(
            first,
            Style::default()
                .fg(palette.text)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.extend(chunks.map(|chunk| {
        Line::from(Span::styled(
            format!("   {chunk}"),
            Style::default().fg(palette.text),
        ))
    }));
}

fn push_tool_output(lines: &mut Vec<Line<'static>>, content: &str, width: usize, palette: Palette) {
    let failed = content.starts_with("error:") || content.starts_with("denied:");
    let style = if failed {
        Style::default().fg(palette.error)
    } else {
        Style::default().fg(palette.muted)
    };
    let available = width.saturating_sub(5).max(1);
    let mut wrapped: Vec<String> = Vec::new();
    for raw in content.lines() {
        wrapped.extend(wrap_text(raw, available));
    }
    let omitted = wrapped.len().saturating_sub(MAX_TOOL_PREVIEW_LINES);
    for (index, chunk) in wrapped.iter().take(MAX_TOOL_PREVIEW_LINES).enumerate() {
        let prefix = if index == 0 { "   └ " } else { "     " };
        lines.push(Line::from(Span::styled(format!("{prefix}{chunk}"), style)));
    }
    if omitted > 0 {
        lines.push(Line::from(Span::styled(
            format!("     … +{omitted} lines"),
            Style::default()
                .fg(palette.muted)
                .add_modifier(Modifier::DIM),
        )));
    }
}

fn split_committed(pending: &str) -> (&str, &str) {
    match pending.rfind('\n') {
        Some(at) => (&pending[..=at], &pending[at + 1..]),
        None => ("", pending),
    }
}

fn push_markdown(lines: &mut Vec<Line<'static>>, text: &str, width: usize, palette: Palette) {
    for mut line in crate::ui::markdown::render(text, width.saturating_sub(1), palette) {
        line.spans.insert(0, Span::raw(" "));
        lines.push(line);
    }
}
