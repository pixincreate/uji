use async_trait::async_trait;

use crate::llm::providers::stream::{Api, Parts, stream};
use crate::session::model::ToolCall;

use crate::llm::{Compat, Delta, LlmConfig, LlmError, LlmRequest, LlmResponse, Protocol, send};

use super::transformer::{Chunk, Request};

pub struct OpenAi {
    pub base_url: String,
    pub api_key: Option<String>,
    pub compat: Compat,
}

impl OpenAi {
    pub fn new(config: &LlmConfig) -> Self {
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".into());
        Self {
            api_key: config.resolve_key(),
            compat: Compat::resolve(&base_url, config.compat),
            base_url,
        }
    }

    async fn post(
        &self,
        client: &reqwest::Client,
        request: &Request<'_>,
    ) -> Result<reqwest::Response, LlmError> {
        let url = format!("{}/chat/completions", self.base_url);
        let mut builder = client.post(&url);
        if let Some(key) = &self.api_key {
            builder = builder.bearer_auth(key);
        }
        send(builder, request).await
    }
}

fn truncated(finish_reason: Option<&str>) -> Result<(), LlmError> {
    if finish_reason == Some("length") {
        return Err(LlmError::output_limit());
    }
    Ok(())
}

#[async_trait]
impl Api for OpenAi {
    type Event = Chunk;

    async fn send(
        &self,
        client: &reqwest::Client,
        request: &LlmRequest<'_>,
    ) -> Result<reqwest::Response, LlmError> {
        self.post(client, &Request::build(request, self.compat))
            .await
    }

    fn read(&self, event: Chunk, parts: &mut Parts<'_>) {
        if let Some(delta) = event.delta_reasoning() {
            parts.push_reasoning(delta);
        }
        if let Some(delta) = event.delta_text() {
            parts.push_text(delta);
        }
        if let Some(reason) = event.finish_reason() {
            parts.finish(Some(reason));
        }
        if let Some(reported) = event.usage {
            parts.usage = reported.into();
        }
        event.accumulate(&mut parts.acc);
    }

    fn finished(&self, parts: &Parts<'_>) -> Result<(), LlmError> {
        if parts.complete || !self.compat.finish_reason {
            Ok(())
        } else {
            Err(LlmError::truncated_stream())
        }
    }

    fn settle(&self, parts: &Parts<'_>, tool_calls: &[ToolCall]) -> Result<(), LlmError> {
        if !tool_calls.is_empty() {
            return Ok(());
        }
        truncated(parts.finish_reason.as_deref())?;
        if parts.text.is_empty() {
            return Err(LlmError::empty_response());
        }
        Ok(())
    }
}

#[async_trait]
impl Protocol for OpenAi {
    async fn call(
        &self,
        client: &reqwest::Client,
        request: &LlmRequest<'_>,
        on_delta: &mut (dyn FnMut(Delta) + Send),
    ) -> Result<LlmResponse, LlmError> {
        stream(self, client, request, on_delta).await
    }
}
