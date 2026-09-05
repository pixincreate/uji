//! Diesel row models for the session/message tables (opencode's `sql.ts`
//! counterpart) plus conversions into the domain types.

use super::model::{Session, StoredMessage, Time};
use crate::storage::StorageError;

/// Row for the `session` table.
#[derive(diesel::Queryable, diesel::Selectable, Debug)]
#[diesel(table_name = crate::storage::schema::sessions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct SessionRow {
    /// Session id (`UUIDv7` text).
    pub id: String,
    /// Parent session id, if forked.
    pub parent_id: Option<String>,
    /// Human-readable title.
    pub title: String,
    /// Working directory.
    pub directory: String,
    /// Creation time (epoch ms).
    pub time_created: i64,
    /// Last update time (epoch ms).
    pub time_updated: i64,
}

/// Row for the `message` table. `kind` maps to the SQL `type` column.
#[derive(diesel::Queryable, diesel::Selectable, Debug)]
#[diesel(table_name = crate::storage::schema::messages)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct MessageRow {
    /// Message id (`UUIDv7` text).
    pub id: String,
    /// Owning session id.
    pub session_id: String,
    /// Per-session sequence number.
    pub seq: i64,
    /// SQL `type` tag (`user`, `assistant`, `system`).
    pub kind: String,
    /// Creation time (epoch ms).
    pub time_created: i64,
    /// JSON-encoded [`super::model::Message`] payload.
    pub data: String,
}

impl TryFrom<SessionRow> for Session {
    type Error = uuid::Error;

    fn try_from(row: SessionRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id.parse()?,
            parent_id: row.parent_id.map(|id| id.parse()).transpose()?,
            title: row.title,
            directory: row.directory,
            time: Time {
                created: row.time_created,
                updated: row.time_updated,
            },
        })
    }
}

impl TryFrom<MessageRow> for StoredMessage {
    type Error = StorageError;

    fn try_from(row: MessageRow) -> Result<Self, Self::Error> {
        let message = serde_json::from_str(&row.data)?;
        Ok(Self {
            id: row.id.parse()?,
            seq: row.seq,
            time_created: row.time_created,
            message,
        })
    }
}
