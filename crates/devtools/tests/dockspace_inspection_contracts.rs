#![cfg(feature = "docking")]

use std::collections::BTreeSet;

use open_gpui::{WindowFrameSubmissionSnapshot, WindowId};
use open_gpui_devtools::{
    DevtoolsCaptureProvider as _, DevtoolsRegistry, ProbeSnapshotError,
    docking::{self, DockingSubmissionOutcome},
};
use open_gpui_docking::{
    DockInspectionError, DockModelSurfaceInspection, DockMountInspection, DockNativeInspection,
    DockRegionInspection, DockSurfaceInspection, dockspace::model::SurfaceId,
};

fn inspection() -> DockSurfaceInspection {
    DockSurfaceInspection {
        model_surface_count: 2,
        open_pane_count: 3,
        registered_pane_count: 4,
        surfaces: vec![
            DockModelSurfaceInspection {
                surface: SurfaceId::new(10),
                open_pane_count: 2,
                contained_count: 1,
            },
            DockModelSurfaceInspection {
                surface: SurfaceId::new(20),
                open_pane_count: 1,
                contained_count: 0,
            },
        ],
        mounts: vec![DockMountInspection {
            window_id: 7,
            attached: true,
            serial: 5,
            waiting_for_update: false,
            update_pending: false,
            submission: Some(WindowFrameSubmissionSnapshot::submitted_for_test(
                WindowId::from(7),
                91,
            )),
            native_binding: None,
            regions: vec![DockRegionInspection {
                surface: SurfaceId::new(10),
                dirty: false,
                packet_deferred: Some(false),
                input_serial: Some(5),
                input_native_binding: None,
                input_binding_current: None,
                last_presented_serial: Some(5),
                current_presentation: true,
            }],
        }],
        update_pending: false,
        active_gesture: false,
        pending_presentation: true,
        pending_presentation_reports: false,
        queued_input_count: 2,
        queued_native_effect_count: 0,
        native_window_binding_count: 1,
        pending_native_input: true,
        pending_native_create: false,
        native: Some(DockNativeInspection {
            admission_frozen: true,
            bridge_available: false,
            quiescent: false,
            transport_empty: false,
            retained_report_count: 4,
            pending_host_report_count: 2,
            callback_window_count: 1,
            root_window_count: 1,
            retiring_binding_count: 3,
            pending_root_registration_count: 1,
            root_retirement_count: 2,
            pending_root_terminal_count: 1,
            pending_root_quiescence_count: 2,
            pump_scheduled: true,
            continuation_pending: false,
        }),
        last_error: None,
    }
}

#[test]
fn default_docking_projects_owned_facts_without_consuming_or_inventing_authority() {
    let source = inspection();
    let before = source.clone();
    let projected = docking::docking_surface_inspection(&source);
    let capture = docking::docking_surface_capture(&source);
    assert_eq!(capture, docking::docking_surface_capture(&source));
    assert!(capture.diff_from(&capture).is_empty());
    assert_eq!(source, before);
    assert_eq!(projected.summary.model_surface_count, 2);
    assert_eq!(projected.summary.open_pane_count, 3);
    assert_eq!(projected.summary.registered_pane_count, 4);
    assert_eq!(projected.summary.physical_mount_count, 1);
    assert_eq!(projected.summary.queued_input_count, 2);
    assert!(projected.summary.pending_native_input);
    assert_eq!(projected.surfaces[0].contained_count, 1);
    assert_eq!(
        projected.mounts[0].regions[0].surface_target_id,
        projected.surfaces[0].target_id
    );
    let submission = projected.mounts[0].submission.as_ref().unwrap();
    assert_eq!(submission.outcome, DockingSubmissionOutcome::Submitted);
    assert_eq!(submission.accepted_frame_generation, Some(91));
    assert_eq!(submission.submitted_frame_generation, Some(91));
    let native = projected.native.unwrap();
    assert!(native.admission_frozen);
    assert!(!native.bridge_available);
    assert!(!native.quiescent);
    assert_eq!(native.retained_report_count, 4);
    assert_eq!(native.pending_host_report_count, 2);
    assert_eq!(native.pending_root_terminal_count, 1);
    assert_eq!(native.pending_root_quiescence_count, 2);
    assert_eq!(capture.targets.targets.len(), 4);
    assert_eq!(capture.domains.len(), 1);
    assert_eq!(capture.snapshots.len(), 1);
    assert!(capture.events.is_empty());
    assert!(capture.diagnostics.is_empty());
    assert_eq!(
        capture.snapshots[0].tree,
        *docking::docking_surface_probe_snapshot(&source).tree()
    );
    let serialized = serde_json::to_string(&capture).unwrap();
    assert!(!serialized.contains("capabilities"));
    assert!(!serialized.contains("NativeWindowBinding"));
    assert!(!serialized.contains("NativeSurfaceBinding"));
}

