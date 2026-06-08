use crate::{
    DockController, DockGraph, DockNode, DockSpaceId, DockViewportClosePolicy,
    DockViewportRuntimeHandle, DockViewportShouldCloseStatus, DockWorkspace, host_test_support::*,
};
use open_gpui::{AppContext as _, TestAppContext, VisualTestContext};

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
