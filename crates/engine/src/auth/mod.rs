pub mod callback;
pub mod flow;
pub mod pkce;

pub use flow::{Grant, OAuthConfig, TokenBody, Tokens, login, refresh};

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("cannot listen on 127.0.0.1:{port} for the oauth callback: {reason}")]
    Callback { port: u16, reason: String },
    #[error("timed out waiting for the browser to complete sign-in")]
    TimedOut,
    #[error("oauth state mismatch; the sign-in was not completed in this session")]
    StateMismatch,
    #[error("http: {0}")]
    Http(String),
    #[error("{endpoint} returned {status}: {body}")]
    Endpoint {
        endpoint: &'static str,
        status: u16,
        body: String,
    },
    #[error("{endpoint} returned an unexpected body: {body}")]
    Malformed {
        endpoint: &'static str,
        body: String,
    },
    #[error("this provider does not support subscription sign-in")]
    Unsupported,
}
