use std::collections::HashSet;

use crate::session::model::{Message, StoredMessage};

pub fn sanitize(stored: &[StoredMessage]) -> Vec<Message> {
    let messages = stored.iter().map(|entry| &entry.message);
    let mut answered: HashSet<&str> = HashSet::new();
    for message in messages.clone() {
        if let Message::Tool { tool_call_id, .. } = message {
            answered.insert(tool_call_id.as_str());
        }
    }

    let mut kept: HashSet<String> = HashSet::new();
    let mut out = Vec::with_capacity(stored.len());
    for message in messages {
        match message {
            Message::Assistant {
                text,
                tool_calls,
                reasoning_content,
            } if !tool_calls.is_empty() => {
                let complete = tool_calls
                    .iter()
                    .all(|call| answered.contains(call.id.as_str()));
                if complete {
                    for call in tool_calls {
                        kept.insert(call.id.clone());
                    }
                    out.push(message.clone());
                } else if !text.is_empty() {
                    out.push(Message::Assistant {
                        text: text.clone(),
                        tool_calls: Vec::new(),
                        reasoning_content: reasoning_content.clone(),
                    });
                }
            }
            Message::Tool { tool_call_id, .. } => {
                if kept.contains(tool_call_id.as_str()) {
                    out.push(message.clone());
                }
            }
            _ => out.push(message.clone()),
        }
    }
    out
}
