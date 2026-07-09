use open_gpui_devtools::{
    DEVTOOLS_REPORT_SCHEMA_VERSION, DevtoolsCapture, DevtoolsDomainId, DevtoolsDomainKind,
    DevtoolsDomainSnapshot, DevtoolsEventKind, DevtoolsEventRecord, DevtoolsReport,
    DevtoolsReportSeverity, DevtoolsSession, DevtoolsTargetId, DevtoolsTargetKind,
    DevtoolsTargetSnapshot, DevtoolsTargetTree, ProbeId, SnapshotDiagnostic, SnapshotEnvelope,
    SnapshotKind, SnapshotNode, SnapshotTree,
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

#[test]
fn report_rules_detect_layout_scroll_and_bounds_issues() {
    let target_id = DevtoolsTargetId::new("app");
    let layout_node =
        SnapshotNode::new("layout.node", "Layout node").with_payload(serde_json::json!({
            "bounds": {
                "origin": { "x": 0.0, "y": 0.0 },
                "size": { "width": 0.0, "height": 24.0 }
            },
            "scroll_offset": { "x": 120.0, "y": 0.0 },
            "max_scroll_offset": { "x": 40.0, "y": 20.0 }
        }));
    let envelope = SnapshotEnvelope::new(
        ProbeId::new("layout.rules").unwrap(),
        SnapshotKind::Layout,
        SnapshotTree::new([SnapshotNode::new("layout.root", "Layout").with_child(layout_node)]),
    );
    let domain = DevtoolsDomainSnapshot::new(
        DevtoolsDomainId::new("layout.rules"),
        target_id.clone(),
        DevtoolsDomainKind::Layout,
        "Layout",
    )
    .with_snapshot(envelope.clone());
    let capture = capture_with_domains(target_id, [domain], [envelope]);

    let report = DevtoolsReport::from_capture(&capture);
    let finding_ids = finding_ids(&report);

    assert!(finding_ids.contains(&"devtools.layout.invalid-bounds.layout.rules.layout.node"));
    assert!(
        finding_ids
            .contains(&"devtools.layout.scroll-offset-out-of-range.layout.rules.layout.node")
    );
    assert_eq!(report.summary.warning_count, 2);
}

#[test]
fn report_rules_detect_timeline_order_and_terminal_motion_requests() {
    let target_id = DevtoolsTargetId::new("app");
    let timeline = SnapshotEnvelope::new(
        ProbeId::new("timeline.rules").unwrap(),
        SnapshotKind::Timeline,
        SnapshotTree::new([SnapshotNode::new("timeline.root", "Timeline")
            .with_child(
                SnapshotNode::new("timeline.event.two", "Event two")
                    .with_payload(serde_json::json!({ "order": 2 })),
            )
            .with_child(
                SnapshotNode::new("timeline.event.one", "Event one")
                    .with_payload(serde_json::json!({ "order": 1 })),
            )]),
    );
    let motion = SnapshotEnvelope::new(
        ProbeId::new("motion.rules").unwrap(),
        SnapshotKind::Motion,
        SnapshotTree::new([SnapshotNode::new("motion.driver", "Frame driver")
            .with_payload(serde_json::json!({ "last_reset_reason": "prune-terminal" }))
            .with_child(
                SnapshotNode::new("motion.driver.last-demand", "Last frame demand")
                    .with_payload(serde_json::json!({ "needs_frame": true })),
            )]),
    );
    let timeline_domain = DevtoolsDomainSnapshot::new(
        DevtoolsDomainId::new("timeline.rules"),
        target_id.clone(),
        DevtoolsDomainKind::Timeline,
        "Timeline",
    )
    .with_snapshot(timeline.clone());
    let motion_domain = DevtoolsDomainSnapshot::new(
        DevtoolsDomainId::new("motion.rules"),
        target_id.clone(),
        DevtoolsDomainKind::Motion,
        "Motion",
    )
    .with_snapshot(motion.clone());
    let capture = capture_with_domains(
        target_id,
        [timeline_domain, motion_domain],
        [timeline, motion],
    );

    let report = DevtoolsReport::from_capture(&capture);
    let finding_ids = finding_ids(&report);

    assert!(
        finding_ids
            .contains(&"devtools.timeline.order-regression.timeline.rules.timeline.event.one")
    );
    assert!(
        finding_ids.contains(&"devtools.motion.terminal-frame-demand.motion.rules.motion.driver")
    );
    assert_eq!(report.summary.warning_count, 2);
}

#[test]
fn report_rules_detect_command_form_and_resource_findings_without_leaking_values() {
    let target_id = DevtoolsTargetId::new("app");
    let command_domain = DevtoolsDomainSnapshot::new(
        DevtoolsDomainId::new("command.rules"),
        target_id.clone(),
        DevtoolsDomainKind::Command,
        "Command",
    )
    .with_summary(serde_json::json!({
        "conflict_count": 1,
        "diagnostic_count": 2,
        "has_conflicts": true,
        "has_pending_commands": true,
        "pending_count": 1
    }));
    let form_domain = DevtoolsDomainSnapshot::new(
        DevtoolsDomainId::new("form.rules"),
        target_id.clone(),
        DevtoolsDomainKind::Data,
        "Form",
    )
    .with_summary(serde_json::json!({
        "status": "SubmitFailed",
        "field_count": 2,
        "error_count": 1,
        "submit_count": 1,
        "redacted_values": 1
    }));
    let resource = SnapshotEnvelope::new(
        ProbeId::new("resource.rules").unwrap(),
        SnapshotKind::Resource,
        SnapshotTree::new([SnapshotNode::new("resource.root", "Resources").with_child(
            SnapshotNode::new("resource.query.projects", "Projects").with_payload(
                serde_json::json!({
                    "status": "Error",
                    "error": "request token=secret failed",
                    "fetch_attempts": 3
                }),
            ),
        )]),
    );
    let resource_domain = DevtoolsDomainSnapshot::new(
        DevtoolsDomainId::new("resource.rules"),
        target_id.clone(),
        DevtoolsDomainKind::Data,
        "Resource",
    )
    .with_summary(serde_json::json!({
        "resource_count": 1,
        "mutation_count": 0,
        "paginated_count": 0,
        "redacted_values": 0
    }))
    .with_snapshot(resource.clone());
    let capture = capture_with_domains(
        target_id,
        [command_domain, form_domain, resource_domain],
        [resource],
    );

    let report = DevtoolsReport::from_capture(&capture);
    let finding_ids = finding_ids(&report);
    let serialized = serde_json::to_string(&report).unwrap();

    assert!(finding_ids.contains(&"devtools.command.keybinding-conflicts.command.rules"));
    assert!(finding_ids.contains(&"devtools.command.keybinding-diagnostics.command.rules"));
    assert!(finding_ids.contains(&"devtools.command.pending-keymap.command.rules"));
    assert!(finding_ids.contains(&"devtools.form.validation-errors.form.rules"));
    assert!(finding_ids.contains(&"devtools.form.submit-failed.form.rules"));
    assert!(
        finding_ids.contains(&"devtools.resource.error.resource.rules.resource.query.projects")
    );
    assert!(
        finding_ids.contains(&"devtools.resource.retrying.resource.rules.resource.query.projects")
    );
    assert!(!serialized.contains("secret"), "{serialized}");
    assert_eq!(report.summary.warning_count, 5);
    assert_eq!(report.summary.info_count, 2);
}

#[test]
fn report_rules_keep_clean_domain_summaries_quiet() {
    let target_id = DevtoolsTargetId::new("app");
    let command_domain = DevtoolsDomainSnapshot::new(
        DevtoolsDomainId::new("command.clean"),
        target_id.clone(),
        DevtoolsDomainKind::Command,
        "Command",
    )
    .with_summary(serde_json::json!({
        "conflict_count": 0,
        "diagnostic_count": 0,
        "has_conflicts": false,
        "has_pending_commands": false,
        "pending_count": 0
    }));
    let form_domain = DevtoolsDomainSnapshot::new(
        DevtoolsDomainId::new("form.clean"),
        target_id.clone(),
        DevtoolsDomainKind::Data,
        "Form",
    )
    .with_summary(serde_json::json!({
        "status": "Idle",
        "field_count": 1,
        "error_count": 0,
        "submit_count": 0,
        "redacted_values": 1
    }));
    let layout = SnapshotEnvelope::new(
        ProbeId::new("layout.clean").unwrap(),
        SnapshotKind::Layout,
        SnapshotTree::new([SnapshotNode::new("layout.root", "Layout").with_child(
            SnapshotNode::new("layout.node", "Layout node").with_payload(serde_json::json!({
                "bounds": {
                    "origin": { "x": 0.0, "y": 0.0 },
                    "size": { "width": 320.0, "height": 240.0 }
                },
                "scroll_offset": { "x": 12.0, "y": 8.0 },
                "max_scroll_offset": { "x": 40.0, "y": 20.0 }
            })),
        )]),
    );
    let timeline = SnapshotEnvelope::new(
        ProbeId::new("timeline.clean").unwrap(),
        SnapshotKind::Timeline,
        SnapshotTree::new([SnapshotNode::new("timeline.root", "Timeline")
            .with_child(
                SnapshotNode::new("timeline.event.one", "Event one")
                    .with_payload(serde_json::json!({ "order": 1 })),
            )
            .with_child(
                SnapshotNode::new("timeline.event.two", "Event two")
                    .with_payload(serde_json::json!({ "order": 2 })),
            )]),
    );
    let motion = SnapshotEnvelope::new(
        ProbeId::new("motion.clean").unwrap(),
        SnapshotKind::Motion,
        SnapshotTree::new([SnapshotNode::new("motion.driver", "Frame driver")
            .with_payload(serde_json::json!({ "last_reset_reason": "finish" }))
            .with_child(
                SnapshotNode::new("motion.driver.last-demand", "Last frame demand")
                    .with_payload(serde_json::json!({ "needs_frame": false })),
            )]),
    );
    let resource = SnapshotEnvelope::new(
        ProbeId::new("resource.clean").unwrap(),
        SnapshotKind::Resource,
        SnapshotTree::new([SnapshotNode::new("resource.root", "Resources").with_child(
            SnapshotNode::new("resource.query.projects", "Projects").with_payload(
                serde_json::json!({
                    "status": "Success",
                    "error": null,
                    "fetch_attempts": 1
                }),
            ),
        )]),
    );
    let layout_domain = DevtoolsDomainSnapshot::new(
        DevtoolsDomainId::new("layout.clean"),
        target_id.clone(),
        DevtoolsDomainKind::Layout,
        "Layout",
    )
    .with_snapshot(layout.clone());
    let timeline_domain = DevtoolsDomainSnapshot::new(
        DevtoolsDomainId::new("timeline.clean"),
        target_id.clone(),
        DevtoolsDomainKind::Timeline,
        "Timeline",
    )
    .with_snapshot(timeline.clone());
    let motion_domain = DevtoolsDomainSnapshot::new(
        DevtoolsDomainId::new("motion.clean"),
        target_id.clone(),
        DevtoolsDomainKind::Motion,
        "Motion",
    )
    .with_snapshot(motion.clone());
    let resource_domain = DevtoolsDomainSnapshot::new(
        DevtoolsDomainId::new("resource.clean"),
        target_id.clone(),
        DevtoolsDomainKind::Data,
        "Resource",
    )
    .with_summary(serde_json::json!({
        "resource_count": 1,
        "mutation_count": 0,
        "paginated_count": 0,
        "redacted_values": 0
    }))
    .with_snapshot(resource.clone());
    let capture = capture_with_domains(
        target_id,
        [
            command_domain,
            form_domain,
            layout_domain,
            timeline_domain,
            motion_domain,
            resource_domain,
        ],
        [layout, timeline, motion, resource],
    );

    let report = DevtoolsReport::from_capture(&capture);
    let domain_rule_prefixes = [
        "devtools.layout.",
        "devtools.timeline.",
        "devtools.motion.",
        "devtools.command.",
        "devtools.form.",
        "devtools.resource.",
    ];

    assert!(
        report.findings.iter().all(|finding| !domain_rule_prefixes
            .iter()
            .any(|prefix| finding.id.starts_with(prefix))),
        "{:?}",
        report.findings
    );
    assert_eq!(report.summary.finding_count, 0);
}

fn capture_with_domains<const D: usize, const S: usize>(
    target_id: DevtoolsTargetId,
    domains: [DevtoolsDomainSnapshot; D],
    snapshots: [SnapshotEnvelope; S],
) -> DevtoolsCapture {
    DevtoolsCapture::new(
        DevtoolsTargetTree::new([DevtoolsTargetSnapshot::new(
            target_id,
            DevtoolsTargetKind::App,
            "App",
        )]),
        domains,
        Vec::<DevtoolsEventRecord>::new(),
        snapshots,
        Vec::<SnapshotDiagnostic>::new(),
    )
}

fn finding_ids(report: &DevtoolsReport) -> Vec<&str> {
    report
        .findings
        .iter()
        .map(|finding| finding.id.as_str())
        .collect()
}
