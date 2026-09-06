use std::io::{BufRead, BufReader};

use crate::llm::{Llm, LlmConfig, LlmError, LlmRequest, status_error};

use super::transformer::{AnthropicRequest, AnthropicResponse, AnthropicStreamEvent};

pub struct Anthropic {
    pub base_url: String,
    pub api_key: Option<String>,
}

impl Anthropic {
    pub fn new(config: &LlmConfig) -> Self {
        let api_key = config.api_key.clone().or_else(|| {
            std::env::var("ANTHROPIC_API_KEY")
                .ok()
                .filter(|key| !key.is_empty())
        });
        Self {
            base_url: config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.anthropic.com/v1".into()),
            api_key,
        }
    }

    fn post(
        &self,
        client: &reqwest::blocking::Client,
        request: &AnthropicRequest,
    ) -> Result<reqwest::blocking::Response, LlmError> {
        let url = format!("{}/messages", self.base_url);
        let mut builder = client.post(&url).header("anthropic-version", "2023-06-01");
        if let Some(key) = &self.api_key {
            builder = builder.header("x-api-key", key);
        }
        builder
            .json(request)
            .send()
            .map_err(|err| LlmError::Http(err.to_string()))
    }
}

impl Llm for Anthropic {
    fn id(&self) -> &'static str {
        "anthropic"
    }

    fn send_request(
        &self,
        client: &reqwest::blocking::Client,
        request: &LlmRequest,
    ) -> Result<String, LlmError> {
        let provider_request = AnthropicRequest::from(request);
        let response = self.post(client, &provider_request)?;
        if !response.status().is_success() {
            return Err(status_error(response));
        }
        let body = response
            .text()
            .map_err(|err| LlmError::Http(err.to_string()))?;
        let parsed: AnthropicResponse =
            serde_json::from_str(&body).map_err(|err| LlmError::Provider(err.to_string()))?;
        let text = parsed.text();
        if text.is_empty() {
            return Err(LlmError::Provider("empty response".into()));
        }
        Ok(text)
    }

    fn stream(
        &self,
        client: &reqwest::blocking::Client,
        request: &LlmRequest,
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<String, LlmError> {
        let mut provider_request = AnthropicRequest::from(request);
        provider_request.stream = true;
        let response = self.post(client, &provider_request)?;
        if !response.status().is_success() {
            return Err(status_error(response));
        }

        let mut full = String::new();
        let reader = BufReader::new(response);
        for line in reader.lines() {
            let line = line.map_err(|err| LlmError::Http(err.to_string()))?;
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            let Ok(event) = serde_json::from_str::<AnthropicStreamEvent>(data) else {
                continue;
            };
            if let Some(delta) = event.delta_text() {
                on_delta(delta);
                full.push_str(delta);
            }
        }
        Ok(full)
    }
}
