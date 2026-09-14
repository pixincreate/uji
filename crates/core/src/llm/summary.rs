use std::sync::Arc;

use super::{Llm, LlmRequest, Usage};
use crate::session::model::{Message, StoredMessage};

const PROMPT: &str = "You compact coding sessions. You are given the earlier part of a \
conversation between a user and a coding agent. Write a summary that lets the agent keep \
working without the original transcript.\n\n\
Cover, in this order and only where they apply:\n\
- What the user asked for, including constraints and preferences they stated.\n\
- Decisions taken and the reasoning behind them.\n\
- Files created, edited, or investigated, with their paths.\n\
- What is done, what is verified, and what is still outstanding.\n\n\
Be specific: keep identifiers, paths, commands, and error strings verbatim. Do not invent \
anything that is not in the transcript. Reply with the summary alone, no preamble.";

const MAX_INPUT: usize = 200_000;

pub struct Summarized {
    pub summary: String,
    pub usage: Option<Usage>,
}

pub async fn generate(
    client: &reqwest::Client,
    provider: Arc<dyn Llm>,
    model: String,
    stored: &[StoredMessage],
) -> Option<Summarized> {
    let transcript = transcript(stored);
    if transcript.trim().is_empty() {
        return None;
    }
    let request = LlmRequest {
        model,
        system: Some(PROMPT.to_string()),
        messages: vec![Message::User { text: transcript }],
        tools: Vec::new(),
    };
    let response = provider.send_request(client, &request).await.ok()?;
    let summary = response.text.trim().to_string();
    if summary.is_empty() {
        return None;
    }
    Some(Summarized {
        summary,
        usage: response.usage,
    })
}

fn transcript(stored: &[StoredMessage]) -> String {
    let mut chunks: Vec<String> = Vec::new();
    let mut budget = MAX_INPUT;
    for entry in stored.iter().rev() {
        let label = match &entry.message {
            Message::User { .. } => "user",
            Message::Assistant { .. } => "assistant",
            Message::Tool { name, .. } => name.as_str(),
            Message::System { .. } => "system",
            Message::Error { .. } => "error",
            Message::Compaction { .. } => "earlier summary",
        };
        let text = entry.message.text();
        if text.trim().is_empty() {
            continue;
        }
        let chunk = format!("[{label}] {text}\n\n");
        let size = chunk.chars().count();
        if size > budget {
            break;
        }
        budget = budget.saturating_sub(size);
        chunks.push(chunk);
    }
    chunks.reverse();
    chunks.concat()
}
