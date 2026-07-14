use super::*;
use open_gpui_devtools::{
    DevtoolsArtifactJsonlSink, DevtoolsArtifactSink, DevtoolsDiffKind, DevtoolsDiffStatus,
    DevtoolsSessionFrame, SnapshotEnvelope,
};

fn assert_devtools_canaries_absent(serialized: &str) {
    for &canary in pages::components::FORM_DEVTOOLS_DOGFOOD_CANARIES
        .iter()
        .chain(pages::focus_a11y::FOCUS_A11Y_SENSITIVE_TEXT)
    {
        assert!(
            !serialized.contains(canary),
            "DevTools canary leaked: {canary}"
        );
    }
}

fn form_snapshot(frame: &DevtoolsSessionFrame) -> &SnapshotEnvelope {
    frame
        .capture
        .snapshots
        .iter()
        .find(|snapshot| snapshot.probe_id.as_str() == "form")
        .expect("form snapshot in DevTools frame")
}

fn assert_form_validation_projection(
    snapshot: &SnapshotEnvelope,
    expected_status: &str,
    expected_validating_field_count: usize,
) {
    let root = snapshot.tree.nodes.first().expect("form projection root");
    let payload = root.payload.as_ref().expect("form projection payload");
    let validating_fields = root
        .children
        .iter()
        .filter(|field| {
            field
                .payload
                .as_ref()
                .and_then(|payload| payload["meta"]["validating"].as_bool())
                == Some(true)
        })
        .count();

    assert_eq!(payload["status"], expected_status);
    assert_eq!(
        payload["validating_field_count"].as_u64(),
        Some(expected_validating_field_count as u64)
    );
    assert_eq!(validating_fields, expected_validating_field_count);
    if expected_validating_field_count == 0 {
        assert!(root.children.iter().all(|field| {
            field
                .payload
                .as_ref()
                .and_then(|payload| payload["meta"]["validating"].as_bool())
                == Some(false)
        }));
    }
}

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
            "timeline.motion-frame",
            "gpui.runtime.gallery"
        ]
    );
    let form_snapshot = pages::components::form_devtools_dogfood_snapshot();
    let expected_form_redactions = form_snapshot.fields.len()
        + form_snapshot.errors.len()
        + form_snapshot
            .fields
            .iter()
            .map(|field| field.meta.errors.len())
            .sum::<usize>();
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
        expected_form_redactions
    );
    assert_devtools_canaries_absent(&serde_json::to_string(&collection).unwrap());
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
    assert_eq!(collection.diagnostics.len(), 1);
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
    assert!(target_ids.contains(&"gpui.runtime.gallery"));
    assert!(target_ids.contains(&"gallery.shell.devtools"));
    assert!(target_ids.contains(&"probe.command.registry"));
    assert!(target_ids.contains(&"probe.layout.scroll-viewport"));
    assert!(domain_labels.contains(&"command"));
    assert!(domain_labels.contains(&"gallery-shell"));
    assert!(domain_labels.contains(&"gpui-runtime"));
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
            .any(|event| event.id() == "gallery.shell-live-facts")
    );
    assert!(
        capture
            .events
            .iter()
            .any(|event| event.id() == "gpui.frame-metadata")
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
    assert_eq!(capture.snapshot_collection().snapshots.len(), 11);
    assert_devtools_canaries_absent(&serde_json::to_string(&capture).unwrap());
}

#[test]
fn devtools_gallery_session_frame_exposes_history_and_diff() {
    let frame = pages::devtools::devtools_gallery_session_frame();
    let export = pages::devtools::devtools_gallery_session_export();
    let state = pages::devtools::devtools_gallery_state();
    let session_frame = state.session_frame().expect("session frame summary");
    let diff = frame
        .diff_from_previous
        .as_ref()
        .expect("second refresh has a previous-frame diff");

    assert_eq!(frame.generation, 2);
    assert_eq!(frame.previous_generation, Some(1));
    assert_eq!(session_frame.generation, 2);
    assert_eq!(session_frame.previous_generation, Some(1));
    assert_eq!(session_frame.diff_row_count, diff.rows.len());
    assert_eq!(export.retained_frames, 2);
    assert_eq!(export.current_generation, Some(2));
    assert_eq!(export.frames.len(), 2);
    assert_form_validation_projection(form_snapshot(&export.frames[0]), "Validating", 1);
    assert_form_validation_projection(form_snapshot(&export.frames[1]), "SubmitFailed", 0);
    assert!(diff.summary.changed > 0);
    assert!(state.diff_rows().iter().any(|row| {
        row.status == DevtoolsDiffStatus::Changed
            && (row.identity.contains("gpui.runtime.gallery")
                || row.identity.contains("gpui.frame-metadata")
                || row.identity.contains("gallery.motion-frame-demand")
                || row.identity.contains("gallery.shell"))
    }));
    assert!(diff.rows.iter().any(|row| {
        row.kind == DevtoolsDiffKind::Snapshot
            && row.status == DevtoolsDiffStatus::Changed
            && row.identity.contains("gpui.runtime.gallery")
    }));
    assert!(diff.rows.iter().any(|row| {
        row.kind == DevtoolsDiffKind::Snapshot
            && row.status == DevtoolsDiffStatus::Changed
            && row.identity == "form:form"
    }));

    let export_json = serde_json::to_string(&export).unwrap();
    assert!(export_json.contains("open-gpui-devtools-session/v1"));
    assert!(!export_json.contains("raw_text"));
    assert!(!export_json.contains("clipboard_contents"));
    assert_devtools_canaries_absent(&export_json);
}

