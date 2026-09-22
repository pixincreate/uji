use async_trait::async_trait;

use crate::llm::providers::stream::{Api, Parts, stream};

use crate::llm::{Delta, LlmConfig, LlmError, LlmRequest, LlmResponse, Protocol, send};

use super::transformer::{Request, Response};

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
        request: &Request<'_>,
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
impl Api for Gemini {
    type Event = Response;

    async fn send(
        &self,
        client: &reqwest::Client,
        request: &LlmRequest<'_>,
    ) -> Result<reqwest::Response, LlmError> {
        let provider_request = Request::from(request);
        self.post(client, request.model, &provider_request).await
    }

    fn read(&self, event: Response, parts: &mut Parts<'_>) {
        let thoughts = event.thoughts();
        if !thoughts.is_empty() {
            parts.push_reasoning(&thoughts);
        }
        let text = event.text();
        if !text.is_empty() {
            parts.push_text(&text);
        }
        if let Some(reported) = event.usage_metadata {
            parts.usage = reported.into();
        }
        parts.hit_limit |= event.truncated();
        if event.finished() {
            parts.finish(None);
        }
        event.accumulate(&mut parts.acc);
    }
}

#[async_trait]
impl Protocol for Gemini {
    async fn call(
        &self,
        client: &reqwest::Client,
        request: &LlmRequest<'_>,
        on_delta: &mut (dyn FnMut(Delta) + Send),
    ) -> Result<LlmResponse, LlmError> {
        stream(self, client, request, on_delta).await
    }
}
