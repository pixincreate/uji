pub mod providers;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;

use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use crate::credential;
use crate::session::model::Message;
use crate::session::store::SessionStorage;

pub use providers::anthropic::Anthropic;
pub use providers::google::Gemini;
pub use providers::not_configured::NotConfigured;
pub use providers::ollama::Ollama;
pub use providers::openai::OpenAi;

pub enum Auth {
    None,
    Bearer { token: String },
    ApiKey { header: String, key: String },
}

impl Auth {
    pub(crate) fn apply(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self {
            Auth::None => builder,
            Auth::Bearer { token } => builder.bearer_auth(token),
            Auth::ApiKey { header, key } => builder.header(header.as_str(), key.as_str()),
        }
    }
}

pub struct LlmRequest {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<Message>,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("http: {0}")]
    Http(String),
    #[error("auth")]
    Auth,
    #[error("provider: {0}")]
    Provider(String),
}

pub(crate) async fn status_error(response: reqwest::Response) -> LlmError {
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    if status == 401 || status == 403 {
        LlmError::Auth
    } else {
        LlmError::Http(format!("{status}: {body}"))
    }
}

#[async_trait]
pub trait Llm: Send + Sync {
    fn id(&self) -> &'static str;
    async fn send_request(
        &self,
        client: &reqwest::Client,
        request: &LlmRequest,
    ) -> Result<String, LlmError>;
    async fn stream(
        &self,
        client: &reqwest::Client,
        request: &LlmRequest,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<String, LlmError> {
        let text = self.send_request(client, request).await?;
        on_delta(text.clone());
        Ok(text)
    }
}

#[derive(Default)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wire {
    #[serde(rename = "openai-chat")]
    OpenAiChat,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "ollama")]
    OllamaNative,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub wire: Wire,
    pub base_url: String,
    pub auth_env: Option<String>,
}

impl Provider {
    pub fn default_model(&self) -> &'static str {
        models(&self.id).first().copied().unwrap_or("")
    }
}

static PROVIDERS: LazyLock<Vec<Provider>> =
    LazyLock::new(|| serde_json::from_str(include_str!("providers.json")).unwrap_or_default());

pub fn providers() -> &'static [Provider] {
    PROVIDERS.as_slice()
}

pub fn provider(id: &str) -> Option<&'static Provider> {
    PROVIDERS.iter().find(|p| p.id == id)
}

pub fn provider_by_name(name: &str) -> Option<&'static Provider> {
    PROVIDERS.iter().find(|p| p.name == name)
}

static MODELS: LazyLock<HashMap<&'static str, Vec<&'static str>>> =
    LazyLock::new(|| serde_json::from_str(include_str!("models.json")).unwrap_or_default());

pub fn models(id: &str) -> &'static [&'static str] {
    MODELS.get(id).map_or(&[], Vec::as_slice)
}

pub fn resolve(config: &LlmConfig) -> Arc<dyn Llm> {
    if config.provider.is_empty() {
        return Arc::new(NotConfigured);
    }
    let Some(provider) = provider(&config.provider) else {
        return Arc::new(OpenAi::new(config));
    };
    if provider.id == "custom" {
        return Arc::new(OpenAi::new(config));
    }
    let resolved = LlmConfig {
        provider: config.provider.clone(),
        model: config.model.clone(),
        base_url: Some(provider.base_url.clone()),
        api_key: config.api_key.clone(),
    };
    match provider.wire {
        Wire::OpenAiChat => Arc::new(OpenAi::new(&resolved)),
        Wire::Anthropic => Arc::new(Anthropic::new(&resolved)),
        Wire::Gemini => Arc::new(Gemini::new(&resolved)),
        Wire::OllamaNative => Arc::new(Ollama::new(&resolved)),
    }
}

pub enum StreamEvent {
    Delta(String),
    Done(String),
    Failed(String),
}

pub async fn stream_turn(
    client: &reqwest::Client,
    provider: &dyn Llm,
    model: String,
    messages: Vec<Message>,
    on_event: &mut (dyn FnMut(StreamEvent) + Send),
) {
    let request = LlmRequest {
        model,
        system: None,
        messages,
    };
    let result = provider
        .stream(client, &request, &mut |delta| {
            on_event(StreamEvent::Delta(delta));
        })
        .await;
    let event = match result {
        Ok(text) => StreamEvent::Done(text),
        Err(err) => StreamEvent::Failed(err.to_string()),
    };
    on_event(event);
}

pub(crate) async fn response_lines(
    response: reqwest::Response,
    mut on_line: impl FnMut(&str) + Send,
) -> Result<(), LlmError> {
    let mut buf = String::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|err| LlmError::Http(err.to_string()))?;
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = buf.find('\n') {
            let line = buf[..pos].trim_end_matches('\r').to_string();
            buf.drain(..=pos);
            on_line(&line);
        }
    }
    Ok(())
}

pub fn resolve_from_storage(storage: &mut dyn SessionStorage) -> (Arc<dyn Llm>, String, String) {
    let provider_id = storage
        .get_setting("llm.provider")
        .ok()
        .flatten()
        .unwrap_or_default();
    let model = storage
        .get_setting("llm.model")
        .ok()
        .flatten()
        .unwrap_or_else(|| {
            provider(&provider_id).map_or_else(String::new, |p| p.default_model().to_string())
        });
    let base_url = storage.get_setting("llm.base_url").ok().flatten();
    let api_key = credential::get(&provider_id);
    let config = LlmConfig {
        provider: provider_id.clone(),
        model: model.clone(),
        base_url,
        api_key,
    };
    (resolve(&config), model, provider_id)
}
