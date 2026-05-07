use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub struct ProjectEntry {
    pub slug: String,
    pub path: PathBuf,
}

pub struct SessionEntry {
    pub session_id: String,
    pub path: PathBuf,
    pub mtime: u64,
}

pub fn discover_projects(source: &Path, filter: Option<&str>) -> Result<Vec<ProjectEntry>> {
    let mut projects = Vec::new();

    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let slug = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if slug.is_empty() {
            continue;
        }
        if let Some(f) = filter {
            if slug != f {
                continue;
            }
        }
        projects.push(ProjectEntry { slug, path });
    }

    projects.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(projects)
}

fn collect_jsonl(dir: &Path, session_id: &str, sessions: &mut Vec<SessionEntry>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            let mtime = entry
                .metadata()?
                .modified()?
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            sessions.push(SessionEntry {
                session_id: session_id.to_string(),
                path,
                mtime,
            });
        }
    }
    Ok(())
}

pub fn discover_sessions(project_dir: &Path, include_subagents: bool) -> Result<Vec<SessionEntry>> {
    let mut sessions = Vec::new();

    for entry in std::fs::read_dir(project_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            // Top-level session files: stem is the session UUID
            let session_id = path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if session_id.is_empty() {
                continue;
            }
            let mtime = entry
                .metadata()?
                .modified()?
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            sessions.push(SessionEntry {
                session_id,
                path,
                mtime,
            });
        } else if include_subagents && path.is_dir() {
            // <session-uuid>/subagents/*.jsonl — session UUID is the dir name
            let session_id = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if session_id.is_empty() {
                continue;
            }
            let subagents_dir = path.join("subagents");
            if subagents_dir.is_dir() {
                collect_jsonl(&subagents_dir, &session_id, &mut sessions)?;
            }
        }
    }

    sessions.sort_by(|a, b| a.session_id.cmp(&b.session_id));
    Ok(sessions)
}

pub struct PlanEntry {
    pub slug: String,
    pub path: PathBuf,
    pub mtime: u64,
}

pub fn discover_plans(plans_dir: &Path) -> Result<Vec<PlanEntry>> {
    let mut plans = Vec::new();

    for entry in std::fs::read_dir(plans_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("md") {
            let slug = path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if slug.is_empty() {
                continue;
            }
            let mtime = entry
                .metadata()?
                .modified()?
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            plans.push(PlanEntry { slug, path, mtime });
        }
    }

    plans.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(plans)
}
