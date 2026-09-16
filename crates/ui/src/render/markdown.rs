use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::render::style::Palette;

const BULLETS: &[&str] = &["•", "◦", "▪"];

struct Token {
    text: String,
    style: Style,
    spaced: bool,
}

#[derive(Default)]
struct Block {
    tokens: Vec<Token>,
    indent: String,
    hanging: String,
    pending_space: bool,
}

impl Block {
    fn open(&mut self, indent: String, hanging: String) {
        if self.tokens.is_empty() {
            self.indent = indent;
            self.hanging = hanging;
        }
    }

    fn push(&mut self, text: &str, style: Style) {
        let mut spaced = self.pending_space;
        for word in text.split(' ') {
            if word.is_empty() {
                spaced = true;
                continue;
            }
            self.tokens.push(Token {
                text: word.to_string(),
                style,
                spaced: spaced && !self.tokens.is_empty(),
            });
            spaced = true;
        }
        self.pending_space = text.ends_with(' ');
    }

    fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    fn drain(&mut self, width: usize) -> Vec<Line<'static>> {
        let tokens = std::mem::take(&mut self.tokens);
        let indent = std::mem::take(&mut self.indent);
        let hanging = std::mem::take(&mut self.hanging);
        self.pending_space = false;
        let usable = width.saturating_sub(indent.chars().count()).max(1);
        let mut lines = Vec::new();
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut used = 0usize;
        let mut first = true;
        for token in tokens {
            let len = token.text.chars().count();
            let gap = usize::from(token.spaced && used > 0);
            if used > 0 && used + gap + len > usable {
                lines.push(finish_line(
                    std::mem::take(&mut spans),
                    if first { &indent } else { &hanging },
                ));
                first = false;
                used = 0;
            }
            if used > 0 && token.spaced {
                spans.push(Span::raw(" "));
                used += 1;
            }
            spans.push(Span::styled(token.text, token.style));
            used += len;
        }
        if !spans.is_empty() {
            lines.push(finish_line(spans, if first { &indent } else { &hanging }));
        }
        lines
    }
}

