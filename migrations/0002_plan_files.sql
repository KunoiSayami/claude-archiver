-- Plan files with full revision history.
-- Each distinct (slug, modified_at) pair is a separate revision.
CREATE TABLE IF NOT EXISTS plan_files (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    slug        TEXT    NOT NULL,
    content     TEXT    NOT NULL,
    modified_at INTEGER NOT NULL,
    UNIQUE (slug, modified_at)
);
