//! Adapter-owned viewport route arbitration regression tests.

use crate::{
    DockNodeId, DockPolicy, DockSpaceId, DockViewportAdapter, DockViewportCoordinateSpaceRecord,
    DockViewportDropPayload, DockViewportDropRoute, DockViewportDropRouteRequest,
    DockViewportLifecycleRecord, DockViewportPlatformSignals, DockViewportRouteSelectionRecord,
    DockViewportRouteSelectionSource, DockViewportRuntimeStatus, DockViewportTargetContext,
    DockViewportTargetHit, DockViewportWindowFacts,
    viewport_test_support::{bounds, handle, item, register_viewport, space},
};
use open_gpui::{AnyWindowHandle, WindowBounds, point, px};
use slotmap::Key;

fn signals_with_receiver(
    target_context: DockViewportTargetContext,
    receiver: AnyWindowHandle,
) -> DockViewportPlatformSignals {
    DockViewportPlatformSignals::from_target_context(target_context)
        .with_event_receiver_window(receiver)
}

#[test]
fn local_only_receiver_match_records_trusted_hovered_route_and_coordinate_status() {
    let source = space("source");
    let target = space("target");
    let source_window = handle(1);
    let target_window = handle(2);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, source.clone(), source_window);
    register_viewport(&mut adapter, target.clone(), target_window);
    adapter.update_snapshot(
        &source,
        DockViewportWindowFacts::local_only_window_bounds_for_test(WindowBounds::Windowed(bounds(
            0.0, 0.0, 320.0, 240.0,
        ))),
        bounds(0.0, 0.0, 320.0, 240.0),
    );
    adapter.update_snapshot(
        &target,
        DockViewportWindowFacts::local_only_window_bounds_for_test(WindowBounds::Windowed(bounds(
            0.0, 0.0, 320.0, 240.0,
        ))),
        bounds(10.0, 20.0, 300.0, 200.0),
    );
    let request = DockViewportDropRouteRequest::from_platform_signals(
        source,
        DockNodeId::null(),
        DockViewportDropPayload::Item(item("a")),
        point(px(30.0), px(50.0)),
        None,
        signals_with_receiver(
            DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
            target_window,
        )
        .with_global_window_bounds(false),
    );

    let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

    assert_eq!(
        route,
        DockViewportDropRoute::KnownViewport {
            target: DockViewportTargetHit::with_facts_generation(
                target.clone(),
                target_window,
                point(px(20.0), px(30.0)),
                1,
            ),
            source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
        }
    );
    let mut status = DockViewportRuntimeStatus::default();
    status.record_route(&request, &route, None);
    let route_record = status
        .last_route
        .as_ref()
        .expect("route should be recorded");
    assert_eq!(
        route_record.selection_source,
        Some(DockViewportRouteSelectionRecord::TrustedHoveredWindow),
        "local-only cross-window routing must stay attributed to trusted hovered-window facts"
    );

    let target_snapshot = adapter
        .snapshot(&target)
        .expect("target viewport should be registered");
    let lifecycle = DockViewportLifecycleRecord::from_snapshot(target, target_snapshot);
    assert_eq!(
        lifecycle
            .coordinate_status
            .map(|status| status.coordinate_space),
        Some(DockViewportCoordinateSpaceRecord::WindowLocal),
        "diagnostics should show that this successful route did not use global rectangle bounds"
    );
}

#[test]
fn local_only_receiver_mismatch_rejects_cross_viewport_route() {
    let source = space("source");
    let target = space("target");
    let source_window = handle(1);
    let target_window = handle(2);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, source.clone(), source_window);
    register_viewport(&mut adapter, target, target_window);
    adapter.update_snapshot(
        &source,
        DockViewportWindowFacts::local_only_window_bounds_for_test(WindowBounds::Windowed(bounds(
            0.0, 0.0, 320.0, 240.0,
        ))),
        bounds(0.0, 0.0, 320.0, 240.0),
    );
    adapter.update_snapshot(
        &space("target"),
        DockViewportWindowFacts::local_only_window_bounds_for_test(WindowBounds::Windowed(bounds(
            0.0, 0.0, 320.0, 240.0,
        ))),
        bounds(10.0, 20.0, 300.0, 200.0),
    );
    let request = DockViewportDropRouteRequest::from_platform_signals(
        source,
        DockNodeId::null(),
        DockViewportDropPayload::Item(item("a")),
        point(px(30.0), px(50.0)),
        None,
        signals_with_receiver(
            DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
            source_window,
        )
        .with_global_window_bounds(false),
    );

    assert_eq!(
        adapter.resolve_payload_drop_route(&request, &DockPolicy::default()),
        DockViewportDropRoute::Unavailable,
        "trusted hovered-window ids are not enough when event coordinates belong to another window"
    );
}

#[test]
fn global_screen_rectangle_route_records_front_to_back_fallback_source() {
    let source = DockSpaceId::from("source");
    let target = DockSpaceId::from("target");
    let source_window = handle(1);
    let target_window = handle(2);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, source.clone(), source_window);
    register_viewport(&mut adapter, target.clone(), target_window);
    adapter.update_snapshot(
        &source,
        DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
            0.0, 0.0, 320.0, 240.0,
        ))),
        bounds(0.0, 0.0, 320.0, 240.0),
    );
    adapter.update_snapshot(
        &target,
        DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
            400.0, 0.0, 320.0, 240.0,
        ))),
        bounds(10.0, 20.0, 300.0, 200.0),
    );
    let request = DockViewportDropRouteRequest::from_platform_signals(
        source,
        DockNodeId::null(),
        DockViewportDropPayload::Item(item("a")),
        point(px(430.0), px(50.0)),
        None,
        DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new().with_window_stack([target_window, source_window]),
        ),
    );

    let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

    assert_eq!(
        route,
        DockViewportDropRoute::KnownViewport {
            target: DockViewportTargetHit::with_facts_generation(
                target,
                target_window,
                point(px(20.0), px(30.0)),
                1,
            ),
            source: DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
        }
    );
    let mut status = DockViewportRuntimeStatus::default();
    status.record_route(&request, &route, None);
    assert_eq!(
        status
            .last_route
            .as_ref()
            .and_then(|route| route.selection_source),
        Some(DockViewportRouteSelectionRecord::FrontToBackWindowStackFallback)
    );
}
