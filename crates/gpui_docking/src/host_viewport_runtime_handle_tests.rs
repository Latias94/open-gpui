use crate::{
    DockActionApplyError, DockController, DockGraph, DockItemId, DockNode, DockNodeId, DockSpaceId,
    DockViewportClosePolicy, DockViewportDropOutcomeKind, DockViewportDropPayload,
    DockViewportDropRoute, DockViewportDropRouteOutcome, DockViewportPlatformSignals,
    DockViewportRouteTarget, DockViewportRuntimeHandle, DockViewportShouldCloseStatus,
    DockViewportTargetContext, DockViewportTearOffOpenOutcome, DockViewportTearOffRequest,
    DockWorkspace, DropZone, SplitAxis,
    debug::DockDebugRegion,
    drag::DockDragPayload,
    drop_preview::DockDropPreviewKind,
    drop_runtime::DockHostDropSceneFact,
    drop_target::{DockDropResolveSource, DockLeafDropTarget, DockResolvedDropTargetKind},
    host_test_support::*,
};
use open_gpui::{
    AppContext as _, Focusable, TestAppContext, VisualTestContext, WindowBounds, WindowOptions,
    point, px,
};

fn tear_off_request(
    source_space: DockSpaceId,
    source_tabs: DockNodeId,
    item: DockItemId,
) -> DockViewportTearOffRequest {
    DockViewportTearOffRequest {
        source_space,
        source_tabs,
        payload: DockViewportDropPayload::Item(item),
        release_position: point(px(900.0), px(900.0)),
        suggested_window_bounds: None,
    }
}

fn leaf_host_scene_fact(root: DockNodeId, target_tabs: DockNodeId) -> DockHostDropSceneFact {
    DockHostDropSceneFact::Leaf(DockLeafDropTarget {
        root,
        target_tabs,
        bounds: floating_bounds(0.0, 0.0, 360.0, 220.0),
        is_central: false,
    })
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
    assert_eq!(runtime.registered_viewport_spaces().len(), 1);

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
fn viewport_runtime_handle_rejects_stale_host_scene_frame_facts(cx: &mut TestAppContext) {
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(target_space.clone(), target_tabs);

    let mut workspace = DockWorkspace::new(target_space.clone(), graph);
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
        .expect("target viewport should open");
    let window_bounds = opened
        .window
        .update(cx, |_, window, _| window.window_bounds())
        .expect("target window should be live");
    let window_bounds = WindowBounds::Windowed(window_bounds.get_bounds());
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);

    let first = runtime
        .begin_viewport_host_scene_frame(
            target_space.clone(),
            opened.window.window_id(),
            window_bounds,
            host_bounds,
            point(px(120.0), px(100.0)),
        )
        .expect("first scene frame should register")
        .frame;
    assert!(runtime.push_viewport_host_scene_frame_fact(
        &first,
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));

    let second = runtime
        .begin_viewport_host_scene_frame(
            target_space.clone(),
            opened.window.window_id(),
            window_bounds,
            host_bounds,
            point(px(120.0), px(100.0)),
        )
        .expect("second scene frame should register")
        .frame;
    assert!(
        !runtime.push_viewport_host_scene_frame_fact(
            &first,
            leaf_host_scene_fact(target_tabs, target_tabs),
        ),
        "facts captured by an older render frame must not populate a newer scene"
    );
    assert!(runtime.push_viewport_host_scene_frame_fact(
        &second,
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));
}

