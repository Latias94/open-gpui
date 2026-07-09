#![cfg(feature = "docking")]

use open_gpui::WindowId;
use open_gpui_devtools::{
    DevtoolsDiffKind, DevtoolsDiffStatus, DevtoolsReport, docking,
    docking::{
        DOCKING_PLATFORM_VIEWPORT_WINDOWS_UNSUPPORTED, DOCKING_VIEWPORT_ROUTE_FACTS_MISSING,
        DOCKING_VIEWPORT_ROUTE_FACTS_STALE,
    },
};
use open_gpui_docking::{
    DockItemId, DockSpaceId,
    advanced::{
        DockViewportDropOutcomeKind, DockViewportDropOutcomeRecord, DockViewportInputStatus,
        DockViewportLifecycleRecord, DockViewportPayloadRecord,
        DockViewportPlatformCapabilityRecord, DockViewportPlatformRequestStatus,
        DockViewportRestoreReadinessRecord, DockViewportRouteStatus, DockViewportRuntimeStatus,
        DockViewportStaleStatusReason, DockViewportTearOffOutcomeKind,
        DockViewportTearOffPlacementRecord, DockViewportTearOffRecord,
        DockViewportVisualAffordanceRecord, DockVisualAffordanceDebugLayer,
        DockVisualAffordanceDebugSummary,
    },
};

#[test]
fn docking_runtime_inspection_projects_public_status_rows() {
    let status = runtime_status(false);
    let inspection = docking::docking_runtime_inspection(&status);

    assert_eq!(inspection.summary.platform_capabilities_present, true);
    assert_eq!(inspection.summary.platform_viewport_windows, Some(false));
    assert_eq!(inspection.summary.viewport_lifecycle_count, 1);
    assert_eq!(inspection.summary.route_ready_count, 1);
    assert_eq!(inspection.summary.runtime_event_count, 2);
    assert_eq!(inspection.summary.visual_affordance_count, 1);
    assert_eq!(inspection.summary.diagnostic_count, 1);
    assert_eq!(
        inspection
            .platform_capabilities
            .as_ref()
            .map(|capabilities| capabilities.platform_viewport_windows),
        Some(false)
    );
    assert_eq!(
        inspection.placement_restore.as_ref().map(|placement| (
            placement.matched,
            placement.missing,
            placement.has_missing
        )),
        Some((2, 1, true))
    );

    let viewport = &inspection.viewport_lifecycle[0];
    assert_eq!(viewport.space, "primary");
    assert_eq!(viewport.window_id, WindowId::from(7).as_u64());
    assert_eq!(viewport.route_status, "route-ready");
    assert_eq!(viewport.input_status, "receives-input");
    assert_eq!(viewport.resize_requested, true);

    let drop_outcome = inspection
        .runtime_events
        .iter()
        .find(|row| row.event_id == "docking.last-drop-outcome")
        .expect("last drop outcome row is present");
    assert_eq!(drop_outcome.label, "Last drop outcome");
    assert_eq!(drop_outcome.payload["kind"], "Error");

    let tear_off = inspection
        .runtime_events
        .iter()
        .find(|row| row.event_id == "docking.last-tear-off")
        .expect("last tear-off row is present");
    assert_eq!(tear_off.payload["kind"], "Completed");

    let visual = &inspection.visual_affordances[0];
    assert_eq!(visual.space, "primary");
    assert_eq!(visual.layer_count, 2);
    assert_eq!(visual.active_layer_id.as_deref(), Some("active-layer"));
    assert_eq!(visual.active_has_label, true);

    let serialized = serde_json::to_string(&inspection).unwrap();
    assert!(serialized.contains("docking.platform_viewport_windows.unsupported"));
    assert!(!serialized.contains("Sensitive Editor Label"));

    let capture_json = serde_json::to_string(&docking::docking_runtime_capture(&status)).unwrap();
    assert!(capture_json.contains("\"has_label\":true"));
    assert!(!capture_json.contains("Sensitive Editor Label"));
}

#[test]
fn docking_runtime_capture_attaches_explicit_capability_diagnostics() {
    let unsupported = docking::docking_runtime_capture(&runtime_status(false));
    let supported = docking::docking_runtime_capture(&runtime_status(true));
    let absent = docking::docking_runtime_capture(&DockViewportRuntimeStatus::default());

    assert!(
        unsupported
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DOCKING_PLATFORM_VIEWPORT_WINDOWS_UNSUPPORTED)
    );
    assert!(
        unsupported.domains[0]
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DOCKING_PLATFORM_VIEWPORT_WINDOWS_UNSUPPORTED)
    );
    assert!(supported.diagnostics.is_empty());
    assert!(supported.domains[0].diagnostics.is_empty());
    assert!(absent.diagnostics.is_empty());
    assert!(absent.domains[0].diagnostics.is_empty());
}

