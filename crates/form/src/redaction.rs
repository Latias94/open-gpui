use serde::{Deserialize, Serialize};

/// Value representation safe for diagnostics and devtools snapshots.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RedactedValue {
    /// The value was intentionally omitted.
    Redacted,
    /// A textual summary that does not expose the full value.
    Summary(String),
    /// The value is safe to expose as JSON.
    Json(serde_json::Value),
}

/// Policy used when producing form diagnostic snapshots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RedactionPolicy {
    /// Hide all values and expose only meta state.
    RedactAll,
    /// Expose a small diagnostic summary for values.
    Summarize,
    /// Expose JSON values unchanged.
    Expose,
}

impl Default for RedactionPolicy {
    fn default() -> Self {
        Self::RedactAll
    }
}

impl RedactionPolicy {
    /// Applies the policy to a JSON value.
    pub fn apply(&self, value: serde_json::Value) -> RedactedValue {
        match self {
            Self::RedactAll => RedactedValue::Redacted,
            Self::Summarize => RedactedValue::Summary(value_summary(&value)),
            Self::Expose => RedactedValue::Json(value),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_redacts_values() {
        let value = serde_json::json!({"token": "secret"});

        assert_eq!(
            RedactionPolicy::default().apply(value),
            RedactedValue::Redacted
        );
    }
}