fn finish_line(mut spans: Vec<Span<'static>>, indent: &str) -> Line<'static> {
    if !indent.is_empty() {
        spans.insert(0, Span::raw(indent.to_string()));
    }
    Line::from(spans)
}

fn heading_style(level: HeadingLevel, palette: Palette) -> Style {
    match level {
        HeadingLevel::H1 | HeadingLevel::H2 => palette.accent_style(),
        _ => Style::default()
            .fg(palette.text)
            .add_modifier(Modifier::BOLD),
    }
}

struct Renderer {
    lines: Vec<Line<'static>>,
    block: Block,
    style: Style,
    styles: Vec<Style>,
    list: Vec<Option<u64>>,
    quote: usize,
    in_code: bool,
    marker: Option<String>,
    width: usize,
    palette: Palette,
}

impl Renderer {
    fn new(width: usize, palette: Palette) -> Self {
        Self {
            lines: Vec::new(),
            block: Block::default(),
            style: Style::default().fg(palette.text),
            styles: Vec::new(),
            list: Vec::new(),
            quote: 0,
            in_code: false,
            marker: None,
            width,
            palette,
        }
    }

    fn flush(&mut self) {
        if !self.block.is_empty() {
            self.lines.extend(self.block.drain(self.width));
        }
    }

    fn blank(&mut self) {
        if self.lines.last().is_some_and(|line| line.width() > 0) {
            self.lines.push(Line::from(""));
        }
    }

    fn break_block(&mut self) {
        self.flush();
        self.blank();
    }

    fn open(&mut self) {
        let marker = self.marker.take();
        let (indent, hanging) = indents(self.list.len(), self.quote, marker.as_deref());
        self.block.open(indent, hanging);
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                self.flush();
                if self.list.is_empty() {
                    self.blank();
                }
            }
            Tag::Item => {
                self.flush();
                self.marker = Some(item_marker(&mut self.list));
            }
            Tag::Heading { level, .. } => {
                self.break_block();
                self.style = heading_style(level, self.palette);
            }
            Tag::Emphasis => self.push_style(Modifier::ITALIC),
            Tag::Strong => self.push_style(Modifier::BOLD),
            Tag::Strikethrough => self.push_style(Modifier::CROSSED_OUT),
            Tag::Link { .. } => self.push_style(Modifier::UNDERLINED),
            Tag::List(start) => {
                self.flush();
                if self.list.is_empty() {
                    self.blank();
                }
                self.list.push(start);
            }
            Tag::BlockQuote(_) => {
                self.break_block();
                self.quote += 1;
            }
            Tag::CodeBlock(kind) => {
                self.break_block();
                self.in_code = true;
                if let CodeBlockKind::Fenced(lang) = kind
                    && !lang.is_empty()
                {
                    self.lines.push(Line::from(Span::styled(
                        format!("  {lang}"),
                        Style::default()
                            .fg(self.palette.muted)
                            .add_modifier(Modifier::DIM),
                    )));
                }
            }
            _ => {}
        }
    }

    fn push_style(&mut self, modifier: Modifier) {
        self.styles.push(self.style);
        self.style = self.style.add_modifier(modifier);
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link => {
                self.style = self
                    .styles
                    .pop()
                    .unwrap_or_else(|| Style::default().fg(self.palette.text));
            }
            TagEnd::List(_) => {
                self.list.pop();
            }
            TagEnd::BlockQuote(_) => self.quote = self.quote.saturating_sub(1),
            TagEnd::CodeBlock => self.in_code = false,
            TagEnd::Paragraph | TagEnd::Item => self.flush(),
            TagEnd::Heading(_) => {
                self.flush();
                self.style = Style::default().fg(self.palette.text);
            }
            _ => {}
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) if self.in_code => {
                let code = Style::default().fg(self.palette.code);
                self.lines.extend(
                    text.lines()
                        .map(|raw| Line::from(Span::styled(format!("  {raw}"), code))),
                );
            }
            Event::Text(text) => {
                self.open();
                let style = self.style;
                self.block.push(&text, style);
            }
            Event::Code(text) => {
                self.open();
                self.block
                    .push(&text, Style::default().fg(self.palette.code));
            }
            Event::SoftBreak => {
                let style = self.style;
                self.block.push(" ", style);
            }
            Event::HardBreak => self.flush(),
            Event::Rule => {
                self.break_block();
                self.lines.push(Line::from(Span::styled(
                    "─".repeat(self.width.min(60)),
                    Style::default().fg(self.palette.muted),
                )));
            }
            _ => {}
        }
    }

    fn finish(mut self) -> Vec<Line<'static>> {
        self.flush();
        while self.lines.last().is_some_and(|line| line.width() == 0) {
            self.lines.pop();
        }
        self.lines
    }
}

pub(crate) fn defines_reference(source: &str) -> bool {
    source.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with('[') && line.contains("]:")
    })
}

pub(crate) fn settled(source: &str) -> usize {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let mut depth = 0usize;
    let mut settled = 0usize;
    let mut closed = 0usize;
    for (event, range) in Parser::new_ext(source, options).into_offset_iter() {
        match event {
            Event::Start(_) => {
                if depth == 0 {
                    settled = closed;
                }
                depth = depth.saturating_add(1);
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    closed = range.end;
                }
            }
            Event::Rule if depth == 0 => {
                settled = closed;
                closed = range.end;
            }
            _ => {}
        }
    }
    settled
}

pub(crate) fn render_from(
    source: &str,
    width: usize,
    palette: Palette,
    continuing: bool,
) -> Vec<Line<'static>> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let mut renderer = Renderer::new(width, palette);
    if continuing {
        renderer.lines.push(Line::from(""));
    }
    for event in Parser::new_ext(source, options) {
        renderer.event(event);
    }
    renderer.finish()
}

fn item_marker(list: &mut [Option<u64>]) -> String {
    let depth = list.len().saturating_sub(1);
    match list.last_mut() {
        Some(Some(index)) => {
            let marker = format!("{index}.");
            *index += 1;
            marker
        }
        _ => BULLETS[depth % BULLETS.len()].to_string(),
    }
}

fn indents(list_depth: usize, quote: usize, marker: Option<&str>) -> (String, String) {
    let mut pad = "  ".repeat(list_depth.saturating_sub(1));
    if quote > 0 {
        pad = format!("{}│ ", "  ".repeat(quote.saturating_sub(1)));
    }
    match marker {
        Some(marker) => {
            let hang = " ".repeat(marker.chars().count() + 1);
            (format!("{pad}{marker} "), format!("{pad}{hang}"))
        }
        None => (pad.clone(), pad),
    }
}
