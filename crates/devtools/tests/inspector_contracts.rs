use open_gpui_devtools::{
    DevtoolsCapture, DevtoolsDiffStatus, DevtoolsDomainId, DevtoolsDomainKind,
    DevtoolsDomainSnapshot, DevtoolsEventIdentity, DevtoolsEventKind, DevtoolsEventRecord,
    DevtoolsEventRecorder, DevtoolsInspectorDetailKind, DevtoolsInspectorError,
    DevtoolsInspectorState, DevtoolsRegistry, DevtoolsSession, DevtoolsSnapshotCategory,
    DevtoolsTargetId, DevtoolsTargetKind, DevtoolsTargetSnapshot, DevtoolsTargetTree, ProbeId,
    SnapshotCollection, SnapshotDiagnostic, SnapshotEnvelope, SnapshotKind, SnapshotNode,
    SnapshotRedactionSummary, SnapshotTree,
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

    let copied = selected.copy_selected_detail().unwrap();
    assert_eq!(
        copied.detail_kind,
        DevtoolsInspectorDetailKind::LegacySnapshot
    );
    assert_eq!(copied.action_label, "Copy selected detail JSON");
    assert_eq!(copied.feedback_label, "Selected detail JSON copied");
    assert!(copied.pretty_json.contains("\"probe_id\": \"resource\""));

    let exported_detail = selected.export_selected_detail().unwrap();
    assert_eq!(exported_detail.action_label, "Export selected detail JSON");
    assert_eq!(
        exported_detail.feedback_label,
        "Selected detail JSON exported"
    );
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
    assert_eq!(
        state
            .selected_event_identity()
            .map(|identity| identity.sequence),
        Some(0)
    );
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
    assert_eq!(
        state.active_detail_kind(),
        Some(DevtoolsInspectorDetailKind::DomainSnapshot)
    );
    assert_eq!(detail.copy_label, "Copy selected detail JSON");
    assert_eq!(detail.export_label, "Export selected detail JSON");
    assert_eq!(detail.feedback_label, "Selected detail JSON is ready");
    assert_eq!(
        state.selected_detail_json().unwrap()["probe_id"],
        "timeline"
    );
}

