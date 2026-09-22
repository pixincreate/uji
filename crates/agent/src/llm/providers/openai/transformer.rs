use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::dialect;
use crate::llm::providers::acc::{ToolAcc, arguments_of};
use crate::llm::{Compat, LlmRequest, ToolSpec, fit_thinking};
use crate::session::model::{self as session, ToolCall};

#[derive(Serialize)]
pub struct Request<'a> {
    pub model: &'a str,
    pub messages: Vec<Message<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Tool<'a>>,
    pub stream: bool,
    pub stream_options: StreamOptions,
    #[serde(flatten)]
    pub dialect: Map<String, Value>,
}

#[derive(Serialize)]
pub struct StreamOptions {
    pub include_usage: bool,
}

#[derive(Serialize)]
pub struct Tool<'a> {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub function: ToolFunction<'a>,
}

#[derive(Serialize)]
pub struct ToolFunction<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub parameters: &'a Value,
}

#[derive(Serialize)]
pub struct Message<'a> {
    pub role: &'static str,
    pub content: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<Call<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<&'a str>,
}

#[derive(Serialize)]
pub struct Call<'a> {
    pub id: &'a str,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub function: CallFunction<'a>,
}

#[derive(Serialize)]
pub struct CallFunction<'a> {
    pub name: &'a str,
    pub arguments: String,
}

impl<'a> Request<'a> {
    pub fn build(request: &'a LlmRequest<'a>, compat: Compat) -> Self {
        let (limit, _) = fit_thinking(request.effort, request.max_output);
        Self {
            model: request.model,
            messages: request
                .system
                .map(Message::system)
                .into_iter()
                .chain(
                    request
                        .messages
                        .iter()
                        .filter_map(|message| Message::from_session(message, compat)),
                )
                .collect(),
            tools: request.tools.iter().map(Tool::from).collect(),
            stream: true,
            stream_options: StreamOptions {
                include_usage: true,
            },
            dialect: dialect::fields(compat, request.effort, limit),
        }
    }
}

impl<'a> Message<'a> {
    fn new(role: &'static str, content: Option<&'a str>) -> Self {
        Self {
            role,
            content,
            tool_calls: Vec::new(),
            tool_call_id: None,
            name: None,
        }
    }

    fn system(text: &'a str) -> Self {
        Self::new("system", Some(text))
    }

    fn from_session(message: &'a session::Message, compat: Compat) -> Option<Self> {
        match message {
            session::Message::User { text } => Some(Self::new("user", Some(text))),
            session::Message::System { text } => Some(Self::system(text)),
            session::Message::Assistant {
                text, tool_calls, ..
            } => Some(Self {
                tool_calls: tool_calls.iter().map(Call::from).collect(),
                ..Self::new("assistant", (!text.is_empty()).then_some(text.as_str()))
            }),
            session::Message::Tool {
                tool_call_id,
                content,
                name,
            } => Some(Self {
                tool_call_id: Some(tool_call_id),
                name: compat.tool_result_name.then_some(name.as_str()),
                ..Self::new("tool", Some(content))
            }),
            session::Message::Shell { .. }
            | session::Message::Error { .. }
            | session::Message::Compaction { .. } => None,
        }
    }
}

impl<'a> From<&'a ToolCall> for Call<'a> {
    fn from(call: &'a ToolCall) -> Self {
        Self {
            id: &call.id,
            kind: "function",
            function: CallFunction {
                name: &call.name,
                arguments: arguments_of(&call.arguments).to_string(),
            },
        }
    }
}

impl<'a> From<&'a ToolSpec> for Tool<'a> {
    fn from(tool: &'a ToolSpec) -> Self {
        Self {
            kind: "function",
            function: ToolFunction {
                name: &tool.name,
                description: &tool.description,
                parameters: &tool.parameters,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    pub prompt_tokens_details: Option<PromptDetails>,
    #[serde(default)]
    pub prompt_cache_hit_tokens: u64,
    #[serde(default)]
    pub cached_tokens: u64,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct PromptDetails {
    #[serde(default)]
    pub cached_tokens: u64,
    #[serde(default)]
    pub cache_write_tokens: u64,
}

impl From<Usage> for crate::llm::Usage {
    fn from(usage: Usage) -> Self {
        let details = usage.prompt_tokens_details.unwrap_or_default();
        let cache_read = details
            .cached_tokens
            .max(usage.prompt_cache_hit_tokens)
            .max(usage.cached_tokens);
        let cache_write = details.cache_write_tokens;
        Self {
            input: usage
                .prompt_tokens
                .saturating_sub(cache_read)
                .saturating_sub(cache_write),
            output: usage.completion_tokens,
            cache_read,
            cache_write,
        }
    }
}

#[derive(Deserialize)]
pub struct Chunk {
    #[serde(default)]
    pub choices: Vec<Delta>,
    pub usage: Option<Usage>,
}

#[derive(Deserialize)]
pub struct Delta {
    pub delta: DeltaContent,
    pub finish_reason: Option<String>,
}

#[derive(Deserialize)]
pub struct DeltaContent {
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<DeltaToolCall>,
}

#[derive(Deserialize)]
pub struct DeltaToolCall {
    pub index: usize,
    pub id: Option<String>,
    pub function: Option<DeltaFunction>,
}

#[derive(Deserialize)]
pub struct DeltaFunction {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

impl Chunk {
    fn choice(&self) -> Option<&Delta> {
        self.choices.first()
    }

    pub fn delta_text(&self) -> Option<&str> {
        self.choice()?.delta.content.as_deref()
    }

    pub fn delta_reasoning(&self) -> Option<&str> {
        self.choice()?.delta.reasoning_content.as_deref()
    }

    pub fn finish_reason(&self) -> Option<&str> {
        self.choice()?.finish_reason.as_deref()
    }

    pub fn accumulate(&self, acc: &mut ToolAcc) {
        let Some(choice) = self.choice() else {
            return;
        };
        for call in &choice.delta.tool_calls {
            let entry = acc.entry(call.index);
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
}
