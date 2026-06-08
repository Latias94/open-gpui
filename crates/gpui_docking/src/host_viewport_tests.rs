use crate::{
    DockController, DockGraph, DockHost, DockLayoutRect, DockNode, DockSpaceId,
    DockViewportAdapter, DockViewportClosePolicy, DockViewportCloseStatus, DockViewportOpenStatus,
    DockViewportPlacement, DockViewportPlacementLayout, DockViewportRuntime,
    DockViewportRuntimeHandle, DockViewportShouldCloseStatus, DockViewportTargetContext,
    DockViewportWindowBounds, DockViewportWindowState, DockWorkspace, debug::DockDebugRegion,
    host_test_support::*,
};
use open_gpui::{
    AnyWindowHandle, AppContext as _, TestAppContext, VisualTestContext, WindowBounds,
    WindowHandle, WindowId, point, px, size,
};

#[open_gpui::test]
fn viewport_adapter_opens_and_reuses_controller_backed_window(cx: &mut TestAppContext) {
    let primary_space = DockSpaceId::from("primary");
    let secondary_space = DockSpaceId::from("secondary");
    let mut graph = DockGraph::new();
    let primary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    let secondary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(primary_space.clone(), primary_tabs);
    graph.set_root(secondary_space.clone(), secondary_tabs);

    let mut workspace = DockWorkspace::new(primary_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let mut adapter = DockViewportAdapter::new();

    let opened = cx
        .update(|app| {
            adapter.open_viewport(
                controller.clone(),
                secondary_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("secondary viewport should open");
    assert_eq!(opened.space, secondary_space);
    assert_eq!(opened.status, DockViewportOpenStatus::Opened);
    assert_eq!(
        adapter.window_for_space(&secondary_space),
        Some(opened.window)
    );

    let opened_window = opened
        .window
        .downcast::<DockHost>()
        .expect("viewport window should render DockHost");
    let host = opened_window
        .root(cx)
        .expect("opened viewport should expose DockHost root");
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(opened.window, cx);
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("b") }).is_some(),
        "secondary viewport should render the shared controller's secondary panel"
    );

    let reused = cx
        .update(|app| {
            adapter.open_viewport(
                controller.clone(),
                secondary_space.clone(),
                viewport_window_options(480.0, 260.0),
                app,
            )
        })
        .expect("live secondary viewport should be reused");
    assert_eq!(reused.status, DockViewportOpenStatus::Reused);
    assert_eq!(reused.window, opened.window);
    assert_eq!(adapter.len(), 1);
}

#[open_gpui::test]
fn viewport_target_context_from_window_marks_event_window_as_hovered(cx: &mut TestAppContext) {
    let alpha_space = DockSpaceId::from("alpha");
    let zeta_space = DockSpaceId::from("zeta");
    let (alpha_graph, _alpha_root) = tabs_graph(&["a"], 0);
    let (zeta_graph, _zeta_root) = tabs_graph(&["b"], 0);
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
            None,
            WindowBounds::Windowed(floating_bounds(100.0, 100.0, 300.0, 200.0)),
            floating_bounds(0.0, 0.0, 300.0, 200.0),
        );
    }

    zeta_window
        .update(cx, |_, window, _| window.activate_window())
        .expect("zeta window should be live");
    let context = alpha_window
        .update(cx, |_, window, app| {
            DockViewportTargetContext::from_window(window, app)
        })
        .expect("alpha window should be live");

    assert_eq!(context.hovered_window, Some(alpha_handle.window_id()));
    assert_eq!(context.active_window, Some(zeta_handle.window_id()));
    assert_eq!(
        adapter
            .hit_test_screen_with_context(point(px(125.0), px(150.0)), &context)
            .map(|hit| hit.space),
        Some(alpha_space),
        "current event window should win viewport arbitration as hovered"
    );
}

#[open_gpui::test]
fn viewport_adapter_opens_with_saved_placement_options(cx: &mut TestAppContext) {
    let secondary_space = DockSpaceId::from("secondary");
    let mut graph = DockGraph::new();
    let secondary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(secondary_space.clone(), secondary_tabs);

    let mut workspace = DockWorkspace::new(secondary_space.clone(), graph);
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let mut adapter = DockViewportAdapter::new();
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
            adapter.open_viewport(controller.clone(), secondary_space.clone(), options, app)
        })
        .expect("secondary viewport should open with saved placement");

    assert_eq!(opened.status, DockViewportOpenStatus::Opened);
    assert_eq!(
        opened
            .window
            .update(cx, |_, window, _| window.window_bounds())
            .expect("opened window should still be live"),
        saved_window_bounds
    );
}

