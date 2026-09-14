use std::collections::HashSet;

use crate::session::model::{Message, StoredMessage};

const SUMMARY_HEADER: &str = "Summary of the earlier part of this conversation:";

pub fn build(stored: &[StoredMessage]) -> Vec<Message> {
    let (summary, rest) = split_at_compaction(stored);
    let mut out = Vec::with_capacity(rest.len() + 1);
    if let Some(summary) = summary {
        out.push(Message::System {
            text: format!("{SUMMARY_HEADER}\n\n{summary}"),
        });
    }
    out.extend(sanitize(rest));
    out
}

fn split_at_compaction(stored: &[StoredMessage]) -> (Option<&str>, &[StoredMessage]) {
    let found = stored
        .iter()
        .rposition(|entry| matches!(entry.message, Message::Compaction { .. }));
    let Some(at) = found else {
        return (None, stored);
    };
    let summary = match &stored[at].message {
        Message::Compaction { summary, .. } => Some(summary.as_str()),
        _ => None,
    };
    (summary, &stored[at.saturating_add(1)..])
}

pub fn sanitize(stored: &[StoredMessage]) -> Vec<Message> {
    let messages = || stored.iter().map(|entry| &entry.message);
    let mut has_result: HashSet<&str> = HashSet::new();
    for message in messages() {
        if let Message::Tool { tool_call_id, .. } = message {
            has_result.insert(tool_call_id.as_str());
        }
    }

    let mut requested: HashSet<&str> = HashSet::new();
    let mut out = Vec::with_capacity(stored.len());
    for message in messages() {
        match message {
            Message::Assistant {
                text,
                tool_calls,
                reasoning_content,
            } if !tool_calls.is_empty() => {
                let complete = tool_calls
                    .iter()
                    .all(|call| has_result.contains(call.id.as_str()));
                if complete {
                    for call in tool_calls {
                        requested.insert(call.id.as_str());
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
                if requested.contains(tool_call_id.as_str()) {
                    out.push(message.clone());
                }
            }
            Message::Compaction { .. } => {}
            _ => out.push(message.clone()),
        }
    }
    out
}

const CHARS_PER_TOKEN: usize = 4;

fn estimate(text: &str) -> u64 {
    u64::try_from(text.chars().count() / CHARS_PER_TOKEN).unwrap_or(u64::MAX)
}

pub fn estimate_tokens(stored: &[StoredMessage]) -> u64 {
    let (summary, rest) = split_at_compaction(stored);
    let carried = summary.map_or(0, estimate);
    rest.iter()
        .map(|entry| estimate(entry.message.text()))
        .fold(carried, u64::saturating_add)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cut {
    pub from: usize,
    pub through: i64,
    pub compacted: usize,
}

impl Cut {
    pub fn span(self) -> usize {
        self.compacted.saturating_sub(self.from)
    }
}

pub fn find_cut(stored: &[StoredMessage], keep_recent: u64) -> Option<Cut> {
    let marker = stored
        .iter()
        .rposition(|entry| matches!(entry.message, Message::Compaction { .. }));
    let offset = marker.map_or(0, |at| at.saturating_add(1));
    let start = &stored[offset..];

    let mut budget = keep_recent;
    let mut first_kept = None;
    for (at, entry) in start.iter().enumerate().rev() {
        let size = estimate(entry.message.text());
        budget = budget.saturating_sub(size);
        if matches!(entry.message, Message::User { .. }) {
            first_kept = Some(at);
            if budget == 0 {
                break;
            }
        }
    }

    let first_kept = first_kept?;
    if first_kept == 0 {
        return None;
    }
    Some(Cut {
        from: marker.unwrap_or(0),
        through: start[first_kept.saturating_sub(1)].seq,
        compacted: offset.saturating_add(first_kept),
    })
}
