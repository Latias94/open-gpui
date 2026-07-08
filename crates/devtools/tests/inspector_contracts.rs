use open_gpui_devtools::{
    DevtoolsInspectorState, ProbeId, SnapshotCollection, SnapshotDiagnostic, SnapshotEnvelope,
    SnapshotKind, SnapshotNode, SnapshotRedactionSummary, SnapshotTree,
};

#[test]
fn inspector_projects_snapshots_into_filterable_rows() {
    let state = DevtoolsInspectorState::new(collection()).with_filter("form");
    let rows = state.snapshot_rows();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].probe_id.as_str(), "form");
    assert_eq!(rows[0].kind_label, "form");
    assert_eq!(rows[0].root_nodes, 1);
    assert_eq!(rows[0].total_nodes, 2);
    assert_eq!(rows[0].redacted_values, 1);
    assert!(rows[0].selected);
}

#[test]
fn inspector_selection_and_export_do_not_mutate_collection() {
    let state = DevtoolsInspectorState::new(collection());
    let selected = state
        .clone()
        .select_probe(&ProbeId::new("resource").unwrap())
        .unwrap();

    assert_eq!(state.selected_probe_id().unwrap().as_str(), "form");
    assert_eq!(selected.selected_probe_id().unwrap().as_str(), "resource");

    let exported = selected.selected_snapshot_json().unwrap();
    assert_eq!(exported["probe_id"], "resource");
    assert_eq!(exported["redaction"]["redacted_values"], 1);
    assert_eq!(exported["tree"]["nodes"][0]["label"], "Projects");
}

#[test]
fn inspector_surfaces_diagnostics_for_failed_probes() {
    let state = DevtoolsInspectorState::new(collection());

    assert_eq!(state.diagnostics().len(), 1);
    assert_eq!(state.diagnostics()[0].probe_id.as_str(), "motion");
    assert!(
        state.diagnostics()[0]
            .message
            .contains("motion runtime unavailable")
    );
}

fn collection() -> SnapshotCollection {
    SnapshotCollection {
        snapshots: vec![form_snapshot(), resource_snapshot()],
        diagnostics: vec![SnapshotDiagnostic::new(
            ProbeId::new("motion").unwrap(),
            "runtime.unavailable",
            "motion runtime unavailable",
        )],
    }
}

fn form_snapshot() -> SnapshotEnvelope {
    let mut redaction = SnapshotRedactionSummary::default();
    redaction.record_redacted("account.password");
    SnapshotEnvelope::new(
        ProbeId::new("form").unwrap(),
        SnapshotKind::Form,
        SnapshotTree::new([SnapshotNode::new("form", "Profile form").with_child(
            SnapshotNode::new("field:email", "Email")
                .with_payload(serde_json::json!({"dirty": true})),
        )]),
    )
    .with_redaction(redaction)
}

fn resource_snapshot() -> SnapshotEnvelope {
    let mut redaction = SnapshotRedactionSummary::default();
    redaction.record_redacted("projects payload");
    SnapshotEnvelope::new(
        ProbeId::new("resource").unwrap(),
        SnapshotKind::Resource,
        SnapshotTree::new([SnapshotNode::new("projects", "Projects")
            .with_payload(serde_json::json!({"status": "stale"}))]),
    )
    .with_redaction(redaction)
}
