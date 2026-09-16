use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::auth::{Grant, Tokens};

const SERVICE: &str = "uji";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Credential {
    #[serde(rename = "api_key")]
    ApiKey { key: String },
    #[serde(rename = "oauth")]
    OAuth {
        access: String,
        refresh: String,
        expires_at: i64,
    },
}

impl Credential {
    pub fn from_grant(grant: &Grant) -> Self {
        match grant {
            Grant::ApiKey(key) => Self::ApiKey { key: key.clone() },
            Grant::Tokens(tokens) => Self::from_tokens(tokens),
        }
    }

    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self::OAuth {
            access: tokens.access.clone(),
            refresh: tokens.refresh.clone(),
            expires_at: tokens.expires_at,
        }
    }

    pub fn api_key(&self) -> Option<&str> {
        match self {
            Self::ApiKey { key } => Some(key),
            Self::OAuth { .. } => None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn get(provider: &str) -> Option<String> {
    load(provider)
        .as_ref()
        .and_then(Credential::api_key)
        .map(ToString::to_string)
}

pub fn load(provider: &str) -> Option<Credential> {
    let raw =
        keyring_get(provider).or_else(|| read_file().and_then(|map| map.get(provider).cloned()))?;
    Some(parse(&raw))
}

pub fn set(provider: &str, key: &str) -> Result<(), CredentialError> {
    store(
        provider,
        &Credential::ApiKey {
            key: key.to_string(),
        },
    )
}

pub fn store(provider: &str, credential: &Credential) -> Result<(), CredentialError> {
    let raw = serde_json::to_string(credential)?;
    if keyring_set(provider, &raw).is_ok() {
        return Ok(());
    }
    let mut map = read_file().unwrap_or_default();
    map.insert(provider.to_string(), raw);
    write_file(&map)
}

fn parse(raw: &str) -> Credential {
    serde_json::from_str(raw).unwrap_or_else(|_| Credential::ApiKey {
        key: raw.to_string(),
    })
}

fn keyring_get(provider: &str) -> Option<String> {
    let entry = keyring::Entry::new(SERVICE, provider).ok()?;
    entry.get_password().ok()
}

fn keyring_set(provider: &str, key: &str) -> Result<(), keyring::Error> {
    let entry = keyring::Entry::new(SERVICE, provider)?;
    entry.set_password(key)
}

fn path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/share/uji/auth.json")
}

fn read_file() -> Option<HashMap<String, String>> {
    let text = std::fs::read_to_string(path()).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_file(map: &HashMap<String, String>) -> Result<(), CredentialError> {
    let path = path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(map)?;
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(text.as_bytes())?;
    }
    #[cfg(not(unix))]
    std::fs::write(&path, text)?;
    Ok(())
}
