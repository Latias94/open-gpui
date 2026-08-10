#![cfg(feature = "docking")]

use open_gpui::{
    Bounds, PlatformWindowCapabilities, PlatformWindowCreationCapabilities,
    PlatformWindowMutationCapabilities, QuitMode, WindowActivationPolicy, WindowBounds,
    WindowCoordinateSpace, WindowCreationSupport, WindowId, WindowInitialPresentationOrder,
    WindowKind, WindowMutationRequest, WindowMutationSupport, WindowOptions, WindowPlatformFacts,
    point, px, size,
};
use open_gpui_devtools::{
    DevtoolsDiffKind, DevtoolsDiffStatus, DevtoolsRegistry, DevtoolsReport, docking,
    docking::{
        DOCKING_PLATFORM_VIEWPORT_WINDOWS_UNSUPPORTED, DOCKING_VIEWPORT_ROUTE_FACTS_MISSING,
        DOCKING_VIEWPORT_ROUTE_FACTS_STALE,
    },
};
use open_gpui_docking::{
    DockItemId, DockSpaceId, DockSurface, DockSurfacePrimaryWindowOpenOutcome,
    DockSurfaceViewportOpenOutcome,
    advanced::{
        DockViewportDropOutcomeKind, DockViewportDropOutcomeRecord, DockViewportInputStatus,
        DockViewportLifecycleRecord, DockViewportPayloadRecord,
        DockViewportPlatformCapabilityRecord, DockViewportPlatformRequestStatus,
        DockViewportPlatformSyncDispatch, DockViewportPlatformSyncDomain,
        DockViewportPlatformSyncObservation, DockViewportPlatformSyncObservationOutcome,
        DockViewportPlatformSyncObservedRecord, DockViewportPlatformSyncRecord,
        DockViewportPlatformSyncRequest, DockViewportRestoreReadinessRecord,
        DockViewportRouteStatus, DockViewportRuntimeStatus, DockViewportStaleStatusReason,
        DockViewportTearOffOutcomeKind, DockViewportTearOffPlacementRecord,
        DockViewportTearOffRecord, DockViewportVisualAffordanceRecord,
        DockViewportWindowOwnershipStatus, DockViewportWindowProfileRecord,
        DockVisualAffordanceDebugLayer, DockVisualAffordanceDebugSummary,
    },
};
use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

#[open_gpui::test]
fn docking_surface_inspection_projects_window_session_authority(
    cx: &mut open_gpui::TestAppContext,
) {
    let (session, runtime) = cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .build(cx)
            .expect("surface should build");
        (
            surface.window_session_status(cx),
            surface.viewports().runtime_status(cx),
        )
    });

    let inspection = docking::docking_surface_inspection(session, &runtime);
    assert_eq!(inspection.session.phase, "vacant");
    assert_eq!(inspection.session.generation, 0);
    assert_eq!(inspection.session.anchor_window_id, None);
    assert_eq!(inspection.session.reason_kind, None);
    assert_eq!(inspection.session.reason_detail, None);
    assert_eq!(inspection.session.owned_window_count, 0);
    assert_eq!(inspection.session.opening_window_count, 0);
    assert_eq!(inspection.session.active_window_count, 0);
    assert_eq!(inspection.session.retiring_window_count, 0);
    assert_eq!(inspection.session.terminal_ticket_count, 0);
    assert_eq!(inspection.session.pending_terminal_ticket_count, 0);
    assert_eq!(inspection.session.runtime_empty, None);

    let capture = docking::docking_surface_capture(session, &runtime);
    let surface_target = capture
        .targets
        .targets
        .iter()
        .find(|target| target.id.as_str() == "docking.surface")
        .expect("surface target should be present");
    let runtime_target = capture
        .targets
        .targets
        .iter()
        .find(|target| target.id.as_str() == "docking.runtime")
        .expect("runtime target should be present");
    assert_eq!(runtime_target.parent_id.as_ref(), Some(&surface_target.id));
    assert!(capture.domains.iter().any(|domain| {
        domain.id.as_str() == "docking.surface"
            && domain
                .summary
                .as_ref()
                .is_some_and(|summary| summary["phase"] == "vacant" && summary["generation"] == 0)
    }));
    assert!(capture.snapshots.iter().any(|snapshot| {
        snapshot.probe_id.as_str() == "docking.surface"
            && serde_json::to_value(snapshot)
                .is_ok_and(|value| value.to_string().contains("window-session"))
    }));
}

