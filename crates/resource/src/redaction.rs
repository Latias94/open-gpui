use serde::{Deserialize, Serialize};

/// Value representation safe for resource diagnostics and devtools snapshots.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RedactedResourceValue {
    /// The value was intentionally omitted.
    Redacted,
    /// A textual summary that does not expose the full payload.
    Summary(String),
    /// The value is safe to expose as JSON.
    Json(serde_json::Value),
}

/// Policy used when producing resource diagnostic snapshots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceRedactionPolicy {
    /// Hide all payloads and expose only lifecycle metadata.
    RedactAll,
    /// Expose a small diagnostic summary for payloads.
    Summarize,
    /// Expose JSON payloads unchanged.
    Expose,
}

impl Default for ResourceRedactionPolicy {
    fn default() -> Self {
        Self::RedactAll
    }
}

impl ResourceRedactionPolicy {
    /// Applies the policy to a JSON value.
    pub fn apply(&self, value: serde_json::Value) -> RedactedResourceValue {
        match self {
            Self::RedactAll => RedactedResourceValue::Redacted,
            Self::Summarize => RedactedResourceValue::Summary(value_summary(&value)),
            Self::Expose => RedactedResourceValue::Json(value),
        }
    }
}

impl RedactedResourceValue {
    /// Returns true when the value was redacted.
    pub fn is_redacted(&self) -> bool {
        matches!(self, Self::Redacted)
    }

    /// Returns the exposed JSON value, when available.
    pub fn as_json(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Json(value) => Some(value),
            Self::Redacted | Self::Summary(_) => None,
        }
    }
}

fn value_summary(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_owned(),
        serde_json::Value::Bool(_) => "bool".to_owned(),
        serde_json::Value::Number(_) => "number".to_owned(),
        serde_json::Value::String(value) => format!("string:{} chars", value.chars().count()),
        serde_json::Value::Array(values) => format!("array:{} items", values.len()),
        serde_json::Value::Object(values) => format!("object:{} keys", values.len()),
    }
}
