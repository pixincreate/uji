use std::time::Duration;

use tokio::sync::oneshot;

use crate::session::model::ToolCall;

use super::request::Usage;

pub enum ToolDecision {
    Allow { arguments: String },
    Deny { reason: String },
}

pub enum StreamEvent {
    Delta(String),
    SteerRequest {
        reply: oneshot::Sender<Option<String>>,
    },
    Compacted {
        summary: String,
        usage: Option<Usage>,
    },
    Restarted {
        attempt: u32,
        of: u32,
        wait: Duration,
    },
    AssistantStep {
        text: String,
        tool_calls: Vec<ToolCall>,
        reasoning_content: Option<String>,
    },
    ToolResult {
        tool_call_id: String,
        name: String,
        content: String,
    },
    ToolDecisionRequest {
        tool: ToolCall,
        subject: String,
        reply: oneshot::Sender<ToolDecision>,
    },
    RunLuaTool {
        name: String,
        arguments: String,
        reply: oneshot::Sender<String>,
    },
    Done {
        text: String,
        reasoning_content: Option<String>,
    },
    Usage(Usage),
    Cancelled,
    Failed(String),
}
