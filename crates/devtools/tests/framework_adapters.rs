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
        MotionFrameDemand, MotionFrameDriver, MotionFrameHostResetReason, MotionFrameReason,
    };
    use std::time::Duration;

    let demand = MotionFrameDemand::NeedsFrame(MotionFrameReason::UpdateRender);
    let demand_snapshot = motion::motion_frame_demand_probe_snapshot(demand);
    let mut driver = MotionFrameDriver::new();
    driver.reset(MotionFrameHostResetReason::Retarget);
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

#[cfg(feature = "docking")]
#[test]
fn framework_adapters_convert_docking_runtime_status() {
    use open_gpui_docking::advanced::{
        DockViewportPlatformCapabilityRecord, DockViewportRestoreReadinessRecord,
        DockViewportRuntimeStatus,
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

    let snapshot = docking::docking_runtime_probe_snapshot(&status);
    let serialized = serde_json::to_string(&snapshot.tree()).unwrap();

    assert!(serialized.contains("Viewport runtime"));
    assert!(serialized.contains("platform_viewport_windows"));
    assert!(serialized.contains("\"matched\":2"));
    assert!(serialized.contains("\"missing\":1"));
}
