use super::*;
use open_gpui_devtools::{DevtoolsDiffKind, DevtoolsDiffStatus};

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
    assert!(target_ids.contains(&"probe.command.registry"));
    assert!(target_ids.contains(&"probe.layout.scroll-viewport"));
    assert!(domain_labels.contains(&"command"));
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
    assert!(diff.summary.changed > 0);
    assert!(state.diff_rows().iter().any(|row| {
        row.status == DevtoolsDiffStatus::Changed
            && (row.identity.contains("gpui.runtime.gallery")
                || row.identity.contains("gpui.frame-metadata")
                || row.identity.contains("gallery.motion-frame-demand"))
    }));
    assert!(diff.rows.iter().any(|row| {
        row.kind == DevtoolsDiffKind::Snapshot
            && row.status == DevtoolsDiffStatus::Changed
            && row.identity.contains("gpui.runtime.gallery")
    }));

    let export_json = serde_json::to_string(&export).unwrap();
    assert!(export_json.contains("open-gpui-devtools-session/v1"));
    assert!(!export_json.contains("raw_text"));
    assert!(!export_json.contains("clipboard_contents"));
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
}
