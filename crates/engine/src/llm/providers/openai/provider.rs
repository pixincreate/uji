use async_trait::async_trait;

use crate::llm::{
    LlmConfig, LlmError, LlmRequest, LlmResponse, Protocol, response_lines, send, status_error,
};

use super::transformer::{OpenAiChunk, OpenAiRequest, OpenAiToolAcc};

pub struct OpenAi {
    pub base_url: String,
    pub api_key: Option<String>,
}

impl OpenAi {
    pub fn new(config: &LlmConfig) -> Self {
        Self {
            base_url: config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".into()),
            api_key: config.resolve_key(),
        }
    }

    async fn post(
        &self,
        client: &reqwest::Client,
        request: &OpenAiRequest,
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
impl Protocol for OpenAi {
    async fn call(
        &self,
        client: &reqwest::Client,
        request: &LlmRequest,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<LlmResponse, LlmError> {
        let mut provider_request = OpenAiRequest::from(request);
        provider_request.stream = true;
        provider_request.stream_options = Some(super::transformer::StreamOptions {
            include_usage: true,
        });
        let response = self.post(client, &provider_request).await?;
        if !response.status().is_success() {
            return Err(status_error(response).await);
        }

        let mut full = String::new();
        let mut reasoning = String::new();
        let mut finish_reason = None;
        let mut complete = false;
        let mut usage = None;
        let mut acc = OpenAiToolAcc::default();
        response_lines(response, |line| {
            let Some(data) = line.strip_prefix("data: ") else {
                return;
            };
            if data == "[DONE]" {
                complete = true;
                return;
            }
            if let Ok(chunk) = serde_json::from_str::<OpenAiChunk>(data) {
                if let Some(delta) = chunk.delta_text() {
                    on_delta(delta.to_string());
                    full.push_str(delta);
                }
                if let Some(delta) = chunk.delta_reasoning() {
                    reasoning.push_str(delta);
                }
                if let Some(reason) = chunk.finish_reason() {
                    finish_reason = Some(reason.to_string());
                    complete = true;
                }
                if let Some(reported) = chunk.usage {
                    usage = Some(reported.into());
                }
                acc.apply(&chunk);
            }
        })
        .await?;
        if !complete {
            return Err(LlmError::truncated_stream());
        }
        let tool_calls = acc.finish();
        if tool_calls.is_empty() {
            truncated(finish_reason.as_deref())?;
            if full.is_empty() {
                return Err(LlmError::empty_response());
            }
        }
        Ok(LlmResponse {
            text: full,
            tool_calls,
            reasoning_content: (!reasoning.is_empty()).then_some(reasoning),
            usage,
        })
    }
}
