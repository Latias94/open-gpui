use open_gpui_devtools::{
    DevtoolsCapture, DevtoolsDomainId, DevtoolsDomainKind, DevtoolsDomainSnapshot,
    DevtoolsEventKind, DevtoolsEventRecord, DevtoolsEventRecorder, DevtoolsInspectorDetailKind,
    DevtoolsInspectorError, DevtoolsInspectorState, DevtoolsSnapshotCategory, DevtoolsTargetId,
    DevtoolsTargetKind, DevtoolsTargetSnapshot, DevtoolsTargetTree, ProbeId, SnapshotCollection,
    SnapshotDiagnostic, SnapshotEnvelope, SnapshotKind, SnapshotNode, SnapshotRedactionSummary,
    SnapshotTree,
};

#[test]
fn inspector_projects_snapshots_into_filterable_rows() {
    let state = DevtoolsInspectorState::new(collection()).with_filter("form");
    let rows = state.snapshot_rows();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].probe_id.as_str(), "form");
    assert_eq!(rows[0].category, DevtoolsSnapshotCategory::Data);
    assert_eq!(rows[0].category_label, "data");
    assert_eq!(rows[0].kind_label, "form");
    assert_eq!(rows[0].root_nodes, 1);
    assert_eq!(rows[0].total_nodes, 2);
    assert_eq!(rows[0].redacted_values, 1);
    assert!(rows[0].selected);
}

#[test]
fn inspector_summarizes_visible_snapshot_categories() {
    let state = DevtoolsInspectorState::new(collection());
    let summaries = state.category_summaries();

    let data = summaries
        .iter()
        .find(|summary| summary.category == DevtoolsSnapshotCategory::Data)
        .expect("data category summary");
    let diagnostic = summaries
        .iter()
        .find(|summary| summary.category == DevtoolsSnapshotCategory::Diagnostic)
        .expect("diagnostic category summary");

    assert_eq!(data.category_label, "data");
    assert_eq!(data.snapshot_count, 2);
    assert_eq!(data.root_nodes, 2);
    assert_eq!(data.total_nodes, 3);
    assert_eq!(data.redacted_values, 2);
    assert_eq!(data.diagnostics, 0);
    assert_eq!(diagnostic.snapshot_count, 0);
    assert_eq!(diagnostic.diagnostics, 1);
}

#[test]
fn inspector_filter_matches_category_labels_and_moves_selection() {
    let state = DevtoolsInspectorState::new(ecosystem_collection()).with_filter("timeline");
    let rows = state.snapshot_rows();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].probe_id.as_str(), "timeline");
    assert_eq!(rows[0].category, DevtoolsSnapshotCategory::Timeline);
    assert_eq!(rows[0].kind_label, "timeline");
    assert!(rows[0].selected);
    assert_eq!(state.selected_probe_id().unwrap().as_str(), "timeline");
}

