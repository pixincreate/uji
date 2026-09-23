pub mod bridge;
pub mod cancel;
pub mod catalog;
pub mod context;
pub mod discover;
pub mod error;
pub mod request;
pub mod summary;
pub mod title;
pub mod tuning;
pub mod wire;

pub use bridge::{Answer, Bridge, Call, Dispatch, Failure};
pub use cancel::CancelToken;
pub use catalog::{
    Budget, Catalog, Incomplete, LlmConfig, MAX_RESERVE, Model, OAuthSession, Origin, Patch,
    Provider, Selection, authenticated, resolve_from_storage,
};
pub use error::{HttpError, LlmError};
pub use request::{LlmRequest, LlmResponse, ToolSpec, Usage};
pub use tuning::{DEFAULT_MAX_OUTPUT, Effort, Retention};
pub use wire::{STREAM_IDLE, http_client};

pub enum Llm {
    Wired(Box<Bridge>),
    NotConfigured,
}

impl Llm {
    pub async fn route(&self, client: &reqwest::Client) -> Result<serde_json::Value, LlmError> {
        match self {
            Self::Wired(bridge) => bridge.route(client).await,
            Self::NotConfigured => Err(LlmError::not_configured()),
        }
    }

    pub async fn call(
        &self,
        client: &reqwest::Client,
        request: &LlmRequest<'_>,
    ) -> Result<LlmResponse, LlmError> {
        match self {
            Self::Wired(bridge) => bridge.call(client, request).await,
            Self::NotConfigured => Err(LlmError::not_configured()),
        }
    }
}
