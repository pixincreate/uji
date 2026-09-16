use async_trait::async_trait;

use crate::llm::{LlmError, LlmRequest, LlmResponse, Protocol};

pub struct NotConfigured;

#[async_trait]
impl Protocol for NotConfigured {
    async fn call(
        &self,
        _client: &reqwest::Client,
        _request: &LlmRequest,
        _on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<LlmResponse, LlmError> {
        Ok(LlmResponse {
            text: "Please run /login to configure a provider".into(),
            tool_calls: Vec::new(),
            reasoning_content: None,
            usage: None,
        })
    }
}
