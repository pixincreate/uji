//! Domain model for sessions and messages (opencode-shaped).

use serde::{Deserialize, Serialize};

use super::id::{MessageId, SessionId};

/// Creation / last-update timestamps in epoch milliseconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Time {
    /// When the record was created.
    pub created: i64,
    /// When the record was last updated.
    pub updated: i64,
}

/// A session record — id, optional parent, title, working directory and
/// timestamps. Token/cost rollups land here once [`crate::ai`] is wired.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session id.
    pub id: SessionId,
    /// Session this one forked from, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<SessionId>,
    /// Human-readable title.
    pub title: String,
    /// Working directory the session was created in.
    pub directory: String,
    /// Creation / update timestamps.
    pub time: Time,
}

/// A message payload — tagged by `type`, extensible (tool/shell later).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    /// Text submitted by the user.
    #[serde(rename = "user")]
    User {
        /// Message text.
        text: String,
    },
    /// Text produced by the assistant.
    #[serde(rename = "assistant")]
    Assistant {
        /// Message text.
        text: String,
    },
    /// Harness-generated system notice.
    #[serde(rename = "system")]
    System {
        /// Message text.
        text: String,
    },
}

impl Message {
    /// The SQL `type` tag for this payload.
    pub fn type_name(&self) -> &'static str {
        match self {
            Message::User { .. } => "user",
            Message::Assistant { .. } => "assistant",
            Message::System { .. } => "system",
        }
    }

    /// The message text, regardless of variant.
    pub fn text(&self) -> &str {
        match self {
            Message::User { text } | Message::Assistant { text } | Message::System { text } => text,
        }
    }
}

/// A message as stored in a session: payload plus row metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    /// Unique message id.
    pub id: MessageId,
    /// Per-session sequence number (insertion order).
    pub seq: i64,
    /// Epoch milliseconds when the message was created.
    pub time_created: i64,
    /// The message payload.
    #[serde(flatten)]
    pub message: Message,
}
