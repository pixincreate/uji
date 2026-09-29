use std::ffi::c_void;
use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use uji_native::abi::{self, Out};
use uji_native::{CStruct, native};

const START: u8 = 1;
const END: u8 = 2;
const TEXT: u8 = 3;
const CODE: u8 = 4;
const HTML: u8 = 5;
const BREAK: u8 = 6;
const RULE: u8 = 7;
const TASK: u8 = 8;

#[repr(C)]
#[derive(CStruct, Clone, Copy, Default)]
pub(crate) struct Mark {
    pub(crate) kind: u8,
    pub(crate) tag: u8,
    pub(crate) flag: u8,
    pub(crate) number: i64,
    pub(crate) offset: usize,
    pub(crate) length: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[repr(C)]
#[derive(CStruct)]
pub(crate) struct Marks {
    pub(crate) items: *const Mark,
    pub(crate) count: usize,
    pub(crate) strings: *const u8,
}

struct Tape {
    marks: Vec<Mark>,
    strings: Vec<u8>,
}

impl Tape {
    fn text(&mut self, text: &str) -> (usize, usize) {
        let offset = self.strings.len();
        self.strings.extend_from_slice(text.as_bytes());
        (offset, text.len())
    }

    fn push(&mut self, mark: Mark, range: &Range<usize>) {
        self.marks.push(Mark {
            start: range.start.saturating_add(1),
            end: range.end,
            ..mark
        });
    }
}

fn level(level: HeadingLevel) -> i64 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn detail(tape: &mut Tape, mark: &mut Mark, text: &str) {
    (mark.offset, mark.length) = tape.text(text);
    mark.flag = 1;
}

fn opened(tape: &mut Tape, tag: &Tag<'_>) -> Mark {
    let mut mark = Mark {
        kind: START,
        ..Mark::default()
    };
    mark.tag = match tag {
        Tag::Paragraph => 1,
        Tag::Heading { level: at, .. } => {
            mark.number = level(*at);
            mark.flag = 2;
            2
        }
        Tag::BlockQuote(_) => 3,
        Tag::CodeBlock(kind) => {
            if let CodeBlockKind::Fenced(language) = kind {
                detail(tape, &mut mark, language);
            }
            4
        }
        Tag::List(start) => {
            if let Some(start) = start.and_then(|start| i64::try_from(start).ok()) {
                mark.number = start;
                mark.flag = 2;
            }
            5
        }
        Tag::Item => 6,
        Tag::Emphasis => 7,
        Tag::Strong => 8,
        Tag::Strikethrough => 9,
        Tag::Link { dest_url, .. } => {
            detail(tape, &mut mark, dest_url);
            10
        }
        Tag::Image { dest_url, .. } => {
            detail(tape, &mut mark, dest_url);
            11
        }
        Tag::Table(_) => 12,
        Tag::TableHead => 13,
        Tag::TableRow => 14,
        Tag::TableCell => 15,
        _ => 16,
    };
    mark
}

fn closed(tag: TagEnd) -> u8 {
    match tag {
        TagEnd::Paragraph => 1,
        TagEnd::Heading(_) => 2,
        TagEnd::BlockQuote(_) => 3,
        TagEnd::CodeBlock => 4,
        TagEnd::List(_) => 5,
        TagEnd::Item => 6,
        TagEnd::Emphasis => 7,
        TagEnd::Strong => 8,
        TagEnd::Strikethrough => 9,
        TagEnd::Link => 10,
        TagEnd::Image => 11,
        TagEnd::Table => 12,
        TagEnd::TableHead => 13,
        TagEnd::TableRow => 14,
        TagEnd::TableCell => 15,
        _ => 16,
    }
}

fn record(tape: &mut Tape, event: Event<'_>, range: &Range<usize>) {
    let body = |tape: &mut Tape, kind: u8, text: &str| {
        let (offset, length) = tape.text(text);
        Mark {
            kind,
            offset,
            length,
            ..Mark::default()
        }
    };
    let mark = match event {
        Event::Start(tag) => opened(tape, &tag),
        Event::End(tag) => Mark {
            kind: END,
            tag: closed(tag),
            ..Mark::default()
        },
        Event::Text(text) => body(tape, TEXT, &text),
        Event::Code(text) => body(tape, CODE, &text),
        Event::Html(text) | Event::InlineHtml(text) => body(tape, HTML, &text),
        Event::SoftBreak => Mark {
            kind: BREAK,
            ..Mark::default()
        },
        Event::HardBreak => Mark {
            kind: BREAK,
            flag: 1,
            ..Mark::default()
        },
        Event::Rule => Mark {
            kind: RULE,
            ..Mark::default()
        },
        Event::TaskListMarker(done) => Mark {
            kind: TASK,
            flag: u8::from(done),
            ..Mark::default()
        },
        _ => return,
    };
    tape.push(mark, range);
}

#[native]
fn markdown(source: &str, out: Out<Marks>) -> *mut c_void {
    let mut extensions = Options::empty();
    extensions.insert(Options::ENABLE_STRIKETHROUGH);
    extensions.insert(Options::ENABLE_TABLES);
    extensions.insert(Options::ENABLE_TASKLISTS);
    let mut tape = Tape {
        marks: Vec::new(),
        strings: Vec::with_capacity(source.len()),
    };
    for (event, range) in Parser::new_ext(source, extensions).into_offset_iter() {
        record(&mut tape, event, &range);
    }
    out.write(Marks {
        items: tape.marks.as_ptr(),
        count: tape.marks.len(),
        strings: tape.strings.as_ptr(),
    });
    abi::handle(Box::new(tape))
}