#[open_gpui::test]
fn docking_surface_capture_providers_namespace_every_capture_identity(
    cx: &mut open_gpui::TestAppContext,
) {
    let (left_session, left_runtime, right_session, right_runtime) = cx.update(|cx| {
        let left = DockSurface::builder("left")
            .build(cx)
            .expect("left surface should build");
        let right = DockSurface::builder("right")
            .build(cx)
            .expect("right surface should build");
        (
            left.window_session_status(cx),
            left.viewports().runtime_status(cx),
            right.window_session_status(cx),
            right.viewports().runtime_status(cx),
        )
    });

    let mut registry = DevtoolsRegistry::default();
    registry
        .register_capture_provider(
            docking::docking_surface_capture_provider("dock surface", move || {
                (left_session, left_runtime.clone())
            })
            .expect("left provider id should be valid"),
        )
        .expect("left provider should register");
    registry
        .register_capture_provider(
            docking::docking_surface_capture_provider("dock-surface", move || {
                (right_session, right_runtime.clone())
            })
            .expect("right provider id should be valid"),
        )
        .expect("right provider should register");

    let capture = registry.collect_capture();
    assert!(capture.diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.code.as_str(),
            "capture.duplicate_target" | "capture.duplicate_domain" | "capture.duplicate_probe"
        )
    }));

    let surface_targets = capture
        .targets
        .targets
        .iter()
        .filter(|target| target.label == "Docking surface")
        .collect::<Vec<_>>();
    let runtime_targets = capture
        .targets
        .targets
        .iter()
        .filter(|target| target.label == "Docking runtime")
        .collect::<Vec<_>>();
    assert_eq!(surface_targets.len(), 2);
    assert_eq!(runtime_targets.len(), 2);
    assert_ne!(surface_targets[0].id, surface_targets[1].id);

    let surface_ids = surface_targets
        .iter()
        .map(|target| target.id.clone())
        .collect::<BTreeSet<_>>();
    let runtime_parents = runtime_targets
        .iter()
        .map(|target| {
            target
                .parent_id
                .clone()
                .expect("each provider runtime should belong to its surface")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(runtime_parents, surface_ids);
    assert_eq!(
        capture
            .domains
            .iter()
            .map(|domain| domain.id.clone())
            .collect::<BTreeSet<_>>()
            .len(),
        4
    );
    assert_eq!(
        capture
            .snapshots
            .iter()
            .map(|snapshot| snapshot.probe_id.clone())
            .collect::<BTreeSet<_>>()
            .len(),
        4
    );
}

#[open_gpui::test]
fn docking_surface_inspection_projects_active_and_shutting_down_owned_windows(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.update(|cx| cx.set_quit_mode(QuitMode::Explicit));
    let (surface, anchor, dependent) = cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .allow_platform_viewports(true)
            .build(cx)
            .expect("surface should build");
        let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("primary should open, got {outcome:?}"),
        };
        let dependent = match surface
            .viewports()
            .open("secondary", WindowOptions::default(), cx)
        {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("dependent should open, got {outcome:?}"),
        };
        (surface, anchor, dependent)
    });

    let active = cx.update(|cx| {
        docking::docking_surface_inspection(
            surface.window_session_status(cx),
            &surface.viewports().runtime_status(cx),
        )
    });
    assert_eq!(active.session.phase, "active");
    assert_eq!(active.session.owned_window_count, 2);
    assert_eq!(active.session.opening_window_count, 0);
    assert_eq!(active.session.active_window_count, 2);
    assert_eq!(active.session.retiring_window_count, 0);

    let shutting_down = Rc::new(RefCell::new(None));
    cx.update(|cx| {
        let surface = surface.clone();
        let shutting_down = shutting_down.clone();
        cx.on_window_closed(move |cx, window_id| {
            if window_id == dependent.window_id() {
                shutting_down.replace(Some(docking::docking_surface_inspection(
                    surface.window_session_status(cx),
                    &surface.viewports().runtime_status(cx),
                )));
            }
        })
        .detach();
    });

    let close = cx.simulate_window_close_request(anchor);
    assert!(!close.native_close_allowed());
    assert!(close.logical_window_removed());
    cx.run_until_parked();

    let shutting_down = shutting_down
        .borrow()
        .clone()
        .expect("dependent terminal should expose the in-flight shutdown state");
    assert_eq!(shutting_down.session.phase, "shutting-down");
    assert!(shutting_down.session.owned_window_count >= 1);
    assert_eq!(shutting_down.session.opening_window_count, 0);
    assert_eq!(shutting_down.session.active_window_count, 0);
    assert_eq!(
        shutting_down.session.retiring_window_count,
        shutting_down.session.owned_window_count
    );

    let closed = cx.update(|cx| {
        docking::docking_surface_inspection(
            surface.window_session_status(cx),
            &surface.viewports().runtime_status(cx),
        )
    });
    assert_eq!(closed.session.phase, "closed");
    assert_eq!(closed.session.owned_window_count, 0);
}

