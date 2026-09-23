use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{Mutex, oneshot};

use crate::auth::{self, Tokens};
use crate::credential::{self, Credential};
use crate::session::model::{Message, ToolCall};

use super::catalog::{LlmConfig, OAuthSession};
use super::error::{HttpError, LlmError};
use super::request::{LlmRequest, LlmResponse, ToolSpec, Usage};

pub type Dispatch = Arc<dyn Fn(Call) + Send + Sync>;

pub struct Call {
    pub wire: String,
    pub request: Value,
    pub reply: oneshot::Sender<Result<LlmResponse, LlmError>>,
}

#[derive(Serialize)]
struct Payload<'a> {
    model: &'a str,
    system: Option<&'a str>,
    messages: &'a [Message],
    tools: &'a [ToolSpec],
    effort: &'static str,
    max_output: u32,
    cache: &'static str,
    provider: Endpoint<'a>,
    auth: Auth,
}

#[derive(Serialize)]
struct Route<'a> {
    wire: &'a str,
    provider: Endpoint<'a>,
    auth: Auth,
}

#[derive(Serialize)]
struct Endpoint<'a> {
    id: &'a str,
    base_url: &'a str,
    compat: &'a Value,
}

#[derive(Serialize)]
struct Auth {
    key: Option<String>,
    oauth: Option<OAuth>,
}

#[derive(Serialize)]
struct OAuth {
    token: String,
    headers: BTreeMap<String, String>,
    identity_prompt: Option<String>,
}

#[derive(Deserialize)]
pub struct Answer {
    #[serde(default)]
    text: String,
    reasoning: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ToolCall>,
    usage: Option<Usage>,
}

impl From<Answer> for LlmResponse {
    fn from(answer: Answer) -> Self {
        Self {
            text: answer.text,
            tool_calls: answer.tool_calls,
            reasoning: answer.reasoning,
            usage: answer.usage,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Failure {
    Http {
        status: Option<u16>,
        message: String,
    },
    Auth {
        status: u16,
    },
    Provider {
        message: String,
    },
}

impl From<Failure> for LlmError {
    fn from(failure: Failure) -> Self {
        match failure {
            Failure::Http { status, message } => Self::Http(HttpError {
                status,
                body: message,
            }),
            Failure::Auth { status } => Self::Auth(status),
            Failure::Provider { message } => Self::Provider(message),
        }
    }
}

pub struct Bridge {
    wire: String,
    provider: String,
    base_url: String,
    compat: Value,
    key: Option<String>,
    oauth: Option<OAuthSession>,
    tokens: Mutex<Option<Tokens>>,
    dispatch: Dispatch,
}

impl Bridge {
    pub fn new(wire: String, config: &LlmConfig, dispatch: Dispatch) -> Self {
        Self {
            wire,
            provider: config.provider.clone(),
            base_url: config.base_url.clone().unwrap_or_default(),
            compat: config.compat.clone(),
            key: config.resolve_key(),
            tokens: Mutex::new(config.oauth.as_ref().map(|session| session.tokens.clone())),
            oauth: config.oauth.clone(),
            dispatch,
        }
    }

    async fn auth(&self, client: &reqwest::Client) -> Result<Auth, LlmError> {
        let oauth = match &self.oauth {
            Some(session) => Some(OAuth {
                token: self.bearer(client, session).await?,
                headers: session.config.request_headers.clone(),
                identity_prompt: session.config.identity_prompt.clone(),
            }),
            None => None,
        };
        Ok(Auth {
            key: self.key.clone(),
            oauth,
        })
    }

    async fn bearer(
        &self,
        client: &reqwest::Client,
        session: &OAuthSession,
    ) -> Result<String, LlmError> {
        let mut guard = self.tokens.lock().await;
        let current = guard
            .clone()
            .ok_or_else(|| LlmError::Provider("no oauth tokens".into()))?;
        if !current.is_expired(auth::flow::now()) {
            return Ok(current.access);
        }
        let refreshed = auth::refresh(client, &session.config, &current.refresh)
            .await
            .map_err(|err| LlmError::Provider(err.to_string()))?;
        if let Err(err) =
            credential::store(&session.provider_id, &Credential::from_tokens(&refreshed))
        {
            return Err(LlmError::Provider(format!(
                "refreshed the session but could not save it: {err}"
            )));
        }
        let access = refreshed.access.clone();
        *guard = Some(refreshed);
        Ok(access)
    }

    pub async fn route(&self, client: &reqwest::Client) -> Result<Value, LlmError> {
        let auth = self.auth(client).await?;
        encode(&Route {
            wire: &self.wire,
            provider: self.endpoint(),
            auth,
        })
    }

    fn endpoint(&self) -> Endpoint<'_> {
        Endpoint {
            id: &self.provider,
            base_url: &self.base_url,
            compat: &self.compat,
        }
    }

    pub async fn call(
        &self,
        client: &reqwest::Client,
        request: &LlmRequest<'_>,
    ) -> Result<LlmResponse, LlmError> {
        let auth = self.auth(client).await?;
        let request = self.payload(request, auth)?;
        let (reply, answer) = oneshot::channel();
        (self.dispatch)(Call {
            wire: self.wire.clone(),
            request,
            reply,
        });
        answer.await.unwrap_or_else(|_| {
            Err(LlmError::Provider(format!(
                "the {} wire stopped without replying",
                self.wire
            )))
        })
    }

    fn payload(&self, request: &LlmRequest<'_>, auth: Auth) -> Result<Value, LlmError> {
        encode(&Payload {
            model: request.model,
            system: request.system,
            messages: request.messages,
            tools: request.tools,
            effort: request.effort.name(),
            max_output: request.max_output,
            cache: request.cache.name(),
            provider: self.endpoint(),
            auth,
        })
    }
}

fn encode(value: &impl Serialize) -> Result<Value, LlmError> {
    serde_json::to_value(value)
        .map_err(|err| LlmError::Provider(format!("could not encode the request: {err}")))
}