#[open_gpui::test]
fn viewport_runtime_handle_retain_close_clears_scene_and_reopens_layout(cx: &mut TestAppContext) {
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
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
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
    assert!(runtime.begin_viewport_host_scene(
        secondary_space.clone(),
        opened.window.window_id(),
        WindowBounds::Windowed(floating_bounds(10.0, 20.0, 360.0, 220.0)),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        point(px(120.0), px(100.0)),
    ));
    assert!(
        runtime
            .last_host_scene_screen_position(&secondary_space)
            .is_some()
    );

    assert!(
        runtime
            .handle_window_should_close(opened.window.window_id())
            .allows_close(),
        "RetainLayout should allow GPUI to close the platform viewport"
    );
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
    assert_eq!(
        runtime.last_host_scene_screen_position(&secondary_space),
        None,
        "closing a retained viewport should discard stale host scene facts"
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&secondary_space),
            vec![item("b")],
            "RetainLayout close must not mutate logical graph layout"
        );
    });

    let reopened = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("retained dock space should reopen through runtime handle");
    let reopened_window = reopened
        .window
        .downcast::<crate::DockHost>()
        .expect("reopened viewport should render DockHost");
    let reopened_host = reopened_window
        .root(cx)
        .expect("reopened viewport should expose DockHost root");
    cx.run_until_parked();
    let reopened_visual = VisualTestContext::from_window(reopened.window, cx);

    assert!(
        selector_for(
            &reopened_visual,
            &reopened_host,
            DockDebugRegion::Panel { item: item("b") },
        )
        .is_some(),
        "reopened retained layout should render the original panel"
    );
}