#[open_gpui::test]
fn viewport_runtime_opens_and_reuses_controller_backed_window(cx: &mut TestAppContext) {
    let primary_space = DockSpaceId::from("primary");
    let secondary_space = DockSpaceId::from("secondary");
    let mut graph = DockGraph::new();
    let primary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    let secondary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(primary_space.clone(), primary_tabs);
    graph.set_root(secondary_space.clone(), secondary_tabs);

    let mut workspace = DockWorkspace::new(primary_space, graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let mut runtime = DockViewportRuntime::new(controller);

    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("secondary viewport should open through runtime");
    assert_eq!(opened.status, DockViewportOpenStatus::Opened);
    assert_eq!(
        runtime.adapter().window_for_space(&secondary_space),
        Some(opened.window)
    );

    let reused = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                viewport_window_options(480.0, 260.0),
                app,
            )
        })
        .expect("live viewport should be reused through runtime");
    assert_eq!(reused.status, DockViewportOpenStatus::Reused);
    assert_eq!(reused.window, opened.window);
    assert_eq!(runtime.adapter().len(), 1);
}

#[open_gpui::test]
fn viewport_runtime_should_close_observes_policy_changes_after_open(cx: &mut TestAppContext) {
    let secondary_space = DockSpaceId::from("secondary");
    let mut graph = DockGraph::new();
    let secondary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(secondary_space.clone(), secondary_tabs);

    let mut workspace = DockWorkspace::new(secondary_space.clone(), graph);
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let mut runtime = DockViewportRuntime::new(controller);

    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("secondary viewport should open through runtime");
    let mut visual = VisualTestContext::from_window(opened.window, cx);

    assert!(
        visual.simulate_close(),
        "default RetainLayout policy should allow the already-open window to close"
    );

    runtime.set_close_policy(DockViewportClosePolicy::Prevent);
    assert!(
        !visual.simulate_close(),
        "updated Prevent policy should veto the already-open window"
    );
    assert_eq!(
        runtime
            .handle_window_should_close(opened.window.window_id())
            .status,
        DockViewportShouldCloseStatus::Vetoed
    );

    runtime.set_close_policy(DockViewportClosePolicy::RetainLayout);
    assert!(
        visual.simulate_close(),
        "restored RetainLayout policy should allow the already-open window again"
    );
}

#[open_gpui::test]
fn viewport_runtime_should_close_allows_windows_after_mapping_cleanup(cx: &mut TestAppContext) {
    let secondary_space = DockSpaceId::from("secondary");
    let mut graph = DockGraph::new();
    let secondary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(secondary_space.clone(), secondary_tabs);

    let mut workspace = DockWorkspace::new(secondary_space.clone(), graph);
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let mut runtime = DockViewportRuntime::new(controller);

    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("secondary viewport should open through runtime");
    let mut visual = VisualTestContext::from_window(opened.window, cx);

    runtime.set_close_policy(DockViewportClosePolicy::Prevent);
    assert!(
        !visual.simulate_close(),
        "Prevent should veto a close while the window still belongs to a runtime mapping"
    );

    let cleanup = runtime.handle_window_closed(opened.window.window_id());
    assert_eq!(cleanup.status, DockViewportCloseStatus::Closed);
    assert_eq!(runtime.adapter().window_for_space(&secondary_space), None);
    assert_eq!(
        runtime
            .handle_window_should_close(opened.window.window_id())
            .status,
        DockViewportShouldCloseStatus::UnknownWindow
    );
    assert!(
        visual.simulate_close(),
        "Prevent should not veto once docking no longer owns the window mapping"
    );
}

#[open_gpui::test]
fn viewport_runtime_installs_should_close_hook_when_reusing_registered_window(
    cx: &mut TestAppContext,
) {
    let secondary_space = DockSpaceId::from("secondary");
    let mut graph = DockGraph::new();
    let secondary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(secondary_space.clone(), secondary_tabs);

    let mut workspace = DockWorkspace::new(secondary_space.clone(), graph);
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, _host, mut visual) = open_controller_space(
        cx,
        controller.clone(),
        secondary_space.clone(),
        size(px(360.0), px(220.0)),
    );
    let window: AnyWindowHandle = window.into();
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(secondary_space.clone(), window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller,
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let reused = cx
        .update(|app| {
            runtime.open_viewport(secondary_space, viewport_window_options(480.0, 260.0), app)
        })
        .expect("registered live viewport should be reused through runtime");

    assert_eq!(reused.status, DockViewportOpenStatus::Reused);
    assert_eq!(reused.window, window);
    assert!(
        visual.simulate_close(),
        "runtime should install a RetainLayout should-close hook when it reuses a registered window"
    );
}

