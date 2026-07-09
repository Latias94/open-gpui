use open_gpui_devtools::{
    DevtoolsCapture, DevtoolsDomainId, DevtoolsDomainKind, DevtoolsDomainSnapshot,
    DevtoolsTargetId, DevtoolsTargetKind, DevtoolsTargetSnapshot, DevtoolsTargetTree, ProbeId,
    SnapshotCollection, SnapshotDiagnostic, SnapshotEnvelope, SnapshotKind, SnapshotNode,
    SnapshotRedactionSummary, SnapshotTree,
};

#[test]
fn target_snapshots_sanitize_ids_labels_and_metadata() {
    let target = DevtoolsTargetSnapshot::new(
        DevtoolsTargetId::new("window alice@example.com"),
        DevtoolsTargetKind::Custom("token=raw-secret".to_owned()),
        "Owner alice@example.com at C:\\Users\\Frank\\app.json",
    )
    .with_metadata(serde_json::json!({
        "api_key": "raw-api-key",
        "path": "C:\\Users\\Frank\\app.json",
    }));
    let serialized = serde_json::to_string(&target).unwrap();

    assert!(!serialized.contains("alice@example.com"), "{serialized}");
    assert!(!serialized.contains("raw-secret"), "{serialized}");
    assert!(!serialized.contains("raw-api-key"), "{serialized}");
    assert!(!serialized.contains("Frank"), "{serialized}");
    assert!(serialized.contains("[redacted"));
}

#[test]
fn domain_snapshots_wrap_legacy_snapshots_without_losing_redaction() {
    let mut redaction = SnapshotRedactionSummary::default();
    redaction.record_redacted("password = raw-note");
    let snapshot = SnapshotEnvelope::new(
        ProbeId::new("form").unwrap(),
        SnapshotKind::Form,
        SnapshotTree::new([SnapshotNode::new("form", "Profile form")]),
    )
    .with_redaction(redaction.clone());
    let target_id = DevtoolsTargetId::from_probe_id(&snapshot.probe_id);
    let domain = DevtoolsDomainSnapshot::from_snapshot(target_id.clone(), snapshot.clone());

    assert_eq!(domain.target_id, target_id);
    assert_eq!(domain.kind, DevtoolsDomainKind::Data);
    assert_eq!(domain.label, "form");
    assert_eq!(domain.snapshot.as_ref().unwrap().redaction, redaction);
    assert_eq!(
        domain.summary.as_ref().unwrap()["snapshot_kind"],
        serde_json::json!("form")
    );
}

#[test]
fn captures_round_trip_legacy_collections_and_build_probe_targets() {
    let collection = SnapshotCollection {
        snapshots: vec![
            SnapshotEnvelope::new(
                ProbeId::new("command.registry").unwrap(),
                SnapshotKind::Command,
                SnapshotTree::new([SnapshotNode::new("command", "Command registry")]),
            ),
            SnapshotEnvelope::new(
                ProbeId::new("layout.scroll").unwrap(),
                SnapshotKind::Layout,
                SnapshotTree::new([SnapshotNode::new("layout", "Scroll layout")]),
            ),
        ],
        diagnostics: vec![SnapshotDiagnostic::new(
            ProbeId::new("docking").unwrap(),
            "runtime.unavailable",
            "docking runtime unavailable",
        )],
    };

    let capture = DevtoolsCapture::from_snapshot_collection(collection.clone());
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

    assert_eq!(
        target_ids,
        ["app", "probe.command.registry", "probe.layout.scroll"]
    );
    assert_eq!(domain_labels, ["command", "layout"]);
    assert_eq!(capture.snapshot_collection(), collection.sanitized());
}

#[test]
fn captures_sanitize_public_struct_literal_bypasses() {
    let capture = DevtoolsCapture::new(
        DevtoolsTargetTree {
            targets: vec![DevtoolsTargetSnapshot {
                id: DevtoolsTargetId::new("owner alice@example.com"),
                kind: DevtoolsTargetKind::Custom("token=raw-target".to_owned()),
                label: "Owner alice@example.com".to_owned(),
                parent_id: None,
                metadata: Some(serde_json::json!({"token": "raw-token"})),
            }],
        },
        [DevtoolsDomainSnapshot {
            id: DevtoolsDomainId::new("domain alice@example.com"),
            target_id: DevtoolsTargetId::new("target alice@example.com"),
            kind: DevtoolsDomainKind::Custom("secret=raw-domain".to_owned()),
            label: "Domain alice@example.com".to_owned(),
            summary: Some(serde_json::json!({"password": "raw-password"})),
            snapshot: None,
            diagnostics: Vec::new(),
        }],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let serialized = serde_json::to_string(&capture).unwrap();

    assert!(!serialized.contains("alice@example.com"), "{serialized}");
    assert!(!serialized.contains("raw-target"), "{serialized}");
    assert!(!serialized.contains("raw-domain"), "{serialized}");
    assert!(!serialized.contains("raw-password"), "{serialized}");
    assert!(serialized.contains("[redacted"));
}

#[test]
fn captures_report_duplicate_and_missing_identities_as_diagnostics() {
    let target = DevtoolsTargetSnapshot::new(
        DevtoolsTargetId::new("target.duplicate"),
        DevtoolsTargetKind::App,
        "App",
    );
    let duplicate_domain_id = DevtoolsDomainId::new("domain.duplicate");
    let snapshot = SnapshotEnvelope::new(
        ProbeId::new("probe.duplicate").unwrap(),
        SnapshotKind::Command,
        SnapshotTree::new([SnapshotNode::new("command", "Command")]),
    );
    let capture = DevtoolsCapture::new(
        DevtoolsTargetTree::new([target.clone(), target]),
        [
            DevtoolsDomainSnapshot::new(
                duplicate_domain_id.clone(),
                DevtoolsTargetId::new("target.duplicate"),
                DevtoolsDomainKind::Command,
                "Command",
            ),
            DevtoolsDomainSnapshot::new(
                duplicate_domain_id,
                DevtoolsTargetId::new("target.missing"),
                DevtoolsDomainKind::Timeline,
                "Timeline",
            ),
        ],
        Vec::new(),
        [snapshot.clone(), snapshot],
        Vec::new(),
    );
    let diagnostic_codes = capture
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<Vec<_>>();

    assert!(diagnostic_codes.contains(&"capture.duplicate_target"));
    assert!(diagnostic_codes.contains(&"capture.duplicate_domain"));
    assert!(diagnostic_codes.contains(&"capture.missing_domain_target"));
    assert!(diagnostic_codes.contains(&"capture.duplicate_probe"));
    assert!(
        capture
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.probe_id.as_str() == "devtools.capture")
    );
}
