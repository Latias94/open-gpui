//! Devtools inspector gallery page.

use open_gpui_devtools::{
    DevtoolsInspectorState, DevtoolsRegistry, ProbeId, SnapshotCollection, SnapshotDiagnostic,
    form, resource,
};
use open_gpui_resource::PaginatedResourceSnapshotView;

use super::components::{form_devtools_dogfood_snapshot, resource_devtools_dogfood_snapshots};

/// Page title.
pub const TITLE: &str = "DevTools";
/// Page summary.
pub const SUMMARY: &str = "Read-only local inspection over redacted snapshot probes.";
/// Foundation signals exercised by this page.
pub const SIGNALS: &[&str] = &[
    "open_gpui_devtools::DevtoolsRegistry",
    "open_gpui_devtools::DevtoolsInspectorState",
    "open_gpui_devtools::DevtoolsInspector",
    "open_gpui_devtools::SnapshotEnvelope",
    "open_gpui_devtools::SnapshotKind",
    "open_gpui_devtools::SnapshotRedactionSummary",
    "open_gpui_devtools::form::form_snapshot_probe",
    "open_gpui_devtools::resource::resource_snapshot_probe",
];

/// Returns the deterministic devtools inspector state used by the gallery.
pub fn devtools_gallery_state() -> DevtoolsInspectorState {
    DevtoolsInspectorState::new(devtools_gallery_collection())
}

/// Returns the deterministic snapshot collection used by the gallery.
pub fn devtools_gallery_collection() -> SnapshotCollection {
    let mut registry = DevtoolsRegistry::default();
    let form_snapshot = form_devtools_dogfood_snapshot();
    let resource_snapshots = resource_devtools_dogfood_snapshots();
    let resource_snapshot = resource_snapshots.resource;
    let mutation_snapshot = resource_snapshots.mutation;

    registry
        .register(
            form::form_snapshot_probe("form", move || form_snapshot.clone())
                .expect("valid form probe"),
        )
        .expect("unique form probe");
    registry
        .register(
            resource::resource_snapshot_probe(
                "resource",
                move || vec![resource_snapshot.clone()],
                move || vec![mutation_snapshot.clone()],
                Vec::<PaginatedResourceSnapshotView>::new,
            )
            .expect("valid resource probe"),
        )
        .expect("unique resource probe");

    let mut collection = registry.collect();
    collection
        .diagnostics
        .extend(unmounted_framework_diagnostics());
    collection
}

fn unmounted_framework_diagnostics() -> Vec<SnapshotDiagnostic> {
    ["theme", "motion", "docking"]
        .into_iter()
        .map(|probe_id| {
            SnapshotDiagnostic::new(
                ProbeId::new(probe_id).unwrap(),
                "runtime.unavailable",
                format!("{probe_id} runtime is not mounted in this gallery page"),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devtools_gallery_state_exposes_redacted_snapshots_and_diagnostics() {
        let state = devtools_gallery_state();
        let rows = state.snapshot_rows();

        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|row| row.probe_id.as_str() == "form"
            && row.kind_label == "form"
            && row.redacted_values == 5));
        assert!(rows.iter().any(|row| row.probe_id.as_str() == "resource"
            && row.kind_label == "resource"
            && row.redacted_values == 2));
        assert_eq!(state.diagnostics().len(), 3);
    }
}