#[test]
fn absent_native_and_submission_facts_stay_absent_instead_of_becoming_success() {
    let mut source = inspection();
    source.native = None;
    source.native_window_binding_count = 0;
    source.pending_native_input = false;
    source.mounts[0].submission = None;
    source.mounts[0].regions[0].packet_deferred = None;
    source.mounts[0].regions[0].last_presented_serial = None;
    source.mounts[0].regions[0].current_presentation = false;
    let projected = docking::docking_surface_inspection(&source);
    assert!(!projected.summary.managed_native);
    assert!(projected.native.is_none());
    assert!(projected.mounts[0].submission.is_none());
    let region = &projected.mounts[0].regions[0];
    assert_eq!(region.packet_deferred, None);
    assert_eq!(region.input_binding_current, None);
    assert_eq!(region.input_matches_mount_binding, None);
    assert_eq!(region.last_presented_serial, None);
    assert!(!region.current_presentation);
    assert!(
        docking::docking_surface_capture(&source)
            .diagnostics
            .is_empty()
    );
}

#[test]
fn host_errors_and_provider_identities_are_sanitized_before_any_export() {
    let mut source = inspection();
    source.last_error = Some(
        "owner alice@example.com password=raw-error-secret C:\\Users\\Frank\\workspace.txt https://example.test/callback?token=raw-query-secret"
            .to_owned(),
    );
    let original = source.clone();
    let projected = docking::docking_surface_inspection(&source);
    let capture = docking::docking_surface_capture(&source);
    let snapshot = docking::docking_surface_probe_snapshot(&source);
    let provider = docking::docking_surface_capture_provider(
        "owner alice@example.com password=raw-provider-secret C:\\Users\\Frank\\workspace.txt",
        move || Ok(source.clone()),
    )
    .unwrap();
    let provided = provider.capture().unwrap();
    let serialized = format!(
        "{}{}{}{}{}",
        serde_json::to_string(&projected).unwrap(),
        serde_json::to_string(&capture).unwrap(),
        serde_json::to_string(snapshot.tree()).unwrap(),
        serde_json::to_string(&provided).unwrap(),
        provider.id(),
    );
    for sensitive in [
        "alice@example.com",
        "raw-error-secret",
        "raw-query-secret",
        "raw-provider-secret",
        "Frank",
    ] {
        assert!(
            !serialized.contains(sensitive),
            "leaked {sensitive}: {serialized}"
        );
    }
    assert!(serialized.contains("[redacted"));
    assert_eq!(projected.redaction.redacted_values, 1);
    assert_eq!(snapshot.redaction().redacted_values, 1);
    assert_eq!(capture.snapshots[0].redaction.redacted_values, 1);
    assert_eq!(provided.snapshots[0].redaction.redacted_values, 1);
    assert_eq!(capture.diagnostics.len(), 1);
    assert_eq!(capture.diagnostics[0].code, docking::DOCKING_HOST_ERROR);
    assert_eq!(provided, provider.capture().unwrap());
    assert!(
        original
            .last_error
            .as_ref()
            .unwrap()
            .contains("raw-error-secret")
    );
}

