use super::*;

#[test]
fn devtools_gallery_collects_registry_backed_snapshots() {
    let collection = pages::devtools::devtools_gallery_collection();
    let probe_ids = collection
        .snapshots
        .iter()
        .map(|snapshot| snapshot.probe_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        probe_ids,
        [
            "accessibility",
            "command.keybindings",
            "command.keymap",
            "command.registry",
            "form",
            "layout.scroll-viewport",
            "motion",
            "resource",
            "theme",
            "timeline.motion-frame"
        ]
    );
    assert_eq!(
        collection
            .snapshots
            .iter()
            .filter(|snapshot| snapshot.kind.as_label() == "command")
            .count(),
        3
    );
    assert_eq!(
        collection
            .snapshots
            .iter()
            .find(|snapshot| snapshot.probe_id.as_str() == "form")
            .unwrap()
            .redaction
            .redacted_values,
        5
    );
    assert_eq!(
        collection
            .snapshots
            .iter()
            .find(|snapshot| snapshot.probe_id.as_str() == "resource")
            .unwrap()
            .redaction
            .redacted_values,
        2
    );
    assert_eq!(collection.diagnostics.len(), 2);
}

#[test]
fn devtools_gallery_capture_projects_targets_domains_and_events() {
    let capture = pages::devtools::devtools_gallery_capture();
    let target_ids = capture
        .targets
        .targets
        .iter()
        .map(|target| target.id.as_str())
        .collect::<Vec<_>>();
    let domain_labels = capture
        .domains
        .iter()
        .map(|domain| domain.kind.as_label())
        .collect::<Vec<_>>();

    assert!(target_ids.contains(&"app"));
    assert!(target_ids.contains(&"probe.command.registry"));
    assert!(target_ids.contains(&"probe.layout.scroll-viewport"));
    assert!(domain_labels.contains(&"command"));
    assert!(domain_labels.contains(&"layout"));
    assert!(domain_labels.contains(&"timeline"));
    assert!(
        capture
            .events
            .iter()
            .any(|event| event.id() == "gallery.motion-frame-demand")
    );
    assert!(
        capture
            .events
            .iter()
            .any(|event| event.scope_id_ref() == Some("gallery.devtools"))
    );
    assert!(
        !capture
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.starts_with("capture.duplicate_"))
    );
    assert_eq!(capture.snapshot_collection().snapshots.len(), 10);
}

#[test]
fn devtools_gallery_snapshots_reflect_component_sample_state() {
    let collection = pages::devtools::devtools_gallery_collection();
    let form = collection
        .snapshots
        .iter()
        .find(|snapshot| snapshot.probe_id.as_str() == "form")
        .expect("form snapshot");
    let resource = collection
        .snapshots
        .iter()
        .find(|snapshot| snapshot.probe_id.as_str() == "resource")
        .expect("resource snapshot");
    let form_json = serde_json::to_string(form).unwrap();
    let resource_json = serde_json::to_string(resource).unwrap();

    assert!(form_json.contains("SubmitFailed"));
    assert!(form_json.contains("submit_count"));
    assert!(resource_json.contains("Refetching"));
    assert!(resource_json.contains("Success"));
    assert!(resource_json.contains("observer_count"));
    assert!(
        collection
            .snapshots
            .iter()
            .any(|snapshot| snapshot.probe_id.as_str() == "command.registry")
    );
    assert!(
        collection
            .snapshots
            .iter()
            .any(|snapshot| snapshot.probe_id.as_str() == "command.keybindings")
    );
    assert!(
        collection
            .snapshots
            .iter()
            .any(|snapshot| snapshot.probe_id.as_str() == "command.keymap")
    );
    assert!(
        collection
            .snapshots
            .iter()
            .any(|snapshot| snapshot.probe_id.as_str() == "layout.scroll-viewport")
    );
    assert!(
        collection
            .snapshots
            .iter()
            .any(|snapshot| snapshot.probe_id.as_str() == "theme")
    );
    assert!(
        collection
            .snapshots
            .iter()
            .any(|snapshot| snapshot.probe_id.as_str() == "accessibility")
    );
    assert!(
        collection
            .snapshots
            .iter()
            .any(|snapshot| snapshot.probe_id.as_str() == "motion")
    );
    assert!(
        collection
            .snapshots
            .iter()
            .any(|snapshot| snapshot.probe_id.as_str() == "timeline.motion-frame")
    );
    assert!(!form_json.contains("gallery-secret"));
    assert!(!resource_json.contains("gallery-secret"));
}

