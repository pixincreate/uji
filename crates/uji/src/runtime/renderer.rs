use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use mlua::{Lua, LuaSerdeExt, Table, Value as LuaValue};
use uji_core::session::model::{Message, ToolCall};
use uji_ui::app::renderer::{Block, BlockRenderer};
use uji_ui::model::Line;

use super::events::{self, Event, RenderMessage};
use crate::api::Api;

pub(crate) struct LuaRenderer {
    lua: Lua,
    api: Rc<Api>,
    labels: RefCell<HashMap<String, Option<String>>>,
}

impl LuaRenderer {
    pub(crate) fn new(lua: Lua, api: Rc<Api>) -> Self {
        Self {
            lua,
            api,
            labels: RefCell::default(),
        }
    }

    fn label(&self, call: &ToolCall) -> Option<String> {
        let tool = self.api.tools().borrow().get(&call.name).map(Rc::clone)?;
        let arguments = serde_json::from_str::<serde_json::Value>(&call.arguments).ok()?;
        let LuaValue::Table(arguments) = self.lua.to_value(&arguments).ok()? else {
            return None;
        };
        let detail = tool.detail(&arguments).unwrap_or_else(|err| {
            self.api.notify(format!("{} subject: {err}", call.name));
            None
        });
        match (tool.display.verb.as_deref(), detail) {
            (Some(verb), Some(detail)) => Some(format!("{verb} {detail}")),
            (Some(verb), None) => Some(format!("{verb} {}", call.name)),
            (None, Some(detail)) => Some(format!("Called {} {detail}", call.name)),
            (None, None) => None,
        }
    }

    fn payload(&self, block: Block<'_>) -> mlua::Result<Table> {
        let message = match block {
            Block::Notice(text) => shown("notice", text),
            Block::Pending { text, .. } => shown("pending", text),
            Block::Thinking(text) => shown("thinking", text),
            Block::Queued(text) => shown("queued", text),
            Block::Message(stored) => RenderMessage {
                kind: stored.message.type_name(),
                text: stored.message.text(),
                name: match &stored.message {
                    Message::Tool { name, .. } => Some(name),
                    _ => None,
                },
                tool_calls: match &stored.message {
                    Message::Assistant { tool_calls, .. } => tool_calls,
                    _ => &[],
                },
            },
        };
        events::payload(&self.lua, &message)
    }
}

fn shown<'a>(kind: &'a str, text: &'a str) -> RenderMessage<'a> {
    RenderMessage {
        kind,
        text,
        name: None,
        tool_calls: &[],
    }
}

impl BlockRenderer for LuaRenderer {
    fn overrides(&self) -> bool {
        self.api.has_handler(RenderMessage::NAME)
    }

    fn tool_label(&self, call: &ToolCall) -> Option<String> {
        let cached = self.labels.borrow().get(&call.id).cloned();
        if let Some(label) = cached {
            return label;
        }
        let label = self.label(call);
        self.labels
            .borrow_mut()
            .insert(call.id.clone(), label.clone());
        label
    }

    fn render(&self, block: Block<'_>) -> Option<Vec<Line>> {
        if !self.overrides() {
            return None;
        }
        let table = self
            .payload(block)
            .map_err(|err| self.api.notify(format!("{}: {err}", RenderMessage::NAME)))
            .ok()?;
        let value = self.api.ask(RenderMessage::NAME, &table)?;
        match crate::api::window::lines_from_lua(value) {
            Ok(lines) => Some(lines),
            Err(err) => {
                self.api.notify(format!("render_message: {err}"));
                None
            }
        }
    }
}
