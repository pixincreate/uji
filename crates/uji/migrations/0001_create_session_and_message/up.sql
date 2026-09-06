-- Initial schema: opencode-shaped session + message tables.
-- IF NOT EXISTS keeps the migration idempotent for databases created by the
-- pre-migration SCHEMA_SQL bootstrap.

CREATE TABLE IF NOT EXISTS session (
    id           TEXT PRIMARY KEY,
    parent_id    TEXT REFERENCES session(id) ON DELETE SET NULL,
    title        TEXT NOT NULL,
    directory    TEXT NOT NULL,
    time_created INTEGER NOT NULL,
    time_updated INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS message (
    id           TEXT PRIMARY KEY,
    session_id   TEXT NOT NULL REFERENCES session(id) ON DELETE CASCADE,
    seq          INTEGER NOT NULL,
    type         TEXT NOT NULL,
    time_created INTEGER NOT NULL,
    data         TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS message_session_seq_idx
    ON message(session_id, seq);

CREATE INDEX IF NOT EXISTS message_session_time_idx
    ON message(session_id, time_created, id);
