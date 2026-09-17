use serde::{Deserialize, Serialize};

use crate::llm::providers::acc::{ToolAcc, arguments_of};
use crate::llm::{LlmRequest, Retention};
use crate::session::model::{Message, ToolCall};

const MAX_TOKENS: &str = "max_tokens";

#[derive(Debug, Clone, Serialize)]
pub struct Thinking {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub budget_tokens: u32,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<&'static str>,
}

impl CacheControl {
    fn new(retention: Retention) -> Option<Self> {
        retention.enabled().then(|| Self {
            kind: "ephemeral",
            ttl: retention.ttl(),
        })
    }
}

#[derive(Serialize)]
pub struct SystemBlock {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

impl SystemBlock {
    fn text(text: String) -> Self {
        Self {
            kind: "text",
            text,
            cache_control: None,
        }
    }
}

#[derive(Serialize)]
pub struct AnthropicRequest<'a> {
    pub model: &'a str,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<Thinking>,
    pub system: Vec<SystemBlock>,
    pub messages: Vec<AnthropicMessage<'a>>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<AnthropicTool<'a>>,
}

impl AnthropicRequest<'_> {
    fn cached(mut self, retention: Retention) -> Self {
        let Some(control) = CacheControl::new(retention) else {
            return self;
        };
        if let Some(block) = self.system.last_mut() {
            block.cache_control = Some(control);
        }
        if let Some(tool) = self.tools.last_mut() {
            tool.cache_control = Some(control);
        }
        if let Some(block) = self
            .messages
            .iter_mut()
            .rfind(|message| message.role == "user")
            .and_then(|message| message.content.last_mut())
        {
            block.cache_control = Some(control);
        }
        self
    }

    pub fn prepend_system(&mut self, text: &str) {
        if self.system.first().is_some_and(|block| block.text == text) {
            return;
        }
        self.system.insert(0, SystemBlock::text(text.to_string()));
    }
}

#[derive(Serialize)]
pub struct AnthropicTool<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub input_schema: &'a serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

#[derive(Serialize)]
pub struct AnthropicMessage<'a> {
    pub role: &'static str,
    pub content: Vec<Block<'a>>,
}

#[derive(Serialize)]
pub struct Block<'a> {
    #[serde(flatten)]
    pub body: AnthropicBlock<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

impl<'a> From<AnthropicBlock<'a>> for Block<'a> {
    fn from(body: AnthropicBlock<'a>) -> Self {
        Self {
            body,
            cache_control: None,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "type")]
pub enum AnthropicBlock<'a> {
    #[serde(rename = "text")]
    Text { text: &'a str },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: &'a str,
        name: &'a str,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: &'a str,
        content: &'a str,
    },
}

impl<'a> From<&'a LlmRequest<'a>> for AnthropicRequest<'a> {
    fn from(request: &'a LlmRequest<'a>) -> Self {
        let mut system = request.system.unwrap_or_default().to_string();
        let mut messages = Vec::with_capacity(request.messages.len());
        for item in request.messages {
            match item {
                Message::User { text } => messages.push(AnthropicMessage {
                    role: "user",
                    content: vec![
                        AnthropicBlock::Text {
                            text: text.as_str(),
                        }
                        .into(),
                    ],
                }),
                Message::Assistant {
                    text, tool_calls, ..
                } => {
                    let mut blocks = Vec::new();
                    if !text.is_empty() {
                        blocks.push(
                            AnthropicBlock::Text {
                                text: text.as_str(),
                            }
                            .into(),
                        );
                    }
                    for call in tool_calls {
                        let input = arguments_of(&call.arguments);
                        blocks.push(
                            AnthropicBlock::ToolUse {
                                id: &call.id,
                                name: &call.name,
                                input,
                            }
                            .into(),
                        );
                    }
                    messages.push(AnthropicMessage {
                        role: "assistant",
                        content: blocks,
                    });
                }
                Message::Tool {
                    tool_call_id,
                    content,
                    ..
                } => messages.push(AnthropicMessage {
                    role: "user",
                    content: vec![
                        AnthropicBlock::ToolResult {
                            tool_use_id: tool_call_id,
                            content,
                        }
                        .into(),
                    ],
                }),
                Message::System { text } => {
                    if !system.is_empty() {
                        system.push('\n');
                    }
                    system.push_str(text);
                }
                Message::Error { .. } | Message::Compaction { .. } => {}
            }
        }
        let tools = request
            .tools
            .iter()
            .map(|tool| AnthropicTool {
                name: &tool.name,
                description: &tool.description,
                input_schema: &tool.parameters,
                cache_control: None,
            })
            .collect();
        let (max_tokens, budget) = crate::llm::fit_thinking(request.effort, request.max_output);
        Self {
            model: request.model,
            max_tokens,
            thinking: (budget > 0).then_some(Thinking {
                kind: "enabled",
                budget_tokens: budget,
            }),
            system: if system.is_empty() {
                Vec::new()
            } else {
                vec![SystemBlock::text(system)]
            },
            messages,
            stream: false,
            tools,
        }
        .cached(request.cache)
    }
}

