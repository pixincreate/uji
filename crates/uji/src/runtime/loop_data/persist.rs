use mlua::Value as LuaValue;
use uji_agent::session::id::{MessageId, now_millis};
use uji_agent::session::model::{Message, StoredMessage, ToolCall};

use super::LoopData;
use crate::runtime::events;

impl LoopData {
    pub(super) fn append(&mut self, message: Message) {
        let kind = message.type_name();
        let text = message.text().to_string();
        let id = self.app.session().id;
        let stored = match self.storage.append_message(&id, message.clone()) {
            Ok(stored) => stored,
            Err(err) => {
                self.inner
                    .report(format!("failed to persist {kind} message: {err}"));
                let seq = self
                    .app
                    .messages()
                    .messages()
                    .last()
                    .map_or(0, |last| last.seq)
                    .saturating_add(1);
                StoredMessage {
                    id: MessageId::new(),
                    seq,
                    time_created: now_millis(),
                    message,
                }
            }
        };
        self.app.conversation().borrow_mut().push(stored);
        self.inner.emit(
            events::MESSAGE_APPENDED,
            &[("type", kind.to_string()), ("text", text.clone())],
        );
        if kind == "error" {
            self.inner.emit(events::ERROR, &[("text", text)]);
        }
        self.dirty = true;
    }
    pub(super) fn persist_assistant_step(
        &mut self,
        text: String,
        tool_calls: Vec<ToolCall>,
        reasoning_content: Option<String>,
    ) {
        if !tool_calls.is_empty() {
            let names = tool_calls
                .iter()
                .map(|call| call.name.as_str())
                .collect::<Vec<_>>()
                .join(",");
            self.inner.emit(events::TOOL_STARTED, &[("tools", names)]);
        }
        self.append(Message::Assistant {
            text,
            tool_calls,
            reasoning_content,
        });
    }

    pub(super) fn persist_tool_result(
        &mut self,
        tool_call_id: String,
        name: String,
        content: String,
    ) {
        let content = self
            .inner
            .ask(
                events::TOOL_FINISHED,
                &[("name", name.clone()), ("content", content.clone())],
            )
            .and_then(replacement_content)
            .unwrap_or(content);
        self.append(Message::Tool {
            tool_call_id,
            name,
            content,
        });
    }
    pub(super) fn fail_assistant(&mut self, error: &str) {
        self.append(Message::Error {
            text: error.to_string(),
        });
    }

    pub(super) fn finish_assistant(&mut self, text: &str, reasoning_content: Option<String>) {
        self.app.take_pending();
        self.append(Message::Assistant {
            text: text.to_string(),
            tool_calls: Vec::new(),
            reasoning_content,
        });
    }
}

fn replacement_content(value: LuaValue) -> Option<String> {
    let LuaValue::Table(table) = value else {
        return None;
    };
    table.get::<Option<String>>("content").ok().flatten()
}
