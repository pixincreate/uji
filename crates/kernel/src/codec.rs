use base64::Engine;
use base64::engine::GeneralPurpose;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use std::ops::Range;

use mlua::{BString, Lua, Table, Value};
use pulldown_cmark::{
    CodeBlockKind, Event, HeadingLevel, Options as Extensions, Parser, Tag, TagEnd,
};
use rand::RngCore;
use sha2::{Digest, Sha256};
use uji_macros::{FromLua, function};
use unicode_width::UnicodeWidthStr;

#[derive(FromLua)]
struct Base64Options {
    #[lua(default)]
    url: bool,
    #[lua(default = true)]
    pad: bool,
}

impl Base64Options {
    fn engine(&self) -> &'static GeneralPurpose {
        match (self.url, self.pad) {
            (false, true) => &STANDARD,
            (false, false) => &STANDARD_NO_PAD,
            (true, true) => &URL_SAFE,
            (true, false) => &URL_SAFE_NO_PAD,
        }
    }
}

#[function(base64)]
fn encode(data: &mlua::LuaString, opts: &Base64Options) -> String {
    opts.engine().encode(data.as_bytes())
}

#[function(base64)]
fn decode(text: &mlua::LuaString, opts: &Base64Options) -> mlua::Result<BString> {
    opts.engine()
        .decode(text.as_bytes())
        .map(BString::from)
        .map_err(mlua::Error::external)
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

fn opened(lua: &Lua, tag: &Tag<'_>) -> mlua::Result<(&'static str, Value)> {
    let text = |text: &str| lua.create_string(text).map(Value::String);
    Ok(match tag {
        Tag::Paragraph => ("paragraph", Value::Nil),
        Tag::Heading { level: at, .. } => ("heading", Value::Integer(level(*at))),
        Tag::BlockQuote(_) => ("blockquote", Value::Nil),
        Tag::CodeBlock(CodeBlockKind::Fenced(language)) => ("code_block", text(language)?),
        Tag::CodeBlock(CodeBlockKind::Indented) => ("code_block", Value::Nil),
        Tag::List(start) => (
            "list",
            start
                .and_then(|start| i64::try_from(start).ok())
                .map_or(Value::Nil, Value::Integer),
        ),
        Tag::Item => ("item", Value::Nil),
        Tag::Emphasis => ("emphasis", Value::Nil),
        Tag::Strong => ("strong", Value::Nil),
        Tag::Strikethrough => ("strikethrough", Value::Nil),
        Tag::Link { dest_url, .. } => ("link", text(dest_url)?),
        Tag::Image { dest_url, .. } => ("image", text(dest_url)?),
        Tag::Table(_) => ("table", Value::Nil),
        Tag::TableHead => ("table_head", Value::Nil),
        Tag::TableRow => ("table_row", Value::Nil),
        Tag::TableCell => ("table_cell", Value::Nil),
        _ => ("other", Value::Nil),
    })
}

fn closed(tag: TagEnd) -> &'static str {
    match tag {
        TagEnd::Paragraph => "paragraph",
        TagEnd::Heading(_) => "heading",
        TagEnd::BlockQuote(_) => "blockquote",
        TagEnd::CodeBlock => "code_block",
        TagEnd::List(_) => "list",
        TagEnd::Item => "item",
        TagEnd::Emphasis => "emphasis",
        TagEnd::Strong => "strong",
        TagEnd::Strikethrough => "strikethrough",
        TagEnd::Link => "link",
        TagEnd::Image => "image",
        TagEnd::Table => "table",
        TagEnd::TableHead => "table_head",
        TagEnd::TableRow => "table_row",
        TagEnd::TableCell => "table_cell",
        _ => "other",
    }
}

fn event(lua: &Lua, event: Event<'_>, range: Range<usize>) -> mlua::Result<Option<Table>> {
    let entry = |kind: &str, first: Value, second: Value| -> mlua::Result<Option<Table>> {
        let table = lua.create_table()?;
        table.raw_set(1, kind)?;
        table.raw_set(2, first)?;
        table.raw_set(3, second)?;
        table.raw_set(4, range.start.saturating_add(1))?;
        table.raw_set(5, range.end)?;
        Ok(Some(table))
    };
    let text = |text: &str| lua.create_string(text).map(Value::String);
    match event {
        Event::Start(tag) => {
            let (name, detail) = opened(lua, &tag)?;
            entry("start", text(name)?, detail)
        }
        Event::End(tag) => entry("end", text(closed(tag))?, Value::Nil),
        Event::Text(body) => entry("text", text(&body)?, Value::Nil),
        Event::Code(body) => entry("code", text(&body)?, Value::Nil),
        Event::Html(body) | Event::InlineHtml(body) => entry("html", text(&body)?, Value::Nil),
        Event::SoftBreak => entry("break", text("soft")?, Value::Nil),
        Event::HardBreak => entry("break", text("hard")?, Value::Nil),
        Event::Rule => entry("rule", Value::Nil, Value::Nil),
        Event::TaskListMarker(done) => entry("task", Value::Boolean(done), Value::Nil),
        _ => Ok(None),
    }
}

fn parse(lua: &Lua, source: &str) -> mlua::Result<Table> {
    let mut extensions = Extensions::empty();
    extensions.insert(Extensions::ENABLE_STRIKETHROUGH);
    extensions.insert(Extensions::ENABLE_TABLES);
    extensions.insert(Extensions::ENABLE_TASKLISTS);
    let events = lua.create_table()?;
    for (parsed, range) in Parser::new_ext(source, extensions).into_offset_iter() {
        if let Some(table) = event(lua, parsed, range)? {
            events.raw_push(table)?;
        }
    }
    Ok(events)
}

#[function]
fn sha256(data: &mlua::LuaString) -> BString {
    BString::from(Sha256::digest(data.as_bytes()).to_vec())
}

#[function]
fn random(count: usize) -> BString {
    let mut bytes = vec![0; count];
    rand::rng().fill_bytes(&mut bytes);
    BString::from(bytes)
}

#[function]
fn lossy(data: &mlua::LuaString) -> String {
    data.to_string_lossy()
}

#[function]
fn width(text: &mlua::LuaString) -> usize {
    text.to_string_lossy().width()
}

#[function]
fn markdown(lua: &Lua, source: &mlua::LuaString) -> mlua::Result<Table> {
    parse(lua, &source.to_string_lossy())
}
