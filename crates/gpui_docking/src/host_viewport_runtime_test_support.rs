use crate::{
    DockController, DockDropDelivery, DockGraph, DockItemId, DockNode, DockNodeId, DockSpaceId,
    DockViewportDropPayload, DockViewportDropRoute, DockViewportDropRouteRequest,
    DockViewportInputStatus, DockViewportRuntime, DockViewportRuntimeHandle,
    DockViewportShouldCloseStatus, DockViewportTargetContext, DockViewportTearOffRequest,
    DockViewportWindowFacts, DockWorkspace,
    drag::DockDragPayload,
    drop_runtime::DockHostDropSceneFact,
    drop_target::DockLeafDropTarget,
    host_test_support::*,
    interaction::{DockPayloadDropReleaseOrigin, DockRuntimeDragSession},
};
use open_gpui::{
    AnyWindowHandle, AppContext as _, TestAppContext, WindowBounds, WindowId, WindowOptions, point,
    px,
};

pub(crate) fn tear_off_request(
    source_space: DockSpaceId,
    source_tabs: DockNodeId,
    item: DockItemId,
) -> DockViewportTearOffRequest {
    DockViewportTearOffRequest::new(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item),
        point(px(900.0), px(900.0)),
        None,
    )
}

pub(crate) fn viewport_input_status(
    runtime: &DockViewportRuntimeHandle,
    space: &DockSpaceId,
) -> Option<DockViewportInputStatus> {
    runtime
        .runtime_status()
        .viewport_lifecycle
        .iter()
        .find(|record| &record.space == space)
        .map(|record| record.input_status)
}

pub(crate) fn leaf_host_scene_fact(
    root: DockNodeId,
    target_tabs: DockNodeId,
) -> DockHostDropSceneFact {
    DockHostDropSceneFact::Leaf(DockLeafDropTarget {
        root,
        target_tabs,
        bounds: floating_bounds(0.0, 0.0, 360.0, 220.0),
        is_central: false,
    })
}

pub(crate) fn target_center_host_position() -> open_gpui::Point<open_gpui::Pixels> {
    center_drop_position(floating_bounds(0.0, 0.0, 360.0, 220.0))
}

pub(crate) fn assert_tabs_node_items(
    graph: &DockGraph,
    tabs: DockNodeId,
    expected_items: &[DockItemId],
    context: &str,
) {
    let DockNode::Tabs { items, selected } = graph
        .node(tabs)
        .unwrap_or_else(|| panic!("{context}: tabs node should exist"))
    else {
        panic!("{context}: node should be tabs");
    };
    assert_eq!(items.as_slice(), expected_items, "{context}");
    assert_eq!(selected.as_ref(), expected_items.first(), "{context}");
}

pub(crate) fn hovered_window_route_request_for_test(
    source_space: impl Into<DockSpaceId>,
    source_node: DockNodeId,
    payload: DockViewportDropPayload,
    release_position: open_gpui::Point<open_gpui::Pixels>,
    suggested_window_bounds: Option<WindowBounds>,
    hovered_window: AnyWindowHandle,
    release_origin: DockPayloadDropReleaseOrigin,
) -> DockViewportDropRouteRequest {
    DockViewportDropRouteRequest::from_platform_signals_with_origin(
        source_space,
        source_node,
        payload,
        release_position,
        suggested_window_bounds,
        crate::DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new().with_trusted_hovered_window(hovered_window),
        )
        .with_event_receiver_window(hovered_window)
        .with_global_window_bounds(true),
        release_origin,
    )
}

pub(crate) fn freeze_should_close_plan(
    cx: &mut TestAppContext,
    runtime: &DockViewportRuntimeHandle,
    window_id: WindowId,
) {
    let should_close = cx.update(|app| runtime.handle_window_should_close_with_app(window_id, app));
    assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);
}

pub(crate) fn cache_known_viewport_preview_for_test(
    runtime: &mut DockViewportRuntime,
    source_space: DockSpaceId,
    source_tabs: DockNodeId,
    target_space: &DockSpaceId,
    target_window: AnyWindowHandle,
    target_tabs: DockNodeId,
    cx: &mut TestAppContext,
) -> crate::interaction::DockRuntimeDragSession {
    let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let host_position = center_drop_position(host_bounds);
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(window_bounds),
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        target_space,
        target_window.window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));

    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    let session = runtime.begin_payload_drag(&payload);
    let request = hovered_window_route_request_for_test(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        target_window,
        DockPayloadDropReleaseOrigin::HoveredHost,
    )
    .with_drag_session(Some(session.clone()));
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    let update = runtime.update_routed_drop_preview(&resolution, &payload);
    assert!(update.changed());

    session
}

pub(crate) fn close_window_quietly_for_test(window: AnyWindowHandle, cx: &mut TestAppContext) {
    let _ = window.update(cx, |_, window, _| window.remove_window());
}

pub(crate) fn focus_backend_window_for_test(window: AnyWindowHandle, cx: &mut TestAppContext) {
    window
        .update(cx, |_, window, _| window.activate_window())
        .expect("test viewport should activate");
}

