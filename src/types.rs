use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Top-level discriminant — we first parse only the "type" field,
/// then branch to the concrete struct.
#[derive(Debug, Deserialize)]
pub struct EventKind {
    #[serde(rename = "type")]
    pub kind: String,
}

// ── User message ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserEvent {
    pub uuid: String,
    #[serde(default)]
    pub parent_uuid: Option<String>,
    pub session_id: String,
    pub timestamp: String,
    #[serde(default)]
    pub is_sidechain: bool,
    pub message: UserMessage,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserMessage {
    #[serde(default, deserialize_with = "deserialize_content")]
    pub content: Vec<ContentBlock>,
}

/// `content` may be a JSON array of blocks OR a plain string.
/// Normalise a bare string into a single text ContentBlock.
fn deserialize_content<'de, D>(de: D) -> Result<Vec<ContentBlock>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Value::deserialize(de)?;
    match v {
        Value::Array(arr) => {
            serde_json::from_value(Value::Array(arr)).map_err(serde::de::Error::custom)
        }
        Value::String(s) => Ok(vec![ContentBlock {
            kind: "text".to_string(),
            text: Some(s),
            extra: serde_json::Map::new(),
        }]),
        Value::Null => Ok(vec![]),
        other => Err(serde::de::Error::custom(format!(
            "unexpected content type: {}",
            other
        ))),
    }
}

// ── Assistant message ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantEvent {
    pub uuid: String,
    #[serde(default)]
    pub parent_uuid: Option<String>,
    pub session_id: String,
    pub timestamp: String,
    #[serde(default)]
    pub is_sidechain: bool,
    pub message: AssistantMessage,
    #[serde(default)]
    pub cwd: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantMessage {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub content: Vec<ContentBlock>,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

// ── Content blocks ────────────────────────────────────────────────────────────

/// We store the whole content array as JSON, but we also need to be able
/// to serialize it back, so derive Serialize too.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub text: Option<String>,
    // tool_use / tool_result fields kept as raw value to avoid schema churn
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// ── Token usage ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: i64,
    #[serde(default)]
    pub cache_creation_input_tokens: i64,
    #[serde(default)]
    pub cache_read_input_tokens: i64,
    #[serde(default)]
    pub output_tokens: i64,
}

// ── ai-title event ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTitleEvent {
    pub session_id: String,
    pub ai_title: String,
}

// ── Parsed row ready for DB insertion ────────────────────────────────────────

#[derive(Debug)]
pub struct MessageRow {
    pub uuid: String,
    pub session_id: String,
    pub parent_uuid: Option<String>,
    pub msg_type: String,
    pub timestamp: String,
    pub content_json: String,
    pub is_sidechain: bool,
    pub model: Option<String>,
    pub stop_reason: Option<String>,
    pub input_tokens: Option<i64>,
    pub cache_creation_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cwd: Option<String>,
}
