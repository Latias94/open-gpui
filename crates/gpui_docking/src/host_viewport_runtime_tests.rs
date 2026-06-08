use crate::{
    DockController, DockGraph, DockHost, DockNode, DockSpaceId, DockViewportAdapter,
    DockViewportClosePolicy, DockViewportCloseStatus, DockViewportOpenStatus, DockViewportRuntime,
    DockViewportShouldCloseStatus, DockWorkspace, host_test_support::*,
};
use open_gpui::{
    AnyWindowHandle, AppContext as _, TestAppContext, VisualTestContext, WindowHandle, WindowId,
    px, size,
};

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
