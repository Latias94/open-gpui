//! Payload and identifier helpers shared by snapshot adapters.

use serde::Serialize;

/// Converts a serializable payload into sanitized JSON for a snapshot node.
pub fn summary_payload<T>(payload: T) -> serde_json::Value
where
    T: Serialize,
{
    match serde_json::to_value(payload) {
        Ok(value) => sanitize_json_value(value),
        Err(error) => serde_json::json!({
            "serialization_error": sanitize_sensitive_text(&error.to_string()),
        }),
    }
}

/// Sanitizes every string in a JSON value, including object keys.
pub fn sanitize_json_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sanitize_json_value).collect())
        }
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| (sanitize_sensitive_text(&key), sanitize_json_value(value)))
                .collect(),
        ),
        serde_json::Value::String(value) => {
            serde_json::Value::String(sanitize_sensitive_text(&value))
        }
        other => other,
    }
}

/// Builds a deterministic id segment path from possibly user-provided labels.
pub fn stable_node_id<I, S>(parts: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let segments = parts
        .into_iter()
        .map(|part| stable_segment(part.as_ref()))
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if segments.is_empty() {
        "node".to_owned()
    } else {
        segments.join(".")
    }
}

/// Removes sensitive fragments from diagnostic text, labels, ids, and payload strings.
pub fn sanitize_sensitive_text(value: &str) -> String {
    let redacted = redact_email_like(value);
    let redacted = redact_url_queries(&redacted);
    let redacted = redact_sensitive_assignments(&redacted);
    let redacted = redact_path_like_tokens(&redacted);

    redacted.trim().to_owned()
}

fn stable_segment(value: &str) -> String {
    let sanitized = sanitize_sensitive_text(value);
    let mut segment = String::new();
    let mut last_was_separator = false;

    for character in sanitized.chars().flat_map(char::to_lowercase) {
        let is_allowed = character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.');
        if is_allowed {
            segment.push(character);
            last_was_separator = false;
        } else if !last_was_separator {
            segment.push('-');
            last_was_separator = true;
        }
    }

    let segment = segment.trim_matches(['-', '.']).to_owned();
    if segment.len() <= 64 {
        segment
    } else {
        format!("{}-{:016x}", &segment[..48], stable_hash(&segment))
    }
}

fn stable_hash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn redact_email_like(value: &str) -> String {
    let mut output = value.to_owned();
    while let Some(at) = output.find('@') {
        let bytes = output.as_bytes();
        let mut start = at;
        while start > 0 && is_email_local_byte(bytes[start - 1]) {
            start -= 1;
        }

        let mut end = at + 1;
        while end < bytes.len() && is_email_domain_byte(bytes[end]) {
            end += 1;
        }

        let candidate = &output[start..end];
        if candidate.contains('.') && candidate.len() > 3 {
            output.replace_range(start..end, "[redacted-email]");
        } else {
            break;
        }
    }
    output
}

fn is_email_local_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'%' | b'+' | b'-')
}

fn is_email_domain_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-')
}

fn redact_url_queries(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.char_indices().peekable();

    while let Some((index, character)) = chars.next() {
        if character == '?' {
            output.push_str("?[redacted-query]");
            while let Some((_, next)) = chars.peek() {
                if next.is_whitespace() || *next == '#' {
                    break;
                }
                chars.next();
            }
        } else {
            output.push_str(&value[index..index + character.len_utf8()]);
        }
    }

    output
}

fn redact_sensitive_assignments(value: &str) -> String {
    const KEYS: &[&str] = &[
        "access_token",
        "api_key",
        "apikey",
        "auth",
        "bearer",
        "password",
        "secret",
        "token",
    ];

    let mut output = Vec::new();
    let mut tokens = value.split_whitespace().peekable();

    while let Some(token) = tokens.next() {
        let lower = token.to_ascii_lowercase();
        if lower == "bearer" {
            output.push("bearer [redacted]".to_owned());
            tokens.next();
            continue;
        }
        output.push(redact_sensitive_token(token, KEYS));
    }

    output.join(" ")
}

fn redact_sensitive_token(token: &str, keys: &[&str]) -> String {
    let lower = token.to_ascii_lowercase();
    for key in keys {
        if *key == "bearer" && lower.starts_with("bearer:") {
            return "bearer [redacted]".to_owned();
        }

        for separator in ['=', ':'] {
            let pattern = format!("{key}{separator}");
            if let Some(index) = lower.find(&pattern) {
                let value_start = index + pattern.len();
                if value_start < token.len() {
                    let mut redacted = token[..value_start].to_owned();
                    redacted.push_str("[redacted]");
                    return redacted;
                }
            }
        }
    }

    token.to_owned()
}

fn redact_path_like_tokens(value: &str) -> String {
    value
        .split_whitespace()
        .map(|token| {
            if is_path_like(token) {
                "[redacted-path]"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_path_like(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    token.starts_with('/')
        || token.starts_with('~')
        || lower.contains(":\\")
        || lower.contains("\\users\\")
        || lower.contains("/users/")
        || lower.contains("/home/")
        || lower.contains("\\home\\")
}
