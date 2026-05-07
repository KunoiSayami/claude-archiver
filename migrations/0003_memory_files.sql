-- Memory files with full revision history.
-- scope is "global" or a project slug.
-- Each distinct (scope, name, modified_at) triple is a separate revision.
CREATE TABLE IF NOT EXISTS memory_files (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    scope       TEXT    NOT NULL,
    name        TEXT    NOT NULL,
    content     TEXT    NOT NULL,
    modified_at INTEGER NOT NULL,
    UNIQUE (scope, name, modified_at)
);
