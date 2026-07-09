#![cfg(any(
    feature = "docking",
    feature = "gpui",
    feature = "motion",
    feature = "ui-components"
))]

#[cfg(feature = "docking")]
use open_gpui_devtools::docking;
#[cfg(feature = "gpui")]
use open_gpui_devtools::gpui;
#[cfg(feature = "motion")]
use open_gpui_devtools::motion;
#[cfg(feature = "ui-components")]
use open_gpui_devtools::ui_components;

#[cfg(feature = "ui-components")]
#[test]
fn framework_adapters_convert_theme_snapshots() {
    use open_gpui_ui_components::ThemeSnapshot;

    let snapshot = ui_components::theme_probe_snapshot(ThemeSnapshot::dark());
    let serialized = serde_json::to_string(&snapshot.tree()).unwrap();

    assert!(serialized.contains("dark"));
    assert!(serialized.contains("color_count"));
    assert!(serialized.contains("semantic.surface"));
    assert!(serialized.contains("semantic.focus_ring"));
}

#[cfg(feature = "ui-components")]
#[test]
fn framework_adapters_convert_accessibility_evidence() {
    use open_gpui_ui_components::COMPONENT_A11Y_EVIDENCE;

    let snapshot = ui_components::a11y_evidence_probe_snapshot(COMPONENT_A11Y_EVIDENCE);
    let serialized = serde_json::to_string(&snapshot.tree()).unwrap();

    assert!(serialized.contains("Accessibility contracts"));
    assert!(serialized.contains("Button"));
    assert!(serialized.contains("\"role\":\"button\""));
    assert!(serialized.contains("\"actions\":[\"click\"]"));
    assert!(serialized.contains("\"valid\":true"));
    assert!(!serialized.contains("\"Click\""));
}

#[cfg(feature = "motion")]
#[test]
fn framework_adapters_convert_motion_frame_demand_and_driver() {
    use open_gpui_motion::{
        MotionFrameDemand, MotionFrameDriver, MotionFrameReason, MotionFrameResetReason,
    };
    use std::time::Duration;

    let demand = MotionFrameDemand::NeedsFrame(MotionFrameReason::UpdateRender);
    let demand_snapshot = motion::motion_frame_demand_probe_snapshot(demand);
    let mut driver = MotionFrameDriver::new();
    driver.reset(MotionFrameResetReason::Retarget);
    let _ = driver.sample_elapsed(Duration::from_millis(16), |clock| (clock.elapsed(), demand));
    let driver_snapshot = motion::motion_frame_driver_probe_snapshot(&driver);
    let serialized = format!(
        "{}{}",
        serde_json::to_string(&demand_snapshot.tree()).unwrap(),
        serde_json::to_string(&driver_snapshot.tree()).unwrap()
    );

    assert!(serialized.contains("\"needs_frame\":true"));
    assert!(serialized.contains("update-render"));
    assert!(serialized.contains("\"last_reset_reason\":\"retarget\""));
    assert!(!serialized.contains("UpdateRender"));
    assert!(!serialized.contains("Retarget"));
    assert!(serialized.contains("requested_frames"));
    assert!(serialized.contains("last_elapsed_ms"));
}

#[cfg(feature = "gpui")]
#[test]
fn framework_adapters_convert_scroll_viewport_snapshots() {
    use open_gpui::{ScrollViewportChangeSource, ScrollViewportSnapshot, bounds, point, px, size};

    let viewport = ScrollViewportSnapshot::new(
        7,
        ScrollViewportChangeSource::InitialLayout,
        bounds(point(px(1.0), px(2.0)), size(px(300.0), px(200.0))),
        point(px(4.0), px(8.0)),
        point(px(40.0), px(80.0)),
        size(px(600.0), px(500.0)),
    );
    let snapshot = gpui::scroll_viewport_probe_snapshot(viewport);
    let diagnostic = gpui::scroll_viewport_unavailable_diagnostic(
        open_gpui_devtools::ProbeId::new("scroll").unwrap(),
    );
    let serialized = serde_json::to_string(&snapshot.tree()).unwrap();

    assert!(serialized.contains("initial-layout"));
    assert!(serialized.contains("\"generation\":7"));
    assert!(serialized.contains("\"width\":300.0"));
    assert_eq!(diagnostic.code, "runtime.unavailable");
}

