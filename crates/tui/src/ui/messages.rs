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

        let mut grouping = Grouping::after(!lines.is_empty());
        let conversation = ctx.app.messages();
        for stored in conversation.messages() {
            if grouping.separates(stored) {
                lines.push(Line::from(""));
            }
            push_message(&mut lines, stored, width);
        }
        push_pending(&mut lines, ctx, width);

        let total = lines.len();
        let start = ctx.app.resolve_scroll(total.saturating_sub(height), height);
        let end = start.saturating_add(height).min(total);
        let paragraph = Paragraph::new(lines[start..end].to_vec());
        let paragraph = match block {
            Some(block) => paragraph.block(block),
            None => paragraph,
        };
        surface.render_widget(paragraph);
    }
}

#[derive(Default)]
struct Grouping {
    started: bool,
    open_group: bool,
}

impl Grouping {
    fn after(started: bool) -> Self {
        Self {
            started,
            open_group: false,
        }
    }

    fn separates(&mut self, stored: &StoredMessage) -> bool {
        let continues = self.open_group && matches!(stored.message, Message::Tool { .. });
        let separates = self.started && !continues;
        self.started = true;
        self.open_group = opens_tool_group(stored);
        separates
    }
}

fn opens_tool_group(stored: &StoredMessage) -> bool {
    match &stored.message {
        Message::Assistant { tool_calls, .. } => !tool_calls.is_empty(),
        Message::Tool { .. } => true,
        _ => false,
    }
}

fn push_message(lines: &mut Vec<Line<'static>>, stored: &StoredMessage, width: usize) {
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
    }
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

fn push_pending(lines: &mut Vec<Line<'static>>, ctx: &Context<'_>, width: usize) {
    let Some(pending) = ctx.app.pending() else {
        return;
    };
    if !lines.is_empty() {
        lines.push(Line::from(""));
    }
    let (committed, tail) = split_committed(pending);
    if !committed.is_empty() {
        push_markdown(lines, committed, width);
    }
    if !tail.is_empty() {
        push_wrapped(
            lines,
            tail,
            width,
            Style::default().fg(TEXT),
            " ",
            Fill::Line,
        );
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
