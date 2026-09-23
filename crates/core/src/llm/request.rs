use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::session::model::{Message, ToolCall};

use super::tuning::{Effort, Retention};

pub struct LlmRequest<'a> {
    pub model: &'a str,
    pub system: Option<&'a str>,
    pub messages: &'a [Message],
    pub tools: &'a [ToolSpec],
    pub effort: Effort,
    pub max_output: u32,
    pub cache: Retention,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl Usage {
    pub fn prefix(&self) -> u64 {
        self.input
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write)
    }

    pub fn total(&self) -> u64 {
        self.prefix().saturating_add(self.output)
    }

    pub fn add(&mut self, other: Usage) {
        self.input = self.input.saturating_add(other.input);
        self.output = self.output.saturating_add(other.output);
        self.cache_read = self.cache_read.saturating_add(other.cache_read);
        self.cache_write = self.cache_write.saturating_add(other.cache_write);
    }
}

#[derive(Debug)]
pub struct LlmResponse {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub reasoning: Option<String>,
    pub usage: Option<Usage>,
}
