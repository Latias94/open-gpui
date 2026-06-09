use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockController, DockGraph, DockHost,
    DockItemId, DockNode, DockOpApplyError, DockSpaceId, DockViewportAdapter,
    DockViewportClosePolicy, DockViewportCloseStatus, DockViewportOpenStatus, DockViewportRuntime,
    DockViewportShouldCloseStatus, DockViewportTearOffOpenOutcome, DockViewportTearOffRequest,
    DockWorkspace,
    host_test_support::*,
    viewport_tear_off::{
        DockViewportTearOffBeginOutcome, DockViewportTearOffCancelReason,
        DockViewportTearOffCompletionOutcome, DockViewportTearOffTick,
    },
};
use open_gpui::{
    AnyWindowHandle, AppContext as _, TestAppContext, VisualTestContext, WindowHandle, WindowId,
    point, px, size,
};

fn tear_off_request(
    source_space: DockSpaceId,
    source_tabs: crate::DockNodeId,
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
fn viewport_runtime_tear_off_opens_viewport_then_moves_item(cx: &mut TestAppContext) {
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
    let mut runtime = DockViewportRuntime::new(controller.clone());

    let outcome = cx
        .update(|app| {
            runtime.open_tear_off_viewport(
                tear_off_request(primary_space.clone(), source_tabs, item("a")),
                detached_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("tear-off viewport should open through runtime");

    let DockViewportTearOffOpenOutcome::Completed(completed) = outcome else {
        panic!("tear-off should complete after opening a viewport");
    };
    assert_eq!(completed.action, DockActionOutcome::Changed);
    assert_eq!(completed.pending.target_space, detached_space);
    assert_eq!(runtime.pending_tear_off_len(), 0);
    assert_eq!(
        runtime.adapter().window_for_space(&detached_space),
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
fn viewport_runtime_tear_off_duplicate_request_is_idempotent(cx: &mut TestAppContext) {
    let primary_space = DockSpaceId::from("primary");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    graph.set_root(primary_space.clone(), source_tabs);

    let mut workspace = DockWorkspace::new(primary_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let mut runtime = DockViewportRuntime::new(controller);

    let first = runtime.begin_tear_off_request_at(
        tear_off_request(primary_space.clone(), source_tabs, item("a")),
        detached_space.clone(),
        DockViewportTearOffTick::new(1),
    );
    let second = runtime.begin_tear_off_request_at(
        tear_off_request(primary_space, source_tabs, item("a")),
        DockSpaceId::from("other"),
        DockViewportTearOffTick::new(2),
    );

    assert!(matches!(first, DockViewportTearOffBeginOutcome::Pending(_)));
    let DockViewportTearOffBeginOutcome::Duplicate(existing) = second else {
        panic!("duplicate request should not create a second pending entry");
    };
    assert_eq!(existing.target_space, detached_space);
    assert_eq!(runtime.pending_tear_off_len(), 1);
    assert!(runtime.adapter().is_empty());
}

#[open_gpui::test]
fn viewport_runtime_tear_off_cancels_when_source_item_closes_before_window_created(
    cx: &mut TestAppContext,
) {
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
    let mut runtime = DockViewportRuntime::new(controller.clone());

    runtime.begin_tear_off_request_at(
        tear_off_request(primary_space.clone(), source_tabs, item("a")),
        detached_space.clone(),
        DockViewportTearOffTick::new(1),
    );
    cx.update_entity(&controller, |controller, _| {
        controller
            .apply_action(&DockAction::CloseItem {
                space: primary_space.clone(),
                item: item("a"),
            })
            .expect("source item close should commit before window creation");
    });

    let outcome = cx.update(|app| {
        runtime.complete_tear_off_viewport_at(
            &item("a"),
            WindowHandle::<DockHost>::new(WindowId::from(930)),
            DockViewportTearOffTick::new(2),
            app,
        )
    });

    let DockViewportTearOffCompletionOutcome::Cancelled(cancelled) = outcome else {
        panic!("completion should cancel when the source item is gone");
    };
    assert_eq!(
        cancelled.reason,
        DockViewportTearOffCancelReason::SourceMissing
    );
    assert_eq!(runtime.pending_tear_off_len(), 0);
    assert_eq!(runtime.adapter().window_for_space(&detached_space), None);
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&primary_space),
            vec![item("b")]
        );
        assert!(
            controller
                .graph()
                .collect_items_in_space(&detached_space)
                .is_empty()
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_tear_off_cancels_when_source_item_moves_before_window_created(
    cx: &mut TestAppContext,
) {
    let primary_space = DockSpaceId::from("primary");
    let other_space = DockSpaceId::from("other");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        active: 0,
    });
    let other_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        active: 0,
    });
    graph.set_root(primary_space.clone(), source_tabs);
    graph.set_root(other_space.clone(), other_tabs);

    let mut workspace = DockWorkspace::new(primary_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let mut runtime = DockViewportRuntime::new(controller.clone());

    runtime.begin_tear_off_request_at(
        tear_off_request(primary_space.clone(), source_tabs, item("a")),
        detached_space.clone(),
        DockViewportTearOffTick::new(1),
    );
    cx.update_entity(&controller, |controller, _| {
        controller
            .apply_action(&DockAction::MoveTab {
                source_space: primary_space.clone(),
                source_tabs,
                item: item("a"),
                target_space: other_space.clone(),
                target_tabs: other_tabs,
                zone: crate::DropZone::Center,
                insert_index: None,
            })
            .expect("source item move should commit before window creation");
    });

    let outcome = cx.update(|app| {
        runtime.complete_tear_off_viewport_at(
            &item("a"),
            WindowHandle::<DockHost>::new(WindowId::from(931)),
            DockViewportTearOffTick::new(2),
            app,
        )
    });

    let DockViewportTearOffCompletionOutcome::Cancelled(cancelled) = outcome else {
        panic!("completion should cancel when the source item moved");
    };
    assert_eq!(
        cancelled.reason,
        DockViewportTearOffCancelReason::SourceMoved
    );
    assert_eq!(runtime.pending_tear_off_len(), 0);
    assert_eq!(runtime.adapter().window_for_space(&detached_space), None);
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&other_space),
            vec![item("c"), item("a")]
        );
        assert!(
            controller
                .graph()
                .collect_items_in_space(&detached_space)
                .is_empty()
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_tear_off_expiration_clears_pending_without_graph_mutation(
    cx: &mut TestAppContext,
) {
    let primary_space = DockSpaceId::from("primary");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    graph.set_root(primary_space.clone(), source_tabs);

    let mut workspace = DockWorkspace::new(primary_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let mut runtime = DockViewportRuntime::new(controller.clone());

    runtime.begin_tear_off_request_at(
        tear_off_request(primary_space.clone(), source_tabs, item("a")),
        detached_space.clone(),
        DockViewportTearOffTick::new(1),
    );
    let expired = runtime.expire_tear_off_requests_at(DockViewportTearOffTick::new(602));

    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].reason, DockViewportTearOffCancelReason::Expired);
    assert_eq!(runtime.pending_tear_off_len(), 0);
    assert!(runtime.adapter().is_empty());
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&primary_space),
            vec![item("a")]
        );
        assert!(
            controller
                .graph()
                .collect_items_in_space(&detached_space)
                .is_empty()
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_tear_off_commit_failure_cleans_runtime_mapping(cx: &mut TestAppContext) {
    let primary_space = DockSpaceId::from("primary");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        active: 0,
    });
    graph.set_root(primary_space.clone(), source_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(primary_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let mut runtime = DockViewportRuntime::new(controller.clone());
    let window = WindowHandle::<DockHost>::new(WindowId::from(932));

    runtime.begin_tear_off_request_at(
        tear_off_request(primary_space.clone(), source_tabs, item("a")),
        detached_space.clone(),
        DockViewportTearOffTick::new(1),
    );
    let outcome = cx.update(|app| {
        runtime.complete_tear_off_viewport_at(
            &item("a"),
            window,
            DockViewportTearOffTick::new(2),
            app,
        )
    });

    let DockViewportTearOffCompletionOutcome::CommitFailed(failure) = outcome else {
        panic!("non-empty destination space should fail the tear-off move transaction");
    };
    assert_eq!(
        failure.error,
        DockActionApplyError::Graph(DockOpApplyError::TargetSpaceNotEmpty {
            space: detached_space.clone()
        })
    );
    assert_eq!(runtime.pending_tear_off_len(), 0);
    assert_eq!(runtime.adapter().window_for_space(&detached_space), None);
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&primary_space),
            vec![item("a")]
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            vec![item("c")]
        );
    });
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
    let controller = cx.new(|_| DockController::new(DockWorkspace::new(space(), DockGraph::new())));
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
