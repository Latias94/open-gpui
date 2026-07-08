use super::*;

#[test]
fn devtools_gallery_collects_form_and_resource_through_registry() {
    let collection = pages::devtools::devtools_gallery_collection();
    let probe_ids = collection
        .snapshots
        .iter()
        .map(|snapshot| snapshot.probe_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(probe_ids, ["form", "resource"]);
    assert_eq!(collection.snapshots[0].redaction.redacted_values, 5);
    assert_eq!(collection.snapshots[1].redaction.redacted_values, 2);
    assert_eq!(collection.diagnostics.len(), 3);
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
    assert!(!form_json.contains("gallery-secret"));
    assert!(!resource_json.contains("gallery-secret"));
}

#[test]
fn devtools_gallery_reports_unmounted_framework_diagnostics() {
    let state = pages::devtools::devtools_gallery_state();
    let diagnostics = state.diagnostics();

    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.code == open_gpui_devtools::SnapshotDiagnostic::COLLECTION_FAILED
            || diagnostic.code == "runtime.unavailable"
    }));
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.probe_id.as_str() == "theme")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.probe_id.as_str() == "motion")
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
    assert!(source.contains("DevtoolsRegistry::default()"));
}
