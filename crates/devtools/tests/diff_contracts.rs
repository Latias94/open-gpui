use open_gpui_devtools::{
    DevtoolsCapture, DevtoolsDiffKind, DevtoolsDiffStatus, DevtoolsDomainId, DevtoolsDomainKind,
    DevtoolsDomainSnapshot, DevtoolsEventKind, DevtoolsEventRecord, DevtoolsTargetId,
    DevtoolsTargetKind, DevtoolsTargetSnapshot, DevtoolsTargetTree, ProbeId, SnapshotDiagnostic,
    SnapshotEnvelope, SnapshotKind, SnapshotNode, SnapshotTree,
};

#[test]
fn capture_diff_reports_added_removed_changed_and_unchanged_rows() {
    let previous = capture_with_parts(
        "runtime.main",
        "Runtime",
        1,
        ["runtime.shared"],
        ["runtime.removed"],
    );
    let current = capture_with_parts(
        "runtime.main",
        "Runtime changed",
        2,
        ["runtime.shared"],
        ["runtime.added"],
    );

    let diff = current.diff_from(&previous);

    assert!(diff.summary.added > 0);
    assert!(diff.summary.removed > 0);
    assert!(diff.summary.changed > 0);
    assert!(diff.summary.unchanged > 0);
    assert!(diff.rows.iter().any(|row| {
        row.kind == DevtoolsDiffKind::Target
            && row.identity == "runtime.main"
            && row.status == DevtoolsDiffStatus::Changed
    }));
    assert!(
        diff.rows.iter().any(|row| {
            row.identity == "runtime.added" && row.status == DevtoolsDiffStatus::Added
        })
    );
    assert!(diff.rows.iter().any(|row| {
        row.identity == "runtime.removed" && row.status == DevtoolsDiffStatus::Removed
    }));
}

#[test]
fn event_identity_includes_scope_sequence_and_event_id() {
    let previous = DevtoolsCapture::default();
    let current = DevtoolsCapture::new(
        DevtoolsTargetTree::default(),
        Vec::<DevtoolsDomainSnapshot>::new(),
        [
            DevtoolsEventRecord::new("refresh", "Refresh", DevtoolsEventKind::Instant)
                .scope_id("scope.a"),
            DevtoolsEventRecord::new("refresh", "Refresh", DevtoolsEventKind::Instant)
                .scope_id("scope.b"),
        ],
        Vec::<SnapshotEnvelope>::new(),
        Vec::new(),
    );

    let diff = current.diff_from(&previous);
    let event_identities = diff
        .rows
        .iter()
        .filter(|row| row.kind == DevtoolsDiffKind::Event)
        .map(|row| row.identity.as_str())
        .collect::<Vec<_>>();

    assert_eq!(event_identities, ["scope.a:0:refresh", "scope.b:0:refresh"]);
}

#[test]
fn redaction_induced_identity_collision_is_explicit() {
    let previous = DevtoolsCapture::default();
    let target_id = DevtoolsTargetId::new("runtime.shared");
    let current = DevtoolsCapture::new(
        DevtoolsTargetTree::new([
            DevtoolsTargetSnapshot::new(
                target_id.clone(),
                DevtoolsTargetKind::Runtime,
                "Runtime A",
            ),
            DevtoolsTargetSnapshot::new(target_id, DevtoolsTargetKind::Runtime, "Runtime B"),
        ]),
        Vec::<DevtoolsDomainSnapshot>::new(),
        Vec::<DevtoolsEventRecord>::new(),
        Vec::<SnapshotEnvelope>::new(),
        Vec::new(),
    );

    let diff = current.diff_from(&previous);
    let collision = diff
        .rows
        .iter()
        .find(|row| row.status == DevtoolsDiffStatus::Collision)
        .expect("collision row");

    assert_eq!(collision.kind, DevtoolsDiffKind::Target);
    assert_eq!(collision.identity, "runtime.shared");
    assert_eq!(
        collision.diagnostic.as_ref().unwrap().code,
        "diff.identity_collision"
    );
    assert_eq!(diff.summary.collisions, 1);
}

#[test]
fn diff_json_is_sanitized() {
    let previous = DevtoolsCapture::default();
    let current = DevtoolsCapture::new(
        DevtoolsTargetTree::new([DevtoolsTargetSnapshot::new(
            DevtoolsTargetId::new("runtime.secret"),
            DevtoolsTargetKind::Runtime,
            "alice@example.com token=secret",
        )]),
        Vec::<DevtoolsDomainSnapshot>::new(),
        Vec::<DevtoolsEventRecord>::new(),
        Vec::<SnapshotEnvelope>::new(),
        [SnapshotDiagnostic::new(
            ProbeId::new("provider.secret").unwrap(),
            "provider.failed",
            "C:\\Users\\alice\\token.txt password=hunter2",
        )],
    );

    let diff = current.diff_from(&previous);
    let json = serde_json::to_string(&diff).unwrap();

    assert!(!json.contains("alice@example.com"));
    assert!(!json.contains("hunter2"));
    assert!(!json.contains("C:\\Users\\alice"));
    assert!(json.contains("[redacted-email]"));
    assert!(json.contains("[redacted-path]"));
}

#[test]
fn identical_captures_have_only_unchanged_rows() {
    let capture = capture_with_parts("runtime.main", "Runtime", 1, ["runtime.shared"], []);

    let diff = capture.diff_from(&capture);

    assert!(diff.is_empty());
    assert!(diff.summary.unchanged > 0);
    assert!(
        diff.rows
            .iter()
            .all(|row| row.status == DevtoolsDiffStatus::Unchanged)
    );
}

fn capture_with_parts<const S: usize, const E: usize>(
    main_target_id: &str,
    main_label: &str,
    value: usize,
    stable_targets: [&str; S],
    edge_targets: [&str; E],
) -> DevtoolsCapture {
    let main_target_id = DevtoolsTargetId::new(main_target_id);
    let domain_id = DevtoolsDomainId::from_parts(["runtime", "domain"]);
    let mut targets = vec![DevtoolsTargetSnapshot::new(
        main_target_id.clone(),
        DevtoolsTargetKind::Runtime,
        main_label,
    )];
    targets.extend(stable_targets.into_iter().map(|id| {
        DevtoolsTargetSnapshot::new(DevtoolsTargetId::new(id), DevtoolsTargetKind::Runtime, id)
    }));
    targets.extend(edge_targets.into_iter().map(|id| {
        DevtoolsTargetSnapshot::new(DevtoolsTargetId::new(id), DevtoolsTargetKind::Runtime, id)
    }));

    let domain = DevtoolsDomainSnapshot::new(
        domain_id.clone(),
        main_target_id.clone(),
        DevtoolsDomainKind::Diagnostic,
        "Runtime domain",
    )
    .with_summary(serde_json::json!({ "value": value }));
    let event = DevtoolsEventRecord::new(
        "runtime.changed",
        "Runtime changed",
        DevtoolsEventKind::Instant,
    )
    .target_id(main_target_id)
    .domain_id(domain_id)
    .with_payload(serde_json::json!({ "value": value }));
    let snapshot = SnapshotEnvelope::new(
        ProbeId::new("runtime.snapshot").unwrap(),
        SnapshotKind::Diagnostic,
        SnapshotTree::new([SnapshotNode::new("runtime", "Runtime")]),
    );

    DevtoolsCapture::new(
        DevtoolsTargetTree::new(targets),
        [domain],
        [event],
        [snapshot],
        Vec::new(),
    )
}
