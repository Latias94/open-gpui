use crate::{
    DockController, DockGraph, DockItemId, DockNode, DockNodeId, DockSpaceId, DockTransactionError,
    DockViewportClosePolicy, DockViewportDropRoute, DockViewportRuntimeHandle,
    DockViewportShouldCloseStatus, DockViewportTargetContext, DockViewportTearOffOpenOutcome,
    DockViewportTearOffRequest, DockWorkspace, debug::DockDebugRegion,
    drop_runtime::DockHostDropSceneFact, drop_target::DockLeafDropTarget, host_test_support::*,
};
use open_gpui::{AppContext as _, TestAppContext, VisualTestContext, WindowBounds, point, px};

fn tear_off_request(
    source_space: DockSpaceId,
    source_tabs: DockNodeId,
    item: DockItemId,
) -> DockViewportTearOffRequest {
    DockViewportTearOffRequest {
        source_space,
        source_tabs,
        item,
        release_position: point(px(900.0), px(900.0)),
        suggested_window_bounds: None,
    }
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
fn viewport_runtime_handle_opens_tear_off_viewport_and_moves_item(cx: &mut TestAppContext) {
    let primary_space = DockSpaceId::from("primary");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        active: 0,
    });
    graph.set_root(primary_space.clone(), source_tabs);

    let mut workspace = DockWorkspace::new(primary_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let outcome = cx
        .update(|app| {
            runtime.open_tear_off_viewport(
                tear_off_request(primary_space.clone(), source_tabs, item("a")),
                detached_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("tear-off viewport should open through runtime handle");

    let DockViewportTearOffOpenOutcome::Completed(completed) = outcome else {
        panic!("tear-off should complete through the handle");
    };
    assert_eq!(completed.pending.target_space, detached_space);
    assert_eq!(runtime.borrow().pending_tear_off_len(), 0);
    assert_eq!(
        runtime.borrow().adapter().window_for_space(&detached_space),
        Some(completed.registration.window)
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&primary_space),
            vec![item("b")]
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            vec![item("a")]
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_handle_resolves_drop_route_with_current_policy(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);

    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                target_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("target viewport should open through runtime handle");
    let target_window_bounds = opened
        .window
        .update(cx, |_, window, _| window.window_bounds())
        .expect("target window should be live");
    let target_window_bounds = WindowBounds::Windowed(target_window_bounds.get_bounds());
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    assert!(runtime.update_viewport_snapshot(
        &target_space,
        None,
        target_window_bounds,
        host_bounds
    ));
    let target_point = point(
        target_window_bounds.get_bounds().origin.x + px(20.0),
        target_window_bounds.get_bounds().origin.y + px(40.0),
    );

    let route = cx.update(|app| {
        runtime.resolve_drop_route_with_context(
            source_space.clone(),
            source_tabs,
            item("a"),
            target_point,
            Some(target_window_bounds),
            &DockViewportTargetContext::from_app(app).with_hovered_window(opened.window),
            app,
        )
    });

    assert_eq!(
        route,
        DockViewportDropRoute::KnownViewport {
            hit: crate::DockViewportHit {
                space: target_space,
                host_position: point(px(20.0), px(40.0)),
            },
            window: opened.window,
        }
    );
}

#[open_gpui::test]
fn viewport_runtime_handle_drop_route_uses_workspace_platform_policy(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    graph.set_root(source_space.clone(), source_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let release_position = point(px(900.0), px(900.0));

    let rejected = cx.update(|app| {
        runtime.resolve_drop_route_with_context(
            source_space.clone(),
            source_tabs,
            item("a"),
            release_position,
            None,
            &DockViewportTargetContext::new(),
            app,
        )
    });
    assert!(
        matches!(
            rejected,
            DockViewportDropRoute::Rejected(crate::DockPolicyError::PlatformViewportsDisabled)
        ),
        "default workspace policy should reject outside-all-viewports route"
    );

    cx.update_entity(&controller, |controller, _| {
        controller.policy_mut().set_allow_platform_viewports(true);
    });
    let tear_off = cx.update(|app| {
        runtime.resolve_drop_route_with_context(
            source_space.clone(),
            source_tabs,
            item("a"),
            release_position,
            None,
            &DockViewportTargetContext::new(),
            app,
        )
    });
    assert!(matches!(
        tear_off,
        DockViewportDropRoute::TearOff(DockViewportTearOffRequest {
            source_space: routed_source,
            source_tabs: routed_tabs,
            item: routed_item,
            release_position: routed_position,
            suggested_window_bounds: None,
        }) if routed_source == source_space
            && routed_tabs == source_tabs
            && routed_item == item("a")
            && routed_position == release_position
    ));
}

#[open_gpui::test]
fn viewport_runtime_handle_rejects_known_viewport_drop_without_host_scene(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                target_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("target viewport should open");
    let target_window_bounds = opened
        .window
        .update(cx, |_, window, _| window.window_bounds())
        .expect("target window should be live");
    let target_window_bounds = WindowBounds::Windowed(target_window_bounds.get_bounds());
    assert!(runtime.update_viewport_snapshot(
        &target_space,
        None,
        target_window_bounds,
        floating_bounds(0.0, 0.0, 360.0, 220.0)
    ));

    let target_point = point(
        target_window_bounds.get_bounds().origin.x + px(120.0),
        target_window_bounds.get_bounds().origin.y + px(100.0),
    );
    let result = cx.update(|app| {
        let route = runtime.resolve_drop_route_with_context(
            source_space.clone(),
            source_tabs,
            item("a"),
            target_point,
            None,
            &DockViewportTargetContext::from_app(app).with_hovered_window(opened.window),
            app,
        );
        runtime.commit_drop_route(&source_space, source_tabs, &item("a"), route, app)
    });

    assert_eq!(
        result,
        Err(
            DockTransactionError::ViewportTargetRequiresLocalResolution {
                space: target_space.clone()
            }
            .into()
        )
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("a")]
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&target_space),
            vec![item("b")]
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_handle_commits_known_viewport_drop_through_host_scene(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                target_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("target viewport should open");
    let target_window_bounds = opened
        .window
        .update(cx, |_, window, _| window.window_bounds())
        .expect("target window should be live");
    let target_window_bounds = WindowBounds::Windowed(target_window_bounds.get_bounds());
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        opened.window.window_id(),
        target_window_bounds,
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        point(px(120.0), px(100.0)),
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        opened.window.window_id(),
        DockHostDropSceneFact::Leaf(DockLeafDropTarget {
            root: target_tabs,
            target_tabs,
            bounds: floating_bounds(0.0, 0.0, 360.0, 220.0),
            is_central: false,
        }),
    ));

    let result = cx.update(|app| {
        let route = runtime.resolve_drop_route_with_context(
            source_space.clone(),
            source_tabs,
            item("a"),
            runtime
                .last_host_scene_screen_position(&target_space)
                .expect("target scene should expose a screen position"),
            None,
            &DockViewportTargetContext::from_app(app).with_hovered_window(opened.window),
            app,
        );
        runtime.commit_drop_route(&source_space, source_tabs, &item("a"), route, app)
    });

    assert_eq!(result, Ok(crate::DockActionOutcome::Changed));
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, active } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should still exist")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a")]);
        assert_eq!(*active, 1);
    });
}