#[test]
fn devtools_gallery_layout_snapshot_reflects_scroll_viewport_geometry() {
    let collection = pages::devtools::devtools_gallery_collection();
    let layout = collection
        .snapshots
        .iter()
        .find(|snapshot| snapshot.probe_id.as_str() == "layout.scroll-viewport")
        .expect("scroll layout snapshot");
    let layout_json = serde_json::to_string(layout).unwrap();

    assert_eq!(layout.kind.as_label(), "layout");
    assert!(layout_json.contains("Scroll viewport layout"));
    assert!(layout_json.contains("initial-layout"));
    assert!(layout_json.contains("\"generation\":42"));
    assert!(layout_json.contains("\"width\":640.0"));
    assert!(layout_json.contains("\"x\":8.0"));
    assert!(!layout_json.contains("InitialLayout"));
}

#[test]
fn devtools_gallery_command_snapshots_reflect_command_runtime_facts() {
    let collection = pages::devtools::devtools_gallery_collection();
    let command_json = collection
        .snapshots
        .iter()
        .filter(|snapshot| snapshot.kind.as_label() == "command")
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .join("\n");

    assert!(command_json.contains("Command registry"));
    assert!(command_json.contains("gallery.command_palette.open"));
    assert!(command_json.contains("\"conflict_count\":1"));
    assert!(command_json.contains("\"diagnostic_count\":2"));
    assert!(command_json.contains("invalid-context"));
    assert!(command_json.contains("missing-action"));
    assert!(command_json.contains("\"pending\":true"));
    assert!(command_json.contains("Pending commands"));
}

#[test]
fn devtools_gallery_timeline_snapshot_reflects_motion_frame_demand() {
    let collection = pages::devtools::devtools_gallery_collection();
    let timeline = collection
        .snapshots
        .iter()
        .find(|snapshot| snapshot.probe_id.as_str() == "timeline.motion-frame")
        .expect("motion timeline snapshot");
    let timeline_json = serde_json::to_string(timeline).unwrap();

    assert_eq!(timeline.kind.as_label(), "timeline");
    assert!(timeline_json.contains("Motion frame demand"));
    assert!(timeline_json.contains("\"needs_frame\":true"));
    assert!(timeline_json.contains("update-render"));
    assert!(!timeline_json.contains("UpdateRender"));
}

#[test]
fn devtools_gallery_reports_unmounted_framework_diagnostics() {
    let state = pages::devtools::devtools_gallery_state();
    let diagnostics = state.diagnostics();

    assert_eq!(diagnostics.len(), 2);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "runtime.unavailable")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.probe_id.as_str() == "scroll")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.probe_id.as_str() == "docking")
    );
}

#[test]
fn devtools_gallery_does_not_keep_static_demo_snapshot_builders() {
    let source = include_str!("../../src/pages/devtools.rs");

    assert!(!source.contains("fn theme_snapshot"));
    assert!(!source.contains("fn form_snapshot"));
    assert!(!source.contains("fn resource_snapshot"));
    assert!(!source.contains("fn docking_snapshot"));
    assert!(source.contains("DevtoolsCapture::from_snapshot_collection"));
    assert!(source.contains("DevtoolsRegistry::default()"));
    assert!(source.contains("register_capture_provider_fn"));
}