#[open_gpui::test]
fn viewport_runtime_handle_merge_back_close_moves_content_to_fallback(cx: &mut TestAppContext) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
        active: 1,
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::with_close_policy(
        controller.clone(),
        DockViewportClosePolicy::MergeBack {
            target_space: main_space.clone(),
        },
    );
    cx.update(|app| runtime.observe_window_closed(app).detach());

    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                detached_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("detached viewport should open");

    assert!(
        runtime
            .handle_window_should_close(opened.window.window_id())
            .allows_close(),
        "merge-back policy should allow GPUI to close before graph merge"
    );
    opened
        .window
        .update(cx, |_, window, _| window.remove_window())
        .expect("detached viewport should still be live");
    cx.run_until_parked();

    assert_eq!(
        runtime.borrow().adapter().window_for_space(&detached_space),
        None
    );
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, active } = controller
            .graph()
            .node(main_tabs)
            .expect("fallback tabs should remain")
        else {
            panic!("fallback root should be tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a"), item("c")]);
        assert_eq!(*active, 2);
        assert!(
            controller
                .graph()
                .collect_items_in_space(&detached_space)
                .is_empty(),
            "merge-back close should empty the detached logical space"
        );
    });
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
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        opened.window.window_id(),
        target_window_bounds,
        host_bounds,
        point(px(0.0), px(0.0))
    ));
    let target_point = point(
        target_window_bounds.get_bounds().origin.x + px(20.0),
        target_window_bounds.get_bounds().origin.y + px(40.0),
    );

    let route = cx.update(|app| {
        runtime.resolve_payload_drop_route_with_platform_signals(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_point,
            Some(target_window_bounds),
            DockViewportPlatformSignals::from_app(app).with_hovered_window(opened.window),
            app,
        )
    });

    assert_eq!(
        route,
        DockViewportDropRoute::KnownViewport {
            hit: crate::DockViewportHit {
                space: target_space.clone(),
                host_position: point(px(20.0), px(40.0)),
            },
            window: opened.window,
        }
    );
    assert_eq!(
        runtime
            .runtime_status()
            .last_route
            .as_ref()
            .map(|record| &record.target),
        Some(&DockViewportRouteTarget::KnownViewport {
            space: target_space,
            window_id: opened.window.window_id(),
            host_position: point(px(20.0), px(40.0)),
        }),
        "runtime status should expose the last resolved known-viewport route"
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
        runtime.resolve_payload_drop_route_with_context(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            None,
            DockViewportTargetContext::new(),
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
    assert!(
        matches!(
            runtime
                .runtime_status()
                .last_route
                .as_ref()
                .map(|record| &record.target),
            Some(DockViewportRouteTarget::Rejected {
                reason: crate::DockPolicyError::PlatformViewportsDisabled,
            })
        ),
        "runtime status should record the rejected route"
    );

    cx.update_entity(&controller, |controller, _| {
        controller.policy_mut().set_allow_platform_viewports(true);
    });
    let tear_off = cx.update(|app| {
        runtime.resolve_payload_drop_route_with_context(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            None,
            DockViewportTargetContext::new(),
            app,
        )
    });
    assert!(matches!(
        tear_off,
        DockViewportDropRoute::TearOff(DockViewportTearOffRequest {
            source_space: routed_source,
            source_tabs: routed_tabs,
            payload: DockViewportDropPayload::Item(routed_item),
            release_position: routed_position,
            suggested_window_bounds: None,
        }) if routed_source == source_space
            && routed_tabs == source_tabs
            && routed_item == item("a")
            && routed_position == release_position
    ));
    assert!(
        matches!(
            runtime
                .runtime_status()
                .last_route
                .as_ref()
                .map(|record| &record.target),
            Some(DockViewportRouteTarget::TearOff {
                release_position: recorded_position,
            }) if *recorded_position == release_position
        ),
        "runtime status should record the tear-off route"
    );
}

#[open_gpui::test]
fn viewport_runtime_handle_commits_tear_off_drop_route(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        active: 0,
    });
    graph.set_root(source_space.clone(), source_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let release_position = point(px(900.0), px(900.0));
    let suggested_window_bounds =
        WindowBounds::Windowed(floating_bounds(880.0, 880.0, 360.0, 240.0));

    let outcome = cx
        .update(|app| {
            let route = runtime.resolve_payload_drop_route_with_context(
                source_space.clone(),
                source_tabs,
                DockViewportDropPayload::Item(item("a")),
                release_position,
                Some(suggested_window_bounds),
                DockViewportTargetContext::new(),
                app,
            );
            runtime.commit_payload_drop_route_with_outcome(
                &source_space,
                source_tabs,
                DockViewportDropPayload::Item(item("a")),
                route,
                app,
            )
        })
        .expect("tear-off route should commit through runtime handle");

    let activation = outcome.activation_target();
    let DockViewportDropRouteOutcome::TearOff(DockViewportTearOffOpenOutcome::Completed(completed)) =
        outcome
    else {
        panic!("tear-off route should open a viewport and complete the move");
    };
    assert_eq!(completed.action, crate::DockActionOutcome::Changed);
    assert_eq!(
        activation.as_ref().map(|target| target.window),
        Some(completed.registration.window),
        "tear-off completion should surface the new viewport activation target"
    );
    assert_eq!(completed.pending.request.release_position, release_position);
    assert_eq!(
        completed.pending.request.suggested_window_bounds,
        Some(suggested_window_bounds)
    );
    assert_eq!(
        completed.pending.target_space.as_str(),
        "source:tear-off:a:0"
    );
    assert_eq!(
        runtime
            .borrow()
            .adapter()
            .window_for_space(&completed.pending.target_space),
        Some(completed.registration.window)
    );
    let opened_window = completed
        .registration
        .window
        .downcast::<crate::DockHost>()
        .expect("tear-off viewport should render DockHost");
    let opened_host = opened_window
        .root(cx)
        .expect("tear-off viewport should expose DockHost root");
    cx.read_entity(&opened_host, |host, _| {
        assert!(
            host.viewport_runtime().is_some(),
            "tear-off viewport should keep the runtime-aware host path for dock-back"
        );
    });
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("b")]
        );
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&completed.pending.target_space),
            vec![item("a")]
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_handle_commits_stack_tear_off_drop_route(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
        active: 1,
    });
    graph.set_root(source_space.clone(), source_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let release_position = point(px(900.0), px(900.0));
    let suggested_window_bounds =
        WindowBounds::Windowed(floating_bounds(880.0, 880.0, 360.0, 240.0));

    let outcome = cx
        .update(|app| {
            let route = runtime.resolve_payload_drop_route_with_context(
                source_space.clone(),
                source_tabs,
                DockViewportDropPayload::Tabs,
                release_position,
                Some(suggested_window_bounds),
                DockViewportTargetContext::new(),
                app,
            );
            runtime.commit_payload_drop_route_with_outcome(
                &source_space,
                source_tabs,
                DockViewportDropPayload::Tabs,
                route,
                app,
            )
        })
        .expect("stack tear-off route should commit through runtime handle");

    let activation = outcome.activation_target();
    let DockViewportDropRouteOutcome::TearOff(DockViewportTearOffOpenOutcome::Completed(completed)) =
        outcome
    else {
        panic!("stack tear-off route should open a viewport and complete the move");
    };
    assert_eq!(completed.action, crate::DockActionOutcome::Changed);
    assert_eq!(
        activation.as_ref().map(|target| target.window),
        Some(completed.registration.window),
        "stack tear-off completion should surface the new viewport activation target"
    );
    assert_eq!(
        completed.pending.target_space.as_str(),
        "source:tear-off:tabs:0"
    );
    let opened_window = completed
        .registration
        .window
        .downcast::<crate::DockHost>()
        .expect("stack tear-off viewport should render DockHost");
    let opened_host = opened_window
        .root(cx)
        .expect("stack tear-off viewport should expose DockHost root");
    cx.read_entity(&opened_host, |host, _| {
        assert!(
            host.viewport_runtime().is_some(),
            "stack tear-off viewport should keep the runtime-aware host path for dock-back"
        );
    });
    cx.read_entity(&controller, |controller, _| {
        assert!(
            controller
                .graph()
                .collect_items_in_space(&source_space)
                .is_empty()
        );
        let detached_root = controller
            .graph()
            .root(&completed.pending.target_space)
            .expect("detached stack should become the target root");
        let DockNode::Tabs { items, active } = controller
            .graph()
            .node(detached_root)
            .expect("detached root should exist")
        else {
            panic!("detached root should be tabs");
        };
        assert_eq!(items, &vec![item("a"), item("c")]);
        assert_eq!(*active, 1);
    });
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
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        opened.window.window_id(),
        target_window_bounds,
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        point(px(0.0), px(0.0))
    ));

    let target_point = point(
        target_window_bounds.get_bounds().origin.x + px(120.0),
        target_window_bounds.get_bounds().origin.y + px(100.0),
    );
    let result = cx.update(|app| {
        let route = runtime.resolve_payload_drop_route_with_platform_signals(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_point,
            None,
            DockViewportPlatformSignals::from_app(app).with_hovered_window(opened.window),
            app,
        );
        assert!(
            matches!(
                &route,
                DockViewportDropRoute::KnownViewport { window, .. } if *window == opened.window
            ),
            "known viewport route should carry the destination window"
        );
        runtime.commit_payload_drop_route_with_outcome(
            &source_space,
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            route,
            app,
        )
    });

    assert_eq!(result, Err(DockActionApplyError::DropTargetUnavailable));
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
    let source_opened = cx
        .update(|app| {
            runtime.open_viewport(
                source_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("source viewport should open");
    source_opened
        .window
        .update(cx, |_, window, _| window.activate_window())
        .expect("source viewport should be activatable before drop");
    let before_drop_context = opened
        .window
        .update(cx, |_, window, app| {
            DockViewportPlatformSignals::from_window(window, app).target_context()
        })
        .expect("target window should be live");
    assert_eq!(
        before_drop_context.active_window,
        Some(source_opened.window.window_id()),
        "source viewport should be active before the routed drop commits"
    );
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
        let release_position = runtime
            .last_host_scene_screen_position(&target_space)
            .expect("target scene should expose a screen position");
        let result = runtime.commit_payload_drop_from_screen_with_platform_signals(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            None,
            DockViewportPlatformSignals::from_app(app).with_hovered_window(opened.window),
            app,
        );
        assert!(
            matches!(
                runtime
                    .runtime_status()
                    .last_route
                    .as_ref()
                    .map(|record| &record.target),
                Some(DockViewportRouteTarget::KnownViewport { window_id, .. })
                    if *window_id == opened.window.window_id()
            ),
            "screen release seam should record the destination viewport route"
        );
        result
    });

    let DockViewportDropRouteOutcome::Action(action) = result.expect("route should commit") else {
        panic!("known viewport drop should produce a normal action outcome");
    };
    assert_eq!(action.action, crate::DockActionOutcome::Changed);
    assert_eq!(
        action
            .activation
            .as_ref()
            .map(|activation| activation.window),
        Some(opened.window),
        "known viewport drop should request activation of the destination window"
    );
    assert_eq!(
        action
            .activation
            .as_ref()
            .and_then(|activation| activation.focus_item.clone()),
        Some(item("a")),
        "known viewport drop should request focus for the moved item"
    );
    let status = runtime.runtime_status();
    assert_eq!(
        status.last_drop_outcome.as_ref().map(|record| record.kind),
        Some(DockViewportDropOutcomeKind::Action),
        "runtime status should record the routed action outcome"
    );
    assert_eq!(
        status
            .last_activation
            .as_ref()
            .map(|activation| activation.window_id),
        Some(opened.window.window_id()),
        "runtime status should record the destination activation"
    );
    assert_eq!(
        status
            .last_activation
            .as_ref()
            .and_then(|activation| activation.focus_item.clone()),
        Some(item("a")),
        "runtime status should record the focused item"
    );
    let after_drop_context = source_opened
        .window
        .update(cx, |_, window, app| {
            DockViewportPlatformSignals::from_window(window, app).target_context()
        })
        .expect("source window should be live");
    assert_eq!(
        after_drop_context.active_window,
        Some(opened.window.window_id()),
        "successful routed drop should activate the destination viewport"
    );
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
fn host_render_drop_consumes_routed_viewport_activation(cx: &mut TestAppContext) {
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

    let panel_a = test_view(cx, "A");
    let panel_a_focus = cx.read_entity(&panel_a, |panel, cx| panel.focus_handle(cx));
    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel_a);
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let target_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let target_opened = cx
        .update(|app| {
            runtime.open_viewport(
                target_space.clone(),
                WindowOptions {
                    window_bounds: Some(target_bounds),
                    ..Default::default()
                },
                app,
            )
        })
        .expect("target viewport should open");
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_opened.window.window_id(),
        target_bounds,
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        point(px(120.0), px(100.0)),
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_opened.window.window_id(),
        DockHostDropSceneFact::Leaf(DockLeafDropTarget {
            root: target_tabs,
            target_tabs,
            bounds: floating_bounds(0.0, 0.0, 360.0, 220.0),
            is_central: false,
        }),
    ));

    let source_bounds = WindowBounds::Windowed(floating_bounds(520.0, 100.0, 360.0, 220.0));
    let source_opened = cx
        .update(|app| {
            runtime.open_viewport(
                source_space.clone(),
                WindowOptions {
                    window_bounds: Some(source_bounds),
                    ..Default::default()
                },
                app,
            )
        })
        .expect("source viewport should open");
    source_opened
        .window
        .update(cx, |_, window, _| window.activate_window())
        .expect("source viewport should be activatable before host drop");
    let source_window = source_opened
        .window
        .downcast::<crate::DockHost>()
        .expect("source viewport should render DockHost");
    let source_window_bounds = source_window
        .update(cx, |_, window, _| window.window_bounds().get_bounds())
        .expect("source window should be live");
    let release_screen_position = runtime
        .last_host_scene_screen_position(&target_space)
        .expect("target scene should expose a screen position");
    let release_in_source_window = point(
        release_screen_position.x - source_window_bounds.origin.x,
        release_screen_position.y - source_window_bounds.origin.y,
    );
    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );

    let changed = source_window
        .update(cx, |host, window, cx| {
            host.drop_payload_from_render(
                &payload,
                source_space.clone(),
                release_in_source_window,
                window,
                cx,
            )
        })
        .expect("source host should commit the routed render drop");
    assert!(changed, "host render drop should report a graph change");
    cx.run_until_parked();

    let after_drop_context = source_opened
        .window
        .update(cx, |_, window, app| {
            DockViewportPlatformSignals::from_window(window, app).target_context()
        })
        .expect("source window should be live");
    assert_eq!(
        after_drop_context.active_window,
        Some(target_opened.window.window_id()),
        "host interaction should consume the routed activation target"
    );
    target_opened
        .window
        .update(cx, |_, window, cx| {
            assert_eq!(
                window.focused(cx),
                Some(panel_a_focus),
                "target viewport should focus the moved panel after rendered drop"
            );
        })
        .expect("target window should still be live");
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
fn host_render_route_preview_uses_route_debug_selector(cx: &mut TestAppContext) {
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
    let runtime = DockViewportRuntimeHandle::new(controller);

    let target_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let target_opened = cx
        .update(|app| {
            runtime.open_viewport(
                target_space.clone(),
                WindowOptions {
                    window_bounds: Some(target_bounds),
                    ..Default::default()
                },
                app,
            )
        })
        .expect("target viewport should open");
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_opened.window.window_id(),
        target_bounds,
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        point(px(120.0), px(100.0)),
    ));

    let source_bounds = WindowBounds::Windowed(floating_bounds(520.0, 100.0, 360.0, 220.0));
    let source_opened = cx
        .update(|app| {
            runtime.open_viewport(
                source_space.clone(),
                WindowOptions {
                    window_bounds: Some(source_bounds),
                    ..Default::default()
                },
                app,
            )
        })
        .expect("source viewport should open");
    let source_window = source_opened
        .window
        .downcast::<crate::DockHost>()
        .expect("source viewport should render DockHost");
    let source_host = source_window
        .root(cx)
        .expect("source viewport should expose DockHost root");
    let source_window_bounds = source_window
        .update(cx, |_, window, _| window.window_bounds().get_bounds())
        .expect("source window should be live");
    let target_screen_position = point(
        target_bounds.get_bounds().origin.x + px(120.0),
        target_bounds.get_bounds().origin.y + px(100.0),
    );
    let route_position_in_source_window = point(
        target_screen_position.x - source_window_bounds.origin.x,
        target_screen_position.y - source_window_bounds.origin.y,
    );
    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );

    source_window
        .update(cx, |host, window, cx| {
            host.begin_host_drop_scene_from_render(
                &payload,
                floating_bounds(0.0, 0.0, 360.0, 220.0),
                route_position_in_source_window,
                window,
                cx,
            );
        })
        .expect("source host should update route preview");
    cx.run_until_parked();
    let source_visual = VisualTestContext::from_window(source_window.into(), cx);

    assert!(
        selector_for(
            &source_visual,
            &source_host,
            DockDebugRegion::DropRoutePreview {
                kind: DockDropPreviewKind::KnownViewportRoute
            }
        )
        .is_some(),
        "known viewport route should render through the route preview selector"
    );
    assert!(
        selector_for(&source_visual, &source_host, DockDebugRegion::DropPreview).is_none(),
        "known viewport route preview should not be exposed as a local drop preview"
    );
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
fn runtime_opened_viewports_dock_back_from_source_only_release(cx: &mut TestAppContext) {
    let target_space = DockSpaceId::from("main");
    let source_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    graph.set_root(target_space.clone(), target_tabs);
    graph.set_root(source_space.clone(), source_tabs);

    let mut workspace = DockWorkspace::new(target_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let target_opened = cx
        .update(|app| {
            runtime.open_viewport(
                target_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("target viewport should open");
    let source_opened = cx
        .update(|app| {
            runtime.open_viewport(
                source_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("source viewport should open");
    let target_window = target_opened
        .window
        .downcast::<crate::DockHost>()
        .expect("target viewport should render DockHost");
    let target_host = target_window
        .root(cx)
        .expect("target viewport should expose DockHost root");
    cx.run_until_parked();

    let mut target_visual = VisualTestContext::from_window(target_opened.window, cx);
    let target_tabs_selector = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::Tabs { node: target_tabs },
    )
    .expect("target tabs selector should be emitted");
    assert!(
        runtime
            .last_host_scene_screen_position(&target_space)
            .is_some(),
        "ordinary target viewport render should publish a runtime host scene"
    );
    let target_position = debug_bounds(&mut target_visual, &target_tabs_selector).center();
    let target_window_bounds = target_opened
        .window
        .update(cx, |_, window, _| window.window_bounds())
        .expect("target window should still be live")
        .get_bounds();
    let release_screen_position = point(
        target_window_bounds.origin.x + target_position.x,
        target_window_bounds.origin.y + target_position.y,
    );
    let source_release_signals = source_opened
        .window
        .update(cx, |_, window, app| {
            DockViewportPlatformSignals::from_window(window, app)
        })
        .expect("source window should still be live");
    // TestPlatform normalizes runtime-opened window origins to zero. Override only the source
    // snapshot so this models a native detached window releasing over main, not over itself.
    assert!(runtime.begin_viewport_host_scene(
        source_space.clone(),
        source_opened.window.window_id(),
        WindowBounds::Windowed(floating_bounds(520.0, 0.0, 360.0, 220.0)),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        point(px(0.0), px(0.0)),
    ));

    let result = cx.update(|app| {
        runtime.commit_payload_drop_from_screen_with_platform_signals(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_screen_position,
            None,
            source_release_signals,
            app,
        )
    });

    let DockViewportDropRouteOutcome::Action(action) = result.expect("dock-back should commit")
    else {
        panic!("dock-back should resolve to a normal action");
    };
    assert!(
        matches!(
            runtime
                .runtime_status()
                .last_route
                .as_ref()
                .map(|record| &record.target),
            Some(DockViewportRouteTarget::KnownViewport { space, .. }) if space == &target_space
        ),
        "dock-back should route to the target viewport, got {:?}",
        runtime.runtime_status().last_route
    );
    assert_eq!(action.action, crate::DockActionOutcome::Changed);
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
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            Vec::<DockItemId>::new(),
            "source viewport should be emptied by a successful dock-back"
        );
    });
}

#[open_gpui::test]
fn runtime_opened_viewports_support_cross_window_stack_drag(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
        active: 1,
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
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
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

    let source_stack = selector_for(
        &source_visual,
        &source_host,
        DockDebugRegion::Tabs { node: source_tabs },
    )
    .expect("source tabs selector should be emitted");
    let target_stack = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::Tabs { node: target_tabs },
    )
    .expect("target tabs selector should be emitted");
    let source_bounds = debug_bounds(&mut source_visual, &source_stack);
    let start = point(
        source_bounds.origin.x + source_bounds.size.width - px(8.0),
        source_bounds.origin.y + px(12.0),
    );
    let threshold = point(start.x + px(24.0), start.y);
    let end = debug_bounds(&mut target_visual, &target_stack).center();

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
        assert_eq!(items, &vec![item("b"), item("a"), item("c")]);
        assert_eq!(*active, 2);
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
fn viewport_runtime_handle_commits_known_viewport_stack_drop_through_host_scene(
    cx: &mut TestAppContext,
) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
        active: 1,
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
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
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
        let route = runtime.resolve_payload_drop_route_with_platform_signals(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Tabs,
            runtime
                .last_host_scene_screen_position(&target_space)
                .expect("target scene should expose a screen position"),
            None,
            DockViewportPlatformSignals::from_app(app).with_hovered_window(opened.window),
            app,
        );
        runtime
            .commit_payload_drop_route_with_outcome(
                &source_space,
                source_tabs,
                DockViewportDropPayload::Tabs,
                route,
                app,
            )
            .and_then(|outcome| outcome.action_result())
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
        assert_eq!(items, &vec![item("b"), item("a"), item("c")]);
        assert_eq!(*active, 2);
    });
}