#[test]
fn docking_runtime_inspection_projects_public_status_rows() {
    let status = runtime_status(false);
    let inspection = docking::docking_runtime_inspection(&status);

    assert_eq!(inspection.summary.platform_capabilities_present, true);
    assert_eq!(inspection.summary.platform_viewport_windows, Some(false));
    assert_eq!(inspection.summary.owned_window_count, 3);
    assert_eq!(inspection.summary.opening_window_count, 1);
    assert_eq!(inspection.summary.active_window_count, 1);
    assert_eq!(inspection.summary.retiring_window_count, 1);
    assert_eq!(inspection.summary.viewport_lifecycle_count, 1);
    assert_eq!(inspection.summary.window_profile_count, 1);
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
        inspection.window_profiles,
        vec![docking::DockingViewportWindowProfileRow {
            space: "primary".to_string(),
            window_id: WindowId::from(7).as_u64(),
            window_kind: "floating".to_string(),
            capabilities: docking::DockingWindowCapabilitiesRow {
                creation: docking::DockingWindowCreationCapabilityRow {
                    focus_on_appearing: docking::DockingWindowCreationSupport::Supported,
                    transient_for: docking::DockingWindowCreationSupport::Supported,
                    initial_presentation_order:
                        docking::DockingWindowInitialPresentationOrder::BeforeVisibility,
                },
                mutations: docking::DockingWindowMutationCapabilityRow {
                    position: docking::DockingWindowMutationSupport::CreationOnly,
                    size: docking::DockingWindowMutationSupport::Live,
                    windowed: docking::DockingWindowMutationSupport::Live,
                    maximized: docking::DockingWindowMutationSupport::Live,
                    fullscreen: docking::DockingWindowMutationSupport::Live,
                    minimized: docking::DockingWindowMutationSupport::Unsupported,
                    restore_bounds: docking::DockingWindowMutationSupport::CreationOnly,
                    pointer_input: docking::DockingWindowMutationSupport::Live,
                    activation_policy: docking::DockingWindowMutationSupport::Live,
                    alpha: docking::DockingWindowMutationSupport::CreationOnly,
                    topmost: docking::DockingWindowMutationSupport::Unsupported,
                    taskbar_visibility: docking::DockingWindowMutationSupport::Unsupported,
                    coordinate_space: docking::DockingWindowCoordinateSpace::WindowLocal,
                },
            },
        }]
    );
    let inspection_json = serde_json::to_value(&inspection).unwrap();
    let capabilities = &inspection_json["window_profiles"][0]["capabilities"];
    assert_eq!(capabilities["creation"]["focus_on_appearing"], "supported");
    assert_eq!(capabilities["creation"]["transient_for"], "supported");
    assert_eq!(
        capabilities["creation"]["initial_presentation_order"],
        "before-visibility"
    );
    assert_eq!(capabilities["mutations"]["activation_policy"], "live");
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
fn docking_runtime_inspection_preserves_presentation_established_visibility() {
    let mut status = runtime_status(true);
    status.window_profiles[0]
        .capabilities
        .creation
        .initial_presentation_order =
        WindowInitialPresentationOrder::PresentationEstablishesVisibility;

    let inspection = docking::docking_runtime_inspection(&status);
    let inspection_json = serde_json::to_value(inspection).unwrap();

    assert_eq!(
        inspection_json["window_profiles"][0]["capabilities"]["creation"]["initial_presentation_order"],
        "presentation-establishes-visibility"
    );
}

