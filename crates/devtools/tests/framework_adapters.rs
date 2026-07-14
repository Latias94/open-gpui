#![cfg(any(
    feature = "docking",
    feature = "gpui",
    feature = "motion",
    feature = "ui-components"
))]

#[cfg(feature = "ui-components")]
use open_gpui::VisualContext as _;
#[cfg(feature = "docking")]
use open_gpui_devtools::docking;
#[cfg(feature = "gpui")]
use open_gpui_devtools::gpui;
#[cfg(feature = "motion")]
use open_gpui_devtools::motion;
#[cfg(feature = "ui-components")]
use open_gpui_devtools::ui_components;

#[cfg(feature = "ui-components")]
struct WindowOverlayProjectionProbe {
    runtime: open_gpui_ui_components::gpui_adapter::WindowOverlayRuntime,
}

#[cfg(feature = "ui-components")]
impl open_gpui::Render for WindowOverlayProjectionProbe {
    fn render(
        &mut self,
        _: &mut open_gpui::Window,
        _: &mut open_gpui::Context<Self>,
    ) -> impl open_gpui::IntoElement {
        open_gpui::div()
    }
}

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
#[open_gpui::test]
fn framework_adapters_project_window_overlay_runtime_without_raw_layer_ids(
    cx: &mut open_gpui::TestAppContext,
) {
    use open_gpui_ui_components::gpui_adapter::{
        OverlayLayerRegistration, OverlayOwnership, WindowOverlayRuntime,
    };
    use open_gpui_ui_components::{Dialog, Popover};

    const PARENT_CANARY: &str = "u4a-canary-parent-019f4ad7-4d33";
    const CHILD_CANARY: &str = "u4a-canary-child-ac26-94bc135cc634";

    let (view, cx) = cx.add_window_view(|window, cx| WindowOverlayProjectionProbe {
        runtime: WindowOverlayRuntime::for_window(window, cx),
    });
    cx.update_window_entity(&view, |probe, window, cx| {
        let parent_state = Popover::new("devtools-parent", "Open", "Parent")
            .open(true)
            .state();
        let _parent = probe
            .runtime
            .register_layer(
                OverlayLayerRegistration::new(
                    PARENT_CANARY,
                    parent_state.overlay().policy().clone(),
                    OverlayOwnership::Controlled,
                ),
                window,
                cx,
            )
            .expect("parent overlay should register");

        let child_state = Dialog::new("devtools-child", "Open", "Child", "Body")
            .open(true)
            .state();
        let _child = probe
            .runtime
            .register_layer(
                OverlayLayerRegistration::new(
                    CHILD_CANARY,
                    child_state.overlay().policy().clone(),
                    OverlayOwnership::Controlled,
                )
                .parent(PARENT_CANARY),
                window,
                cx,
            )
            .expect("child overlay should register");
    });
    assert!(cx.update(|window, cx| window.dispatch_keystroke(
        open_gpui::Keystroke::parse("escape").expect("Escape should parse"),
        cx,
    )));
    let runtime_snapshot = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .snapshot(window, cx)
            .expect("runtime snapshot should belong to the test window")
    });
    assert_eq!(runtime_snapshot.layers()[0].id().as_str(), PARENT_CANARY);
    assert_eq!(runtime_snapshot.layers()[1].id().as_str(), CHILD_CANARY);

    let projection = ui_components::window_overlay_probe_snapshot(&runtime_snapshot);
    let root = &projection.tree().nodes[0];
    let parent = root.children[0]
        .payload
        .as_ref()
        .expect("parent projection payload");
    let child = root.children[1]
        .payload
        .as_ref()
        .expect("child projection payload");
    let debug = format!("{projection:?}");
    let serialized = serde_json::to_string(projection.tree()).unwrap();

    assert_eq!(root.children.len(), 2);
    assert_eq!(parent["id"], "overlay-layer-1");
    assert_eq!(parent["parent"], serde_json::Value::Null);
    assert_eq!(parent["kind"], "non-modal-dismissible");
    assert_eq!(child["id"], "overlay-layer-2");
    assert_eq!(child["parent"], "overlay-layer-1");
    assert_eq!(child["kind"], "modal");
    assert_eq!(child["phase"], "close-requested");
    assert_eq!(child["presence"], "open");
    assert_eq!(child["pending_open"], false);
    assert_eq!(child["pending_reason"], "escape-key");
    assert_eq!(child["keyboard_eligible"], true);
    assert_eq!(child["modal_pointer_barrier"], true);
    assert_eq!(child["focus_active"], true);
    assert_eq!(child["focus_entered"], false);

    for canary in [PARENT_CANARY, CHILD_CANARY] {
        assert!(!debug.contains(canary));
        assert!(!serialized.contains(canary));
        assert!(root.children.iter().all(|node| !node.id.contains(canary)));
        assert!(
            root.children
                .iter()
                .all(|node| !node.label.contains(canary))
        );
    }
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
fn framework_adapters_convert_gpui_runtime_metadata() {
    use open_gpui::{ScrollViewportChangeSource, ScrollViewportSnapshot, bounds, point, px, size};
    use open_gpui_devtools::DevtoolsCaptureProvider;

    let viewport = ScrollViewportSnapshot::new(
        12,
        ScrollViewportChangeSource::InitialLayout,
        bounds(point(px(1.0), px(2.0)), size(px(320.0), px(240.0))),
        point(px(8.0), px(16.0)),
        point(px(80.0), px(160.0)),
        size(px(640.0), px(480.0)),
    );
    let runtime = gpui::GpuiRuntimeSnapshot {
        runtime_id: "gallery".to_owned(),
        generation: 3,
        windows: vec![gpui::GpuiRuntimeWindowSnapshot {
            window_id: 42,
            display_id: Some("display-1".to_owned()),
            active: true,
            focused: true,
            bounds: Some(gpui::GpuiRuntimeRectSnapshot {
                origin: gpui::GpuiRuntimePointSnapshot { x: 0.0, y: 0.0 },
                size: gpui::GpuiRuntimeSizeSnapshot {
                    width: 800.0,
                    height: 600.0,
                },
            }),
            content_size: Some(gpui::GpuiRuntimeSizeSnapshot {
                width: 780.0,
                height: 560.0,
            }),
            scale_factor: Some(1.0),
        }],
        focus: Some(gpui::GpuiRuntimeFocusSnapshot {
            active_window_id: Some(42),
            focused_window_id: Some(42),
            focus_scope_count: 2,
            focus_handle_count: 5,
        }),
        input: Some(gpui::GpuiRuntimeInputSnapshot {
            key_down_count: 4,
            pointer_event_count: 3,
            scroll_event_count: 2,
            text_input_event_count: 1,
            ime_event_count: 0,
            clipboard_event_count: 1,
            last_event_kind: Some("key-down".to_owned()),
        }),
        frame: Some(gpui::GpuiRuntimeFrameSnapshot {
            requested_frames: 9,
            painted_frames: 8,
            animation_frame_count: 2,
            last_frame_duration_ms: Some(16.0),
            last_presented_generation: Some(3),
        }),
        scroll_viewports: vec![gpui::GpuiRuntimeScrollSnapshot::from_scroll_viewport(
            viewport,
        )],
        diagnostics: Vec::new(),
    };

    let capture = gpui::gpui_runtime_capture(&runtime);
    let probe = gpui::gpui_runtime_probe_snapshot(&runtime);
    let provider_runtime = runtime.clone();
    let provider =
        gpui::gpui_runtime_capture_provider("gpui.runtime", move || provider_runtime.clone())
            .expect("valid GPUI runtime provider");
    let provider_capture = provider.capture().expect("provider capture succeeds");
    let serialized = format!(
        "{}{}{}",
        serde_json::to_string(&capture).unwrap(),
        serde_json::to_string(&probe.tree()).unwrap(),
        serde_json::to_string(&provider_capture).unwrap()
    );

    assert!(serialized.contains("gpui-runtime"));
    assert!(serialized.contains("gpui.input-metadata"));
    assert!(serialized.contains("gpui.frame-metadata"));
    assert!(serialized.contains("\"key_down_count\":4"));
    assert!(serialized.contains("\"text_input_event_count\":1"));
    assert!(serialized.contains("\"clipboard_event_count\":1"));
    assert!(serialized.contains("initial-layout"));
    assert!(serialized.contains("\"window_count\":1"));
    assert!(!serialized.contains("clipboard_contents"));
    assert!(!serialized.contains("raw_text"));
    assert_eq!(capture.events.len(), 2);
    assert_eq!(capture.domains.len(), 1);
    assert_eq!(capture.snapshots.len(), 1);
    assert_eq!(provider_capture.events.len(), 2);
}

#[cfg(feature = "gpui")]
#[test]
fn gpui_inspector_surface_exposes_category_debug_selectors() {
    let source = include_str!("../src/gpui/render.rs");

    assert!(source.contains("devtools-inspector:category-summaries"));
    assert!(source.contains("devtools-inspector:category:{category_label}"));
    assert!(source.contains("devtools-inspector:target-list"));
    assert!(source.contains("devtools-inspector:target:{target_id}"));
    assert!(source.contains("devtools-inspector:domain-list"));
    assert!(source.contains("devtools-inspector:domain:{domain_id}"));
    assert!(source.contains("devtools-inspector:event-list"));
    assert!(source.contains("event_identity.as_key()"));
    assert!(source.contains("devtools-inspector:event:{event_identity_key}"));
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