#[open_gpui::test]
fn viewport_runtime_handle_observes_window_closed_cleanup(cx: &mut TestAppContext) {
    let secondary_space = DockSpaceId::from("secondary");
    let mut graph = DockGraph::new();
    let secondary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(secondary_space.clone(), secondary_tabs);

    let mut workspace = DockWorkspace::new(secondary_space.clone(), graph);
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    cx.update(|app| runtime.observe_window_closed(app).detach());

    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("secondary viewport should open through runtime handle");
    assert_eq!(runtime.borrow().adapter().len(), 1);

    opened
        .window
        .update(cx, |_, window, _| window.remove_window())
        .expect("opened viewport should still be live");
    cx.run_until_parked();

    assert_eq!(
        runtime
            .borrow()
            .adapter()
            .window_for_space(&secondary_space),
        None
    );
}

#[open_gpui::test]
fn viewport_runtime_handle_prevents_platform_close_when_policy_prevents(cx: &mut TestAppContext) {
    let secondary_space = DockSpaceId::from("secondary");
    let mut graph = DockGraph::new();
    let secondary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(secondary_space.clone(), secondary_tabs);

    let mut workspace = DockWorkspace::new(secondary_space.clone(), graph);
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
        .expect("secondary viewport should open through runtime handle");
    let mut visual = VisualTestContext::from_window(opened.window, cx);

    assert_eq!(
        runtime.close_policy(),
        DockViewportClosePolicy::RetainLayout
    );
    assert_eq!(
        runtime
            .handle_window_should_close(opened.window.window_id())
            .status,
        DockViewportShouldCloseStatus::Allowed
    );

    runtime.set_close_policy(DockViewportClosePolicy::Prevent);
    assert_eq!(runtime.close_policy(), DockViewportClosePolicy::Prevent);
    assert!(
        !visual.simulate_close(),
        "updated Prevent policy should veto GPUI should-close before the window closes"
    );
    assert_eq!(
        runtime
            .handle_window_should_close(opened.window.window_id())
            .status,
        DockViewportShouldCloseStatus::Vetoed
    );
    assert_eq!(
        runtime
            .borrow()
            .adapter()
            .window_for_space(&secondary_space),
        Some(opened.window)
    );
}

#[open_gpui::test]
fn viewport_runtime_handle_allows_platform_close_with_retain_policy(cx: &mut TestAppContext) {
    let secondary_space = DockSpaceId::from("secondary");
    let mut graph = DockGraph::new();
    let secondary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(secondary_space.clone(), secondary_tabs);

    let mut workspace = DockWorkspace::new(secondary_space.clone(), graph);
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);

    let opened = cx
        .update(|app| {
            runtime.open_viewport(secondary_space, viewport_window_options(360.0, 220.0), app)
        })
        .expect("secondary viewport should open through runtime handle");
    let mut visual = VisualTestContext::from_window(opened.window, cx);

    assert!(
        visual.simulate_close(),
        "RetainLayout policy should allow GPUI should-close to continue"
    );
    assert_eq!(
        runtime
            .handle_window_should_close(opened.window.window_id())
            .status,
        DockViewportShouldCloseStatus::Allowed
    );
}

#[open_gpui::test]
fn viewport_runtime_window_closed_cleans_mapping_after_prevent_policy(cx: &mut TestAppContext) {
    let controller = cx.new(|_| DockController::from_graph(space(), DockGraph::new()));
    let secondary_space = DockSpaceId::from("secondary");
    let window: AnyWindowHandle = WindowHandle::<DockHost>::new(WindowId::from(909)).into();
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(secondary_space.clone(), window);

    let mut runtime =
        DockViewportRuntime::from_adapter(controller, adapter, DockViewportClosePolicy::Prevent);

    let outcome = runtime.handle_window_closed(window.window_id());

    assert_eq!(outcome.status, DockViewportCloseStatus::Closed);
    assert_eq!(outcome.space, Some(secondary_space.clone()));
    assert_eq!(runtime.adapter().window_for_space(&secondary_space), None);
}
