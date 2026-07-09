use open_gpui_devtools::{
    DEVTOOLS_REPORT_SCHEMA_VERSION, DevtoolsCapture, DevtoolsDomainId, DevtoolsDomainKind,
    DevtoolsDomainSnapshot, DevtoolsEventKind, DevtoolsEventRecord, DevtoolsReport,
    DevtoolsReportSeverity, DevtoolsSession, DevtoolsTargetId, DevtoolsTargetKind,
    DevtoolsTargetSnapshot, DevtoolsTargetTree, ProbeId, SnapshotDiagnostic, SnapshotEnvelope,
};

#[test]
fn report_summarizes_capture_and_renders_markdown() {
    let capture = DevtoolsCapture::new(
        DevtoolsTargetTree::new([DevtoolsTargetSnapshot::new(
            DevtoolsTargetId::new("app"),
            DevtoolsTargetKind::App,
            "App",
        )]),
        Vec::<DevtoolsDomainSnapshot>::new(),
        Vec::<DevtoolsEventRecord>::new(),
        Vec::<SnapshotEnvelope>::new(),
        Vec::<SnapshotDiagnostic>::new(),
    );

    let report = DevtoolsReport::from_capture(&capture);
    let json = serde_json::to_string(&report).unwrap();
    let markdown = report.to_markdown();

    assert_eq!(report.schema_version, DEVTOOLS_REPORT_SCHEMA_VERSION);
    assert_eq!(report.summary.target_count, 1);
    assert_eq!(report.summary.finding_count, 0);
    assert!(json.contains("open-gpui-devtools-report/v1"));
    assert!(markdown.contains("# Open GPUI DevTools Report"));
    assert!(markdown.contains("No findings."));
}

#[test]
fn report_promotes_collection_failures_to_error_findings() {
    let capture = DevtoolsCapture::new(
        DevtoolsTargetTree::default(),
        Vec::<DevtoolsDomainSnapshot>::new(),
        Vec::<DevtoolsEventRecord>::new(),
        Vec::<SnapshotEnvelope>::new(),
        [SnapshotDiagnostic::collection_failed(
            ProbeId::new("resource").unwrap(),
            "resource token=secret failed",
        )],
    );

    let report = DevtoolsReport::from_capture(&capture);
    let serialized = serde_json::to_string(&report).unwrap();

    assert!(report.has_finding_at_or_above(DevtoolsReportSeverity::Error));
    assert_eq!(report.summary.error_count, 1);
    assert!(!serialized.contains("secret"), "{serialized}");
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.id == "capture-diagnostic.probe.collection_failed")
    );
}

#[test]
fn report_detects_structural_missing_target_links() {
    let missing_target = DevtoolsTargetId::new("missing.target");
    let domain = DevtoolsDomainSnapshot::new(
        DevtoolsDomainId::new("resource.domain"),
        missing_target.clone(),
        DevtoolsDomainKind::Data,
        "Resource",
    );
    let event = DevtoolsEventRecord::new("refresh", "Refresh", DevtoolsEventKind::Instant)
        .target_id(missing_target);
    let capture = DevtoolsCapture::new(
        DevtoolsTargetTree::default(),
        [domain],
        [event],
        Vec::<SnapshotEnvelope>::new(),
        Vec::<SnapshotDiagnostic>::new(),
    );

    let report = DevtoolsReport::from_capture(&capture);
    let finding_ids = report
        .findings
        .iter()
        .map(|finding| finding.id.as_str())
        .collect::<Vec<_>>();

    assert!(finding_ids.contains(&"devtools.domain.missing-target.resource.domain"));
    assert!(
        finding_ids
            .iter()
            .any(|id| id.starts_with("devtools.event.missing-target."))
    );
    assert!(report.summary.error_count >= 1);
    assert!(report.summary.warning_count >= 1);
}

#[test]
fn report_from_session_export_uses_current_frame_and_diff_summary() {
    let mut session = DevtoolsSession::new("app.session", {
        let mut registry = open_gpui_devtools::DevtoolsRegistry::default();
        registry
            .register_capture_provider_fn("provider", || {
                Ok(DevtoolsCapture::new(
                    DevtoolsTargetTree::new([DevtoolsTargetSnapshot::new(
                        DevtoolsTargetId::new("app"),
                        DevtoolsTargetKind::App,
                        "App",
                    )]),
                    Vec::<DevtoolsDomainSnapshot>::new(),
                    Vec::<DevtoolsEventRecord>::new(),
                    Vec::<SnapshotEnvelope>::new(),
                    Vec::<SnapshotDiagnostic>::new(),
                ))
            })
            .unwrap();
        registry
    });
    session.refresh().unwrap();
    session.refresh().unwrap();

    let export = session.export();
    let report = DevtoolsReport::from_session_export(&export);

    assert_eq!(report.source.session_id.as_deref(), Some("app.session"));
    assert_eq!(report.source.generation, Some(2));
    assert_eq!(report.source.retained_frames, Some(2));
    assert!(report.summary.diff_row_count > 0);
}
