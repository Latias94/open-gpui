//! DevTools adapters for `open-gpui-form` snapshots.

use open_gpui_form::{FieldSnapshot, FormSnapshot, RedactedValue};

use crate::{
    DevtoolsCapture, DevtoolsDomainId, DevtoolsDomainKind, DevtoolsDomainSnapshot,
    DevtoolsTargetId, DevtoolsTargetKind, DevtoolsTargetSnapshot, DevtoolsTargetTree, ProbeId,
    ProbeSnapshotError, SnapshotEnvelope, SnapshotKind, SnapshotNode, SnapshotProbe,
    SnapshotProbeSnapshot, SnapshotRedactionSummary, SnapshotTree,
    adapters::{sanitize_sensitive_text, snapshot_node_with_payload, summary_payload},
};

/// Converts a form snapshot into a DevTools snapshot tree and redaction summary.
pub fn form_probe_snapshot(snapshot: &FormSnapshot) -> SnapshotProbeSnapshot {
    let (tree, redaction) = form_tree_and_redaction(snapshot);
    SnapshotProbeSnapshot::new(tree).with_redaction(redaction)
}

/// Converts a form snapshot into a DevTools envelope with the supplied probe id.
pub fn form_snapshot_envelope(probe_id: ProbeId, snapshot: &FormSnapshot) -> SnapshotEnvelope {
    let (tree, redaction) = form_tree_and_redaction(snapshot);
    SnapshotEnvelope::new(probe_id, SnapshotKind::Form, tree).with_redaction(redaction)
}

/// Converts a form snapshot into a first-party DevTools capture.
pub fn form_capture(probe_id: ProbeId, snapshot: &FormSnapshot) -> DevtoolsCapture {
    let envelope = form_snapshot_envelope(probe_id.clone(), snapshot);
    let target_id = DevtoolsTargetId::from_parts(["form", probe_id.as_str()]);
    let domain_id = DevtoolsDomainId::from_parts(["form", probe_id.as_str()]);
    let target =
        DevtoolsTargetSnapshot::new(target_id.clone(), DevtoolsTargetKind::Runtime, "Form state")
            .with_metadata(serde_json::json!({
                "probe_id": probe_id.as_str(),
                "domain": "data",
                "semantic_id": "form",
            }));
    let domain =
        DevtoolsDomainSnapshot::new(domain_id, target_id, DevtoolsDomainKind::Data, "Form state")
            .with_summary(serde_json::json!({
                "status": &snapshot.status,
                "field_count": snapshot.fields.len(),
                "error_count": snapshot.errors.len(),
                "submit_count": snapshot.submit_count,
                "redacted_values": envelope.redaction.redacted_values,
            }))
            .with_snapshot(envelope.clone());

    DevtoolsCapture::new(
        DevtoolsTargetTree::new([target]),
        [domain],
        Vec::new(),
        [envelope],
        Vec::new(),
    )
}

/// Builds a closure-backed form snapshot probe.
pub fn form_snapshot_probe<F>(
    id: impl Into<String>,
    snapshot: F,
) -> Result<
    SnapshotProbe<impl Fn() -> Result<SnapshotProbeSnapshot, ProbeSnapshotError> + Send + Sync>,
    ProbeSnapshotError,
>
where
    F: Fn() -> FormSnapshot + Send + Sync + 'static,
{
    SnapshotProbe::new(id, SnapshotKind::Form, move || {
        Ok(form_probe_snapshot(&snapshot()))
    })
}

/// Converts one field snapshot into a DevTools node.
pub fn field_snapshot_node(
    field: &FieldSnapshot,
    redaction: &mut SnapshotRedactionSummary,
) -> SnapshotNode {
    let value = field_value_payload(field, redaction);
    snapshot_node_with_payload(
        ["form", "field", field.path.as_str()],
        format!("Field {}", sanitize_sensitive_text(field.path.as_str())),
        serde_json::json!({
            "id": sanitize_sensitive_text(field.id.as_str()),
            "path": sanitize_sensitive_text(field.path.as_str()),
            "value": value,
            "meta": {
                "dirty": field.meta.dirty,
                "touched": field.meta.touched,
                "visited": field.meta.visited,
                "validating": field.meta.validating,
                "errors": &field.meta.errors,
            },
        }),
    )
}

/// Derives the DevTools redaction summary from a form snapshot.
pub fn form_redaction_summary(snapshot: &FormSnapshot) -> SnapshotRedactionSummary {
    let mut redaction = SnapshotRedactionSummary::default();
    for field in &snapshot.fields {
        record_field_value_redaction(field, &mut redaction);
    }
    redaction
}

fn form_tree_and_redaction(snapshot: &FormSnapshot) -> (SnapshotTree, SnapshotRedactionSummary) {
    let mut redaction = SnapshotRedactionSummary::default();
    let mut root = snapshot_node_with_payload(
        ["form"],
        "Form",
        serde_json::json!({
            "status": &snapshot.status,
            "field_count": snapshot.fields.len(),
            "errors": &snapshot.errors,
            "submit_count": snapshot.submit_count,
        }),
    );

    for field in &snapshot.fields {
        root = root.with_child(field_snapshot_node(field, &mut redaction));
    }

    (SnapshotTree::new([root]), redaction)
}

fn field_value_payload(
    field: &FieldSnapshot,
    redaction: &mut SnapshotRedactionSummary,
) -> serde_json::Value {
    match &field.value {
        RedactedValue::Redacted => {
            record_field_value_redaction(field, redaction);
            serde_json::json!({ "kind": "redacted" })
        }
        RedactedValue::Summary(summary) => summary_payload(serde_json::json!({
            "kind": "summary",
            "summary": summary,
        })),
        RedactedValue::Json(value) => serde_json::json!({
            "kind": "json",
            "value": value,
        }),
    }
}

fn record_field_value_redaction(field: &FieldSnapshot, redaction: &mut SnapshotRedactionSummary) {
    if matches!(field.value, RedactedValue::Redacted) {
        redaction.record_redacted(format!("form field {}", field.path.as_str()));
    }
}
