mod db;
mod parser;
mod scanner;
mod types;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, info, warn};

use db::Db;
use parser::ParsedEvent;

#[derive(Parser)]
#[command(
    name = "claude-archiver",
    about = "Archive Claude Code conversations to SQLite"
)]
struct Args {
    /// Path to the SQLite database
    #[arg(long, value_name = "PATH")]
    db: Option<PathBuf>,

    /// Path to ~/.claude/projects/ (auto-detected if omitted)
    #[arg(long, value_name = "PATH")]
    source: Option<PathBuf>,

    /// Only process this project slug
    #[arg(long, value_name = "SLUG")]
    project: Option<String>,

    /// Re-process files even if mtime is unchanged
    #[arg(long)]
    force: bool,

    /// Run continuously, polling every N seconds (e.g. --watch 30)
    #[arg(long, value_name = "SECONDS")]
    watch: Option<u64>,
}

fn default_db_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home dir"))?;
    Ok(home.join(".claude").join("archive.db"))
}

fn default_source_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home dir"))?;
    Ok(home.join(".claude").join("projects"))
}

fn default_plans_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home dir"))?;
    Ok(home.join(".claude").join("plans"))
}

async fn run_once(
    db: &Db,
    source_path: &PathBuf,
    plans_path: &PathBuf,
    project_filter: Option<&str>,
    force: bool,
) -> Result<()> {
    let projects = scanner::discover_projects(source_path, project_filter)?;
    info!(count = projects.len(), "discovered projects");

    let mut total_files = 0usize;
    let mut skipped = 0usize;
    let mut messages = 0usize;

    for project in &projects {
        debug!(slug = %project.slug, "processing project");
        db.upsert_project(&project.slug, None).await?;

        let sessions = scanner::discover_sessions(&project.path)?;

        for session in sessions {
            let path_str = session.path.to_string_lossy().to_string();
            total_files += 1;

            if !force && db.is_file_current(&path_str, session.mtime).await? {
                debug!(path = %path_str, "skipping unchanged file");
                skipped += 1;
                continue;
            }

            info!(session_id = %session.session_id, slug = %project.slug, "archiving session");

            db.upsert_session(&session.session_id, &project.slug, None, None)
                .await?;

            //tracing::debug!("Read {}", &session.path.display());
            let events = match parser::parse_jsonl(&session.path) {
                Ok(e) => e,
                Err(e) => {
                    warn!(path = %path_str, error = %e, "skipping unreadable file");
                    continue;
                }
            };

            // Derive started_at from the earliest message timestamp
            let started_at = events.iter().find_map(|e| {
                if let ParsedEvent::Message(row) = e {
                    Some(row.timestamp.clone())
                } else {
                    None
                }
            });
            if let Some(ref ts) = started_at {
                db.upsert_session(&session.session_id, &project.slug, None, Some(ts))
                    .await?;
            }

            for event in events {
                match event {
                    ParsedEvent::Message(row) => {
                        db.upsert_message(&row).await?;
                        messages += 1;
                    }
                    ParsedEvent::AiTitle { session_id, title } => {
                        debug!(session_id = %session_id, title = %title, "set ai-title");
                        db.update_session_title(&session_id, &title).await?;
                    }
                    ParsedEvent::Raw {
                        session_id,
                        event_type,
                        raw_json,
                    } => {
                        db.insert_raw_event(&session_id, &event_type, &raw_json)
                            .await?;
                    }
                }
            }

            db.mark_file_processed(&path_str, session.mtime).await?;
        }
    }

    info!(total_files, skipped, messages, "run complete");

    // ── Plan files ────────────────────────────────────────────────────────────
    if plans_path.is_dir() {
        let plans = scanner::discover_plans(plans_path)?;
        let mut plans_archived = 0usize;
        for plan in plans {
            if !force && db.is_plan_current(&plan.slug, plan.mtime).await? {
                continue;
            }
            match std::fs::read_to_string(&plan.path) {
                Ok(content) => {
                    db.insert_plan_revision(&plan.slug, &content, plan.mtime)
                        .await?;
                    debug!(slug = %plan.slug, "archived plan");
                    plans_archived += 1;
                }
                Err(e) => warn!(slug = %plan.slug, error = %e, "could not read plan file"),
            }
        }
        info!(plans_archived, "plans archived");
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    let db_path = match args.db {
        Some(ref p) => p.clone(),
        None => default_db_path()?,
    };
    let source_path = match args.source {
        Some(ref p) => p.clone(),
        None => default_source_path()?,
    };
    let plans_path = default_plans_path()?;

    info!(db = %db_path.display(), source = %source_path.display(), "claude-archiver starting");

    let db = Db::open(&db_path).await?;

    match args.watch {
        None => {
            run_once(
                &db,
                &source_path,
                &plans_path,
                args.project.as_deref(),
                args.force,
            )
            .await?;
            db.close().await;
        }
        Some(interval_secs) => {
            info!(interval_secs, "watch mode enabled — press Ctrl-C to stop");
            loop {
                if let Err(e) = run_once(
                    &db,
                    &source_path,
                    &plans_path,
                    args.project.as_deref(),
                    args.force,
                )
                .await
                {
                    warn!(error = %e, "run failed, will retry");
                }
                debug!(sleep_secs = interval_secs, "sleeping until next poll");
                tokio::time::sleep(Duration::from_secs(interval_secs)).await;
            }
        }
    }

    Ok(())
}
