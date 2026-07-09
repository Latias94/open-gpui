//! Shared helpers for first-party DevTools snapshot adapters.

mod payload;

use serde::Serialize;

use crate::SnapshotNode;

pub use payload::{sanitize_json_value, sanitize_sensitive_text, stable_node_id, summary_payload};

/// Builds a snapshot node with a deterministic, sanitized id.
pub fn snapshot_node<I, S>(id_parts: I, label: impl AsRef<str>) -> SnapshotNode
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    SnapshotNode::new(
        stable_node_id(id_parts),
        sanitize_sensitive_text(label.as_ref()),
    )
}

/// Builds a snapshot node with a deterministic, sanitized id and JSON payload.
pub fn snapshot_node_with_payload<I, S, T>(
    id_parts: I,
    label: impl AsRef<str>,
    payload: T,
) -> SnapshotNode
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    T: Serialize,
{
    snapshot_node(id_parts, label).with_payload(summary_payload(payload))
}
