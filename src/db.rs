use anyhow::Result;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;

use crate::types::MessageRow;

pub struct Db {
    pool: SqlitePool,
}

impl Db {
    pub async fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let url = format!("sqlite://{}?mode=rwc", path.display());
        let options = SqliteConnectOptions::from_str(&url)?
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new().connect_with(options).await?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self { pool })
    }

    #[allow(unused)]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn close(self) {
        self.pool.close().await;
    }

    pub async fn upsert_project(&self, slug: &str, cwd: Option<&str>) -> Result<()> {
        sqlx::query(
            "INSERT INTO projects(slug, cwd) VALUES(?, ?)
             ON CONFLICT(slug) DO UPDATE SET cwd = excluded.cwd",
        )
        .bind(slug)
        .bind(cwd)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn upsert_session(
        &self,
        id: &str,
        project: &str,
        cwd: Option<&str>,
        started_at: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO sessions(id, project, cwd, started_at)
             VALUES(?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               cwd        = COALESCE(excluded.cwd, sessions.cwd),
               started_at = COALESCE(sessions.started_at, excluded.started_at)",
        )
        .bind(id)
        .bind(project)
        .bind(cwd)
        .bind(started_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_session_title(&self, id: &str, title: &str) -> Result<()> {
        sqlx::query("UPDATE sessions SET ai_title = ? WHERE id = ?")
            .bind(title)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn upsert_message(&self, row: &MessageRow) -> Result<()> {
        let is_sidechain = row.is_sidechain as i64;
        sqlx::query(
            "INSERT OR IGNORE INTO messages(
                uuid, session_id, parent_uuid, type, timestamp,
                content_json, is_sidechain, model, stop_reason,
                input_tokens, cache_creation_tokens, cache_read_tokens, output_tokens
             ) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(&row.uuid)
        .bind(&row.session_id)
        .bind(&row.parent_uuid)
        .bind(&row.msg_type)
        .bind(&row.timestamp)
        .bind(&row.content_json)
        .bind(is_sidechain)
        .bind(&row.model)
        .bind(&row.stop_reason)
        .bind(row.input_tokens)
        .bind(row.cache_creation_tokens)
        .bind(row.cache_read_tokens)
        .bind(row.output_tokens)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn insert_raw_event(
        &self,
        session_id: &str,
        event_type: &str,
        raw_json: &str,
    ) -> Result<()> {
        sqlx::query("INSERT INTO raw_events(session_id, event_type, raw_json) VALUES(?,?,?)")
            .bind(session_id)
            .bind(event_type)
            .bind(raw_json)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn is_file_known(&self, path: &str) -> Result<bool> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT modified_at FROM processed_files WHERE path = ?")
                .bind(path)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.is_some())
    }

    pub async fn is_file_current(&self, path: &str, mtime: u64) -> Result<bool> {
        let mtime = mtime as i64;
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT modified_at FROM processed_files WHERE path = ?")
                .bind(path)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(m,)| m) == Some(mtime))
    }

    pub async fn mark_file_processed(&self, path: &str, mtime: u64) -> Result<()> {
        let mtime = mtime as i64;
        sqlx::query(
            "INSERT INTO processed_files(path, modified_at) VALUES(?,?)
             ON CONFLICT(path) DO UPDATE SET modified_at = excluded.modified_at",
        )
        .bind(path)
        .bind(mtime)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn insert_plan_revision(&self, slug: &str, content: &str, mtime: u64) -> Result<()> {
        let mtime = mtime as i64;
        sqlx::query(
            "INSERT INTO plan_files(slug, content, modified_at) VALUES(?,?,?)
             ON CONFLICT(slug, modified_at) DO NOTHING",
        )
        .bind(slug)
        .bind(content)
        .bind(mtime)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn is_plan_current(&self, slug: &str, mtime: u64) -> Result<bool> {
        let mtime = mtime as i64;
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT modified_at FROM plan_files WHERE slug = ? AND modified_at = ?")
                .bind(slug)
                .bind(mtime)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.is_some())
    }
}
