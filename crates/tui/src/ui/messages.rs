use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use uji_screen::model::WindowSpec;

use crate::ui::Context;
use crate::ui::Render;
use crate::ui::Surface;
use crate::ui::style::{MUTED, TEXT, USER_BG, block_for};
use crate::ui::wrap::text as wrap_text;
use uji_core::session::model::{Message, StoredMessage};

pub(crate) struct Messages<'a> {
    pub(crate) window: &'a WindowSpec,
}

impl Render for Messages<'_> {
    fn render(&self, ctx: &Context<'_>, surface: &mut Surface<'_>) {
        let block = block_for(self.window);
        let inner = block
            .as_ref()
            .map_or(surface.area(), |b| b.inner(surface.area()));
        let width = usize::from(inner.width);
        let height = usize::from(inner.height);

        let mut lines = Vec::new();
        for notice in ctx.app.notices() {
            push_wrapped(&mut lines, notice, width, NOTICE_STYLE, " ! ", Fill::Line);
        }
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }
        let leading = lines.len();

        let conversation = ctx.app.messages();
        let pending = ctx.app.pending().unwrap_or_default();
        let (committed, partial) = split_committed(pending);
        let mut transcript = ctx.app.transcript();
        let (folded, streamed) =
            transcript.frame(&conversation, committed, width, push_message, push_markdown);

        let above = leading.saturating_add(folded.len());
        let mut gap = Vec::new();
        if !pending.is_empty() && above > 0 {
            gap.push(Line::from(""));
        }
        let mut tail = Vec::new();
        if !partial.is_empty() {
            push_wrapped(
                &mut tail,
                partial,
                width,
                Style::default().fg(TEXT),
                " ",
                Fill::Line,
            );
        }

        let total = above
            .saturating_add(gap.len())
            .saturating_add(streamed.len())
            .saturating_add(tail.len());
        let start = ctx.app.resolve_scroll(total.saturating_sub(height), height);
        let end = start.saturating_add(height).min(total);
        let window: Vec<Line<'static>> = lines
            .iter()
            .chain(folded.iter())
            .chain(gap.iter())
            .chain(streamed.iter())
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

pub(crate) fn push_message(lines: &mut Vec<Line<'static>>, stored: &StoredMessage, width: usize) {
    match &stored.message {
        Message::User { text } => {
            let style = Style::default().bg(USER_BG).fg(TEXT);
            let fill = Line::from(" ".repeat(width)).style(style);
            lines.push(fill.clone());
            push_wrapped(lines, text, width, style, " ", Fill::Block);
            lines.push(fill);
        }
        Message::Assistant {
            text, tool_calls, ..
        } => {
            if !text.is_empty() {
                push_markdown(lines, text, width);
            }
            for call in tool_calls {
                if !text.is_empty() {
                    lines.push(Line::from(""));
                }
                push_tool_header(lines, &call.name, &call.arguments, width);
            }
        }
        Message::Tool { content, .. } => push_tool_output(lines, content, width),
        Message::System { text } => {
            let style = Style::default().fg(MUTED).add_modifier(Modifier::ITALIC);
            push_wrapped(lines, text, width, style, " ", Fill::Line);
        }
        Message::Error { text } => {
            push_wrapped(lines, text, width, ERROR_STYLE, " ", Fill::Line);
        }
        Message::Compaction { .. } => push_divider(lines, width),
    }
}

fn push_divider(lines: &mut Vec<Line<'static>>, width: usize) {
    let label = " compacted ";
    let rule = width.saturating_sub(label.chars().count() + 2) / 2;
    let bar = "─".repeat(rule);
    lines.push(Line::from(Span::styled(
        format!(" {bar}{label}{bar}"),
        Style::default().fg(MUTED).add_modifier(Modifier::DIM),
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

const NOTICE_STYLE: Style = Style::new().fg(Color::Red);
const ERROR_STYLE: Style = Style::new().fg(Color::Red);

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

fn push_tool_header(lines: &mut Vec<Line<'static>>, name: &str, arguments: &str, width: usize) {
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
        Span::styled(" • ", Style::default().fg(MUTED)),
        Span::styled(
            first,
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ),
    ]));
    for chunk in chunks {
        lines.push(Line::from(Span::styled(
            format!("   {chunk}"),
            Style::default().fg(TEXT),
        )));
    }
}

fn push_tool_output(lines: &mut Vec<Line<'static>>, content: &str, width: usize) {
    let failed = content.starts_with("error:") || content.starts_with("denied:");
    let style = if failed {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(MUTED)
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
            Style::default().fg(MUTED).add_modifier(Modifier::DIM),
        )));
    }
}

fn split_committed(pending: &str) -> (&str, &str) {
    match pending.rfind('\n') {
        Some(at) => (&pending[..=at], &pending[at + 1..]),
        None => ("", pending),
    }
}

fn push_markdown(lines: &mut Vec<Line<'static>>, text: &str, width: usize) {
    for mut line in crate::ui::markdown::render(text, width.saturating_sub(1)) {
        line.spans.insert(0, Span::raw(" "));
        lines.push(line);
    }
}