#[test]
fn docking_runtime_report_surfaces_explicit_route_fact_findings() {
    let mut status = runtime_status(false);
    status.viewport_lifecycle.push(DockViewportLifecycleRecord {
        space: DockSpaceId::from("missing-route"),
        window_id: WindowId::from(8),
        route_status: DockViewportRouteStatus::MissingRouteFacts,
        input_status: DockViewportInputStatus::ReceivesInput,
        platform_request_status: DockViewportPlatformRequestStatus::default(),
        coordinate_status: None,
        facts_generation: 12,
    });
    status.viewport_lifecycle.push(DockViewportLifecycleRecord {
        space: DockSpaceId::from("stale-route"),
        window_id: WindowId::from(9),
        route_status: DockViewportRouteStatus::Stale {
            reason: DockViewportStaleStatusReason::WindowFactsChanged,
        },
        input_status: DockViewportInputStatus::ReceivesInput,
        platform_request_status: DockViewportPlatformRequestStatus::default(),
        coordinate_status: None,
        facts_generation: 13,
    });

    let capture = docking::docking_runtime_capture(&status);
    let report = DevtoolsReport::from_capture(&capture);

    assert!(report.findings.iter().any(|finding| {
        finding.id == format!("capture-diagnostic.{DOCKING_PLATFORM_VIEWPORT_WINDOWS_UNSUPPORTED}")
            && finding.severity.as_label() == "warning"
    }));
    assert!(report.findings.iter().any(|finding| {
        finding.id == format!("capture-diagnostic.{DOCKING_VIEWPORT_ROUTE_FACTS_MISSING}")
            && finding.severity.as_label() == "error"
    }));
    assert!(report.findings.iter().any(|finding| {
        finding.id == format!("capture-diagnostic.{DOCKING_VIEWPORT_ROUTE_FACTS_STALE}")
            && finding.severity.as_label() == "warning"
    }));
}

#[test]
fn docking_runtime_drop_tear_off_and_placement_changes_are_diffable() {
    let mut previous = runtime_status(true);
    previous.placement_restore = None;
    previous.last_drop_outcome = None;
    previous.last_tear_off = None;

    let current = runtime_status(true);
    let previous_capture = docking::docking_runtime_capture(&previous);
    let current_capture = docking::docking_runtime_capture(&current);
    let diff = current_capture.diff_from(&previous_capture);

    assert!(
        diff.rows.iter().any(|row| {
            row.kind == DevtoolsDiffKind::Snapshot
                && row.status == DevtoolsDiffStatus::Changed
                && row.identity.contains("docking.runtime")
        }),
        "placement restore should change the docking snapshot"
    );
    assert!(
        diff.rows.iter().any(|row| {
            row.kind == DevtoolsDiffKind::Event
                && row.status == DevtoolsDiffStatus::Added
                && row.identity.contains("docking.last-drop-outcome")
        }),
        "new drop event should be added"
    );
    assert!(
        diff.rows.iter().any(|row| {
            row.kind == DevtoolsDiffKind::Event
                && row.status == DevtoolsDiffStatus::Added
                && row.identity.contains("docking.last-tear-off")
        }),
        "new tear-off event should be added"
    );
}

fn runtime_status(platform_viewport_windows: bool) -> DockViewportRuntimeStatus {
    let mut status = DockViewportRuntimeStatus::default();
    status.platform_capabilities = Some(DockViewportPlatformCapabilityRecord {
        platform_viewport_windows,
        global_window_bounds: true,
        window_stack: true,
        display_work_area: true,
        dpi_scale: true,
        live_window_move: false,
        no_input_windows: true,
        hovered_window_ignores_no_input: false,
    });
    status.placement_restore = Some(DockViewportRestoreReadinessRecord {
        matched: 2,
        missing: 1,
    });
    status.viewport_lifecycle.push(DockViewportLifecycleRecord {
        space: DockSpaceId::from("primary"),
        window_id: WindowId::from(7),
        route_status: DockViewportRouteStatus::RouteReady,
        input_status: DockViewportInputStatus::ReceivesInput,
        platform_request_status: DockViewportPlatformRequestStatus {
            close_requested: false,
            resize_requested: true,
        },
        coordinate_status: None,
        facts_generation: 11,
    });
    status.last_drop_outcome = Some(DockViewportDropOutcomeRecord {
        kind: DockViewportDropOutcomeKind::Error,
        action: None,
        error: None,
    });
    status.last_tear_off = Some(DockViewportTearOffRecord {
        kind: DockViewportTearOffOutcomeKind::Completed,
        placement_source: Some(DockViewportTearOffPlacementRecord::Suggested),
        source_space: DockSpaceId::from("primary"),
        target_space: DockSpaceId::from("secondary"),
        payload: DockViewportPayloadRecord::Item {
            item: DockItemId::from("editor"),
        },
    });
    status
        .visual_affordances
        .push(DockViewportVisualAffordanceRecord {
            space: DockSpaceId::from("primary"),
            window_id: WindowId::from(7),
            summary: DockVisualAffordanceDebugSummary {
                space: Some("primary".to_owned()),
                frame_generation: Some(3),
                layer_count: 2,
                active_count: 1,
                active: Some(DockVisualAffordanceDebugLayer {
                    id: "active-layer".to_owned(),
                    kind: "guide".to_owned(),
                    scope: "viewport".to_owned(),
                    state: "active".to_owned(),
                    target_node: Some(77),
                    zone: None,
                    payload_index: Some(0),
                    label: Some("Sensitive Editor Label".to_owned()),
                }),
                motion_state: Some("settled".to_owned()),
                churn_signature: "primary:2:1".to_owned(),
            },
        });
    status
}
