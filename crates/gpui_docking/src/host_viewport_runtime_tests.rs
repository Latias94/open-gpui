use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockClassId, DockController, DockGraph,
    DockGraphMutationError, DockHost, DockItemId, DockNode, DockPanel, DockPolicyError,
    DockSpaceId, DockViewportAdapter, DockViewportClosePolicy, DockViewportCloseStatus,
    DockViewportDropPayload, DockViewportDropRoute, DockViewportDropRouteCommit,
    DockViewportDropRouteRequest, DockViewportOpenStatus, DockViewportResolvedDropRoute,
    DockViewportRuntime, DockViewportRuntimeHandle, DockViewportShouldCloseStatus,
    DockViewportTargetContext, DockViewportTargetHit, DockViewportTearOffOpenOutcome,
    DockViewportTearOffOutcomeKind, DockViewportTearOffRequest, DockViewportWindowFacts,
    DockWorkspace,
    drag::DockDragPayload,
    drop_runtime::DockHostDropSceneFact,
    drop_target::DockLeafDropTarget,
    host_test_support::*,
    viewport_tear_off::{
        DockViewportTearOffBeginOutcome, DockViewportTearOffCancelReason,
        DockViewportTearOffCompletionOutcome, DockViewportTearOffTick,
    },
    viewport_test_support::handle,
    workspace_move_transaction::DockWorkspaceMoveTabRequest,
};
use open_gpui::{
    AnyWindowHandle, AppContext as _, TestAppContext, VisualTestContext, WindowBounds,
    WindowHandle, WindowId, WindowOptions, point, px, size,
};