#[open_gpui::test]
fn devtools_gallery_inspector_copies_validating_form_detail(
    test_cx: &mut open_gpui::TestAppContext,
) {
    let (shell, cx) = open_gallery_page_with_shell(test_cx, GalleryPage::Devtools);
    let inspector = cx.update(|_, app| shell.read(app).devtools_inspector().clone());

    scroll_page_selector_into_view(&shell, cx, "gallery-devtools:refresh");
    click(cx, "gallery-devtools:refresh");
    scroll_page_selector_into_view(&shell, cx, "devtools-inspector:row:form");
    click(cx, "devtools-inspector:row:form");

    let (selected_probe, detail_kind) = cx.update(|_, app| {
        let inspector = inspector.read(app);
        (
            inspector
                .state()
                .selected_probe_id()
                .map(|probe_id| probe_id.as_str().to_owned()),
            inspector.state().active_detail_kind(),
        )
    });
    assert_eq!(selected_probe.as_deref(), Some("form"));
    assert_eq!(
        detail_kind,
        Some(open_gpui_devtools::DevtoolsInspectorDetailKind::LegacySnapshot)
    );

    scroll_page_selector_into_view(&shell, cx, "devtools-inspector:copy-detail");
    click(cx, "devtools-inspector:copy-detail");
    let clipboard_json = test_cx
        .read_from_clipboard()
        .and_then(|item| item.text())
        .expect("copied form detail JSON");
    let clipboard_snapshot =
        serde_json::from_str::<SnapshotEnvelope>(&clipboard_json).expect("valid snapshot JSON");

    assert_form_validation_projection(&clipboard_snapshot, "Validating", 1);
    assert_eq!(clipboard_snapshot.redaction.redacted_values, 6);
    assert!(
        clipboard_snapshot.tree.nodes[0]
            .children
            .iter()
            .all(|field| {
                field
                    .payload
                    .as_ref()
                    .is_some_and(|payload| payload["value"]["kind"] == "redacted")
            })
    );
    assert_devtools_canaries_absent(&clipboard_json);
}

#[test]
fn devtools_gallery_headless_artifacts_use_artifact_records() {
    let artifacts = pages::devtools::devtools_gallery_headless_artifacts();
    let mut sink = DevtoolsArtifactJsonlSink::new(Vec::new());

    sink.write_record(&artifacts.session_record)
        .expect("session artifact record writes");
    sink.write_record(&artifacts.report_record)
        .expect("report artifact record writes");

    let jsonl = String::from_utf8(sink.into_inner()).expect("artifact JSONL is utf8");
    let lines = jsonl.lines().collect::<Vec<_>>();

    assert_eq!(artifacts.session_export.current_generation, Some(2));
    assert_eq!(artifacts.report.source.generation, Some(2));
    assert_eq!(
        artifacts.session_record.metadata.producer_id,
        pages::devtools::DEVTOOLS_GALLERY_ARTIFACT_PRODUCER_ID
    );
    assert_eq!(
        artifacts.session_record.metadata.scenario_id.as_deref(),
        Some(pages::devtools::DEVTOOLS_GALLERY_ARTIFACT_SCENARIO_ID)
    );
    assert_eq!(artifacts.session_record.metadata.generation, Some(2));
    assert_eq!(artifacts.report_record.metadata.generation, Some(2));
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("\"artifact_kind\":\"session-export\""));
    assert!(lines[1].contains("\"artifact_kind\":\"report\""));
    let session_json = serde_json::to_string(&artifacts.session_export).unwrap();
    let report_json = serde_json::to_string(&artifacts.report).unwrap();
    assert_devtools_canaries_absent(&jsonl);
    assert_devtools_canaries_absent(&session_json);
    assert_devtools_canaries_absent(&report_json);
}