#[cfg(feature = "gpui")]
#[test]
fn gpui_inspector_surface_exposes_category_debug_selectors() {
    let source = include_str!("../src/gpui.rs");

    assert!(source.contains("devtools-inspector:category-summaries"));
    assert!(source.contains("devtools-inspector:category:{category_label}"));
    assert!(source.contains("devtools-inspector:target-list"));
    assert!(source.contains("devtools-inspector:target:{target_id}"));
    assert!(source.contains("devtools-inspector:domain-list"));
    assert!(source.contains("devtools-inspector:domain:{domain_id}"));
    assert!(source.contains("devtools-inspector:event-list"));
    assert!(source.contains("devtools-inspector:event:{sequence}"));
    assert!(source.contains("devtools-inspector:selected-detail"));
    assert!(source.contains("devtools-inspector:diagnostics"));
    assert!(source.contains("devtools-inspector:row:{probe_id}"));
}

#[cfg(feature = "docking")]
#[test]
fn framework_adapters_convert_docking_runtime_status() {
    use open_gpui_devtools::DevtoolsRegistry;
    use open_gpui_docking::DockSpaceId;
    use open_gpui_docking::advanced::{
        DockViewportInputStatus, DockViewportLifecycleRecord, DockViewportPlatformCapabilityRecord,
        DockViewportPlatformRequestStatus, DockViewportRestoreReadinessRecord,
        DockViewportRouteStatus, DockViewportRuntimeStatus, DockViewportVisualAffordanceRecord,
        DockVisualAffordanceDebugSummary,
    };

    let mut status = DockViewportRuntimeStatus::default();
    status.platform_capabilities = Some(DockViewportPlatformCapabilityRecord {
        platform_viewport_windows: true,
        global_window_bounds: true,
        window_stack: false,
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
        window_id: open_gpui::WindowId::from(7),
        route_status: DockViewportRouteStatus::RouteReady,
        input_status: DockViewportInputStatus::ReceivesInput,
        platform_request_status: DockViewportPlatformRequestStatus {
            close_requested: false,
            resize_requested: true,
        },
        coordinate_status: None,
        facts_generation: 11,
    });
    status
        .visual_affordances
        .push(DockViewportVisualAffordanceRecord {
            space: DockSpaceId::from("primary"),
            window_id: open_gpui::WindowId::from(7),
            summary: DockVisualAffordanceDebugSummary {
                space: Some("primary".to_owned()),
                frame_generation: Some(3),
                layer_count: 2,
                active_count: 1,
                active: None,
                motion_state: Some("settled".to_owned()),
                churn_signature: "primary:2:1".to_owned(),
            },
        });

    let snapshot = docking::docking_runtime_probe_snapshot(&status);
    let capture = docking::docking_runtime_capture(&status);
    let provider_status = status.clone();
    let mut registry = DevtoolsRegistry::default();
    registry
        .register_capture_provider(
            docking::docking_runtime_capture_provider("docking.runtime", move || {
                provider_status.clone()
            })
            .expect("valid docking runtime provider"),
        )
        .expect("unique docking runtime provider");
    let provider_capture = registry.collect_capture();
    let serialized = serde_json::to_string(&snapshot.tree()).unwrap();
    let capture_serialized = serde_json::to_string(&capture).unwrap();

    assert!(serialized.contains("Viewport runtime"));
    assert!(serialized.contains("platform_viewport_windows"));
    assert!(serialized.contains("\"matched\":2"));
    assert!(serialized.contains("\"missing\":1"));
    assert!(serialized.contains("\"route_status\":\"route-ready\""));
    assert!(serialized.contains("\"resize_requested\":true"));
    assert!(capture_serialized.contains("\"kind\":\"Runtime\""));
    assert!(capture_serialized.contains("\"kind\":\"Docking\""));
    assert!(capture_serialized.contains("docking.visual-affordance.0"));
    assert!(capture_serialized.contains("Visual affordance"));
    assert_eq!(capture.domains.len(), 1);
    assert_eq!(capture.events.len(), 1);
    assert_eq!(capture.snapshots.len(), 1);
    assert_eq!(provider_capture.domains.len(), 1);
    assert_eq!(provider_capture.events.len(), 1);
    assert_eq!(provider_capture.snapshots.len(), 1);
}
