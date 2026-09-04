use rusqlite::{Connection, OptionalExtension, params};

use crate::storage::error::Result;
use crate::storage::interface::StorageInterface;

use super::id::{MessageId, SessionId, now_millis};
use super::model::{Message, Session, StoredMessage, Time};

/// Domain operations over sessions and messages — the opencode `SessionStore`
/// surface, layered on top of any `StorageInterface` backend.
pub trait SessionStorage {
    fn create_session(&self, title: &str) -> Result<Session>;
    fn get_session(&self, id: &SessionId) -> Result<Option<Session>>;
    fn latest_session(&self) -> Result<Option<Session>>;
    fn list_sessions(&self) -> Result<Vec<Session>>;
    fn delete_session(&self, id: &SessionId) -> Result<bool>;

    /// Append a message, assigning its `msg_` id and per-session `seq`.
    fn append_message(&self, session_id: &SessionId, message: Message) -> Result<StoredMessage>;
    /// Full ordered message history ("context" in opencode terms).
    fn messages(&self, session_id: &SessionId) -> Result<Vec<StoredMessage>>;
    fn message(&self, id: &MessageId) -> Result<Option<(SessionId, StoredMessage)>>;
}

/// Every sqlite-backed storage gets the full session store for free.
impl<T: StorageInterface<Connection = Connection>> SessionStorage for T {
    fn create_session(&self, title: &str) -> Result<Session> {
        let conn = self.get_connection();
        let id = SessionId::new();
        let now = now_millis();
        let directory = std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        conn.execute(
            "INSERT INTO session (id, parent_id, title, directory, time_created, time_updated)
             VALUES (?1, NULL, ?2, ?3, ?4, ?4)",
            params![id.as_str(), title, directory, now],
        )?;
        Ok(Session {
            id,
            parent_id: None,
            title: title.to_string(),
            directory,
            time: Time { created: now, updated: now },
        })
    }

    fn get_session(&self, id: &SessionId) -> Result<Option<Session>> {
        let conn = self.get_connection();
        let session = conn
            .query_row(&format!("{SESSION_COLUMNS} WHERE id = ?1"), params![id.as_str()], session_from_row)
            .optional()?;
        Ok(session)
    }

    fn latest_session(&self) -> Result<Option<Session>> {
        let conn = self.get_connection();
        let session = conn
            .query_row(
                &format!("{SESSION_COLUMNS} ORDER BY time_updated DESC, id DESC LIMIT 1"),
                params![],
                session_from_row,
            )
            .optional()?;
        Ok(session)
    }

    fn list_sessions(&self) -> Result<Vec<Session>> {
        let conn = self.get_connection();
        let mut stmt = conn.prepare(&format!("{SESSION_COLUMNS} ORDER BY time_updated DESC"))?;
        let rows = stmt.query_map(params![], session_from_row)?;
        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?);
        }
        Ok(sessions)
    }

    fn delete_session(&self, id: &SessionId) -> Result<bool> {
        let conn = self.get_connection();
        let deleted = conn.execute("DELETE FROM session WHERE id = ?1", params![id.as_str()])?;
        Ok(deleted > 0)
    }

    fn append_message(&self, session_id: &SessionId, message: Message) -> Result<StoredMessage> {
        let conn = self.get_connection();
        let seq: i64 = conn.query_row(
            "SELECT COALESCE(MAX(seq), 0) + 1 FROM message WHERE session_id = ?1",
            params![session_id.as_str()],
            |row| row.get(0),
        )?;
        let id = MessageId::new();
        let now = now_millis();
        let data = serde_json::to_string(&message)?;
        conn.execute(
            "INSERT INTO message (id, session_id, seq, type, time_created, data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id.as_str(), session_id.as_str(), seq, message.type_name(), now, data],
        )?;
        conn.execute(
            "UPDATE session SET time_updated = ?1 WHERE id = ?2",
            params![now, session_id.as_str()],
        )?;
        Ok(StoredMessage { id, seq, time_created: now, message })
    }

    fn messages(&self, session_id: &SessionId) -> Result<Vec<StoredMessage>> {
        let conn = self.get_connection();
        let mut stmt = conn.prepare(
            "SELECT id, seq, time_created, data FROM message WHERE session_id = ?1 ORDER BY seq",
        )?;
        let rows = stmt.query_map(params![session_id.as_str()], message_from_row)?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    fn message(&self, id: &MessageId) -> Result<Option<(SessionId, StoredMessage)>> {
        let conn = self.get_connection();
        let row = conn
            .query_row(
                "SELECT id, seq, time_created, data, session_id FROM message WHERE id = ?1",
                params![id.as_str()],
                |row| {
                    let session_id: String = row.get(4)?;
                    let stored = message_from_row(row)?;
                    Ok((SessionId(session_id), stored))
                },
            )
            .optional()?;
        Ok(row)
    }
}

const SESSION_COLUMNS: &str =
    "SELECT id, parent_id, title, directory, time_created, time_updated FROM session";

fn session_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Session> {
    let id: String = row.get(0)?;
    let parent_id: Option<String> = row.get(1)?;
    let title: String = row.get(2)?;
    let directory: String = row.get(3)?;
    let created: i64 = row.get(4)?;
    let updated: i64 = row.get(5)?;
    Ok(Session {
        id: SessionId(id),
        parent_id: parent_id.map(SessionId),
        title,
        directory,
        time: Time { created, updated },
    })
}

fn message_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredMessage> {
    let id: String = row.get(0)?;
    let seq: i64 = row.get(1)?;
    let time_created: i64 = row.get(2)?;
    let data: String = row.get(3)?;
    let message = serde_json::from_str(&data)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(err)))?;
    Ok(StoredMessage {
        id: MessageId(id),
        seq,
        time_created,
        message,
    })
}
