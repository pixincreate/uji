use serde::{Deserialize, Serialize};

use crate::llm::LlmRequest;
use crate::llm::providers::acc::ToolAcc;
use crate::session::model::{Message, ToolCall};

const MAX_TOKENS: &str = "MAX_TOKENS";

#[derive(Debug, Clone, Serialize)]
pub struct GenerationConfig {
    #[serde(rename = "thinkingConfig")]
    pub thinking_config: ThinkingConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThinkingConfig {
    #[serde(rename = "thinkingBudget")]
    pub thinking_budget: u32,
}

#[derive(Serialize)]
pub struct GeminiRequest {
    #[serde(rename = "generationConfig", skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GenerationConfig>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiInstruction>,
    pub contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<GeminiTool>,
}

#[derive(Serialize)]
pub struct GeminiTool {
    #[serde(rename = "functionDeclarations")]
    pub function_declarations: Vec<GeminiFunctionDecl>,
}

#[derive(Serialize)]
pub struct GeminiFunctionDecl {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Serialize)]
pub struct GeminiInstruction {
    pub parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
pub struct GeminiContent {
    pub role: &'static str,
    pub parts: Vec<GeminiPart>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum GeminiPart {
    Text {
        text: String,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: GeminiFunctionCall,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: GeminiFunctionResponse,
    },
}

#[derive(Serialize, Deserialize)]
pub struct GeminiFunctionCall {
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
pub struct GeminiFunctionResponse {
    pub name: String,
    pub response: serde_json::Value,
}

impl From<&LlmRequest> for GeminiRequest {
    fn from(request: &LlmRequest) -> Self {
        let mut system = request.system.clone().unwrap_or_default();
        let mut contents = Vec::with_capacity(request.messages.len());
        for item in &request.messages {
            match item {
                Message::User { text } => contents.push(GeminiContent {
                    role: "user",
                    parts: vec![GeminiPart::Text { text: text.clone() }],
                }),
                Message::Assistant {
                    text, tool_calls, ..
                } => {
                    let mut parts = Vec::new();
                    if !text.is_empty() {
                        parts.push(GeminiPart::Text { text: text.clone() });
                    }
                    for call in tool_calls {
                        let args = serde_json::from_str(&call.arguments)
                            .unwrap_or(serde_json::Value::Null);
                        parts.push(GeminiPart::FunctionCall {
                            function_call: GeminiFunctionCall {
                                name: call.name.clone(),
                                args,
                            },
                        });
                    }
                    contents.push(GeminiContent {
                        role: "model",
                        parts,
                    });
                }
                Message::Tool { name, content, .. } => contents.push(GeminiContent {
                    role: "function",
                    parts: vec![GeminiPart::FunctionResponse {
                        function_response: GeminiFunctionResponse {
                            name: name.clone(),
                            response: serde_json::json!({ "result": content }),
                        },
                    }],
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
        let system_instruction = if system.is_empty() {
            None
        } else {
            Some(GeminiInstruction {
                parts: vec![GeminiPart::Text { text: system }],
            })
        };
        let tools = request
            .tools
            .iter()
            .map(|tool| GeminiTool {
                function_declarations: vec![GeminiFunctionDecl {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    parameters: tool.parameters.clone(),
                }],
            })
            .collect();
        let (_, budget) = crate::llm::fit_thinking(request.effort, request.max_output);
        Self {
            generation_config: (budget > 0).then_some(GenerationConfig {
                thinking_config: ThinkingConfig {
                    thinking_budget: budget,
                },
            }),
            system_instruction,
            contents,
            tools,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiResponse {
    #[serde(default)]
    pub candidates: Vec<GeminiCandidate>,
    #[serde(default)]
    pub usage_metadata: Option<GeminiUsage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCandidate {
    #[serde(default)]
    pub content: Option<GeminiResponseContent>,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct GeminiUsage {
    #[serde(default, rename = "promptTokenCount")]
    pub prompt: u64,
    #[serde(default, rename = "candidatesTokenCount")]
    pub candidates: u64,
    #[serde(default, rename = "cachedContentTokenCount")]
    pub cached: u64,
}

impl From<GeminiUsage> for crate::llm::Usage {
    fn from(usage: GeminiUsage) -> Self {
        Self {
            input: usage.prompt.saturating_sub(usage.cached),
            output: usage.candidates,
            cache_read: usage.cached,
            cache_write: 0,
        }
    }
}

#[derive(Deserialize)]
pub struct GeminiResponseContent {
    pub parts: Vec<GeminiPart>,
}

impl GeminiResponse {
    pub fn finished(&self) -> bool {
        self.candidates
            .iter()
            .any(|candidate| candidate.finish_reason.is_some())
    }

    pub fn truncated(&self) -> bool {
        self.candidates
            .iter()
            .any(|candidate| candidate.finish_reason.as_deref() == Some(MAX_TOKENS))
    }

    fn parts(&self) -> impl Iterator<Item = &GeminiPart> {
        self.candidates
            .iter()
            .filter_map(|candidate| candidate.content.as_ref())
            .flat_map(|content| content.parts.iter())
    }

    pub fn text(&self) -> String {
        self.parts()
            .filter_map(|part| match part {
                GeminiPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    pub fn tool_calls(&self) -> Vec<ToolCall> {
        self.parts()
            .filter_map(|part| match part {
                GeminiPart::FunctionCall { function_call } => Some(ToolCall {
                    id: function_call.name.clone(),
                    name: function_call.name.clone(),
                    arguments: function_call.args.to_string(),
                }),
                _ => None,
            })
            .collect()
    }
}

#[derive(Default)]
pub struct GeminiToolAcc {
    acc: ToolAcc,
}

impl GeminiToolAcc {
    pub fn apply(&mut self, response: &GeminiResponse) {
        let Some(content) = response
            .candidates
            .first()
            .and_then(|candidate| candidate.content.as_ref())
        else {
            return;
        };
        for (index, part) in content.parts.iter().enumerate() {
            if let GeminiPart::FunctionCall { function_call } = part {
                let entry = self.acc.entry(index);
                entry.id.clone_from(&function_call.name);
                entry.name.clone_from(&function_call.name);
                entry.arguments = function_call.args.to_string();
            }
        }
    }

    pub fn finish(self) -> Vec<ToolCall> {
        self.acc.finish()
    }
}
