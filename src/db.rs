use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

use crate::types::MessageRow;

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.create_schema()?;
        Ok(db)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    fn create_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
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
            ",
        )?;
        Ok(())
    }

    pub fn upsert_project(&self, slug: &str, cwd: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO projects(slug, cwd) VALUES(?1, ?2)
             ON CONFLICT(slug) DO UPDATE SET cwd = excluded.cwd",
            params![slug, cwd],
        )?;
        Ok(())
    }

    pub fn upsert_session(
        &self,
        id: &str,
        project: &str,
        cwd: Option<&str>,
        started_at: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions(id, project, cwd, started_at)
             VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
               cwd        = COALESCE(excluded.cwd, sessions.cwd),
               started_at = COALESCE(sessions.started_at, excluded.started_at)",
            params![id, project, cwd, started_at],
        )?;
        Ok(())
    }

    pub fn update_session_title(&self, id: &str, title: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET ai_title = ?1 WHERE id = ?2",
            params![title, id],
        )?;
        Ok(())
    }

    pub fn upsert_message(&self, row: &MessageRow) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO messages(
                uuid, session_id, parent_uuid, type, timestamp,
                content_json, is_sidechain, model, stop_reason,
                input_tokens, cache_creation_tokens, cache_read_tokens, output_tokens
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                row.uuid,
                row.session_id,
                row.parent_uuid,
                row.msg_type,
                row.timestamp,
                row.content_json,
                row.is_sidechain as i32,
                row.model,
                row.stop_reason,
                row.input_tokens,
                row.cache_creation_tokens,
                row.cache_read_tokens,
                row.output_tokens,
            ],
        )?;
        Ok(())
    }

    pub fn insert_raw_event(
        &self,
        session_id: &str,
        event_type: &str,
        raw_json: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO raw_events(session_id, event_type, raw_json) VALUES(?1,?2,?3)",
            params![session_id, event_type, raw_json],
        )?;
        Ok(())
    }

    pub fn is_file_current(&self, path: &str, mtime: u64) -> Result<bool> {
        let stored: Option<u64> = self
            .conn
            .query_row(
                "SELECT modified_at FROM processed_files WHERE path = ?1",
                params![path],
                |row| row.get(0),
            )
            .optional()?;
        Ok(stored == Some(mtime))
    }

    pub fn mark_file_processed(&self, path: &str, mtime: u64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO processed_files(path, modified_at) VALUES(?1,?2)
             ON CONFLICT(path) DO UPDATE SET modified_at = excluded.modified_at",
            params![path, mtime],
        )?;
        Ok(())
    }
}
