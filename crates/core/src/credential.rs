use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::auth::{Grant, Tokens};
use crate::config;

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
    #[error("$HOME is not set")]
    NoHome,
}

pub fn get(provider: &str) -> Option<String> {
    load(provider)
        .as_ref()
        .and_then(Credential::api_key)
        .map(ToString::to_string)
}

pub fn load(provider: &str) -> Option<Credential> {
    keyring_get(provider)
        .map(|raw| parse(&raw))
        .or_else(|| read_file()?.remove(provider))
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
    map.insert(provider.to_string(), credential.clone());
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

fn path() -> Option<PathBuf> {
    config::data_dir().map(|dir| dir.join("auth.json"))
}

fn read_file() -> Option<BTreeMap<String, Credential>> {
    let text = std::fs::read_to_string(path()?).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_file(map: &BTreeMap<String, Credential>) -> Result<(), CredentialError> {
    let path = path().ok_or(CredentialError::NoHome)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(map)?;
    write_private(&path, &text)?;
    Ok(())
}

#[cfg(unix)]
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?
        .write_all(text.as_bytes())
}

#[cfg(not(unix))]
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    std::fs::write(path, text)
}
