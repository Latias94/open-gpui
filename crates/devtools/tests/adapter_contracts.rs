use open_gpui_devtools::adapters::{
    opaque_stable_id, sanitize_sensitive_text, snapshot_node_with_payload, stable_node_id,
    summary_payload,
};
use open_gpui_devtools::{
    DevtoolsInspectorState, DevtoolsRegistry, DevtoolsRegistryError, ProbeId, SnapshotCollection,
    SnapshotDiagnostic, SnapshotEnvelope, SnapshotKind, SnapshotNode, SnapshotProbeSnapshot,
    SnapshotRedactionSummary, SnapshotTree,
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
    assert!(!serialized.contains("alice@example.com"), "{serialized}");
    assert!(!serialized.contains("raw-secret"), "{serialized}");
    assert!(!serialized.contains("Frank"), "{serialized}");
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
fn sanitizer_continues_after_invalid_email_candidates() {
    let sanitized = sanitize_sensitive_text("bad@token owner alice@example.com");

    assert!(sanitized.contains("bad@token"));
    assert!(!sanitized.contains("alice@example.com"));
    assert!(sanitized.contains("[redacted-email]"));
}

#[test]
fn sanitizer_redacts_separated_sensitive_assignments() {
    let sanitized = sanitize_sensitive_text(
        "api_key: raw-secret password = raw-password bearer : raw-bearer token raw-token",
    );

    assert!(!sanitized.contains("raw-secret"));
    assert!(!sanitized.contains("raw-password"));
    assert!(!sanitized.contains("raw-bearer"));
    assert!(!sanitized.contains("raw-token"));
    assert!(sanitized.contains("api_key:[redacted]"));
    assert!(sanitized.contains("password=[redacted]"));
    assert!(sanitized.contains("bearer [redacted]"));
    assert!(sanitized.contains("token [redacted]"));
}

#[test]
fn probe_ids_and_custom_kind_labels_are_sanitized_for_exports() {
    let probe_id = ProbeId::new("owner alice@example.com").unwrap();
    let envelope = SnapshotEnvelope::new(
        probe_id,
        SnapshotKind::Custom("token=raw-secret".to_owned()),
        SnapshotTree::default(),
    );
    let row_state =
        open_gpui_devtools::DevtoolsInspectorState::new(open_gpui_devtools::SnapshotCollection {
            snapshots: vec![envelope.clone()],
            diagnostics: Vec::new(),
        });

    let serialized = serde_json::to_string(&envelope).unwrap();

    assert_eq!(envelope.probe_id.as_str(), "owner [redacted-email]");
    assert_eq!(envelope.kind.as_label(), "token=[redacted]");
    assert_eq!(row_state.snapshot_rows()[0].kind_label, "token=[redacted]");
    assert!(!serialized.contains("alice@example.com"));
    assert!(!serialized.contains("raw-secret"));
    assert!(serialized.contains("[redacted"));
}

#[test]
fn summary_payload_sanitizes_nested_values_and_keys() {
    let payload = summary_payload(serde_json::json!({
        "user alice@example.com": {
            "token": "token=raw-secret",
            "api_key": "raw-api-key",
            "url": "https://example.test/search?q=secret",
        }
    }));
    let serialized = serde_json::to_string(&payload).unwrap();

    assert!(!serialized.contains("alice@example.com"));
    assert!(!serialized.contains("raw-secret"));
    assert!(!serialized.contains("raw-api-key"));
    assert!(!serialized.contains("q=secret"));
}

#[test]
fn dto_export_and_inspector_state_sanitize_public_struct_literal_bypasses() {
    let collection = SnapshotCollection {
        snapshots: vec![SnapshotEnvelope {
            probe_id: ProbeId::new("form").unwrap(),
            kind: SnapshotKind::Custom("token=raw-kind".to_owned()),
            tree: SnapshotTree {
                nodes: vec![SnapshotNode {
                    id: "owner alice@example.com".to_owned(),
                    label: "Owner alice@example.com at C:\\Users\\Frank\\profile.json".to_owned(),
                    payload: Some(serde_json::json!({
                        "api_key": "raw-api-key",
                        "callback": "https://example.test/callback?token=raw-query",
                    })),
                    children: vec![SnapshotNode {
                        id: "child-token".to_owned(),
                        label: "password = raw-child-password".to_owned(),
                        payload: None,
                        children: Vec::new(),
                    }],
                }],
            },
            redaction: SnapshotRedactionSummary {
                redacted_values: 1,
                notes: vec!["password = raw-note-password".to_owned()],
            },
        }],
        diagnostics: vec![SnapshotDiagnostic {
            probe_id: ProbeId::new("diagnostic").unwrap(),
            code: "token=raw-code".to_owned(),
            message: "failed for alice@example.com with api_key: raw-diagnostic-key".to_owned(),
        }],
    };

    let state = DevtoolsInspectorState::new(collection);
    let serialized_snapshot =
        serde_json::to_string(&state.selected_snapshot_json().unwrap()).unwrap();
    let serialized_diagnostics = serde_json::to_string(state.diagnostics()).unwrap();

    assert_eq!(state.snapshot_rows()[0].kind_label, "token=[redacted]");
    assert!(
        !state.selected_snapshot().unwrap().tree.nodes[0]
            .label
            .contains("alice@example.com")
    );
    assert!(
        !state.diagnostics()[0]
            .message
            .contains("raw-diagnostic-key")
    );
    assert!(!serialized_snapshot.contains("alice@example.com"));
    assert!(!serialized_snapshot.contains("Frank"));
    assert!(!serialized_snapshot.contains("raw-kind"));
    assert!(!serialized_snapshot.contains("raw-api-key"));
    assert!(!serialized_snapshot.contains("raw-query"));
    assert!(!serialized_snapshot.contains("raw-child-password"));
    assert!(!serialized_snapshot.contains("raw-note-password"));
    assert!(!serialized_diagnostics.contains("alice@example.com"));
    assert!(!serialized_diagnostics.contains("raw-code"));
    assert!(!serialized_diagnostics.contains("raw-diagnostic-key"));
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

#[test]
fn opaque_stable_id_is_deterministic_without_retaining_source_text() {
    let first = opaque_stable_id("form-field", "violet meadow 744");
    let second = opaque_stable_id("form-field", "violet meadow 744");
    let other = opaque_stable_id("form-field", "silver harbor 319");

    assert_eq!(first, second);
    assert_ne!(first, other);
    assert!(first.starts_with("form-field-"));
    assert!(!first.contains("violet"));
    assert!(!first.contains("meadow"));
}

#[test]
fn adapter_payload_module_stays_private_implementation_detail() {
    let source = include_str!("../src/adapters/mod.rs");

    assert!(source.contains("mod payload;"));
    assert!(!source.contains("pub mod payload;"));
}
