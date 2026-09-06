use crate::llm::{Llm, LlmError, LlmRequest};

pub struct NotConfigured;

impl Llm for NotConfigured {
    fn id(&self) -> &'static str {
        ""
    }

    fn send_request(
        &self,
        _client: &reqwest::blocking::Client,
        _request: &LlmRequest,
    ) -> Result<String, LlmError> {
        Ok("Please run /login to configure a provider".into())
    }
}
