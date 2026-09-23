CREATE TABLE sessions (
    id           TEXT PRIMARY KEY,
    title        TEXT NOT NULL,
    directory    TEXT NOT NULL,
    time_created INTEGER NOT NULL,
    time_updated INTEGER NOT NULL
);

CREATE TABLE messages (
    id           TEXT PRIMARY KEY,
    session_id   TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    seq          INTEGER NOT NULL,
    type         TEXT NOT NULL,
    time_created INTEGER NOT NULL,
    data         TEXT NOT NULL
);

CREATE UNIQUE INDEX messages_session_seq ON messages(session_id, seq);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
