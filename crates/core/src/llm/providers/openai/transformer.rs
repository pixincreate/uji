use serde::{Deserialize, Serialize};

use crate::llm::LlmRequest;
use crate::llm::providers::acc::ToolAcc;
use crate::session::model::{Message, ToolCall};

#[derive(Serialize)]
pub struct OpenAiRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<&'static str>,
    pub model: String,
    pub messages: Vec<OpenAiMessage>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<OpenAiTool>,
}

#[derive(Serialize)]
pub struct StreamOptions {
    pub include_usage: bool,
}

#[derive(Serialize)]
pub struct OpenAiTool {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub function: OpenAiToolFunction,
}

#[derive(Serialize)]
pub struct OpenAiToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Serialize)]
pub struct OpenAiMessage {
    pub role: &'static str,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<OpenAiCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OpenAiCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: OpenAiCallFunction,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OpenAiCallFunction {
    pub name: String,
    pub arguments: String,
}

fn message(role: &'static str, content: Option<String>) -> OpenAiMessage {
    OpenAiMessage {
        role,
        content,
        tool_calls: Vec::new(),
        tool_call_id: None,
    }
}

impl From<&LlmRequest> for OpenAiRequest {
    fn from(request: &LlmRequest) -> Self {
        let mut messages = Vec::with_capacity(request.messages.len() + 1);
        if let Some(system) = &request.system {
            messages.push(message("system", Some(system.clone())));
        }
        for item in &request.messages {
            match item {
                Message::User { text } => messages.push(message("user", Some(text.clone()))),
                Message::Assistant {
                    text, tool_calls, ..
                } => messages.push(OpenAiMessage {
                    role: "assistant",
                    content: if text.is_empty() {
                        None
                    } else {
                        Some(text.clone())
                    },
                    tool_calls: tool_calls
                        .iter()
                        .map(|call| OpenAiCall {
                            id: call.id.clone(),
                            kind: String::from("function"),
                            function: OpenAiCallFunction {
                                name: call.name.clone(),
                                arguments: call.arguments.clone(),
                            },
                        })
                        .collect(),
                    tool_call_id: None,
                }),
                Message::Tool {
                    tool_call_id,
                    content,
                    ..
                } => messages.push(OpenAiMessage {
                    role: "tool",
                    content: Some(content.clone()),
                    tool_calls: Vec::new(),
                    tool_call_id: Some(tool_call_id.clone()),
                }),
                Message::System { text } => {
                    messages.push(message("system", Some(text.clone())));
                }
                Message::Error { .. } | Message::Compaction { .. } => {}
            }
        }
        let tools = request
            .tools
            .iter()
            .map(|tool| OpenAiTool {
                kind: "function",
                function: OpenAiToolFunction {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    parameters: tool.parameters.clone(),
                },
            })
            .collect();
        Self {
            reasoning_effort: match request.effort {
                crate::llm::Effort::Off => None,
                crate::llm::Effort::Minimal => Some("minimal"),
                crate::llm::Effort::Low => Some("low"),
                crate::llm::Effort::Medium => Some("medium"),
                crate::llm::Effort::High => Some("high"),
            },
            model: request.model.clone(),
            messages,
            stream: false,
            stream_options: None,
            tools,
        }
    }
}

#[derive(Deserialize)]
pub struct OpenAiResponse {
    pub choices: Vec<OpenAiChoice>,
    #[serde(default)]
    pub usage: Option<OpenAiUsage>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct OpenAiUsage {
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    #[serde(default)]
    pub prompt_tokens_details: OpenAiPromptDetails,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct OpenAiPromptDetails {
    #[serde(default)]
    pub cached_tokens: u64,
}

impl From<OpenAiUsage> for crate::llm::Usage {
    fn from(usage: OpenAiUsage) -> Self {
        let cache_read = usage.prompt_tokens_details.cached_tokens;
        Self {
            input: usage.prompt_tokens.saturating_sub(cache_read),
            output: usage.completion_tokens,
            cache_read,
            cache_write: 0,
        }
    }
}

#[derive(Deserialize)]
pub struct OpenAiChoice {
    pub message: OpenAiMessageOut,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Deserialize)]
pub struct OpenAiMessageOut {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<OpenAiCall>,
    #[serde(default)]
    pub reasoning_content: Option<String>,
}

impl OpenAiResponse {
    pub fn text(&self) -> Option<&str> {
        self.choices
            .first()
            .and_then(|choice| choice.message.content.as_deref())
    }

    pub fn reasoning_content(&self) -> Option<&str> {
        self.choices
            .first()
            .and_then(|choice| choice.message.reasoning_content.as_deref())
    }

    pub fn finish_reason(&self) -> Option<&str> {
        self.choices
            .first()
            .and_then(|choice| choice.finish_reason.as_deref())
    }

    pub fn tool_calls(&self) -> Vec<ToolCall> {
        self.choices.first().map_or_else(Vec::new, |choice| {
            choice
                .message
                .tool_calls
                .iter()
                .map(|call| ToolCall {
                    id: call.id.clone(),
                    name: call.function.name.clone(),
                    arguments: call.function.arguments.clone(),
                })
                .collect()
        })
    }
}

#[derive(Deserialize)]
pub struct OpenAiChunk {
    #[serde(default)]
    pub choices: Vec<OpenAiDelta>,
    #[serde(default)]
    pub usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
pub struct OpenAiDelta {
    pub delta: OpenAiDeltaContent,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Deserialize)]
pub struct OpenAiDeltaContent {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub reasoning_content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<OpenAiDeltaToolCall>,
}

#[derive(Deserialize)]
pub struct OpenAiDeltaToolCall {
    pub index: usize,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub function: Option<OpenAiDeltaFunction>,
}

#[derive(Deserialize)]
pub struct OpenAiDeltaFunction {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

impl OpenAiChunk {
    pub fn delta_text(&self) -> Option<&str> {
        self.choices
            .first()
            .and_then(|choice| choice.delta.content.as_deref())
    }

    pub fn delta_reasoning(&self) -> Option<&str> {
        self.choices
            .first()
            .and_then(|choice| choice.delta.reasoning_content.as_deref())
    }

    pub fn finish_reason(&self) -> Option<&str> {
        self.choices
            .first()
            .and_then(|choice| choice.finish_reason.as_deref())
    }
}

#[derive(Default)]
pub struct OpenAiToolAcc {
    acc: ToolAcc,
}

impl OpenAiToolAcc {
    pub fn apply(&mut self, chunk: &OpenAiChunk) {
        let Some(choice) = chunk.choices.first() else {
            return;
        };
        for call in &choice.delta.tool_calls {
            let entry = self.acc.entry(call.index);
            if let Some(id) = &call.id {
                entry.id.clone_from(id);
            }
            let Some(function) = &call.function else {
                continue;
            };
            if let Some(name) = &function.name {
                entry.name.clone_from(name);
            }
            if let Some(arguments) = &function.arguments {
                entry.arguments.push_str(arguments);
            }
        }
    }

    pub fn finish(self) -> Vec<ToolCall> {
        self.acc.finish()
    }
}