#[open_gpui::test]
fn runtime_opened_viewports_publish_host_scene_for_cross_window_drop(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let source_opened = cx
        .update(|app| {
            runtime.open_viewport(
                source_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("source viewport should open");
    let target_opened = cx
        .update(|app| {
            runtime.open_viewport(
                target_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("target viewport should open");
    let source_window = source_opened
        .window
        .downcast::<crate::DockHost>()
        .expect("source viewport should render DockHost");
    let target_window = target_opened
        .window
        .downcast::<crate::DockHost>()
        .expect("target viewport should render DockHost");
    let source_host = source_window
        .root(cx)
        .expect("source viewport should expose DockHost root");
    let target_host = target_window
        .root(cx)
        .expect("target viewport should expose DockHost root");
    cx.run_until_parked();
    let mut source_visual = VisualTestContext::from_window(source_opened.window, cx);
    let mut target_visual = VisualTestContext::from_window(target_opened.window, cx);

    let source_tab = selector_for(
        &source_visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_tabs_selector = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::Tabs { node: target_tabs },
    )
    .expect("target tabs selector should be emitted");
    let start = debug_bounds(&mut source_visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let end = debug_bounds(&mut target_visual, &target_tabs_selector).center();

    source_visual.simulate_mouse_down(
        start,
        open_gpui::MouseButton::Left,
        open_gpui::Modifiers::none(),
    );
    source_visual.simulate_mouse_move(
        threshold,
        open_gpui::MouseButton::Left,
        open_gpui::Modifiers::none(),
    );
    target_visual.simulate_mouse_move(
        end,
        open_gpui::MouseButton::Left,
        open_gpui::Modifiers::none(),
    );
    target_visual.simulate_mouse_up(
        end,
        open_gpui::MouseButton::Left,
        open_gpui::Modifiers::none(),
    );
    cx.run_until_parked();

    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, active } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should still exist")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a")]);
        assert_eq!(*active, 1);
    });
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
