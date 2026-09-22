use async_trait::async_trait;
use serde::de::DeserializeOwned;

use crate::llm::providers::acc::ToolAcc;
use crate::llm::request::{LlmRequest, LlmResponse, Usage};
use crate::llm::{Delta, LlmError, Progress, response_lines, status_error};
use crate::session::model::ToolCall;

const DATA: &str = "data: ";
const DONE: &str = "[DONE]";

pub struct Parts<'a> {
    pub text: String,
    pub reasoning: String,
    pub usage: Usage,
    pub finish_reason: Option<String>,
    pub hit_limit: bool,
    pub complete: bool,
    pub acc: ToolAcc,
    sink: &'a mut (dyn FnMut(Delta) + Send),
    moved: bool,
}

impl<'a> Parts<'a> {
    fn new(sink: &'a mut (dyn FnMut(Delta) + Send)) -> Self {
        Self {
            text: String::new(),
            reasoning: String::new(),
            usage: Usage::default(),
            finish_reason: None,
            hit_limit: false,
            complete: false,
            acc: ToolAcc::default(),
            sink,
            moved: false,
        }
    }

    pub fn push_text(&mut self, delta: &str) {
        self.text.push_str(delta);
        (self.sink)(Delta::Text(delta.to_string()));
        self.moved = true;
    }

    pub fn push_reasoning(&mut self, delta: &str) {
        self.reasoning.push_str(delta);
        (self.sink)(Delta::Reasoning(delta.to_string()));
        self.moved = true;
    }

    pub fn finish(&mut self, reason: Option<&str>) {
        if let Some(reason) = reason {
            self.finish_reason = Some(reason.to_string());
        }
        self.complete = true;
        self.moved = true;
    }

    fn progress(&mut self) -> Progress {
        let moved = std::mem::take(&mut self.moved) | self.acc.take_touched();
        Progress::from(moved)
    }
}

#[async_trait]
pub trait Api: Send + Sync {
    type Event: DeserializeOwned;

    async fn send(
        &self,
        client: &reqwest::Client,
        request: &LlmRequest<'_>,
    ) -> Result<reqwest::Response, LlmError>;

    fn read(&self, event: Self::Event, parts: &mut Parts<'_>);

    fn finished(&self, parts: &Parts<'_>) -> Result<(), LlmError> {
        if parts.complete {
            Ok(())
        } else {
            Err(LlmError::truncated_stream())
        }
    }

    fn settle(&self, parts: &Parts<'_>, tool_calls: &[ToolCall]) -> Result<(), LlmError> {
        if tool_calls.is_empty() && parts.hit_limit {
            return Err(LlmError::output_limit());
        }
        Ok(())
    }
}

pub async fn stream<A: Api>(
    api: &A,
    client: &reqwest::Client,
    request: &LlmRequest<'_>,
    on_delta: &mut (dyn FnMut(Delta) + Send),
) -> Result<LlmResponse, LlmError> {
    let response = api.send(client, request).await?;
    if !response.status().is_success() {
        return Err(status_error(response).await);
    }
    let mut parts = Parts::new(on_delta);
    response_lines(response, |line| {
        let Some(data) = line.strip_prefix(DATA) else {
            return Progress::Keepalive;
        };
        if data == DONE {
            parts.finish(None);
        } else if let Ok(event) = serde_json::from_str::<A::Event>(data) {
            api.read(event, &mut parts);
        }
        parts.progress()
    })
    .await?;
    api.finished(&parts)?;
    let tool_calls = std::mem::take(&mut parts.acc).finish()?;
    api.settle(&parts, &tool_calls)?;
    Ok(LlmResponse {
        text: parts.text,
        tool_calls,
        reasoning_content: (!parts.reasoning.is_empty()).then_some(parts.reasoning),
        usage: (parts.usage.total() > 0).then_some(parts.usage),
    })
}
