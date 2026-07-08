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
        ["accessibility", "form", "motion", "resource", "theme"]
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
    assert!(!form_json.contains("gallery-secret"));
    assert!(!resource_json.contains("gallery-secret"));
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
    assert!(source.contains("DevtoolsRegistry::default()"));
}
