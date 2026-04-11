mod db;
mod parser;
mod scanner;
mod types;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, info, trace, warn};

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

    #[command(subcommand)]
    command: Option<Command>,

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

    /// Maximum polling interval when idle, in seconds (default: 1200 = 20 min)
    #[arg(long, value_name = "SECONDS", default_value_t = 1200)]
    max_idle_secs: u64,
}

#[derive(Subcommand)]
enum Command {
    /// Check whether a file path has been recorded in the database
    Check {
        /// Path to the file to check
        path: PathBuf,
    },
}

fn default_db_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home dir"))?;
    Ok(home.join("claude-archive.db"))
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
) -> Result<bool> {
    let projects = scanner::discover_projects(source_path, project_filter)?;
    //info!(count = projects.len(), "discovered projects");

    let mut total_files = 0usize;
    let mut skipped = 0usize;
    let mut messages = 0usize;
    let mut changed = false;
    let mut updated_projects: Vec<&str> = Vec::new();

    for project in &projects {
        trace!(slug = %project.slug, "processing project");
        db.upsert_project(&project.slug, None).await?;

        let sessions = scanner::discover_sessions(&project.path)?;
        let mut project_changed = false;

        for session in sessions {
            let path_str = session.path.to_string_lossy().to_string();
            total_files += 1;

            if !force && db.is_file_current(&path_str, session.mtime).await? {
                trace!(path = %path_str, "skipping unchanged file");
                skipped += 1;
                continue;
            }

            changed = true;
            project_changed = true;
            trace!(session_id = %session.session_id, slug = %project.slug, "archiving session");

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
                        trace!(session_id = %session_id, title = %title, "set ai-title");
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

        if project_changed {
            updated_projects.push(&project.slug);
        }
    }

    let mut plans_archived = 0usize;
    // ── Plan files ────────────────────────────────────────────────────────────
    if plans_path.is_dir() {
        let plans = scanner::discover_plans(plans_path)?;
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
                    changed = true;
                }
                Err(e) => warn!(slug = %plan.slug, error = %e, "could not read plan file"),
            }
        }
    }
    if messages > 0 || plans_archived > 0 {
        info!(
            total_files,
            skipped,
            messages,
            plans_archived,
            projects = ?updated_projects,
            "run complete"
        );
    }

    Ok(changed)
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

    info!(db = %db_path.display(), "claude-archiver starting");

    let db = Db::open(&db_path).await?;

    if let Some(Command::Check { path }) = args.command {
        let path_str = path.to_string_lossy().to_string();
        let known = db.is_file_known(&path_str).await?;
        if known {
            info!(path = %path_str, "file is in database");
        } else {
            info!(path = %path_str, "file is NOT in database");
        }
        db.close().await;
        return Ok(());
    }

    let source_path = match args.source {
        Some(ref p) => p.clone(),
        None => default_source_path()?,
    };
    let plans_path = default_plans_path()?;

    info!(source = %source_path.display(), "archiving");

    match args.watch {
        None => {
            let _ = run_once(
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
            const STEP_DOWN_AFTER: u32 = 5;

            let base_secs = interval_secs;
            let max_idle = if args.max_idle_secs < base_secs {
                warn!(
                    base_secs,
                    max_idle_secs = args.max_idle_secs,
                    "max-idle-interval less than watch interval, clamping"
                );
                base_secs
            } else {
                args.max_idle_secs
            };
            // Fibonacci back-off state: prev is the interval before current,
            // current is the active sleep duration. After STEP_DOWN_AFTER idle
            // polls the next interval = current + prev, capped at max_idle.
            let mut current_secs = base_secs;
            let mut prev_secs = base_secs;
            let mut idle_streak: u32 = 0;

            info!(
                base_secs,
                max_idle_secs = max_idle,
                "watch mode enabled — press Ctrl-C to stop"
            );

            loop {
                let changed = match run_once(
                    &db,
                    &source_path,
                    &plans_path,
                    args.project.as_deref(),
                    args.force,
                )
                .await
                {
                    Ok(c) => c,
                    Err(e) => {
                        warn!(error = %e, "run failed, will retry");
                        false
                    }
                };

                if changed {
                    if current_secs != base_secs {
                        trace!(
                            idle_streak,
                            interval = base_secs,
                            "activity detected: resuming fast polling"
                        );
                    }
                    idle_streak = 0;
                    current_secs = base_secs;
                    prev_secs = base_secs;
                } else {
                    idle_streak += 1;
                    if idle_streak >= STEP_DOWN_AFTER && current_secs < max_idle {
                        let next = current_secs.saturating_add(prev_secs).min(max_idle);
                        trace!(
                            idle_streak,
                            old = current_secs,
                            new = next,
                            "idle: slowing poll frequency"
                        );
                        prev_secs = current_secs;
                        current_secs = next;
                    }
                }

                trace!(sleep_secs = current_secs, "sleeping until next poll");

                // Sleep until next poll, but wake immediately on Ctrl-C.
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(current_secs)) => {}
                    _ = tokio::signal::ctrl_c() => {
                        info!("received Ctrl-C, shutting down");
                        break;
                    }
                }
            }
            db.close().await;
        }
    }

    Ok(())
}
