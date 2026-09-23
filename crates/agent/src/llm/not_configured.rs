use async_trait::async_trait;

use crate::llm::{Delta, LlmError, LlmRequest, LlmResponse, Protocol};

pub struct NotConfigured;

impl NotConfigured {
    pub fn error() -> LlmError {
        LlmError::Provider(String::from(
            "no provider is configured - run /login to set one up",
        ))
    }
}

#[async_trait]
impl Protocol for NotConfigured {
    async fn call(
        &self,
        _client: &reqwest::Client,
        _request: &LlmRequest<'_>,
        _on_delta: &mut (dyn FnMut(Delta) + Send),
    ) -> Result<LlmResponse, LlmError> {
        Err(Self::error())
    }
}
