use open_gpui_devtools::adapters::{
    sanitize_sensitive_text, snapshot_node_with_payload, stable_node_id, summary_payload,
};
use open_gpui_devtools::{
    DevtoolsRegistry, DevtoolsRegistryError, ProbeId, SnapshotDiagnostic, SnapshotEnvelope,
    SnapshotKind, SnapshotNode, SnapshotProbeSnapshot, SnapshotRedactionSummary, SnapshotTree,
};

#[test]
fn adapter_node_helpers_match_manual_snapshot_shape() {
    let helper = snapshot_node_with_payload(
        ["form", "field", "account.email"],
        "Email",
        serde_json::json!({ "dirty": true, "invalid": false }),
    );
    let manual = SnapshotNode::new("form.field.account.email", "Email")
        .with_payload(serde_json::json!({ "dirty": true, "invalid": false }));

    assert_eq!(helper, manual);
}

#[test]
fn adapter_helpers_sanitize_ids_labels_and_payloads() {
    let node = snapshot_node_with_payload(
        ["field", "alice@example.com", "C:\\Users\\Frank\\token.txt"],
        "Owner alice@example.com at C:\\Users\\Frank\\token.txt",
        serde_json::json!({
            "owner alice@example.com": "alice@example.com",
            "callback": "https://example.test/callback?token=raw-secret",
            "path": "C:\\Users\\Frank\\token.txt",
        }),
    );

    let serialized = serde_json::to_string(&node).unwrap();
    assert!(node.id.contains("redacted-email"));
    assert!(node.id.contains("redacted-path"));
    assert!(!serialized.contains("alice@example.com"));
    assert!(!serialized.contains("Frank"));
    assert!(!serialized.contains("raw-secret"));
}

#[test]
fn redaction_summaries_record_merge_and_sanitize_notes() {
    let mut first = SnapshotRedactionSummary::default();
    first.record_redacted("account.email alice@example.com");

    let mut second = SnapshotRedactionSummary::default();
    second.record_redacted("payload token=raw-secret C:\\Users\\Frank\\token.txt");

    let merged = first.merged(second);
    let serialized = serde_json::to_string(&merged).unwrap();

    assert_eq!(merged.redacted_values, 2);
    assert_eq!(merged.notes.len(), 2);
    assert!(!serialized.contains("alice@example.com"));
    assert!(!serialized.contains("raw-secret"));
    assert!(!serialized.contains("Frank"));
}

#[test]
fn diagnostics_strip_sensitive_fragments_and_keep_stable_codes() {
    let diagnostic = SnapshotDiagnostic::collection_failed(
        ProbeId::new("resource").unwrap(),
        "failed for alice@example.com with Bearer raw-secret at C:\\Users\\Frank\\data.json?token=secret",
    );

    assert_eq!(diagnostic.code, SnapshotDiagnostic::COLLECTION_FAILED);
    assert_eq!(
        sanitize_sensitive_text("token=raw-secret"),
        "token=[redacted]"
    );
    assert!(!diagnostic.message.contains("alice@example.com"));
    assert!(!diagnostic.message.contains("raw-secret"));
    assert!(!diagnostic.message.contains("Frank"));
    assert!(diagnostic.message.contains("[redacted"));
}

#[test]
fn summary_payload_sanitizes_nested_values_and_keys() {
    let payload = summary_payload(serde_json::json!({
        "user alice@example.com": {
            "token": "token=raw-secret",
            "url": "https://example.test/search?q=secret",
        }
    }));
    let serialized = serde_json::to_string(&payload).unwrap();

    assert!(!serialized.contains("alice@example.com"));
    assert!(!serialized.contains("raw-secret"));
    assert!(!serialized.contains("q=secret"));
}

#[test]
fn empty_and_diagnostic_trees_remain_serializable() {
    let empty = SnapshotEnvelope::new(
        ProbeId::new("empty").unwrap(),
        SnapshotKind::Custom("empty".to_owned()),
        SnapshotTree::default(),
    );
    let diagnostic = SnapshotEnvelope::new(
        ProbeId::new("diagnostic").unwrap(),
        SnapshotKind::Diagnostic,
        SnapshotTree::new([SnapshotNode::new("diagnostic", "No public facts")]),
    );

    assert_eq!(
        serde_json::to_value(empty).unwrap()["tree"]["nodes"],
        serde_json::json!([])
    );
    assert_eq!(
        serde_json::to_value(diagnostic).unwrap()["kind"],
        "Diagnostic"
    );
}

#[test]
fn duplicate_snapshot_probe_ids_still_fail() {
    let mut registry = DevtoolsRegistry::default();
    registry
        .register_snapshot_probe("form", SnapshotKind::Form, || {
            Ok(SnapshotProbeSnapshot::new(SnapshotTree::default()))
        })
        .unwrap();

    let duplicate = registry.register_snapshot_probe("form", SnapshotKind::Form, || {
        Ok(SnapshotProbeSnapshot::new(SnapshotTree::default()))
    });

    assert!(matches!(
        duplicate,
        Err(DevtoolsRegistryError::DuplicateProbe(id)) if id.as_str() == "form"
    ));
}

#[test]
fn stable_node_id_uses_fallback_for_empty_segments() {
    assert_eq!(stable_node_id(["", "  "]), "node");
}
