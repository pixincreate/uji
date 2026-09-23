pub mod bridge;
pub mod cancel;
pub mod catalog;
pub mod context;
pub mod discover;
pub mod error;
pub mod event;
mod not_configured;
pub mod request;
pub mod summary;
pub mod title;
pub mod tuning;
pub mod wire;

use async_trait::async_trait;

pub use bridge::{Answer, Bridge, Call, Dispatch, Failure};
pub use cancel::CancelToken;
pub use catalog::{
    Budget, Catalog, LlmConfig, MAX_RESERVE, Model, OAuthSession, Origin, Provider, Selection,
    authenticated, resolve, resolve_from_storage,
};
pub use error::{HttpError, LlmError};
pub use event::Delta;
pub use not_configured::NotConfigured;
pub use request::{LlmRequest, LlmResponse, ToolSpec, Usage};
pub use tuning::{DEFAULT_MAX_OUTPUT, Effort, Retention};
pub use wire::{STREAM_IDLE, http_client};

/// A provider's wire protocol. One call, one response; text arrives through
/// `on_delta` as it streams. Callers that do not care about deltas pass
/// [`Protocol::silent`].
#[async_trait]
pub trait Protocol: Send + Sync {
    async fn call(
        &self,
        client: &reqwest::Client,
        request: &LlmRequest<'_>,
        on_delta: &mut (dyn FnMut(Delta) + Send),
    ) -> Result<LlmResponse, LlmError>;
}

/// A delta sink for callers that only want the final response.
pub fn silent() -> impl FnMut(Delta) + Send {
    |_| {}
}

pub enum Llm {
    Wired(Box<Bridge>),
    NotConfigured(NotConfigured),
}

impl Llm {
    pub async fn route(&self, client: &reqwest::Client) -> Result<serde_json::Value, LlmError> {
        match self {
            Self::Wired(bridge) => bridge.route(client).await,
            Self::NotConfigured(_) => Err(NotConfigured::error()),
        }
    }
}

#[async_trait]
impl Protocol for Llm {
    async fn call(
        &self,
        client: &reqwest::Client,
        request: &LlmRequest<'_>,
        on_delta: &mut (dyn FnMut(Delta) + Send),
    ) -> Result<LlmResponse, LlmError> {
        match self {
            Self::Wired(llm) => llm.call(client, request, on_delta).await,
            Self::NotConfigured(llm) => llm.call(client, request, on_delta).await,
        }
    }
}
