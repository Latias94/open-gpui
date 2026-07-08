use open_gpui_devtools::{
    DevtoolsProbe, DevtoolsRegistry, ProbeId, ProbeSnapshotError, SnapshotEnvelope, SnapshotKind,
    SnapshotNode, SnapshotProbe, SnapshotProbeSnapshot, SnapshotRedactionSummary, SnapshotTree,
};

struct StaticProbe {
    id: ProbeId,
    snapshot: SnapshotEnvelope,
}

impl StaticProbe {
    fn new(id: &str, kind: SnapshotKind) -> Self {
        let id = ProbeId::new(id).unwrap();
        let tree = SnapshotTree::new([SnapshotNode::new("root", format!("{id} root"))]);
        let snapshot = SnapshotEnvelope::new(id.clone(), kind, tree);
        Self { id, snapshot }
    }
}

impl DevtoolsProbe for StaticProbe {
    fn id(&self) -> &ProbeId {
        &self.id
    }

    fn snapshot(&self) -> Result<SnapshotEnvelope, ProbeSnapshotError> {
        Ok(self.snapshot.clone())
    }
}

struct FailingProbe {
    id: ProbeId,
}

impl FailingProbe {
    fn new(id: &str) -> Self {
        Self {
            id: ProbeId::new(id).unwrap(),
        }
    }
}

impl DevtoolsProbe for FailingProbe {
    fn id(&self) -> &ProbeId {
        &self.id
    }

    fn snapshot(&self) -> Result<SnapshotEnvelope, ProbeSnapshotError> {
        Err(ProbeSnapshotError::CollectionFailed(
            "probe unavailable".to_owned(),
        ))
    }
}

#[test]
fn registry_collects_snapshots_in_probe_id_order() {
    let mut registry = DevtoolsRegistry::default();
    registry
        .register(StaticProbe::new("theme", SnapshotKind::Theme))
        .unwrap();
    registry
        .register(StaticProbe::new("a11y", SnapshotKind::Accessibility))
        .unwrap();

    let collection = registry.collect();
    let probe_ids = collection
        .snapshots
        .iter()
        .map(|snapshot| snapshot.probe_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(probe_ids, ["a11y", "theme"]);
    assert!(collection.diagnostics.is_empty());
}

#[test]
fn registry_reports_probe_failures_as_diagnostics() {
    let mut registry = DevtoolsRegistry::default();
    registry.register(FailingProbe::new("resource")).unwrap();

    let collection = registry.collect();

    assert!(collection.snapshots.is_empty());
    assert_eq!(collection.diagnostics.len(), 1);
    assert_eq!(collection.diagnostics[0].probe_id.as_str(), "resource");
    assert!(
        collection.diagnostics[0]
            .message
            .contains("probe unavailable")
    );
}

#[test]
fn closure_backed_snapshot_probe_builds_consistent_envelopes() {
    let mut redaction = SnapshotRedactionSummary::default();
    redaction.record_redacted("theme.secret");
    let redaction_for_probe = redaction.clone();
    let probe = SnapshotProbe::new("theme", SnapshotKind::Theme, move || {
        Ok(
            SnapshotProbeSnapshot::new(SnapshotTree::new([SnapshotNode::new(
                "theme",
                "Theme runtime",
            )
            .with_payload(serde_json::json!({"mode": "dark"}))]))
            .with_redaction(redaction_for_probe.clone()),
        )
    })
    .unwrap();

    let snapshot = probe.snapshot().unwrap();

    assert_eq!(snapshot.probe_id.as_str(), "theme");
    assert_eq!(snapshot.kind, SnapshotKind::Theme);
    assert_eq!(snapshot.tree.nodes[0].id, "theme");
    assert_eq!(snapshot.redaction, redaction);
}

#[test]
fn registry_registers_closure_backed_snapshot_probes() {
    let mut registry = DevtoolsRegistry::default();
    registry
        .register_snapshot_probe("resource", SnapshotKind::Resource, || {
            Ok(SnapshotProbeSnapshot::new(SnapshotTree::new([
                SnapshotNode::new("projects", "Projects"),
            ])))
        })
        .unwrap();

    let collection = registry.collect();

    assert_eq!(collection.snapshots.len(), 1);
    assert_eq!(collection.snapshots[0].probe_id.as_str(), "resource");
    assert_eq!(collection.snapshots[0].kind, SnapshotKind::Resource);
}

#[test]
fn snapshot_export_preserves_tree_and_redaction_summary() {
    let mut redaction = SnapshotRedactionSummary::default();
    redaction.record_redacted("password");
    let snapshot = SnapshotEnvelope::new(
        ProbeId::new("form").unwrap(),
        SnapshotKind::Form,
        SnapshotTree::new([SnapshotNode::new("field:password", "Password")
            .with_payload(serde_json::json!({"value": "<redacted>"}))]),
    )
    .with_redaction(redaction);

    let value = serde_json::to_value(snapshot).unwrap();

    assert_eq!(value["probe_id"], "form");
    assert_eq!(value["tree"]["nodes"][0]["id"], "field:password");
    assert_eq!(value["redaction"]["redacted_values"], 1);
    assert_eq!(value["redaction"]["notes"][0], "password");
}