#[test]
fn devtools_gallery_accessibility_artifact_is_bound_to_focus_a11y_scenarios() {
    let artifacts = pages::devtools::devtools_gallery_headless_artifacts();
    let expected_scenarios = pages::focus_a11y::FOCUS_A11Y_SCENARIOS
        .iter()
        .map(|scenario| scenario.scenario_id)
        .collect::<Vec<_>>();
    assert_eq!(artifacts.accessibility_scenarios, expected_scenarios);

    let accessibility = artifacts
        .session_export
        .frames
        .last()
        .expect("current Gallery DevTools frame")
        .capture
        .snapshots
        .iter()
        .find(|snapshot| snapshot.probe_id.as_str() == "accessibility")
        .expect("Focus/A11y accessibility snapshot");
    let root = accessibility
        .tree
        .nodes
        .first()
        .expect("Focus/A11y accessibility snapshot root");
    let snapshot_scenario_ids = root
        .children
        .iter()
        .map(|scenario| {
            scenario
                .payload
                .as_ref()
                .and_then(|payload| payload["scenario_id"].as_str())
                .expect("allowlisted Focus/A11y scenario id")
        })
        .collect::<Vec<_>>();

    assert_eq!(
        snapshot_scenario_ids,
        pages::focus_a11y::FOCUS_A11Y_SCENARIOS
            .iter()
            .map(|scenario| scenario.id)
            .collect::<Vec<_>>()
    );
    assert_eq!(root.payload.as_ref().unwrap()["scenario_count"], 3);
}

#[test]
fn devtools_gallery_headless_fixtures_match_producer_output() {
    let artifacts = pages::devtools::devtools_gallery_headless_artifacts();
    let session_json = serde_json::to_string_pretty(&artifacts.session_export).unwrap();
    let report_json = serde_json::to_string_pretty(&artifacts.report).unwrap();

    assert_devtools_canaries_absent(&session_json);
    assert_devtools_canaries_absent(&report_json);
    assert_fixture_matches("gallery-session.json", &session_json);
    assert_fixture_matches("gallery-report.json", &report_json);
}

#[test]
#[ignore = "regenerates checked-in DevTools fixtures"]
fn regenerate_devtools_gallery_headless_fixtures() {
    let artifacts = pages::devtools::devtools_gallery_headless_artifacts();
    let fixture_dir = gallery_fixture_dir();

    std::fs::create_dir_all(&fixture_dir).expect("fixture directory can be created");
    std::fs::write(
        fixture_dir.join("gallery-session.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&artifacts.session_export).unwrap()
        ),
    )
    .expect("gallery session fixture can be written");
    std::fs::write(
        fixture_dir.join("gallery-report.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&artifacts.report).unwrap()
        ),
    )
    .expect("gallery report fixture can be written");
}

#[test]
fn devtools_gallery_workbench_refreshes_from_shell_live_facts() {
    let mut workbench = pages::devtools::GalleryDevtoolsWorkbench::new(
        pages::devtools::GalleryDevtoolsLiveFacts::new(
            "tokens",
            1040.0,
            "desktop",
            "comfortable",
            "md",
        ),
    );
    let initial_generation = workbench.current_generation().expect("seeded frame");

    let frame = workbench
        .refresh_with_facts(pages::devtools::GalleryDevtoolsLiveFacts::new(
            "devtools", 720.0, "mobile", "compact", "sm",
        ))
        .expect("live facts refresh succeeds");
    let frame_json = serde_json::to_string(&frame).unwrap();

    assert!(frame.generation > initial_generation);
    assert_eq!(
        workbench.refresh_status(),
        pages::devtools::GalleryDevtoolsRefreshStatus::Changed
    );
    assert!(workbench.retained_frames() <= workbench.history_limit());
    assert!(frame_json.contains("\"active_page\":\"devtools\""));
    assert!(frame_json.contains("\"viewport_width_px\":720.0"));
    assert!(frame_json.contains("gallery.shell-live-facts"));

    let sensitive_frame = workbench
        .refresh_with_facts(pages::devtools::GalleryDevtoolsLiveFacts::new(
            "alice@example.com /Users/alice/project token=secret",
            680.0,
            "desktop",
            "comfortable",
            "md",
        ))
        .expect("sensitive-looking live facts refresh succeeds");
    let sensitive_json = serde_json::to_string(&sensitive_frame).unwrap();

    assert!(!sensitive_json.contains("alice@example.com"));
    assert!(!sensitive_json.contains("/Users/alice"));
    assert!(!sensitive_json.contains("secret"));
    assert!(workbench.retained_frames() <= workbench.history_limit());
}

fn assert_fixture_matches(name: &str, actual: &str) {
    let expected = std::fs::read_to_string(gallery_fixture_dir().join(name))
        .unwrap_or_else(|error| panic!("failed to read fixture {name}: {error}"));
    assert_devtools_canaries_absent(&expected);
    assert_eq!(normalize_json(&expected), normalize_json(actual));
}