#[test]
fn json_host_errors_redact_quoted_credentials_before_every_export() {
    for error in [
        r#"{"password":"review-canary"}"#,
        "{\n  \"password\": \"review-canary with spaces\",\n  \"stage\": \"open\"\n}",
        r#"[{"nested":{"api_key":"review-canary"},"stage":"open"}]"#,
    ] {
        let mut source = inspection();
        source.last_error = Some(error.to_owned());
        let projected = docking::docking_surface_inspection(&source);
        let capture = docking::docking_surface_capture(&source);
        let snapshot = docking::docking_surface_probe_snapshot(&source);
        let provider_source = source.clone();
        let provider = docking::docking_surface_capture_provider("json-error", move || {
            Ok(provider_source.clone())
        })
        .unwrap();
        let provided = provider.capture().unwrap();
        let serialized = format!(
            "{}{}{}{}",
            serde_json::to_string(&projected).unwrap(),
            serde_json::to_string(&capture).unwrap(),
            serde_json::to_string(snapshot.tree()).unwrap(),
            serde_json::to_string(&provided).unwrap(),
        );
        assert!(!serialized.contains("review-canary"), "{serialized}");
        assert!(serialized.contains("[redacted]"));
        assert_eq!(projected.redaction.redacted_values, 1);
        assert_eq!(snapshot.redaction().redacted_values, 1);
        assert_eq!(capture.snapshots[0].redaction.redacted_values, 1);
        assert_eq!(provided.snapshots[0].redaction.redacted_values, 1);
        assert_eq!(source.last_error.as_deref(), Some(error));
    }
}

#[test]
fn providers_namespace_targets_domains_probes_nodes_and_diagnostics_without_collisions() {
    let mut source = inspection();
    source.last_error = Some("retained transport error".to_owned());
    let mut registry = DevtoolsRegistry::default();
    for id in ["dock surface", "dock-surface"] {
        let source = source.clone();
        registry
            .register_capture_provider(
                docking::docking_surface_capture_provider(id, move || Ok(source.clone())).unwrap(),
            )
            .unwrap();
    }
    let capture = registry.collect_capture();
    assert_eq!(capture, registry.collect_capture());
    assert_eq!(capture.targets.targets.len(), 8);
    let target_ids: BTreeSet<_> = capture
        .targets
        .targets
        .iter()
        .map(|target| &target.id)
        .collect();
    assert_eq!(target_ids.len(), 8);
    assert!(capture.targets.targets.iter().all(|target| {
        target
            .parent_id
            .as_ref()
            .is_none_or(|parent| target_ids.contains(parent))
    }));
    assert_eq!(
        capture
            .domains
            .iter()
            .map(|domain| &domain.id)
            .collect::<BTreeSet<_>>()
            .len(),
        2
    );
    assert_eq!(
        capture
            .snapshots
            .iter()
            .map(|snapshot| &snapshot.probe_id)
            .collect::<BTreeSet<_>>()
            .len(),
        2
    );
    assert_eq!(capture.diagnostics.len(), 2);
    for domain in &capture.domains {
        assert!(target_ids.contains(&domain.target_id));
        assert_eq!(
            domain.snapshot.as_ref().unwrap().probe_id.as_str(),
            domain.id.as_str()
        );
        assert_eq!(domain.diagnostics[0].probe_id.as_str(), domain.id.as_str());
    }
    let node_ids: Vec<_> = capture
        .snapshots
        .iter()
        .flat_map(|snapshot| {
            snapshot.tree.nodes.iter().flat_map(|root| {
                std::iter::once(&root.id).chain(root.children.iter().map(|child| &child.id))
            })
        })
        .collect();
    assert_eq!(
        node_ids.len(),
        node_ids.iter().collect::<BTreeSet<_>>().len()
    );
    assert_eq!(capture.diff_from(&capture).summary.collisions, 0);
}

#[test]
fn a_busy_provider_returns_an_error_without_publishing_a_stale_or_empty_workspace() {
    let provider = docking::docking_surface_capture_provider("busy.docking", || {
        Err(DockInspectionError::Busy)
    })
    .unwrap();
    assert_eq!(
        provider.capture(),
        Err(ProbeSnapshotError::CollectionFailed(
            "docking host is busy".to_owned()
        ))
    );
    let mut registry = DevtoolsRegistry::default();
    registry.register_capture_provider(provider).unwrap();
    let capture = registry.collect_capture();
    assert!(capture.targets.targets.is_empty());
    assert!(capture.domains.is_empty());
    assert!(capture.snapshots.is_empty());
    assert_eq!(capture.diagnostics.len(), 1);
}
