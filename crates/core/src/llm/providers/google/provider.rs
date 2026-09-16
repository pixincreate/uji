use async_trait::async_trait;

use crate::llm::{
    LlmConfig, LlmError, LlmRequest, LlmResponse, Protocol, Usage, response_lines, send,
    status_error,
};

use super::transformer::{GeminiRequest, GeminiResponse, GeminiToolAcc};

pub struct Gemini {
    pub base_url: String,
    pub api_key: Option<String>,
}

impl Gemini {
    pub fn new(config: &LlmConfig) -> Self {
        let api_key = config.resolve_key();
        Self {
            base_url: config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta".into()),
            api_key,
        }
    }

    async fn post(
        &self,
        client: &reqwest::Client,
        model: &str,
        request: &GeminiRequest,
    ) -> Result<reqwest::Response, LlmError> {
        let url = format!(
            "{}/models/{model}:streamGenerateContent?alt=sse",
            self.base_url
        );
        let mut builder = client.post(&url);
        if let Some(key) = &self.api_key {
            builder = builder.header("x-goog-api-key", key);
        }
        send(builder, request).await
    }
}

#[async_trait]
impl Protocol for Gemini {
    async fn call(
        &self,
        client: &reqwest::Client,
        request: &LlmRequest,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<LlmResponse, LlmError> {
        let provider_request = GeminiRequest::from(request);
        let response = self.post(client, &request.model, &provider_request).await?;
        if !response.status().is_success() {
            return Err(status_error(response).await);
        }

        let mut full = String::new();
        let mut acc = GeminiToolAcc::default();
        let mut usage = Usage::default();
        let mut hit_limit = false;
        let mut complete = false;
        response_lines(response, |line| {
            let Some(data) = line.strip_prefix("data: ") else {
                return;
            };
            if let Ok(parsed) = serde_json::from_str::<GeminiResponse>(data) {
                let text = parsed.text();
                if !text.is_empty() {
                    on_delta(text.clone());
                    full.push_str(&text);
                }
                if let Some(reported) = parsed.usage_metadata {
                    usage = reported.into();
                }
                hit_limit |= parsed.truncated();
                complete |= parsed.finished();
                acc.apply(&parsed);
            }
        })
        .await?;
        if !complete {
            return Err(LlmError::truncated_stream());
        }
        let tool_calls = acc.finish();
        if tool_calls.is_empty() && hit_limit {
            return Err(LlmError::output_limit());
        }
        Ok(LlmResponse {
            text: full,
            tool_calls,
            reasoning_content: None,
            usage: (usage.total() > 0).then_some(usage),
        })
    }
}