#[derive(Deserialize)]
pub struct AnthropicResponse {
    pub content: Vec<AnthropicOutBlock>,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub usage: Option<AnthropicUsage>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct AnthropicUsage {
    #[serde(default, rename = "input_tokens")]
    pub input: u64,
    #[serde(default, rename = "output_tokens")]
    pub output: u64,
    #[serde(default, rename = "cache_read_input_tokens")]
    pub cache_read: u64,
    #[serde(default, rename = "cache_creation_input_tokens")]
    pub cache_write: u64,
}

impl From<AnthropicUsage> for crate::llm::Usage {
    fn from(usage: AnthropicUsage) -> Self {
        Self {
            input: usage.input,
            output: usage.output,
            cache_read: usage.cache_read,
            cache_write: usage.cache_write,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicOutBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

impl AnthropicResponse {
    pub fn truncated(&self) -> bool {
        self.stop_reason.as_deref() == Some(MAX_TOKENS)
    }

    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                AnthropicOutBlock::Text { text } => Some(text.as_str()),
                AnthropicOutBlock::ToolUse { .. } => None,
            })
            .collect()
    }

    pub fn tool_calls(&self) -> Vec<ToolCall> {
        self.content
            .iter()
            .filter_map(|block| match block {
                AnthropicOutBlock::ToolUse { id, name, input } => Some(ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    arguments: input.to_string(),
                }),
                AnthropicOutBlock::Text { .. } => None,
            })
            .collect()
    }
}

#[derive(Deserialize)]
pub struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    pub kind: String,
    pub index: Option<usize>,
    pub content_block: Option<AnthropicStreamBlock>,
    pub delta: Option<AnthropicStreamDelta>,
    #[serde(default)]
    pub message: Option<AnthropicStreamMessage>,
    #[serde(default)]
    pub usage: Option<AnthropicUsage>,
}

#[derive(Deserialize)]
pub struct AnthropicStreamMessage {
    #[serde(default)]
    pub usage: Option<AnthropicUsage>,
}

#[derive(Deserialize)]
pub struct AnthropicStreamBlock {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct AnthropicStreamDelta {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub partial_json: Option<String>,
    #[serde(default)]
    pub stop_reason: Option<String>,
}

impl AnthropicStreamEvent {
    pub fn truncated(&self) -> bool {
        self.delta
            .as_ref()
            .and_then(|delta| delta.stop_reason.as_deref())
            == Some(MAX_TOKENS)
    }

    pub fn input_usage(&self) -> Option<crate::llm::Usage> {
        self.message
            .as_ref()
            .and_then(|message| message.usage)
            .map(|usage| crate::llm::Usage {
                output: 0,
                ..usage.into()
            })
    }

    pub fn output_tokens(&self) -> Option<u64> {
        self.usage.map(|usage| usage.output)
    }

    pub fn text_delta(&self) -> Option<&str> {
        if self.kind == "content_block_delta" {
            self.delta.as_ref().and_then(|delta| {
                if delta.kind == "text_delta" {
                    delta.text.as_deref()
                } else {
                    None
                }
            })
        } else {
            None
        }
    }
}

#[derive(Default)]
pub struct AnthropicToolAcc {
    acc: ToolAcc,
}

impl AnthropicToolAcc {
    pub fn apply(&mut self, event: &AnthropicStreamEvent) {
        let Some(index) = event.index else {
            return;
        };
        match event.kind.as_str() {
            "content_block_start" => {
                if let Some(block) = &event.content_block
                    && block.kind == "tool_use"
                {
                    let entry = self.acc.entry(index);
                    entry.id = block.id.clone().unwrap_or_default();
                    entry.name = block.name.clone().unwrap_or_default();
                }
            }
            "content_block_delta" => {
                if let Some(delta) = &event.delta
                    && delta.kind == "input_json_delta"
                    && let Some(fragment) = &delta.partial_json
                {
                    self.acc.entry(index).arguments.push_str(fragment);
                }
            }
            _ => {}
        }
    }

    pub fn finish(self) -> Result<Vec<ToolCall>, crate::llm::error::LlmError> {
        self.acc.finish()
    }
}
