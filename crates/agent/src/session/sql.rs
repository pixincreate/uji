use super::model::{Session, StoredMessage, Time};
use crate::storage::StorageError;

#[derive(diesel::Queryable, diesel::Selectable, Debug)]
#[diesel(table_name = crate::storage::schema::sessions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct SessionRow {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub directory: String,
    pub time_created: i64,
    pub time_updated: i64,
}

#[derive(diesel::Queryable, diesel::Selectable, Debug)]
#[diesel(table_name = crate::storage::schema::messages)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct MessageRow {
    pub id: String,
    pub session_id: String,
    pub seq: i64,
    pub kind: String,
    pub time_created: i64,
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
