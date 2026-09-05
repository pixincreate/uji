//! Opaque, time-ordered identifiers for sessions and messages.

use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Current wall-clock time in epoch milliseconds.
pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

/// UUIDv7-based session id — time-ordered and index-friendly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub Uuid);

impl SessionId {
    /// Mint a fresh session id.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl FromStr for SessionId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

/// UUIDv7-based message id — time-ordered and index-friendly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageId(pub Uuid);

impl MessageId {
    /// Mint a fresh message id.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl FromStr for MessageId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_monotonically_increasing() {
        let ids: Vec<SessionId> = (0..256).map(|_| SessionId::new()).collect();
        for pair in ids.windows(2) {
            assert!(pair[0].0 < pair[1].0, "ids must strictly increase");
        }
    }

    #[test]
    fn message_ids_are_monotonically_increasing() {
        let ids: Vec<MessageId> = (0..256).map(|_| MessageId::new()).collect();
        for pair in ids.windows(2) {
            assert!(pair[0].0 < pair[1].0, "ids must strictly increase");
        }
    }

    #[test]
    fn display_roundtrips_through_from_str() {
        let id = SessionId::new();
        // ast-grep-ignore: no-expect-in-lib
        let parsed: SessionId = id.to_string().parse().expect("valid uuid text");
        assert_eq!(id, parsed);
    }

    #[test]
    fn rejects_non_uuid_text() {
        assert!("ses_nope".parse::<SessionId>().is_err());
    }
}
