use std::rc::Rc;

use uji_agent::session::model::Message;
use uji_ui::app::renderer::{Block, BlockRenderer};
use uji_ui::model::Line;

use super::events;
use super::inner::Inner;

pub(crate) struct LuaRenderer {
    inner: Rc<Inner>,
}

impl LuaRenderer {
    pub(crate) fn new(inner: Rc<Inner>) -> Self {
        Self { inner }
    }

    fn payload(&self, block: Block<'_>) -> Option<mlua::Table> {
        let table = self.inner.lua.create_table().ok()?;
        let stored = match block {
            Block::Notice(text) => {
                table.set("type", "notice").ok()?;
                table.set("text", text).ok()?;
                return Some(table);
            }
            Block::Pending { text, .. } => {
                table.set("type", "pending").ok()?;
                table.set("text", text).ok()?;
                return Some(table);
            }
            Block::Thinking(text) => {
                table.set("type", "thinking").ok()?;
                table.set("text", text).ok()?;
                return Some(table);
            }
            Block::Queued(text) => {
                table.set("type", "queued").ok()?;
                table.set("text", text).ok()?;
                return Some(table);
            }
            Block::Message(stored) => stored,
        };
        table.set("type", stored.message.type_name()).ok()?;
        table.set("text", stored.message.text()).ok()?;
        match &stored.message {
            Message::Tool { name, .. } => table.set("name", name.as_str()).ok()?,
            Message::Assistant { tool_calls, .. } if !tool_calls.is_empty() => {
                let calls = self.inner.lua.create_table().ok()?;
                for (at, call) in tool_calls.iter().enumerate() {
                    let entry = self.inner.lua.create_table().ok()?;
                    entry.set("name", call.name.as_str()).ok()?;
                    entry.set("arguments", call.arguments.as_str()).ok()?;
                    calls.set(at.saturating_add(1), entry).ok()?;
                }
                table.set("tool_calls", calls).ok()?;
            }
            _ => {}
        }
        Some(table)
    }
}

impl BlockRenderer for LuaRenderer {
    fn overrides(&self) -> bool {
        self.inner.api.has_handler(events::RENDER_MESSAGE)
    }

    fn render(&self, block: Block<'_>) -> Option<Vec<Line>> {
        if !self.overrides() {
            return None;
        }
        let table = self.payload(block)?;
        let value = self.inner.api.ask(events::RENDER_MESSAGE, &table)?;
        match crate::api::window::lines_from_lua(value) {
            Ok(lines) => Some(lines),
            Err(err) => {
                self.inner.report(format!("render_message: {err}"));
                None
            }
        }
    }
}