#[open_gpui::test]
fn viewport_runtime_handle_resolves_rendered_root_edge_scene(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    let target_left_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    let target_right_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        active: 0,
    });
    let target_root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![target_left_tabs, target_right_tabs],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_root);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let target_opened = cx
        .update(|app| {
            runtime.open_viewport(
                target_space.clone(),
                viewport_window_options(420.0, 240.0),
                app,
            )
        })
        .expect("target viewport should open");
    let source_opened = cx
        .update(|app| {
            runtime.open_viewport(
                source_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("source viewport should open");
    let target_window = target_opened
        .window
        .downcast::<crate::DockHost>()
        .expect("target viewport should render DockHost");
    let target_host = target_window
        .root(cx)
        .expect("target viewport should expose DockHost root");
    cx.run_until_parked();

    let mut target_visual = VisualTestContext::from_window(target_opened.window, cx);
    let right_tabs_selector = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::Tabs {
            node: target_right_tabs,
        },
    )
    .expect("right target tabs selector should be emitted");
    assert!(
        runtime
            .last_host_scene_screen_position(&target_space)
            .is_some(),
        "rendered target viewport should publish a host scene"
    );
    let right_tabs_bounds = debug_bounds(&mut target_visual, &right_tabs_selector);
    let target_host_position = point(
        right_tabs_bounds.origin.x + right_tabs_bounds.size.width - px(2.0),
        right_tabs_bounds.center().y,
    );
    let resolved = cx
        .update(|app| runtime.resolve_host_scene_target(&target_space, target_host_position, app))
        .expect("rendered host scene should resolve the root edge");
    assert_eq!(resolved.source, DockDropResolveSource::RootEdge);
    assert!(matches!(
        resolved.kind,
        DockResolvedDropTargetKind::RootEdge {
            root,
            leaf_tabs,
            zone: DropZone::Right,
        } if root == target_root && leaf_tabs == target_right_tabs
    ));

    let target_window_bounds = target_opened
        .window
        .update(cx, |_, window, _| window.window_bounds())
        .expect("target window should still be live")
        .get_bounds();
    let release_screen_position = point(
        target_window_bounds.origin.x + target_host_position.x,
        target_window_bounds.origin.y + target_host_position.y,
    );
    let source_release_signals = source_opened
        .window
        .update(cx, |_, window, app| {
            DockViewportPlatformSignals::from_window(window, app)
        })
        .expect("source window should still be live");
    assert!(runtime.begin_viewport_host_scene(
        source_space.clone(),
        source_opened.window.window_id(),
        WindowBounds::Windowed(floating_bounds(520.0, 0.0, 360.0, 220.0)),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        point(px(0.0), px(0.0)),
    ));

    let result = cx.update(|app| {
        runtime.commit_payload_drop_from_screen_with_platform_signals(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_screen_position,
            None,
            source_release_signals,
            app,
        )
    });

    let DockViewportDropRouteOutcome::Action(action) =
        result.expect("root-edge viewport drop should commit")
    else {
        panic!("root-edge viewport drop should resolve to a normal action");
    };
    assert_eq!(action.action, crate::DockActionOutcome::Changed);
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Split { children, .. } = controller
            .graph()
            .node(target_root)
            .expect("target root should still exist")
        else {
            panic!("target root should remain a split");
        };
        assert_eq!(children.len(), 3);
        let DockNode::Tabs { items, active } = controller
            .graph()
            .node(
                *children
                    .last()
                    .expect("root split should have a right child"),
            )
            .expect("rightmost child should exist")
        else {
            panic!("rightmost child should be tabs");
        };
        assert_eq!(items, &vec![item("a")]);
        assert_eq!(*active, 0);
    });
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
