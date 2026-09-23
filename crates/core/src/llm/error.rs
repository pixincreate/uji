#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("http: {0}")]
    Http(HttpError),
    #[error("authentication rejected ({0}) - check the api key for this provider")]
    Auth(u16),
    #[error("provider: {0}")]
    Provider(String),
}

impl LlmError {
    pub fn not_configured() -> Self {
        Self::Provider(String::from(
            "no provider is configured - run /login to set one up",
        ))
    }
}

#[derive(Debug)]
pub struct HttpError {
    pub status: Option<u16>,
    pub body: String,
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.status {
            Some(status) => write!(f, "{status}: {}", self.body),
            None => f.write_str(&self.body),
        }
    }
}
