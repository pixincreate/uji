use std::collections::BTreeMap;
use std::time::Duration;

use serde::Deserialize;

use super::AuthError;
use super::callback::Callback;
use super::pkce::{Pkce, random_token};

const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);
const EXPIRY_SKEW: i64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenBody {
    Json,
    Form,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateSource {
    #[default]
    Random,
    Verifier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Subject {
    #[default]
    IdToken,
    AccessToken,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Exchange {
    pub grant_type: String,
    pub requested_token: String,
    pub subject_token_type: String,
    #[serde(default)]
    pub subject: Subject,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OAuthConfig {
    pub client_id: String,
    pub authorize_url: String,
    pub token_url: String,
    pub scopes: String,
    pub redirect_port: u16,
    pub redirect_path: String,
    pub token_body: TokenBody,
    #[serde(default)]
    pub state: StateSource,
    #[serde(default)]
    pub authorize_params: BTreeMap<String, String>,
    #[serde(default)]
    pub exchange: Option<Exchange>,
    #[serde(default)]
    pub request_headers: BTreeMap<String, String>,
    #[serde(default)]
    pub identity_prompt: Option<String>,
}

impl OAuthConfig {
    fn redirect_uri(&self) -> String {
        format!(
            "http://localhost:{}{}",
            self.redirect_port, self.redirect_path
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tokens {
    pub access: String,
    pub refresh: String,
    pub expires_at: i64,
}

impl Tokens {
    pub fn is_expired(&self, now: i64) -> bool {
        self.expires_at != 0 && now >= self.expires_at
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Grant {
    Tokens(Tokens),
    ApiKey(String),
}

pub struct Pending {
    pub url: String,
    callback: Callback,
    verifier: String,
    state: String,
    redirect_uri: String,
}

pub fn start(config: &OAuthConfig) -> Result<Pending, AuthError> {
    let callback = Callback::bind(config.redirect_port, &config.redirect_path)?;
    let pkce = Pkce::generate();
    let state = match config.state {
        StateSource::Random => random_token(),
        StateSource::Verifier => pkce.verifier.clone(),
    };
    let redirect_uri = config.redirect_uri();

    let mut params: Vec<(&str, &str)> = vec![
        ("client_id", &config.client_id),
        ("response_type", "code"),
        ("redirect_uri", &redirect_uri),
        ("scope", &config.scopes),
        ("code_challenge", &pkce.challenge),
        ("code_challenge_method", "S256"),
        ("state", &state),
    ];
    params.extend(
        config
            .authorize_params
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str())),
    );

    Ok(Pending {
        url: format!("{}?{}", config.authorize_url, encode_pairs(&params)),
        callback,
        verifier: pkce.verifier,
        state,
        redirect_uri,
    })
}

pub async fn login(
    client: &reqwest::Client,
    config: &OAuthConfig,
    pending: Pending,
) -> Result<Grant, AuthError> {
    let params = tokio::task::block_in_place(|| pending.callback.wait(CALLBACK_TIMEOUT))?;
    if params.get("state").is_some_and(|got| *got != pending.state) {
        return Err(AuthError::StateMismatch);
    }
    let code = params.get("code").cloned().unwrap_or_default();

    let mut fields = vec![
        ("grant_type", "authorization_code"),
        ("client_id", config.client_id.as_str()),
        ("code", code.as_str()),
        ("redirect_uri", pending.redirect_uri.as_str()),
        ("code_verifier", pending.verifier.as_str()),
    ];
    if config.state == StateSource::Verifier {
        fields.push(("state", pending.state.as_str()));
    }

    let parsed = post_token(client, config, "token exchange", &fields).await?;
    match &config.exchange {
        Some(exchange) => Ok(Grant::ApiKey(
            trade(client, config, exchange, &parsed).await?,
        )),
        None => Ok(Grant::Tokens(parsed.into_tokens())),
    }
}

pub async fn refresh(
    client: &reqwest::Client,
    config: &OAuthConfig,
    refresh_token: &str,
) -> Result<Tokens, AuthError> {
    let fields = [
        ("grant_type", "refresh_token"),
        ("client_id", config.client_id.as_str()),
        ("refresh_token", refresh_token),
    ];
    let mut tokens = post_token(client, config, "token refresh", &fields)
        .await?
        .into_tokens();
    if tokens.refresh.is_empty() {
        tokens.refresh = refresh_token.to_string();
    }
    Ok(tokens)
}

async fn trade(
    client: &reqwest::Client,
    config: &OAuthConfig,
    exchange: &Exchange,
    parsed: &TokenResponse,
) -> Result<String, AuthError> {
    let subject = match exchange.subject {
        Subject::IdToken => parsed.id_token.as_deref().unwrap_or_default(),
        Subject::AccessToken => parsed.access_token.as_str(),
    };
    let fields = [
        ("grant_type", exchange.grant_type.as_str()),
        ("client_id", config.client_id.as_str()),
        ("requested_token", exchange.requested_token.as_str()),
        ("subject_token", subject),
        ("subject_token_type", exchange.subject_token_type.as_str()),
    ];
    Ok(post_token(client, config, "api key exchange", &fields)
        .await?
        .access_token)
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    expires_in: i64,
    #[serde(default)]
    id_token: Option<String>,
}

impl TokenResponse {
    fn into_tokens(self) -> Tokens {
        Tokens {
            expires_at: if self.expires_in > 0 {
                now() + self.expires_in - EXPIRY_SKEW
            } else {
                0
            },
            access: self.access_token,
            refresh: self.refresh_token,
        }
    }
}

async fn post_token(
    client: &reqwest::Client,
    config: &OAuthConfig,
    endpoint: &'static str,
    fields: &[(&str, &str)],
) -> Result<TokenResponse, AuthError> {
    let request = client.post(&config.token_url);
    let request = match config.token_body {
        TokenBody::Json => request.json(
            &fields
                .iter()
                .map(|(key, value)| ((*key).to_string(), serde_json::Value::from(*value)))
                .collect::<serde_json::Map<String, serde_json::Value>>(),
        ),
        TokenBody::Form => request
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(encode_pairs(fields)),
    };

    let response = request
        .send()
        .await
        .map_err(|err| AuthError::Http(err.to_string()))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AuthError::Endpoint {
            endpoint,
            status: status.as_u16(),
            body: clip(&body),
        });
    }
    serde_json::from_str(&body).map_err(|_| AuthError::Malformed {
        endpoint,
        body: clip(&body),
    })
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(0))
}

fn encode_pairs(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| {
            format!(
                "{}={}",
                urlencoding::encode(key),
                urlencoding::encode(value)
            )
        })
        .collect::<Vec<_>>()
        .join("&")
}

const MAX_ERROR_BODY_CHARS: usize = 500;

fn clip(body: &str) -> String {
    let trimmed = body.trim();
    match trimmed.char_indices().nth(MAX_ERROR_BODY_CHARS) {
        Some((index, _)) => format!("{}…", &trimmed[..index]),
        None => trimmed.to_string(),
    }
}
