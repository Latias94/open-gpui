use crate::{
    DockController, DockGraph, DockHost, DockLayoutRect, DockNode, DockSpaceId,
    DockViewportAdapter, DockViewportOpenStatus, DockViewportPlacement,
    DockViewportPlacementLayout, DockViewportPlatformSignals, DockViewportRuntimeHandle,
    DockViewportWindowBounds, DockViewportWindowFacts, DockViewportWindowState, DockWorkspace,
    debug::DockDebugRegion, host_test_support::*,
};
use open_gpui::{
    AnyWindowHandle, AppContext as _, TestAppContext, VisualTestContext, WindowBounds, point, px,
    size,
};

#[open_gpui::test]
fn viewport_runtime_handle_opens_and_reuses_controller_backed_window(cx: &mut TestAppContext) {
    let primary_space = DockSpaceId::from("primary");
    let secondary_space = DockSpaceId::from("secondary");
    let mut graph = DockGraph::new();
    let primary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let secondary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(primary_space.clone(), primary_tabs);
    graph.set_root(secondary_space.clone(), secondary_tabs);

    let mut workspace = DockWorkspace::new(primary_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);

    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("secondary viewport should open");
    assert_eq!(opened.space(), &secondary_space);
    assert_eq!(opened.status(), DockViewportOpenStatus::Opened);
    assert_eq!(
        runtime
            .borrow()
            .adapter()
            .window_for_space(&secondary_space),
        Some(opened.window())
    );

    let opened_window = opened
        .window()
        .downcast::<DockHost>()
        .expect("viewport window should render DockHost");
    let host = opened_window
        .root(cx)
        .expect("opened viewport should expose DockHost root");
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(opened.window(), cx);
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("b") }).is_some(),
        "secondary viewport should render the shared controller's secondary panel"
    );

    let reused = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                viewport_window_options(480.0, 260.0),
                app,
            )
        })
        .expect("live secondary viewport should be reused");
    assert_eq!(reused.status(), DockViewportOpenStatus::Reused);
    assert_eq!(reused.window(), opened.window());
    assert_eq!(runtime.registered_viewport_spaces().len(), 1);
}

#[open_gpui::test]
fn viewport_platform_signals_separate_hovered_from_active_window(cx: &mut TestAppContext) {
    let alpha_space = DockSpaceId::from("alpha");
    let zeta_space = DockSpaceId::from("zeta");
    let (alpha_graph, _alpha_root) = tabs_graph(&["a"]);
    let (zeta_graph, _zeta_root) = tabs_graph(&["b"]);
    let (alpha_window, _alpha_host, _alpha_visual) = open_host(
        cx,
        alpha_graph,
        &[("a", "Panel A", "A")],
        size(px(320.0), px(200.0)),
    );
    let (zeta_window, _zeta_host, _zeta_visual) = open_host(
        cx,
        zeta_graph,
        &[("b", "Panel B", "B")],
        size(px(320.0), px(200.0)),
    );
    let alpha_handle: AnyWindowHandle = alpha_window.into();
    let zeta_handle: AnyWindowHandle = zeta_window.into();
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(zeta_space.clone(), zeta_handle);
    adapter.register_viewport(alpha_space.clone(), alpha_handle);
    for space in [&alpha_space, &zeta_space] {
        adapter.update_snapshot(
            space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
                100.0, 100.0, 300.0, 200.0,
            ))),
            floating_bounds(0.0, 0.0, 300.0, 200.0),
        );
    }

    zeta_window
        .update(cx, |_, window, _| window.activate_window())
        .expect("zeta window should be live");
    let (context, capabilities) = alpha_window
        .update(cx, |_, _, app| {
            (
                DockViewportPlatformSignals::from_app(app).target_context(),
                app.viewport_capabilities(),
            )
        })
        .expect("alpha window should be live");

    assert!(!capabilities.window_stack);
    assert_eq!(context.hovered_window(), None);
    assert_eq!(context.active_window(), Some(zeta_handle.window_id()));
    assert_eq!(context.window_stack(), &[]);
    assert_eq!(
        adapter
            .resolve_viewport_target(point(px(125.0), px(150.0)), &context)
            .map(|target| target.space().clone()),
        Some(alpha_space.clone()),
        "active window is diagnostic only and should not arbitrate overlapping hits"
    );

    cx.set_platform_hovered_window(Some(alpha_handle));
    let hovered_context =
        cx.update(|app| DockViewportPlatformSignals::from_app(app).target_context());
    assert_eq!(
        hovered_context.hovered_window(),
        Some(alpha_handle.window_id()),
        "platform hovered window should be captured by from_app"
    );
    assert_eq!(
        adapter
            .resolve_viewport_target(point(px(125.0), px(150.0)), &hovered_context)
            .map(|target| target.space().clone()),
        Some(alpha_space),
        "explicit hovered window should win viewport arbitration"
    );
}

#[open_gpui::test]
fn viewport_runtime_handle_opens_with_saved_placement_options(cx: &mut TestAppContext) {
    let secondary_space = DockSpaceId::from("secondary");
    let mut graph = DockGraph::new();
    let secondary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(secondary_space.clone(), secondary_tabs);

    let mut workspace = DockWorkspace::new(secondary_space.clone(), graph);
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let saved_window_bounds = WindowBounds::Windowed(floating_bounds(80.0, 90.0, 420.0, 260.0));
    let placement = DockViewportPlacementLayout::new(vec![DockViewportPlacement {
        space: secondary_space.clone(),
        display_id: None,
        window_bounds: Some(DockViewportWindowBounds {
            state: DockViewportWindowState::Windowed,
            bounds: DockLayoutRect::from_bounds(saved_window_bounds.get_bounds()),
        }),
        host_bounds: None,
    }]);
    let fallback_options = viewport_window_options(240.0, 160.0);

    let opened = cx
        .update(|app| {
            let options = placement
                .window_options_for_space(&secondary_space, fallback_options)
                .expect("saved placement should produce window options");
            runtime.open_viewport(secondary_space.clone(), options, app)
        })
        .expect("secondary viewport should open with saved placement");

    assert_eq!(opened.status(), DockViewportOpenStatus::Opened);
    assert_eq!(
        opened
            .window()
            .update(cx, |_, window, _| window.window_bounds())
            .expect("opened window should still be live"),
        saved_window_bounds
    );
}
