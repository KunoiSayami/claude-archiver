CREATE TABLE IF NOT EXISTS projects (
    slug TEXT PRIMARY KEY,
    cwd  TEXT
);

CREATE TABLE IF NOT EXISTS sessions (
    id         TEXT PRIMARY KEY,
    project    TEXT NOT NULL REFERENCES projects(slug),
    ai_title   TEXT,
    cwd        TEXT,
    started_at TEXT
);

CREATE TABLE IF NOT EXISTS messages (
    uuid                  TEXT PRIMARY KEY,
    session_id            TEXT NOT NULL REFERENCES sessions(id),
    parent_uuid           TEXT,
    type                  TEXT NOT NULL,
    timestamp             TEXT NOT NULL,
    content_json          TEXT NOT NULL,
    is_sidechain          INTEGER NOT NULL DEFAULT 0,
    model                 TEXT,
    stop_reason           TEXT,
    input_tokens          INTEGER,
    cache_creation_tokens INTEGER,
    cache_read_tokens     INTEGER,
    output_tokens         INTEGER
);

CREATE TABLE IF NOT EXISTS raw_events (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    raw_json   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS processed_files (
    path        TEXT PRIMARY KEY,
    modified_at INTEGER NOT NULL
);
