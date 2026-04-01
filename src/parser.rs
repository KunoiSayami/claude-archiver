use anyhow::Result;
use std::path::Path;

use crate::types::{AiTitleEvent, AssistantEvent, EventKind, MessageRow, UserEvent};

pub enum ParsedEvent {
    Message(MessageRow),
    AiTitle {
        session_id: String,
        title: String,
    },
    Raw {
        session_id: String,
        event_type: String,
        raw_json: String,
    },
}

pub fn parse_jsonl(path: &Path) -> Result<Vec<ParsedEvent>> {
    let content = std::fs::read_to_string(path)?;
    let mut events = Vec::new();

    for (line_no, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Determine the type discriminant first
        let kind: EventKind = match serde_json::from_str(line) {
            Ok(k) => k,
            Err(e) => {
                eprintln!(
                    "warn: {}:{} — skipping unparseable line: {e}",
                    path.display(),
                    line_no + 1
                );
                continue;
            }
        };

        let event = match kind.kind.as_str() {
            "user" => parse_user(line, line_no, path),
            "assistant" => parse_assistant(line, line_no, path),
            "ai-title" => parse_ai_title(line, line_no, path),
            other => {
                // Extract sessionId from raw JSON for storage
                let session_id = extract_session_id(line).unwrap_or_default();
                Some(ParsedEvent::Raw {
                    session_id,
                    event_type: other.to_string(),
                    raw_json: line.to_string(),
                })
            }
        };

        if let Some(e) = event {
            events.push(e);
        }
    }

    Ok(events)
}

fn parse_user(line: &str, line_no: usize, path: &Path) -> Option<ParsedEvent> {
    match serde_json::from_str::<UserEvent>(line) {
        Ok(ev) => {
            let content_json = serde_json::to_string(&ev.message.content).unwrap_or_default();
            Some(ParsedEvent::Message(MessageRow {
                uuid: ev.uuid,
                session_id: ev.session_id,
                parent_uuid: ev.parent_uuid,
                msg_type: "user".to_string(),
                timestamp: ev.timestamp,
                content_json,
                is_sidechain: ev.is_sidechain,
                model: None,
                stop_reason: None,
                input_tokens: None,
                cache_creation_tokens: None,
                cache_read_tokens: None,
                output_tokens: None,
                cwd: ev.cwd,
            }))
        }
        Err(e) => {
            eprintln!(
                "warn: {}:{} — failed to parse user event: {e}",
                path.display(),
                line_no + 1
            );
            None
        }
    }
}

fn parse_assistant(line: &str, line_no: usize, path: &Path) -> Option<ParsedEvent> {
    match serde_json::from_str::<AssistantEvent>(line) {
        Ok(ev) => {
            let content_json = serde_json::to_string(&ev.message.content).unwrap_or_default();
            let usage = ev.message.usage;
            Some(ParsedEvent::Message(MessageRow {
                uuid: ev.uuid,
                session_id: ev.session_id,
                parent_uuid: ev.parent_uuid,
                msg_type: "assistant".to_string(),
                timestamp: ev.timestamp,
                content_json,
                is_sidechain: ev.is_sidechain,
                model: ev.message.model,
                stop_reason: ev.message.stop_reason,
                input_tokens: usage.as_ref().map(|u| u.input_tokens),
                cache_creation_tokens: usage.as_ref().map(|u| u.cache_creation_input_tokens),
                cache_read_tokens: usage.as_ref().map(|u| u.cache_read_input_tokens),
                output_tokens: usage.as_ref().map(|u| u.output_tokens),
                cwd: ev.cwd,
            }))
        }
        Err(e) => {
            eprintln!(
                "warn: {}:{} — failed to parse assistant event: {e}",
                path.display(),
                line_no + 1
            );
            None
        }
    }
}

fn parse_ai_title(line: &str, line_no: usize, path: &Path) -> Option<ParsedEvent> {
    match serde_json::from_str::<AiTitleEvent>(line) {
        Ok(ev) => Some(ParsedEvent::AiTitle {
            session_id: ev.session_id,
            title: ev.ai_title,
        }),
        Err(e) => {
            eprintln!(
                "warn: {}:{} — failed to parse ai-title event: {e}",
                path.display(),
                line_no + 1
            );
            None
        }
    }
}

fn extract_session_id(line: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Partial {
        #[serde(default)]
        session_id: Option<String>,
    }
    serde_json::from_str::<Partial>(line)
        .ok()
        .and_then(|p| p.session_id)
}