pub(crate) fn seed_runtime_host_scene_for_test(
    runtime: &DockViewportRuntimeHandle,
    space: &DockSpaceId,
    window: AnyWindowHandle,
    tabs: DockNodeId,
) {
    let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let host_position = center_drop_position(host_bounds);
    assert!(runtime.begin_viewport_host_scene(
        space.clone(),
        window.window_id(),
        DockViewportWindowFacts::from_window_bounds(window_bounds),
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        space,
        window.window_id(),
        leaf_host_scene_fact(tabs, tabs),
    ));
}

pub(crate) fn cache_known_viewport_preview(
    cx: &mut TestAppContext,
    runtime: &DockViewportRuntimeHandle,
    source_space: DockSpaceId,
    source_node: DockNodeId,
    payload: DockViewportDropPayload,
    release_position: open_gpui::Point<open_gpui::Pixels>,
    hovered_window: impl Into<open_gpui::AnyWindowHandle>,
    drag_session: Option<DockRuntimeDragSession>,
    payload_title: &str,
) -> crate::DockViewportResolvedDropRoute {
    let drag_payload = DockDragPayload::new_item(
        source_space.clone(),
        source_node,
        item("__test__"),
        payload_title.to_string(),
    );
    cache_known_viewport_preview_with_payload(
        cx,
        runtime,
        source_space,
        source_node,
        payload,
        release_position,
        hovered_window,
        drag_session,
        &drag_payload,
    )
}

pub(crate) fn cache_known_viewport_preview_with_payload(
    cx: &mut TestAppContext,
    runtime: &DockViewportRuntimeHandle,
    source_space: DockSpaceId,
    source_node: DockNodeId,
    payload: DockViewportDropPayload,
    release_position: open_gpui::Point<open_gpui::Pixels>,
    hovered_window: impl Into<open_gpui::AnyWindowHandle>,
    drag_session: Option<DockRuntimeDragSession>,
    drag_payload: &DockDragPayload,
) -> crate::DockViewportResolvedDropRoute {
    let request = DockViewportDropRouteRequest::from_target_context(
        source_space,
        source_node,
        payload,
        release_position,
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(hovered_window),
    )
    .with_drag_session(drag_session);
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert!(
        matches!(
            resolution.route(),
            DockViewportDropRoute::KnownViewport { .. }
        ),
        "preview setup should resolve a known viewport route, got {:?}",
        resolution.route()
    );
    let preview_changed =
        cx.update(|app| runtime.update_routed_drop_preview(&resolution, drag_payload, app));
    let preview_target = resolution
        .routed_preview_target_snapshot()
        .expect("known viewport preview should carry a routed preview target");
    let target_space = preview_target.target_space();
    let target_window_id = preview_target
        .target_window_id()
        .expect("known viewport preview should target a window");
    let preview = runtime.routed_drop_preview_for(target_space, target_window_id);
    let _ = (preview_changed, preview);
    resolution
}

pub(crate) fn cache_host_route_preview(
    cx: &mut TestAppContext,
    runtime: &DockViewportRuntimeHandle,
    resolution: &crate::DockViewportResolvedDropRoute,
    payload_title: &str,
    host_space: DockSpaceId,
    host_window_id: open_gpui::WindowId,
    host_position: open_gpui::Point<open_gpui::Pixels>,
) {
    let payload = DockDragPayload::new_item(
        DockSpaceId::from("__test__"),
        Default::default(),
        item("__test__"),
        payload_title.to_string(),
    );
    cache_host_route_preview_with_payload(
        cx,
        runtime,
        resolution,
        &payload,
        host_space,
        host_window_id,
        host_position,
    );
}

pub(crate) fn cache_host_route_preview_with_payload(
    cx: &mut TestAppContext,
    runtime: &DockViewportRuntimeHandle,
    resolution: &crate::DockViewportResolvedDropRoute,
    payload: &DockDragPayload,
    host_space: DockSpaceId,
    host_window_id: open_gpui::WindowId,
    host_position: open_gpui::Point<open_gpui::Pixels>,
) {
    cx.update(|app| {
        runtime.update_host_routed_drop_preview(
            resolution,
            payload,
            host_space,
            host_window_id,
            host_position,
            app,
        );
    });
}

pub(crate) fn fresh_delivery_for_request(
    cx: &mut TestAppContext,
    runtime: &DockViewportRuntimeHandle,
    request: &DockViewportDropRouteRequest,
) -> DockDropDelivery {
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(request, app));
    DockDropDelivery::from_resolution(resolution)
        .expect("fresh current-facts route should mint a delivery")
}

pub(crate) fn backend_route_resolution_fixture(
    cx: &mut TestAppContext,
) -> (
    DockViewportRuntimeHandle,
    open_gpui::AnyWindowHandle,
    DockViewportDropRouteRequest,
) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    cx.set_platform_focused_window_available(false);
    let window_options = || WindowOptions {
        focus: false,
        ..viewport_window_options(360.0, 220.0)
    };
    let _source_opened = cx
        .update(|app| {
            runtime.open_viewport_unchecked_policy(source_space.clone(), window_options(), app)
        })
        .expect("source viewport should open");
    let target_opened = cx
        .update(|app| {
            runtime.open_viewport_unchecked_policy(target_space.clone(), window_options(), app)
        })
        .expect("target viewport should open");
    let request = DockViewportDropRouteRequest::from_target_context(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new(),
    );
    (runtime, target_opened.window(), request)
}