fn tear_off_request(
    source_space: DockSpaceId,
    source_tabs: crate::DockNodeId,
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

fn item_tear_off_key(
    source_space: &DockSpaceId,
    source_tabs: crate::DockNodeId,
    item: DockItemId,
) -> crate::viewport_tear_off::DockViewportTearOffKey {
    DockViewportDropPayload::Item(item).key(source_space, source_tabs)
}

fn leaf_host_scene_fact(
    root: crate::DockNodeId,
    target_tabs: crate::DockNodeId,
) -> DockHostDropSceneFact {
    DockHostDropSceneFact::Leaf(DockLeafDropTarget {
        root,
        target_tabs,
        bounds: floating_bounds(0.0, 0.0, 360.0, 220.0),
        is_central: false,
    })
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
    let runtime = DockViewportRuntimeHandle::new(controller);

    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("secondary viewport should open through runtime");
    assert_eq!(opened.status(), DockViewportOpenStatus::Opened);
    assert_eq!(
        runtime
            .borrow()
            .adapter()
            .window_for_space(&secondary_space),
        Some(opened.window())
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
    assert_eq!(reused.status(), DockViewportOpenStatus::Reused);
    assert_eq!(reused.window(), opened.window());
    assert_eq!(runtime.borrow().adapter().spaces().len(), 1);
}

#[open_gpui::test]
fn viewport_runtime_tracks_recent_activation_for_fallback_priority(cx: &mut TestAppContext) {
    let alpha_space = DockSpaceId::from("alpha");
    let zeta_space = DockSpaceId::from("zeta");
    let mut graph = DockGraph::new();
    let alpha_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    let zeta_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(alpha_space.clone(), alpha_tabs);
    graph.set_root(zeta_space.clone(), zeta_tabs);

    let mut workspace = DockWorkspace::new(alpha_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);

    let zeta_opened = cx
        .update(|app| {
            runtime.open_viewport(
                zeta_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("zeta viewport should open");
    let alpha_opened = cx
        .update(|app| {
            runtime.open_viewport(
                alpha_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("alpha viewport should open");
    cx.run_until_parked();

    alpha_opened
        .window()
        .update(cx, |_, window, _| window.activate_window())
        .expect("alpha viewport should activate");
    cx.run_until_parked();
    zeta_opened
        .window()
        .update(cx, |_, window, _| window.activate_window())
        .expect("zeta viewport should activate");
    cx.run_until_parked();

    assert_eq!(
        runtime.borrow().adapter().spaces_by_fallback_priority(),
        vec![zeta_space, alpha_space]
    );
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
        .expect("tear-off viewport should open through runtime");

    let DockViewportTearOffOpenOutcome::Completed(completed) = outcome else {
        panic!("tear-off should complete after opening a viewport");
    };
    assert_eq!(completed.action(), DockActionOutcome::Changed);
    assert_eq!(completed.pending().target_space(), &detached_space);
    assert_eq!(runtime.borrow().pending_tear_off_len(), 0);
    assert_eq!(
        runtime.borrow().adapter().window_for_space(&detached_space),
        Some(completed.registration().window())
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
    let mut runtime_core = DockViewportRuntime::new(controller);

    let first = runtime_core.begin_tear_off_request_at(
        tear_off_request(primary_space.clone(), source_tabs, item("a")),
        detached_space.clone(),
        None,
        DockViewportTearOffTick::new(1),
    );
    let second = runtime_core.begin_tear_off_request_at(
        tear_off_request(primary_space.clone(), source_tabs, item("a")),
        DockSpaceId::from("other"),
        None,
        DockViewportTearOffTick::new(2),
    );

    assert!(matches!(first, DockViewportTearOffBeginOutcome::Pending(_)));
    let DockViewportTearOffBeginOutcome::Duplicate(existing) = second else {
        panic!("duplicate request should not create a second pending entry");
    };
    assert_eq!(existing.target_space(), &detached_space);
    assert_eq!(runtime_core.pending_tear_off_len(), 1);
    assert!(runtime_core.adapter().spaces().is_empty());

    let runtime = runtime_core.into_handle();

    let duplicate_open = cx
        .update(|app| {
            runtime.open_tear_off_viewport(
                tear_off_request(primary_space, source_tabs, item("a")),
                DockSpaceId::from("other"),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("duplicate tear-off should be idempotent");
    assert!(matches!(
        duplicate_open,
        DockViewportTearOffOpenOutcome::Duplicate(_)
    ));
    assert_eq!(
        runtime
            .runtime_status()
            .last_tear_off
            .as_ref()
            .map(|record| record.kind),
        Some(DockViewportTearOffOutcomeKind::Duplicate),
        "runtime status should record duplicate tear-off outcomes"
    );
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
        None,
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
        let key = item_tear_off_key(&primary_space, source_tabs, item("a"));
        runtime.complete_tear_off_viewport_at(
            &key,
            WindowHandle::<DockHost>::new(WindowId::from(930)),
            DockViewportTearOffTick::new(2),
            app,
        )
    });

    let DockViewportTearOffCompletionOutcome::Cancelled(cancelled) = outcome else {
        panic!("completion should cancel when the source item is gone");
    };
    assert_eq!(
        cancelled.reason(),
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
        None,
        DockViewportTearOffTick::new(1),
    );
    cx.update_entity(&controller, |controller, _| {
        controller
            .commit_tab_move(DockWorkspaceMoveTabRequest {
                source_space: &primary_space,
                source_tabs,
                item: &item("a"),
                target_space: &other_space,
                target_tabs: other_tabs,
                zone: crate::DropZone::Center,
                insert_index: None,
            })
            .expect("source item move should commit before window creation");
    });

    let outcome = cx.update(|app| {
        let key = item_tear_off_key(&primary_space, source_tabs, item("a"));
        runtime.complete_tear_off_viewport_at(
            &key,
            WindowHandle::<DockHost>::new(WindowId::from(931)),
            DockViewportTearOffTick::new(2),
            app,
        )
    });

    let DockViewportTearOffCompletionOutcome::Cancelled(cancelled) = outcome else {
        panic!("completion should cancel when the source item moved");
    };
    assert_eq!(
        cancelled.reason(),
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
        None,
        DockViewportTearOffTick::new(1),
    );
    let expired = runtime.expire_tear_off_requests_at(DockViewportTearOffTick::new(602));

    assert_eq!(expired.len(), 1);
    assert_eq!(
        expired[0].reason(),
        DockViewportTearOffCancelReason::Expired
    );
    assert_eq!(runtime.pending_tear_off_len(), 0);
    assert!(runtime.adapter().spaces().is_empty());
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
        None,
        DockViewportTearOffTick::new(1),
    );
    let outcome = cx.update(|app| {
        let key = item_tear_off_key(&primary_space, source_tabs, item("a"));
        runtime.complete_tear_off_viewport_at(&key, window, DockViewportTearOffTick::new(2), app)
    });

    let DockViewportTearOffCompletionOutcome::CommitFailed(failure) = outcome else {
        panic!("non-empty destination space should fail the tear-off move transaction");
    };
    assert_eq!(
        failure.error().clone(),
        DockActionApplyError::Graph(DockGraphMutationError::TargetSpaceNotEmpty {
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
fn viewport_runtime_tear_off_commit_failure_closes_opened_window(cx: &mut TestAppContext) {
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
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let before_windows = cx.windows().len();
    let outcome = cx
        .update(|app| {
            runtime.open_tear_off_viewport(
                tear_off_request(primary_space.clone(), source_tabs, item("a")),
                detached_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("opening the temporary tear-off window should reach graph commit");

    let DockViewportTearOffOpenOutcome::CommitFailed(failure) = outcome else {
        panic!("non-empty destination space should fail the tear-off move transaction");
    };
    assert_eq!(
        failure.error().clone(),
        DockActionApplyError::Graph(DockGraphMutationError::TargetSpaceNotEmpty {
            space: detached_space.clone()
        })
    );
    assert_eq!(
        runtime.borrow().adapter().window_for_space(&detached_space),
        None
    );
    cx.run_until_parked();
    cx.update(|app| app.refresh_windows());
    assert_eq!(
        cx.windows().len(),
        before_windows,
        "failed tear-off should not leave an orphan GPUI window"
    );
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
fn viewport_runtime_tear_off_rejects_already_open_target_space_without_reuse(
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
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let existing = cx
        .update(|app| {
            runtime.open_viewport(
                detached_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("existing viewport should open");

    let result = cx.update(|app| {
        runtime.open_tear_off_viewport(
            tear_off_request(primary_space.clone(), source_tabs, item("a")),
            detached_space.clone(),
            viewport_window_options(360.0, 220.0),
            app,
        )
    });
    assert!(
        result
            .expect_err("tear-off must not reuse an already open target space")
            .to_string()
            .contains("already open")
    );
    assert_eq!(runtime.borrow().pending_tear_off_len(), 0);
    assert_eq!(
        runtime.borrow().adapter().window_for_space(&detached_space),
        Some(existing.window())
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&primary_space),
            vec![item("a")]
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
    let runtime = DockViewportRuntimeHandle::new(controller);

    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("secondary viewport should open through runtime");
    let mut visual = VisualTestContext::from_window(opened.window(), cx);

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
            .handle_window_should_close(opened.window().window_id())
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
    let runtime = DockViewportRuntimeHandle::new(controller);

    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("secondary viewport should open through runtime");
    let mut visual = VisualTestContext::from_window(opened.window(), cx);

    runtime.set_close_policy(DockViewportClosePolicy::Prevent);
    assert!(
        !visual.simulate_close(),
        "Prevent should veto a close while the window still belongs to a runtime mapping"
    );

    let cleanup =
        cx.update(|app| runtime.handle_window_closed_with_app(opened.window().window_id(), app));
    assert_eq!(cleanup.status(), DockViewportCloseStatus::Closed);
    assert_eq!(
        runtime
            .borrow()
            .adapter()
            .window_for_space(&secondary_space),
        None
    );
    assert_eq!(
        runtime
            .handle_window_should_close(opened.window().window_id())
            .status,
        DockViewportShouldCloseStatus::UnknownWindow
    );
    assert!(
        visual.simulate_close(),
        "Prevent should not veto once docking no longer owns the window mapping"
    );
}

#[open_gpui::test]
fn viewport_runtime_merge_back_close_reports_status_and_moves_tabs(cx: &mut TestAppContext) {
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
    let window = handle(44);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(detached_space.clone(), window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller.clone(),
        adapter,
        DockViewportClosePolicy::MergeBack {
            target_space: main_space.clone(),
        },
    );

    let outcome = cx.update(|app| runtime.handle_window_closed_with_app(window.window_id(), app));

    assert_eq!(outcome.status(), DockViewportCloseStatus::MergedBack);
    assert_eq!(runtime.adapter().window_for_space(&detached_space), None);
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
    });
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
    let runtime_core = DockViewportRuntime::from_adapter(
        controller,
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );
    let runtime = runtime_core.into_handle();

    let reused = cx
        .update(|app| {
            runtime.open_viewport(secondary_space, viewport_window_options(480.0, 260.0), app)
        })
        .expect("registered live viewport should be reused through runtime");

    assert_eq!(reused.status(), DockViewportOpenStatus::Reused);
    assert_eq!(reused.window(), window);
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

    assert_eq!(outcome.status(), DockViewportCloseStatus::Closed);
    assert_eq!(outcome.space(), Some(&secondary_space));
    assert_eq!(runtime.adapter().window_for_space(&secondary_space), None);
}

#[open_gpui::test]
fn viewport_runtime_rejects_stale_known_viewport_commit_after_target_rebind(
    cx: &mut TestAppContext,
) {
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

    let old_window = handle(10);
    let new_window = handle(11);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(target_space.clone(), old_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller.clone(),
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let host_position = point(px(120.0), px(100.0));
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        old_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(window_bounds),
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        old_window.window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));

    let request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_hovered_window(old_window),
    );
    let stale_commit = DockViewportDropRouteCommit::from_route_request(
        &request,
        DockViewportDropRoute::KnownViewport {
            target: DockViewportTargetHit::new(target_space.clone(), old_window, host_position),
        },
    );

    runtime.register_opened_viewport(target_space.clone(), new_window);
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        new_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(window_bounds),
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        new_window.window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));

    let result = cx.update(|app| runtime.commit_payload_drop_route_with_outcome(stale_commit, app));
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
fn viewport_runtime_revalidates_preview_resolved_target_after_scene_changes(
    cx: &mut TestAppContext,
) {
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

    let target_window = handle(21);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(target_space.clone(), target_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller.clone(),
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let host_position = point(px(120.0), px(100.0));
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(window_bounds),
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_window.window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));

    let request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_hovered_window(target_window),
    );
    let resolution = cx.update(|app| runtime.resolve_payload_drop_route_with_commit(&request, app));
    assert!(
        matches!(resolution.route(), DockViewportDropRoute::KnownViewport { target }
            if target.window_id() == target_window.window_id()),
        "preview route should target the registered viewport"
    );

    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(window_bounds),
        host_bounds,
        host_position,
    ));
    let target_after_scene_change =
        cx.update(|app| runtime.resolve_host_scene_target(&target_space, host_position, app));
    assert!(
        target_after_scene_change.is_none(),
        "new frame intentionally has no facts; re-resolving would fail"
    );

    let result = cx.update(|app| {
        runtime.commit_payload_drop_route_with_outcome(resolution.commit().clone(), app)
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
fn viewport_runtime_revalidates_cached_target_against_current_policy(cx: &mut TestAppContext) {
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

    let target_window = handle(23);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(target_space.clone(), target_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller.clone(),
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let host_position = point(px(120.0), px(100.0));
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(window_bounds),
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_window.window_id(),
        DockHostDropSceneFact::Leaf(DockLeafDropTarget {
            root: target_tabs,
            target_tabs,
            bounds: floating_bounds(0.0, 0.0, 360.0, 220.0),
            is_central: true,
        }),
    ));

    let request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_hovered_window(target_window),
    );
    let resolution = cx.update(|app| runtime.resolve_payload_drop_route_with_commit(&request, app));
    assert!(
        resolution.commit().routed_preview_target().is_some(),
        "preview should cache the accepted central target"
    );

    controller.update(cx, |controller, _| {
        controller
            .policy_mut()
            .set_allow_central_region_dock_over(false);
    });

    let result = cx.update(|app| {
        runtime.commit_payload_drop_route_with_outcome(resolution.commit().clone(), app)
    });
    assert_eq!(
        result,
        Err(DockActionApplyError::Policy(
            DockPolicyError::CentralRegionDockOverDisabled
        ))
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
fn viewport_runtime_preview_respects_payload_dock_class_policy(cx: &mut TestAppContext) {
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
    workspace.register_panel(
        item("a"),
        DockPanel::new("Panel A", test_view(cx, "A")).with_dock_class("editor"),
    );
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace
        .policy_mut()
        .allow_dock_class_in_space(target_space.clone(), "inspector");
    let controller = cx.new(|_| DockController::new(workspace));

    let target_window = handle(22);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(target_space.clone(), target_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller.clone(),
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let host_position = point(px(120.0), px(100.0));
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(window_bounds),
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_window.window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));

    let request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_hovered_window(target_window),
    );
    let resolution = cx.update(|app| runtime.resolve_payload_drop_route_with_commit(&request, app));
    assert!(
        matches!(
            resolution.route(),
            DockViewportDropRoute::Rejected(DockPolicyError::DockClassRejected { .. })
        ),
        "policy-rejected cross-viewport targets should render as rejected routes"
    );
    assert!(
        resolution.commit().routed_preview_target().is_none(),
        "policy-rejected cross-viewport targets must not render as accepted previews"
    );

    let result = cx.update(|app| {
        runtime.commit_payload_drop_route_with_outcome(resolution.commit().clone(), app)
    });
    assert_eq!(
        result,
        Err(DockActionApplyError::Policy(
            DockPolicyError::DockClassRejected {
                space: target_space.clone(),
                item: item("a"),
                dock_class: Some(DockClassId::from("editor")),
            }
        ))
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
fn viewport_runtime_source_only_release_uses_last_hovered_viewport(cx: &mut TestAppContext) {
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

    let source_window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let target_window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let source_opened = cx
        .update(|app| {
            runtime.open_viewport(
                source_space.clone(),
                WindowOptions {
                    window_bounds: Some(source_window_bounds),
                    ..Default::default()
                },
                app,
            )
        })
        .expect("source viewport should open");
    let target_opened = cx
        .update(|app| {
            runtime.open_viewport(
                target_space.clone(),
                WindowOptions {
                    window_bounds: Some(target_window_bounds),
                    ..Default::default()
                },
                app,
            )
        })
        .expect("target viewport should open");
    source_opened
        .window()
        .update(cx, |_, window, _| window.activate_window())
        .expect("source viewport should be activatable");
    cx.run_until_parked();

    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(target_window_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        point(px(120.0), px(100.0)),
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_opened.window().window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));
    let preview_request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_hovered_window(target_opened.window()),
    );
    let resolution =
        cx.update(|app| runtime.resolve_payload_drop_route_with_commit(&preview_request, app));
    cx.update(|app| {
        runtime.update_routed_drop_preview(&resolution, "Panel A", app);
    });
    assert_eq!(
        runtime.last_hovered_window(),
        Some(target_opened.window().window_id())
    );

    let release_position = point(px(220.0), px(200.0));
    let request_without_hovered = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        release_position,
        None,
        DockViewportTargetContext::new()
            .with_active_window(source_opened.window())
            .with_window_stack([source_opened.window(), target_opened.window()]),
    );
    let baseline_route =
        cx.update(|app| runtime.resolve_payload_drop_route(&request_without_hovered, app));
    match baseline_route {
        DockViewportDropRoute::Local { .. } => {}
        DockViewportDropRoute::KnownViewport { target }
            if target.window_id() == source_opened.window().window_id() => {}
        other => panic!("unexpected baseline route: {:?}", other),
    }

    let request_with_hovered = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        release_position,
        None,
        DockViewportTargetContext::new()
            .with_hovered_window(target_opened.window())
            .with_active_window(source_opened.window())
            .with_window_stack([source_opened.window(), target_opened.window()]),
    );
    let hovered_route =
        cx.update(|app| runtime.resolve_payload_drop_route(&request_with_hovered, app));
    assert!(
        matches!(
            hovered_route,
            DockViewportDropRoute::KnownViewport { target }
                if target.window_id() == target_opened.window().window_id()
        ),
        "last hovered viewport should override the active source window for source-only releases"
    );

    cx.update(|app| {
        assert!(runtime.clear_routed_drop_preview(app));
    });
    assert_eq!(
        runtime.last_hovered_window(),
        Some(target_opened.window().window_id())
    );
}

#[open_gpui::test]
fn viewport_runtime_scopes_last_hovered_window_to_drag_session(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    graph.set_root(source_space.clone(), source_tabs);
    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let mut runtime = DockViewportRuntime::new(controller);

    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "A".to_string(),
    );
    let session = runtime.begin_payload_drag(&payload);
    let request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new(),
    )
    .with_drag_session(Some(session.clone()));
    let target_window = handle(77);
    let target_position = point(px(120.0), px(100.0));
    let route = DockViewportDropRoute::KnownViewport {
        target: DockViewportTargetHit::new(target_space, target_window, target_position),
    };
    let resolution = DockViewportResolvedDropRoute::new(
        route.clone(),
        DockViewportDropRouteCommit::from_route_request(&request, route),
    );

    runtime.update_routed_drop_preview(&resolution, "Panel A");
    assert_eq!(
        runtime.last_hovered_window_for_drag_session(Some(&session)),
        Some(target_window.window_id())
    );
    assert_eq!(runtime.last_hovered_window_for_drag_session(None), None);

    let local_resolution = DockViewportResolvedDropRoute::new(
        DockViewportDropRoute::Local {
            host_position: target_position,
        },
        DockViewportDropRouteCommit::from_route_request(
            &request,
            DockViewportDropRoute::Local {
                host_position: target_position,
            },
        ),
    );
    runtime.update_routed_drop_preview(&local_resolution, "Panel A");
    assert_eq!(runtime.last_hovered_window(), None);

    runtime.update_routed_drop_preview(&resolution, "Panel A");
    assert!(runtime.finish_payload_drag(&session));
    assert_eq!(runtime.last_hovered_window(), None);

    runtime.update_routed_drop_preview(&resolution, "Panel A");
    let next_session = runtime.begin_payload_drag(&payload);
    assert_ne!(next_session.id(), session.id());
    assert_eq!(runtime.last_hovered_window(), None);
}

#[open_gpui::test]
fn viewport_runtime_rejects_known_viewport_commit_from_stale_drag_session(cx: &mut TestAppContext) {
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

    let target_window = handle(21);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(target_space.clone(), target_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller.clone(),
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let host_position = point(px(120.0), px(100.0));
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(window_bounds),
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_window.window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));

    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    let stale_session = runtime.begin_payload_drag(&payload);
    let request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_hovered_window(target_window),
    )
    .with_drag_session(Some(stale_session.clone()));
    let stale_commit = DockViewportDropRouteCommit::from_route_request(
        &request,
        DockViewportDropRoute::KnownViewport {
            target: DockViewportTargetHit::new(target_space.clone(), target_window, host_position),
        },
    );

    let _replacement = runtime.begin_payload_drag(&payload);
    let result = cx.update(|app| runtime.commit_payload_drop_route_with_outcome(stale_commit, app));
    assert!(matches!(
        result,
        Err(DockActionApplyError::DropDragSessionStale { session })
            if session == stale_session.id()
    ));
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
fn viewport_runtime_rejects_tear_off_commit_from_stale_drag_session(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    graph.set_root(source_space.clone(), source_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let mut runtime = DockViewportRuntime::new(controller.clone());

    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    let stale_session = runtime.begin_payload_drag(&payload);
    let _replacement = runtime.begin_payload_drag(&payload);
    let request = DockViewportTearOffRequest::new(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(900.0), px(900.0)),
        None,
    )
    .with_drag_session(Some(stale_session.clone()));

    let result = cx.update(|app| runtime.prepare_tear_off_drop_route_commit(request, app));
    assert!(matches!(
        result,
        Err(DockActionApplyError::DropDragSessionStale { session })
            if session == stale_session.id()
    ));
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("a")]
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_default_tear_off_bounds_keep_cursor_inside_window(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    graph.set_root(source_space.clone(), source_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntime::new(controller);
    let release_position = point(px(900.0), px(900.0));
    let request = DockViewportTearOffRequest::new(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        release_position,
        None,
    );

    let options = runtime.tear_off_window_options(&request);
    assert_eq!(
        options.window_bounds,
        Some(WindowBounds::Windowed(floating_bounds(
            876.0, 882.0, 360.0, 240.0
        )))
    );
    if let Some(WindowBounds::Windowed(bounds)) = options.window_bounds {
        assert!(bounds.contains(&release_position));
        assert_ne!(bounds.origin, release_position);
    } else {
        panic!("tear-off should use windowed bounds");
    }
}