#[test]
fn docking_runtime_observations_preserve_typed_request_and_committed_facts() {
    let observed_bounds = Bounds::new(point(px(10.0), px(20.0)), size(px(320.0), px(240.0)));
    let mut status = DockViewportRuntimeStatus::default();
    status
        .recent_platform_observations
        .push(DockViewportPlatformSyncObservedRecord {
            window_id: WindowId::from(12),
            observation: DockViewportPlatformSyncObservation {
                domain: DockViewportPlatformSyncDomain::ActivationPolicy,
                generation: 7,
                request: WindowMutationRequest::ActivationPolicy(WindowActivationPolicy {
                    accepts_activation: false,
                    focus_on_click: true,
                }),
                outcome: DockViewportPlatformSyncObservationOutcome::Adjusted,
                facts: WindowPlatformFacts {
                    bounds: observed_bounds,
                    coordinate_space: WindowCoordinateSpace::WindowLocal,
                    physical_geometry: None,
                    window_bounds: WindowBounds::Windowed(observed_bounds),
                    inner_window_bounds: WindowBounds::Windowed(observed_bounds),
                    content_size: observed_bounds.size,
                    scale_factor: 1.5,
                    display_id: None,
                    is_minimized: false,
                    is_maximized: false,
                    is_fullscreen: false,
                    accepts_pointer_input: true,
                    accepts_activation: true,
                    focus_on_click: true,
                    background_appearance: open_gpui::WindowBackgroundAppearance::Opaque,
                    topmost: false,
                    taskbar_visible: true,
                    is_active: true,
                },
            },
        });

    let inspection = docking::docking_runtime_inspection(&status);
    let event = inspection
        .runtime_events
        .iter()
        .find(|event| event.event_id == "docking.platform-observations")
        .expect("terminal observations should be projected as a runtime event");

    assert_eq!(event.payload[0]["window_id"], WindowId::from(12).as_u64());
    assert_eq!(event.payload[0]["generation"], 7);
    assert_eq!(event.payload[0]["request"]["kind"], "activation-policy");
    assert_eq!(event.payload[0]["request"]["accepts_activation"], false);
    assert_eq!(event.payload[0]["request"]["focus_on_click"], true);
    assert_eq!(event.payload[0]["outcome"], "Adjusted");
    assert_eq!(
        event.payload[0]["facts"]["coordinate_space"],
        "window-local"
    );
    assert_eq!(event.payload[0]["facts"]["accepts_pointer_input"], true);
    assert_eq!(event.payload[0]["facts"]["accepts_activation"], true);
    assert_eq!(event.payload[0]["facts"]["focus_on_click"], true);
    assert!(
        event.payload[0]["facts"]
            .get("focus_on_appearing")
            .is_none()
    );
}

#[test]
fn docking_runtime_dispatches_preserve_structured_request_payloads() {
    let mut status = DockViewportRuntimeStatus::default();
    status.last_platform_dispatch = Some(DockViewportPlatformSyncRecord {
        window_id: WindowId::from(18),
        dispatches: vec![DockViewportPlatformSyncDispatch::Queued {
            request: DockViewportPlatformSyncRequest::ActivationPolicy {
                requested: WindowActivationPolicy {
                    accepts_activation: true,
                    focus_on_click: false,
                },
            },
            domain: DockViewportPlatformSyncDomain::ActivationPolicy,
            generation: 11,
        }],
        observations: Vec::new(),
    });

    let inspection = docking::docking_runtime_inspection(&status);
    let event = inspection
        .runtime_events
        .iter()
        .find(|event| event.event_id == "docking.last-platform-dispatch")
        .expect("queued dispatches should be projected as a runtime event");
    let dispatch = &event.payload["dispatches"][0];

    assert_eq!(dispatch["kind"], "queued");
    assert_eq!(dispatch["request"]["kind"], "activation-policy");
    assert_eq!(dispatch["request"]["accepts_activation"], true);
    assert_eq!(dispatch["request"]["focus_on_click"], false);
    assert_eq!(dispatch["domain"], "ActivationPolicy");
    assert_eq!(dispatch["generation"], 11);
    assert!(
        dispatch["request"].is_object(),
        "typed requests must not be degraded to Debug strings"
    );
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
    status.window_ownership = DockViewportWindowOwnershipStatus {
        owned_window_count: 3,
        opening_window_count: 1,
        active_window_count: 1,
        retiring_window_count: 1,
    };
    status.platform_capabilities = Some(DockViewportPlatformCapabilityRecord {
        platform_viewport_windows,
        global_window_bounds: true,
        window_stack: true,
        window_hit_stack: true,
        display_work_area: true,
        dpi_scale: true,
        hovered_window_ignores_no_input: false,
    });
    status
        .window_profiles
        .push(DockViewportWindowProfileRecord {
            space: DockSpaceId::from("primary"),
            window_id: WindowId::from(7),
            window_kind: WindowKind::Floating,
            capabilities: PlatformWindowCapabilities {
                creation: PlatformWindowCreationCapabilities {
                    focus_on_appearing: WindowCreationSupport::Supported,
                    transient_for: WindowCreationSupport::Supported,
                    provisional_presentation: WindowCreationSupport::Unsupported,
                    initial_presentation_order: WindowInitialPresentationOrder::BeforeVisibility,
                },
                mutations: PlatformWindowMutationCapabilities {
                    position: WindowMutationSupport::CreationOnly,
                    size: WindowMutationSupport::Live,
                    windowed: WindowMutationSupport::Live,
                    maximized: WindowMutationSupport::Live,
                    fullscreen: WindowMutationSupport::Live,
                    restore_bounds: WindowMutationSupport::CreationOnly,
                    pointer_input: WindowMutationSupport::Live,
                    activation_policy: WindowMutationSupport::Live,
                    alpha: WindowMutationSupport::CreationOnly,
                    coordinate_space: WindowCoordinateSpace::WindowLocal,
                    ..Default::default()
                },
            },
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
