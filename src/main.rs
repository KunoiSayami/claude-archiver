mod db;
mod parser;
mod scanner;
mod types;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

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
}

fn default_db_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home dir"))?;
    Ok(home.join(".claude").join("archive.db"))
}

fn default_source_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home dir"))?;
    Ok(home.join(".claude").join("projects"))
}

fn main() -> Result<()> {
    let args = Args::parse();

    let db_path = match args.db {
        Some(p) => p,
        None => default_db_path()?,
    };
    let source_path = match args.source {
        Some(p) => p,
        None => default_source_path()?,
    };

    println!("Database : {}", db_path.display());
    println!("Source   : {}", source_path.display());

    let db = Db::open(&db_path)?;

    let projects = scanner::discover_projects(&source_path, args.project.as_deref())?;
    println!("Projects : {}", projects.len());

    let mut total_files = 0usize;
    let mut skipped = 0usize;
    let mut messages = 0usize;

    for project in &projects {
        db.upsert_project(&project.slug, None)?;

        let sessions = scanner::discover_sessions(&project.path)?;

        for session in sessions {
            let path_str = session.path.to_string_lossy().to_string();
            total_files += 1;

            if !args.force && db.is_file_current(&path_str, session.mtime)? {
                skipped += 1;
                continue;
            }

            // Ensure session row exists
            db.upsert_session(&session.session_id, &project.slug, None, None)?;

            let events = match parser::parse_jsonl(&session.path) {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("warn: skipping {} — {e}", session.path.display());
                    continue;
                }
            };

            // Find earliest timestamp for started_at
            let started_at = events.iter().find_map(|e| {
                if let ParsedEvent::Message(row) = e {
                    Some(row.timestamp.clone())
                } else {
                    None
                }
            });
            if let Some(ref ts) = started_at {
                db.upsert_session(&session.session_id, &project.slug, None, Some(ts))?;
            }

            for event in events {
                match event {
                    ParsedEvent::Message(row) => {
                        db.upsert_message(&row)?;
                        messages += 1;
                    }
                    ParsedEvent::AiTitle { session_id, title } => {
                        db.update_session_title(&session_id, &title)?;
                    }
                    ParsedEvent::Raw {
                        session_id,
                        event_type,
                        raw_json,
                    } => {
                        db.insert_raw_event(&session_id, &event_type, &raw_json)?;
                    }
                }
            }

            db.mark_file_processed(&path_str, session.mtime)?;
        }
    }

    println!("Done — {total_files} files ({skipped} skipped), {messages} messages archived.");
    Ok(())
}