fn gallery_fixture_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("gallery crate is under examples")
        .parent()
        .expect("examples has a workspace parent")
        .join("crates")
        .join("devtools")
        .join("tests")
        .join("fixtures")
}

fn normalize_json(value: &str) -> String {
    value.replace("\r\n", "\n").trim_end().to_owned()
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
    let accessibility = collection
        .snapshots
        .iter()
        .find(|snapshot| snapshot.probe_id.as_str() == "accessibility")
        .expect("resolved accessibility snapshot");
    let form_json = serde_json::to_string(form).unwrap();
    let resource_json = serde_json::to_string(resource).unwrap();
    let accessibility_json = serde_json::to_string(accessibility).unwrap();

    assert!(form_json.contains("SubmitFailed"));
    assert!(form_json.contains("submit_count"));
    assert!(resource_json.contains("Refetching"));
    assert!(resource_json.contains("Success"));
    assert!(resource_json.contains("observer_count"));
    assert!(accessibility_json.contains("Resolved accessibility semantics"));
    assert!(accessibility_json.contains("\"contract_id\":\"TextInput\""));
    assert!(accessibility_json.contains("\"contract_id\":\"Textarea\""));
    assert!(accessibility_json.contains("\"contract_id\":\"Field\""));
    assert!(accessibility_json.contains("password-redacted"));
    assert_devtools_canaries_absent(&accessibility_json);
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
            .any(|snapshot| snapshot.probe_id.as_str() == "gpui.runtime.gallery")
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

    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "runtime.unavailable")
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
    assert!(source.contains("DevtoolsSession::new"));
    assert!(source.contains("DevtoolsRegistry::default()"));
    assert!(source.contains("register_capture_provider_fn"));
    assert!(!source.contains("COMPONENT_A11Y_EVIDENCE"));
    assert!(!source.contains("a11y_evidence_probe_snapshot"));
    assert!(source.contains("resolved_semantics_probe_snapshot"));
}

#[test]
fn devtools_gallery_text_control_projections_own_placeholders() {
    let source = include_str!("../../src/pages/devtools.rs");

    assert!(
        !source.contains(".with_placeholder("),
        "text-control semantic projections must be the Gallery placeholder authority"
    );
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

    let event_selector = cx.update(|_, app| {
        inspector
            .read(app)
            .state()
            .event_rows()
            .into_iter()
            .find(|row| row.event_id == "gallery.motion-frame-demand")
            .map(|row| format!("devtools-inspector:event:{}", row.event_identity.as_key()))
            .expect("expected gallery motion frame demand event row")
    });
    scroll_page_selector_into_view(&shell, cx, &event_selector);
    click(cx, &event_selector);
    let (selected_event_key, active_detail_kind, feedback) = cx.update(|_, app| {
        let inspector = inspector.read(app);
        (
            inspector
                .state()
                .selected_event_identity()
                .map(|identity| identity.as_key()),
            inspector.state().active_detail_kind(),
            inspector.feedback_label().map(ToString::to_string),
        )
    });
    assert_eq!(
        selected_event_key.as_deref(),
        event_selector.strip_prefix("devtools-inspector:event:")
    );
    assert_eq!(
        active_detail_kind,
        Some(open_gpui_devtools::DevtoolsInspectorDetailKind::Event)
    );
    assert_eq!(feedback.as_deref(), Some("Selected event #0"));

    assert!(cx.debug_bounds("gallery-devtools:toolbar").is_some());
    assert!(cx.debug_bounds("gallery-devtools:refresh").is_some());
    assert!(cx.debug_bounds("gallery-devtools:frame-history").is_some());
    assert!(cx.debug_bounds("gallery-devtools:diff-state").is_some());
    let generation_before = cx.update(|_, app| {
        shell
            .read(app)
            .devtools_workbench()
            .current_generation()
            .expect("current devtools generation")
    });
    scroll_page_selector_into_view(&shell, cx, "gallery-devtools:refresh");
    click(cx, "gallery-devtools:refresh");
    let (generation_after, refresh_status, selection_status, retained_frames, history_limit) = cx
        .update(|_, app| {
            let shell = shell.read(app);
            let workbench = shell.devtools_workbench();
            (
                workbench
                    .current_generation()
                    .expect("refreshed devtools generation"),
                workbench.refresh_status(),
                workbench.selection_status(),
                workbench.retained_frames(),
                workbench.history_limit(),
            )
        });
    assert!(generation_after > generation_before);
    assert_eq!(
        refresh_status,
        pages::devtools::GalleryDevtoolsRefreshStatus::Changed
    );
    assert_eq!(
        selection_status,
        pages::devtools::GalleryDevtoolsSelectionStatus::Preserved
    );
    assert!(retained_frames <= history_limit);

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