#[open_gpui::test]
fn devtools_gallery_smoke_clicks_inspector_rows_and_actions(cx: &mut open_gpui::TestAppContext) {
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Devtools);
    let inspector = cx.update(|_, app| shell.read(app).devtools_inspector().clone());

    assert!(
        cx.debug_bounds("devtools-inspector:gallery-devtools-inspector:root")
            .is_some(),
        "expected the stateful DevTools inspector controller to render on the gallery page"
    );

    scroll_page_selector_into_view(&shell, cx, "devtools-inspector:row:resource");
    click(cx, "devtools-inspector:row:resource");
    let (selected_probe, active_detail_kind, feedback) = cx.update(|_, app| {
        let inspector = inspector.read(app);
        (
            inspector
                .state()
                .selected_probe_id()
                .map(|probe_id| probe_id.as_str().to_owned()),
            inspector.state().active_detail_kind(),
            inspector.feedback_label().map(ToString::to_string),
        )
    });
    assert_eq!(selected_probe.as_deref(), Some("resource"));
    assert_eq!(
        active_detail_kind,
        Some(open_gpui_devtools::DevtoolsInspectorDetailKind::LegacySnapshot)
    );
    assert_eq!(
        feedback.as_deref(),
        Some("Selected snapshot resource"),
        "expected clicking a legacy snapshot row to update controller feedback"
    );

    scroll_page_selector_into_view(&shell, cx, "devtools-inspector:copy-detail");
    click(cx, "devtools-inspector:copy-detail");
    let feedback = cx.update(|_, app| {
        inspector
            .read(app)
            .feedback_label()
            .map(ToString::to_string)
    });
    assert_eq!(feedback.as_deref(), Some("Selected detail JSON copied"));

    scroll_page_selector_into_view(&shell, cx, "devtools-inspector:target:probe.form");
    click(cx, "devtools-inspector:target:probe.form");
    let (selected_target, active_detail_kind, feedback) = cx.update(|_, app| {
        let inspector = inspector.read(app);
        (
            inspector
                .state()
                .selected_target_id()
                .map(|target_id| target_id.as_str().to_owned()),
            inspector.state().active_detail_kind(),
            inspector.feedback_label().map(ToString::to_string),
        )
    });
    assert_eq!(selected_target.as_deref(), Some("probe.form"));
    assert_eq!(
        active_detail_kind,
        Some(open_gpui_devtools::DevtoolsInspectorDetailKind::DomainSnapshot)
    );
    assert_eq!(
        feedback.as_deref(),
        Some("Selected target probe.form"),
        "expected target row click feedback before selecting its visible domain"
    );

    let form_domain_selector = cx.update(|_, app| {
        inspector
            .read(app)
            .state()
            .domain_rows()
            .into_iter()
            .find(|row| row.label == "form")
            .map(|row| format!("devtools-inspector:domain:{}", row.domain_id.as_str()))
            .expect("expected form domain row")
    });
    scroll_page_selector_into_view(&shell, cx, &form_domain_selector);
    click(cx, &form_domain_selector);
    let (selected_domain, active_detail_kind, feedback) = cx.update(|_, app| {
        let inspector = inspector.read(app);
        (
            inspector
                .state()
                .selected_domain_id()
                .map(|domain_id| domain_id.as_str().to_owned()),
            inspector.state().active_detail_kind(),
            inspector.feedback_label().map(ToString::to_string),
        )
    });
    assert!(
        selected_domain
            .as_deref()
            .is_some_and(|domain_id| domain_id.contains("form")),
        "expected clicking the form domain row to select the form domain, got {selected_domain:?}"
    );
    assert_eq!(
        active_detail_kind,
        Some(open_gpui_devtools::DevtoolsInspectorDetailKind::DomainSnapshot)
    );
    assert!(
        feedback
            .as_deref()
            .is_some_and(|label| label.starts_with("Selected domain ")),
        "expected domain click feedback, got {feedback:?}"
    );

    scroll_page_selector_into_view(&shell, cx, "devtools-inspector:event:0");
    click(cx, "devtools-inspector:event:0");
    let (selected_event, active_detail_kind, feedback) = cx.update(|_, app| {
        let inspector = inspector.read(app);
        (
            inspector.state().selected_event_sequence(),
            inspector.state().active_detail_kind(),
            inspector.feedback_label().map(ToString::to_string),
        )
    });
    assert_eq!(selected_event, Some(0));
    assert_eq!(
        active_detail_kind,
        Some(open_gpui_devtools::DevtoolsInspectorDetailKind::Event)
    );
    assert_eq!(feedback.as_deref(), Some("Selected event #0"));

    scroll_page_selector_into_view(&shell, cx, "devtools-inspector:export-capture");
    click(cx, "devtools-inspector:export-capture");
    let feedback = cx.update(|_, app| {
        inspector
            .read(app)
            .feedback_label()
            .map(ToString::to_string)
    });
    assert_eq!(feedback.as_deref(), Some("DevTools capture JSON exported"));
}