#[test]
fn inspector_event_selection_overrides_domain_snapshot_detail() {
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
        .target_id(timeline_target_id)
        .domain_id(timeline_domain_id)
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

    let event_identity = capture.events[0].identity();
    let state = DevtoolsInspectorState::from_capture(capture)
        .select_event_identity(&event_identity)
        .unwrap();
    let detail = state.selected_detail().expect("selected event detail");

    assert_eq!(
        state.active_detail_kind(),
        Some(DevtoolsInspectorDetailKind::Event)
    );
    assert_eq!(detail.kind, DevtoolsInspectorDetailKind::Event);
    assert_eq!(detail.json["id"], "timeline.frame");
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
fn inspector_selection_commands_move_visible_rows_and_clear_filter() {
    let state = DevtoolsInspectorState::from_capture(DevtoolsCapture::from_snapshot_collection(
        ecosystem_collection(),
    ));

    assert_eq!(
        state.selected_target_id().unwrap().as_str(),
        "probe.command"
    );

    let state = state.select_next_target().unwrap();
    assert_eq!(
        state.selected_target_id().unwrap().as_str(),
        "probe.timeline"
    );

    let state = state.select_previous_target().unwrap();
    assert_eq!(
        state.selected_target_id().unwrap().as_str(),
        "probe.command"
    );

    let filtered = state.with_filter("timeline");
    assert_eq!(filtered.filter(), "timeline");
    assert_eq!(filtered.target_rows().len(), 1);
    assert_eq!(
        filtered.selected_target_id().unwrap().as_str(),
        "probe.timeline"
    );

    let unfiltered = filtered.clear_filter();
    assert_eq!(unfiltered.filter(), "");
    assert_eq!(unfiltered.target_rows().len(), 5);
}

#[test]
fn inspector_export_capture_returns_sanitized_whole_capture_json() {
    let state = DevtoolsInspectorState::from_capture(DevtoolsCapture::from_snapshot_collection(
        collection(),
    ));

    let exported = state.export_capture().unwrap();
    let serialized = exported.pretty_json.clone();

    assert_eq!(exported.label, "DevTools capture JSON");
    assert_eq!(exported.feedback_label, "DevTools capture JSON exported");
    assert!(exported.json["targets"].is_object());
    assert!(serialized.contains("\"diagnostics\""));
    assert!(!serialized.contains("raw-password"));
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

    let event_identity = capture.events[0].identity();
    let state = DevtoolsInspectorState::from_capture(capture)
        .select_target(&target_id)
        .unwrap()
        .select_domain(&domain_id)
        .unwrap()
        .select_event_identity(&event_identity)
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

#[test]
fn inspector_projects_session_frame_and_diff_rows() {
    let mut session = DevtoolsSession::new("gallery.session", registry_for_values([1, 2]));
    session.refresh().unwrap();
    let frame = session.refresh().unwrap();

    let state = DevtoolsInspectorState::from_session_frame(frame);

    let session_frame = state.session_frame().expect("session frame summary");
    assert_eq!(session_frame.session_id, "gallery.session");
    assert_eq!(session_frame.generation, 2);
    assert_eq!(session_frame.previous_generation, Some(1));
    assert_eq!(session_frame.diff_row_count, state.diff_rows().len());
    assert!(
        state
            .diff_rows()
            .iter()
            .any(|row| row.status == DevtoolsDiffStatus::Changed)
    );
}

#[test]
fn inspector_replace_session_frame_preserves_filter_and_selection() {
    let mut session = DevtoolsSession::new("replace.session", registry_for_values([1, 2]));
    let first = session.refresh().unwrap();
    let second = session.refresh().unwrap();
    let target_id = DevtoolsTargetId::from_parts(["runtime", "session"]);

    let state = DevtoolsInspectorState::from_session_frame(first)
        .select_target(&target_id)
        .unwrap()
        .with_filter("runtime");
    let replaced = state.replace_session_frame(second);

    assert_eq!(replaced.filter(), "runtime");
    assert_eq!(replaced.selected_target_id(), Some(&target_id));
    assert_eq!(replaced.session_frame().unwrap().generation, 2);
    assert!(!replaced.diff_rows().is_empty());
}

#[test]
fn inspector_replace_capture_degrades_missing_selection_to_visible_target() {
    let removed_id = DevtoolsTargetId::new("runtime.removed");
    let next_id = DevtoolsTargetId::new("runtime.next");
    let state = DevtoolsInspectorState::from_capture(capture_for_target(&removed_id, 1))
        .select_target(&removed_id)
        .unwrap();

    let replaced = state.replace_capture(capture_for_target(&next_id, 2));

    assert_eq!(replaced.selected_target_id(), Some(&next_id));
    assert_eq!(replaced.selected_domain().unwrap().target_id, next_id);
}

#[test]
fn inspector_event_identity_survives_cross_scope_sequence_collisions() {
    let target_id = DevtoolsTargetId::new("runtime.events");
    let capture = event_identity_capture(&target_id, ["scope.a", "scope.b"]);
    let identity_b = capture.events[1].identity();
    let state = DevtoolsInspectorState::from_capture(capture)
        .select_event_identity(&identity_b)
        .unwrap();

    let replaced =
        state.replace_capture(event_identity_capture(&target_id, ["scope.b", "scope.a"]));

    assert_eq!(
        replaced.selected_event().unwrap().scope_id_ref(),
        Some("scope.b")
    );
    assert_eq!(replaced.selected_event_identity(), Some(&identity_b));
}

#[test]
fn inspector_treats_new_recorder_sequence_as_new_event_instance() {
    let target_id = DevtoolsTargetId::new("runtime.events");
    let first_capture = logical_event_instance_capture(&target_id, 0);
    let first_identity = first_capture.events[0].identity();
    let state = DevtoolsInspectorState::from_capture(first_capture)
        .select_event_identity(&first_identity)
        .unwrap();

    let replaced = state.replace_capture(logical_event_instance_capture(&target_id, 1));
    let selected_identity = replaced
        .selected_event_identity()
        .expect("replacement should select visible logical event as a new instance");

    assert_ne!(selected_identity, &first_identity);
    assert_eq!(selected_identity.scope_id, first_identity.scope_id);
    assert_eq!(selected_identity.event_id, first_identity.event_id);
    assert_eq!(selected_identity.sequence, 1);
}

#[test]
fn event_identity_key_sanitizes_sensitive_and_selector_fragments() {
    let identity = DevtoolsEventIdentity::new(
        "alice@example.com /Users/alice/project",
        7,
        "deploy token=secret value #row [item]",
    );
    let key = identity.as_key();

    assert!(!key.contains("alice@example.com"));
    assert!(!key.contains("/Users/alice"));
    assert!(!key.contains("secret"));
    assert!(!key.contains(' '));
    assert!(!key.contains('#'));
    assert!(!key.contains('['));
    assert!(!key.contains(']'));
    assert!(key.contains("7"));
    assert!(key.contains("deploy"));
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

fn registry_for_values(values: [usize; 2]) -> DevtoolsRegistry {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    let values = Arc::new(values);
    let index = Arc::new(AtomicUsize::new(0));
    let provider_values = Arc::clone(&values);
    let provider_index = Arc::clone(&index);
    let mut registry = DevtoolsRegistry::default();
    registry
        .register_capture_provider_fn("provider.session", move || {
            let index = provider_index
                .fetch_add(1, Ordering::SeqCst)
                .min(provider_values.len() - 1);
            Ok(capture_for_value(provider_values[index]))
        })
        .unwrap();
    registry
}

fn capture_for_value(value: usize) -> DevtoolsCapture {
    let target_id = DevtoolsTargetId::from_parts(["runtime", "session"]);
    capture_for_target(&target_id, value)
}

fn capture_for_target(target_id: &DevtoolsTargetId, value: usize) -> DevtoolsCapture {
    let domain_id = DevtoolsDomainId::from_parts(["runtime", target_id.as_str(), "state"]);
    let target = DevtoolsTargetSnapshot::new(
        target_id.clone(),
        DevtoolsTargetKind::Runtime,
        format!("Runtime {value}"),
    );
    let domain = DevtoolsDomainSnapshot::new(
        domain_id.clone(),
        target_id.clone(),
        DevtoolsDomainKind::Diagnostic,
        "Runtime state",
    )
    .with_summary(serde_json::json!({ "value": value }));
    let event = DevtoolsEventRecord::new(
        "runtime.changed",
        "Runtime changed",
        DevtoolsEventKind::Instant,
    )
    .scope_id("runtime")
    .target_id(target_id.clone())
    .domain_id(domain_id)
    .with_payload(serde_json::json!({ "value": value }));

    DevtoolsCapture::new(
        DevtoolsTargetTree::new([target]),
        [domain],
        [event],
        Vec::<SnapshotEnvelope>::new(),
        Vec::new(),
    )
}

fn event_identity_capture(target_id: &DevtoolsTargetId, scopes: [&str; 2]) -> DevtoolsCapture {
    let domain_id = DevtoolsDomainId::from_parts(["runtime", "events"]);
    let target = DevtoolsTargetSnapshot::new(
        target_id.clone(),
        DevtoolsTargetKind::Runtime,
        "Runtime events",
    );
    let domain = DevtoolsDomainSnapshot::new(
        domain_id.clone(),
        target_id.clone(),
        DevtoolsDomainKind::Timeline,
        "Runtime timeline",
    );
    let events = scopes.map(|scope| {
        DevtoolsEventRecord::new("refresh", "Refresh", DevtoolsEventKind::Instant)
            .scope_id(scope)
            .target_id(target_id.clone())
            .domain_id(domain_id.clone())
    });

    DevtoolsCapture::new(
        DevtoolsTargetTree::new([target]),
        [domain],
        events,
        Vec::<SnapshotEnvelope>::new(),
        Vec::new(),
    )
}

fn logical_event_instance_capture(target_id: &DevtoolsTargetId, sequence: u64) -> DevtoolsCapture {
    let domain_id = DevtoolsDomainId::from_parts(["runtime", "events"]);
    let target = DevtoolsTargetSnapshot::new(
        target_id.clone(),
        DevtoolsTargetKind::Runtime,
        "Runtime events",
    );
    let domain = DevtoolsDomainSnapshot::new(
        domain_id.clone(),
        target_id.clone(),
        DevtoolsDomainKind::Timeline,
        "Runtime timeline",
    );
    let mut recorder = DevtoolsEventRecorder::new("scope.logical", "Logical scope", 8);
    for index in 0..sequence {
        recorder.record(DevtoolsEventRecord::new(
            format!("warmup.{index}"),
            "Warmup",
            DevtoolsEventKind::Instant,
        ));
    }
    recorder.record(
        DevtoolsEventRecord::new("refresh", "Refresh", DevtoolsEventKind::Instant)
            .target_id(target_id.clone())
            .domain_id(domain_id.clone()),
    );
    let events = recorder
        .snapshot()
        .events
        .into_iter()
        .filter(|event| event.id() == "refresh")
        .collect::<Vec<_>>();

    DevtoolsCapture::new(
        DevtoolsTargetTree::new([target]),
        [domain],
        events,
        Vec::<SnapshotEnvelope>::new(),
        Vec::new(),
    )
}
