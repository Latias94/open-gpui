//! Devtools inspector gallery page.

use open_gpui_devtools::{
    DevtoolsInspectorState, ProbeId, SnapshotCollection, SnapshotDiagnostic, SnapshotEnvelope,
    SnapshotKind, SnapshotNode, SnapshotRedactionSummary, SnapshotTree,
};

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
];

/// Returns the deterministic devtools inspector state used by the gallery.
pub fn devtools_gallery_state() -> DevtoolsInspectorState {
    DevtoolsInspectorState::new(devtools_gallery_collection())
}

/// Returns the deterministic snapshot collection used by the gallery.
pub fn devtools_gallery_collection() -> SnapshotCollection {
    SnapshotCollection {
        snapshots: vec![
            theme_snapshot(),
            form_snapshot(),
            resource_snapshot(),
            docking_snapshot(),
        ],
        diagnostics: vec![SnapshotDiagnostic {
            probe_id: ProbeId::new("motion").unwrap(),
            message: "motion runtime is not mounted in this gallery page".to_owned(),
        }],
    }
}

fn theme_snapshot() -> SnapshotEnvelope {
    SnapshotEnvelope::new(
        ProbeId::new("theme").unwrap(),
        SnapshotKind::Theme,
        SnapshotTree::new([SnapshotNode::new("theme", "Theme tokens")
            .with_payload(serde_json::json!({"mode": "light", "density": "regular"}))]),
    )
}

fn form_snapshot() -> SnapshotEnvelope {
    let mut redaction = SnapshotRedactionSummary::default();
    redaction.record_redacted("account.email value");
    SnapshotEnvelope::new(
        ProbeId::new("form").unwrap(),
        SnapshotKind::Form,
        SnapshotTree::new([SnapshotNode::new("profile", "Profile form")
            .with_payload(serde_json::json!({"status": "idle"}))
            .with_child(
                SnapshotNode::new("field:account.email", "Email")
                    .with_payload(serde_json::json!({"dirty": true, "invalid": true})),
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
        SnapshotTree::new([SnapshotNode::new("query:projects", "Projects")
            .with_payload(serde_json::json!({"status": "refetching", "observers": 1}))]),
    )
    .with_redaction(redaction)
}

fn docking_snapshot() -> SnapshotEnvelope {
    SnapshotEnvelope::new(
        ProbeId::new("docking").unwrap(),
        SnapshotKind::Docking,
        SnapshotTree::new([SnapshotNode::new("workspace", "Workspace panes")
            .with_child(SnapshotNode::new("pane:left", "Navigator"))
            .with_child(SnapshotNode::new("pane:center", "Editor"))]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devtools_gallery_state_exposes_redacted_snapshots_and_diagnostics() {
        let state = devtools_gallery_state();
        let rows = state.snapshot_rows();

        assert_eq!(rows.len(), 4);
        assert!(rows.iter().any(|row| row.kind_label == "form"));
        assert!(rows.iter().any(|row| row.redacted_values == 1));
        assert_eq!(state.diagnostics().len(), 1);
    }
}