#[test]
fn inspector_classifies_command_timeline_layout_and_custom_kinds() {
    let state = DevtoolsInspectorState::new(ecosystem_collection());
    let rows = state.snapshot_rows();

    assert!(rows.iter().any(|row| {
        row.probe_id.as_str() == "command"
            && row.category == DevtoolsSnapshotCategory::Command
            && row.kind_label == "command"
    }));
    assert!(rows.iter().any(|row| {
        row.probe_id.as_str() == "timeline"
            && row.category == DevtoolsSnapshotCategory::Timeline
            && row.kind_label == "timeline"
    }));
    assert!(rows.iter().any(|row| {
        row.probe_id.as_str() == "layout"
            && row.category == DevtoolsSnapshotCategory::Layout
            && row.kind_label == "layout"
    }));
    assert!(rows.iter().any(|row| {
        row.probe_id.as_str() == "custom"
            && row.category == DevtoolsSnapshotCategory::Custom
            && row.category_label == "custom"
            && row.kind_label == "plugin"
    }));
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

#[test]
fn inspector_projects_capture_targets_domains_and_events() {
    let base_capture = DevtoolsCapture::from_snapshot_collection(ecosystem_collection());
    let timeline_probe_id = ProbeId::new("timeline").unwrap();
    let timeline_target_id = DevtoolsTargetId::from_probe_id(&timeline_probe_id);
    let timeline_domain_id =
        DevtoolsDomainId::from_probe_snapshot(&timeline_probe_id, &SnapshotKind::Timeline);
    let mut recorder = DevtoolsEventRecorder::with_capacity(8);
    recorder.record(
        DevtoolsEventRecord::new(
            "timeline.frame",
            "Timeline frame",
            DevtoolsEventKind::Instant,
        )
        .target_id(timeline_target_id.clone())
        .domain_id(timeline_domain_id.clone())
        .timestamp_ms(42),
    );
    let event_batch = recorder.snapshot();
    let capture = DevtoolsCapture::new(
        base_capture.targets,
        base_capture.domains,
        event_batch.events,
        base_capture.snapshots,
        base_capture.diagnostics,
    );

    let state = DevtoolsInspectorState::from_capture(capture).with_filter("frame");

    assert_eq!(state.selected_target_id().unwrap(), &timeline_target_id);
    assert_eq!(state.selected_domain_id().unwrap(), &timeline_domain_id);
    assert_eq!(state.selected_event_sequence(), Some(0));
    assert_eq!(
        state
            .target_rows()
            .iter()
            .map(|row| row.target_id.as_str())
            .collect::<Vec<_>>(),
        ["probe.timeline"]
    );
    assert_eq!(state.domain_rows()[0].kind_label, "timeline");
    assert!(state.domain_rows()[0].has_snapshot);
    assert_eq!(state.domain_rows()[0].snapshot_root_nodes, 1);
    assert_eq!(state.domain_rows()[0].event_count, 1);
    assert_eq!(state.event_rows()[0].event_id, "timeline.frame");
    assert_eq!(state.event_rows()[0].timestamp_ms, Some(42));
    assert!(state.event_rows()[0].selected);
    let detail = state.selected_detail().expect("selected detail");
    assert_eq!(detail.kind, DevtoolsInspectorDetailKind::DomainSnapshot);
    assert_eq!(detail.copy_label, "Copy selected detail JSON");
    assert_eq!(detail.export_label, "Export selected detail JSON");
    assert_eq!(detail.feedback_label, "Selected detail JSON is ready");
    assert_eq!(
        state.selected_detail_json().unwrap()["probe_id"],
        "timeline"
    );
}

#[test]
fn inspector_empty_capture_has_no_selected_detail() {
    let state = DevtoolsInspectorState::from_capture(DevtoolsCapture::default());

    assert!(state.target_rows().is_empty());
    assert!(state.domain_rows().is_empty());
    assert!(state.event_rows().is_empty());
    assert!(state.selected_detail().is_none());
    assert!(matches!(
        state.selected_detail_json(),
        Err(DevtoolsInspectorError::NoSelectedDetail)
    ));
}

#[test]
fn inspector_selects_targets_domains_and_event_only_detail() {
    let target_id = DevtoolsTargetId::from_parts(["runtime", "commands"]);
    let domain_id = DevtoolsDomainId::from_parts(["runtime", "commands", "events"]);
    let target = DevtoolsTargetSnapshot::new(
        target_id.clone(),
        DevtoolsTargetKind::Runtime,
        "Command runtime",
    );
    let domain = DevtoolsDomainSnapshot::new(
        domain_id.clone(),
        target_id.clone(),
        DevtoolsDomainKind::Command,
        "Command events",
    );
    let event = DevtoolsEventRecord::new(
        "command.dispatch",
        "Command dispatched",
        DevtoolsEventKind::Instant,
    )
    .target_id(target_id.clone())
    .domain_id(domain_id.clone())
    .with_payload(serde_json::json!({ "command": "workspace.open" }));
    let capture = DevtoolsCapture::new(
        DevtoolsTargetTree::new([target]),
        [domain],
        [event],
        Vec::new(),
        Vec::new(),
    );

    let state = DevtoolsInspectorState::from_capture(capture)
        .select_target(&target_id)
        .unwrap()
        .select_domain(&domain_id)
        .unwrap()
        .select_event(0)
        .unwrap();

    assert_eq!(&state.selected_target().unwrap().id, &target_id);
    assert_eq!(&state.selected_domain().unwrap().id, &domain_id);
    assert_eq!(state.selected_event().unwrap().id(), "command.dispatch");
    assert_eq!(state.target_rows()[0].domain_count, 1);
    assert_eq!(state.target_rows()[0].event_count, 1);
    assert!(state.event_rows()[0].has_payload);
    let detail = state.selected_detail().expect("selected event detail");
    assert_eq!(detail.kind, DevtoolsInspectorDetailKind::Event);
    assert_eq!(detail.json["id"], "command.dispatch");
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

fn ecosystem_collection() -> SnapshotCollection {
    SnapshotCollection {
        snapshots: vec![
            SnapshotEnvelope::new(
                ProbeId::new("command").unwrap(),
                SnapshotKind::Command,
                SnapshotTree::new([SnapshotNode::new("command.registry", "Command registry")]),
            ),
            SnapshotEnvelope::new(
                ProbeId::new("timeline").unwrap(),
                SnapshotKind::Timeline,
                SnapshotTree::new([SnapshotNode::new("timeline.frame", "Frame event")]),
            ),
            SnapshotEnvelope::new(
                ProbeId::new("layout").unwrap(),
                SnapshotKind::Layout,
                SnapshotTree::new([SnapshotNode::new("layout.root", "Root layout")]),
            ),
            SnapshotEnvelope::new(
                ProbeId::new("custom").unwrap(),
                SnapshotKind::Custom("plugin".to_owned()),
                SnapshotTree::new([SnapshotNode::new("plugin.node", "Plugin node")]),
            ),
        ],
        diagnostics: Vec::new(),
    }
}
