use std::ops::Range;

use mlua::{IntoLua, Lua, Table, Value};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use uji_macros::{function, value};

#[value]
#[derive(Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Start,
    End,
    Text,
    Code,
    Html,
    Break,
    Rule,
    Task,
}

#[value]
#[derive(Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum Element {
    Paragraph,
    Heading,
    Blockquote,
    CodeBlock,
    List,
    Item,
    Emphasis,
    Strong,
    Strikethrough,
    Link,
    Image,
    Table,
    TableHead,
    TableRow,
    TableCell,
    Other,
}

impl From<&Tag<'_>> for Element {
    fn from(tag: &Tag<'_>) -> Self {
        match tag {
            Tag::Paragraph => Self::Paragraph,
            Tag::Heading { .. } => Self::Heading,
            Tag::BlockQuote(_) => Self::Blockquote,
            Tag::CodeBlock(_) => Self::CodeBlock,
            Tag::List(_) => Self::List,
            Tag::Item => Self::Item,
            Tag::Emphasis => Self::Emphasis,
            Tag::Strong => Self::Strong,
            Tag::Strikethrough => Self::Strikethrough,
            Tag::Link { .. } => Self::Link,
            Tag::Image { .. } => Self::Image,
            Tag::Table(_) => Self::Table,
            Tag::TableHead => Self::TableHead,
            Tag::TableRow => Self::TableRow,
            Tag::TableCell => Self::TableCell,
            _ => Self::Other,
        }
    }
}

impl From<TagEnd> for Element {
    fn from(tag: TagEnd) -> Self {
        match tag {
            TagEnd::Paragraph => Self::Paragraph,
            TagEnd::Heading(_) => Self::Heading,
            TagEnd::BlockQuote(_) => Self::Blockquote,
            TagEnd::CodeBlock => Self::CodeBlock,
            TagEnd::List(_) => Self::List,
            TagEnd::Item => Self::Item,
            TagEnd::Emphasis => Self::Emphasis,
            TagEnd::Strong => Self::Strong,
            TagEnd::Strikethrough => Self::Strikethrough,
            TagEnd::Link => Self::Link,
            TagEnd::Image => Self::Image,
            TagEnd::Table => Self::Table,
            TagEnd::TableHead => Self::TableHead,
            TagEnd::TableRow => Self::TableRow,
            TagEnd::TableCell => Self::TableCell,
            _ => Self::Other,
        }
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

fn detail(lua: &Lua, tag: &Tag<'_>) -> mlua::Result<Value> {
    match tag {
        Tag::Heading { level: at, .. } => Ok(Value::Integer(level(*at))),
        Tag::CodeBlock(CodeBlockKind::Fenced(language)) => language.as_ref().into_lua(lua),
        Tag::List(start) => Ok(start
            .and_then(|start| i64::try_from(start).ok())
            .map_or(Value::Nil, Value::Integer)),
        Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => dest_url.as_ref().into_lua(lua),
        _ => Ok(Value::Nil),
    }
}

fn entry(
    lua: &Lua,
    kind: Kind,
    first: impl IntoLua,
    second: impl IntoLua,
    range: Range<usize>,
) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.raw_set(1, kind)?;
    table.raw_set(2, first)?;
    table.raw_set(3, second)?;
    table.raw_set(4, range.start.saturating_add(1))?;
    table.raw_set(5, range.end)?;
    Ok(table)
}

fn event(lua: &Lua, event: Event<'_>, range: Range<usize>) -> mlua::Result<Option<Table>> {
    let table = match event {
        Event::Start(tag) => entry(
            lua,
            Kind::Start,
            Element::from(&tag),
            detail(lua, &tag)?,
            range,
        ),
        Event::End(tag) => entry(lua, Kind::End, Element::from(tag), Value::Nil, range),
        Event::Text(body) => entry(lua, Kind::Text, body.as_ref(), Value::Nil, range),
        Event::Code(body) => entry(lua, Kind::Code, body.as_ref(), Value::Nil, range),
        Event::Html(body) | Event::InlineHtml(body) => {
            entry(lua, Kind::Html, body.as_ref(), Value::Nil, range)
        }
        Event::SoftBreak => entry(lua, Kind::Break, "soft", Value::Nil, range),
        Event::HardBreak => entry(lua, Kind::Break, "hard", Value::Nil, range),
        Event::Rule => entry(lua, Kind::Rule, Value::Nil, Value::Nil, range),
        Event::TaskListMarker(done) => entry(lua, Kind::Task, done, Value::Nil, range),
        _ => return Ok(None),
    };
    table.map(Some)
}

#[function]
fn markdown(lua: &Lua, source: &str) -> mlua::Result<Table> {
    let mut extensions = Options::empty();
    extensions.insert(Options::ENABLE_STRIKETHROUGH);
    extensions.insert(Options::ENABLE_TABLES);
    extensions.insert(Options::ENABLE_TASKLISTS);
    let events = lua.create_table()?;
    for (parsed, range) in Parser::new_ext(source, extensions).into_offset_iter() {
        if let Some(table) = event(lua, parsed, range)? {
            events.raw_push(table)?;
        }
    }
    Ok(events)
}
