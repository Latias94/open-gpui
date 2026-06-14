use crate::{
    DockActionApplyError, DockController, DockGraph, DockItemId, DockNode, DockNodeId, DockPanel,
    DockSpaceId, DockViewportClosePolicy, DockViewportDropOutcomeKind, DockViewportDropPayload,
    DockViewportDropRoute, DockViewportDropRouteOutcome, DockViewportDropRouteRequest,
    DockViewportOpenStatus, DockViewportPlatformSignals, DockViewportRouteStatus,
    DockViewportRuntimeHandle, DockViewportShouldCloseStatus, DockViewportStaleStatusReason,
    DockViewportTargetContext, DockViewportTearOffOpenOutcome, DockViewportTearOffRequest,
    DockViewportWindowFacts, DockWorkspace, DropZone, SplitAxis,
    debug::DockDebugRegion,
    drag::DockDragPayload,
    drop_preview::DockDropRoutePreviewKind,
    drop_runtime::DockHostDropSceneFact,
    drop_target::{DockDropResolveSource, DockLeafDropTarget, DockResolvedDropTargetKind},
    host_test_support::*,
    interaction::DockPayloadDropRelease,
    viewport_registry::{DockViewportRouteUnavailableReason, DockViewportStaleReason},
};
use open_gpui::{
    AppContext as _, Focusable, TestAppContext, VisualTestContext, WindowBounds, WindowOptions,
    point, px, size,
};
use slotmap::Key;

fn tear_off_request(
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

fn leaf_host_scene_fact(root: DockNodeId, target_tabs: DockNodeId) -> DockHostDropSceneFact {
    DockHostDropSceneFact::Leaf(DockLeafDropTarget {
        root,
        target_tabs,
        bounds: floating_bounds(0.0, 0.0, 360.0, 220.0),
        is_central: false,
    })
}

fn target_center_host_position() -> open_gpui::Point<open_gpui::Pixels> {
    center_drop_position(floating_bounds(0.0, 0.0, 360.0, 220.0))
}

fn cache_known_viewport_preview(
    cx: &mut TestAppContext,
    runtime: &DockViewportRuntimeHandle,
    source_space: DockSpaceId,
    source_node: DockNodeId,
    payload: DockViewportDropPayload,
    release_position: open_gpui::Point<open_gpui::Pixels>,
    hovered_window: impl Into<open_gpui::AnyWindowHandle>,
    payload_title: &str,
) -> crate::DockViewportResolvedDropRoute {
    let request = DockViewportDropRouteRequest::from_target_context(
        source_space,
        source_node,
        payload,
        release_position,
        None,
        DockViewportTargetContext::new().with_hovered_window(hovered_window),
    );
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert!(
        matches!(
            resolution.route(),
            DockViewportDropRoute::KnownViewport { .. }
        ),
        "preview setup should resolve a known viewport route, got {:?}",
        resolution.route()
    );
    cx.update(|app| {
        runtime.update_routed_drop_preview(&resolution, payload_title, app);
    });
    resolution
}

#[open_gpui::test]
fn viewport_runtime_handle_tracks_payload_drag_session(cx: &mut TestAppContext) {
    let source = DockSpaceId::from("source");
    let source_tabs = DockNodeId::null();
    let workspace = DockWorkspace::new(source.clone(), DockGraph::new());
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let payload = DockDragPayload::new_tabs(source, source_tabs, "Stack".to_string());

    let session = runtime.begin_payload_drag(&payload);
    assert_eq!(session.id(), 1);
    assert_eq!(
        runtime.active_payload_drag_session(&payload),
        Some(session.clone())
    );
    assert_eq!(
        runtime.active_payload_drag_session(&DockDragPayload::new_item(
            DockSpaceId::from("source"),
            source_tabs,
            item("other"),
            "Other".to_string(),
        )),
        None
    );

    assert!(runtime.finish_payload_drag(&session));
    assert_eq!(runtime.active_payload_drag_session(&payload), None);
    assert!(!runtime.finish_payload_drag(&session));
}

#[open_gpui::test]
fn viewport_runtime_handle_auto_observes_window_closed_cleanup(cx: &mut TestAppContext) {
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
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

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
        .window()
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
        selected: Some(item("b")),
    });
    graph.set_root(target_space.clone(), target_tabs);

    let mut workspace = DockWorkspace::new(target_space.clone(), graph);
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
    let window_bounds = opened
        .window()
        .update(cx, |_, window, _| window.window_bounds())
        .expect("target window should be live");
    let window_bounds = WindowBounds::Windowed(window_bounds.get_bounds());
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);

    let first = runtime
        .begin_viewport_host_scene_frame(
            target_space.clone(),
            opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(window_bounds),
            host_bounds,
            target_center_host_position(),
        )
        .expect("first scene frame should register")
        .frame;
    assert!(
        runtime
            .push_viewport_host_scene_frame_fact(
                &first,
                leaf_host_scene_fact(target_tabs, target_tabs),
            )
            .is_some()
    );

    let second = runtime
        .begin_viewport_host_scene_frame(
            target_space.clone(),
            opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(window_bounds),
            host_bounds,
            target_center_host_position(),
        )
        .expect("second scene frame should register")
        .frame;
    assert!(
        runtime
            .push_viewport_host_scene_frame_fact(
                &first,
                leaf_host_scene_fact(target_tabs, target_tabs),
            )
            .is_none(),
        "facts captured by an older render frame must not populate a newer scene"
    );
    assert!(
        runtime
            .push_viewport_host_scene_frame_fact(
                &second,
                leaf_host_scene_fact(target_tabs, target_tabs),
            )
            .is_some()
    );
}

#[open_gpui::test]
fn viewport_runtime_handle_retain_close_clears_scene_and_reopens_layout(cx: &mut TestAppContext) {
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
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

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
        opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
            10.0, 20.0, 360.0, 220.0,
        ))),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        target_center_host_position(),
    ));
    assert!(
        runtime
            .last_host_scene_screen_position(&secondary_space)
            .is_some()
    );

    assert!(
        runtime
            .handle_window_should_close(opened.window().window_id())
            .allows_close(),
        "RetainLayout should allow GPUI to close the platform viewport"
    );
    opened
        .window()
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
        .window()
        .downcast::<crate::DockHost>()
        .expect("reopened viewport should render DockHost");
    let reopened_host = reopened_window
        .root(cx)
        .expect("reopened viewport should expose DockHost root");
    cx.run_until_parked();
    let reopened_visual = VisualTestContext::from_window(reopened.window(), cx);

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
fn viewport_runtime_handle_open_does_not_reuse_close_pending_window(cx: &mut TestAppContext) {
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

    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("secondary viewport should open through runtime handle");

    assert!(
        runtime
            .handle_window_should_close(opened.window().window_id())
            .allows_close(),
        "RetainLayout should allow the platform close"
    );
    let reopened = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("close-pending retained viewport should be replaced, not reused");

    assert_eq!(reopened.status(), DockViewportOpenStatus::Replaced);
    assert_ne!(reopened.window(), opened.window());
    assert_eq!(
        runtime
            .borrow()
            .adapter()
            .window_for_space(&secondary_space),
        Some(reopened.window())
    );
}

#[open_gpui::test]
fn viewport_runtime_handle_merge_back_close_moves_content_to_fallback(cx: &mut TestAppContext) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel(
        item("a"),
        DockPanel::new("Panel A", test_view(cx, "A")).closable(false),
    );
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::with_close_policy(
        controller.clone(),
        DockViewportClosePolicy::MergeBack {
            target_space: main_space.clone(),
        },
    );

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
        cx.update(|app| runtime
            .handle_window_should_close_with_app(opened.window().window_id(), app)
            .allows_close()),
        "merge-back policy should allow GPUI to close before graph merge"
    );
    opened
        .window()
        .update(cx, |_, window, _| window.remove_window())
        .expect("detached viewport should still be live");
    cx.run_until_parked();

    assert_eq!(
        runtime.borrow().adapter().window_for_space(&detached_space),
        None
    );
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(main_tabs)
            .expect("fallback tabs should remain")
        else {
            panic!("fallback root should be tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a"), item("c")]);
        assert_eq!(selected.as_ref(), items.get(2));
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
fn viewport_runtime_handle_merge_back_close_focuses_fallback_selected_item(
    cx: &mut TestAppContext,
) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);

    let panel_c = test_view(cx, "C");
    let panel_c_focus = cx.read_entity(&panel_c, |panel, cx| panel.focus_handle(cx));
    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_focusable_panel_view(item("c"), "Panel C", panel_c);
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::with_close_policy(
        controller,
        DockViewportClosePolicy::MergeBack {
            target_space: main_space.clone(),
        },
    );

    let main_opened = cx
        .update(|app| {
            runtime.open_viewport(
                main_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("main viewport should open");
    let detached_opened = cx
        .update(|app| {
            runtime.open_viewport(
                detached_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("detached viewport should open");

    detached_opened
        .window()
        .update(cx, |_, window, _| window.remove_window())
        .expect("detached viewport should still be live");
    cx.run_until_parked();

    let active_window = main_opened
        .window()
        .update(cx, |_, _, app| {
            DockViewportPlatformSignals::from_app(app).active_window()
        })
        .expect("main viewport should remain live");
    assert_eq!(active_window, Some(main_opened.window().window_id()));
    main_opened
        .window()
        .update(cx, |_, window, cx| {
            assert_eq!(
                window.focused(cx),
                Some(panel_c_focus),
                "merge-back close should focus the selected item in the fallback viewport"
            );
        })
        .expect("main viewport should remain live");
}

#[open_gpui::test]
fn viewport_runtime_handle_opens_tear_off_viewport_and_moves_item(cx: &mut TestAppContext) {
    let primary_space = DockSpaceId::from("primary");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
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
fn viewport_runtime_handle_tears_off_split_floating_from_floating_root(cx: &mut TestAppContext) {
    let primary_space = DockSpaceId::from("primary");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(primary_space.clone(), root);
    let floating_left = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let floating_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let floating_split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![floating_left, floating_right],
        fractions: vec![0.5, 0.5],
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_split,
    });
    graph
        .floating_containers_mut(primary_space.clone())
        .push(crate::DockFloatingContainer {
            node: floating,
            bounds: floating_bounds(10.0, 20.0, 260.0, 150.0),
        });

    let mut workspace = DockWorkspace::new(primary_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let outcome = cx
        .update(|app| {
            runtime.open_tear_off_viewport(
                DockViewportTearOffRequest::new(
                    primary_space.clone(),
                    floating,
                    DockViewportDropPayload::Floating(floating),
                    point(px(900.0), px(900.0)),
                    None,
                ),
                detached_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("split floating tear-off should open through runtime handle");

    let DockViewportTearOffOpenOutcome::Completed(completed) = outcome else {
        panic!("split floating tear-off should complete");
    };
    assert_eq!(completed.pending().request().source_node(), floating);
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&primary_space),
            vec![item("b")]
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            vec![item("a"), item("c")]
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_handle_rejects_floating_tear_off_from_child_tabs_source_node(
    cx: &mut TestAppContext,
) {
    let primary_space = DockSpaceId::from("primary");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let floating_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_tabs,
    });
    graph
        .floating_containers_mut(primary_space.clone())
        .push(crate::DockFloatingContainer {
            node: floating,
            bounds: floating_bounds(10.0, 20.0, 260.0, 150.0),
        });

    let mut workspace = DockWorkspace::new(primary_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let before_windows = cx.windows().len();
    let outcome = cx
        .update(|app| {
            runtime.open_tear_off_viewport(
                DockViewportTearOffRequest::new(
                    primary_space.clone(),
                    floating_tabs,
                    DockViewportDropPayload::Floating(floating),
                    point(px(900.0), px(900.0)),
                    None,
                ),
                detached_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("invalid floating source node should be cancelled after creating an unregistered window");

    let DockViewportTearOffOpenOutcome::Cancelled(cancelled) = outcome else {
        panic!("invalid floating source node should cancel, got {outcome:?}");
    };
    assert_eq!(
        cancelled.reason(),
        crate::DockViewportTearOffCancelReason::SourceMissing
    );
    assert_eq!(
        runtime.borrow().adapter().window_for_space(&detached_space),
        None,
        "cancelled tear-off windows must never be registered as routeable dock viewports"
    );
    cx.run_until_parked();
    cx.update(|app| app.refresh_windows());
    assert_eq!(
        cx.windows().len(),
        before_windows,
        "cancelled tear-off should close the unregistered platform window"
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&primary_space),
            vec![item("a")]
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_handle_tear_off_is_not_route_ready_before_first_host_scene(
    cx: &mut TestAppContext,
) {
    let primary_space = DockSpaceId::from("primary");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    graph.set_root(primary_space.clone(), source_tabs);

    let mut workspace = DockWorkspace::new(primary_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);

    let detached_window: open_gpui::AnyWindowHandle = cx
        .open_window(size(px(360.0), px(220.0)), |_, cx| {
            TestPanel::new("detached", cx)
        })
        .into();
    runtime
        .borrow_mut()
        .register_opened_viewport(detached_space.clone(), detached_window);
    let detached_bounds = WindowBounds::Windowed(floating_bounds(0.0, 0.0, 360.0, 220.0));
    let target_point =
        screen_position_for_host_position(detached_bounds, target_center_host_position());

    assert!(
        !runtime.viewport_route_ready(&detached_space),
        "registered viewports must wait for a rendered host scene before route hits"
    );
    assert_eq!(
        runtime.viewport_route_unavailable_reason(&detached_space),
        Some(DockViewportRouteUnavailableReason::RegisteredNotReady)
    );
    assert_eq!(
        runtime
            .runtime_status()
            .viewport_lifecycle
            .iter()
            .find(|record| record.space == detached_space)
            .map(|record| record.route_status),
        Some(DockViewportRouteStatus::RegisteredNotReady)
    );
    let route_before_scene = cx.update(|app| {
        let request = DockViewportDropRouteRequest::from_platform_signals(
            primary_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("b")),
            target_point,
            None,
            DockViewportPlatformSignals::from_app(app).with_hovered_window(detached_window),
        );
        runtime
            .resolve_payload_drop_delivery(&request, app)
            .route()
            .clone()
    });
    assert_eq!(
        route_before_scene,
        DockViewportDropRoute::Unavailable,
        "registered-but-not-rendered viewports must not be route targets"
    );

    assert!(runtime.begin_viewport_host_scene(
        detached_space.clone(),
        detached_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(detached_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        target_center_host_position()
    ));
    assert!(runtime.viewport_route_ready(&detached_space));
    assert_eq!(
        runtime.viewport_route_unavailable_reason(&detached_space),
        None
    );
    assert_eq!(
        runtime
            .runtime_status()
            .viewport_lifecycle
            .iter()
            .find(|record| record.space == detached_space)
            .map(|record| record.route_status),
        Some(DockViewportRouteStatus::RouteReady)
    );

    cx.update(|app| {
        runtime.mark_viewport_window_snapshot_stale(detached_window.window_id(), app);
    });
    assert!(!runtime.viewport_route_ready(&detached_space));
    assert_eq!(
        runtime.viewport_route_unavailable_reason(&detached_space),
        Some(DockViewportRouteUnavailableReason::Stale(
            DockViewportStaleReason::WindowFactsChanged
        ))
    );
    assert_eq!(
        runtime
            .runtime_status()
            .viewport_lifecycle
            .iter()
            .find(|record| record.space == detached_space)
            .map(|record| record.route_status),
        Some(DockViewportRouteStatus::Stale {
            reason: DockViewportStaleStatusReason::WindowFactsChanged
        })
    );
    assert!(runtime.begin_viewport_host_scene(
        detached_space.clone(),
        detached_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(detached_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        target_center_host_position()
    ));
    assert!(runtime.viewport_route_ready(&detached_space));
    assert_eq!(
        runtime
            .runtime_status()
            .viewport_lifecycle
            .iter()
            .find(|record| record.space == detached_space)
            .map(|record| record.route_status),
        Some(DockViewportRouteStatus::RouteReady)
    );

    let route_after_scene_without_target = cx.update(|app| {
        let request = DockViewportDropRouteRequest::from_platform_signals(
            primary_space,
            source_tabs,
            DockViewportDropPayload::Item(item("b")),
            target_point,
            None,
            DockViewportPlatformSignals::from_app(app).with_hovered_window(detached_window),
        );
        runtime
            .resolve_payload_drop_delivery(&request, app)
            .route()
            .clone()
    });
    assert_eq!(
        route_after_scene_without_target,
        DockViewportDropRoute::Unavailable,
        "route-ready only makes the viewport hittable; it still needs a current drop target"
    );
}

#[open_gpui::test]
fn viewport_runtime_handle_resolves_drop_route_with_current_policy(cx: &mut TestAppContext) {
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
        .window()
        .update(cx, |_, window, _| window.window_bounds())
        .expect("target window should be live");
    let target_window_bounds = WindowBounds::Windowed(target_window_bounds.get_bounds());
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(target_window_bounds),
        host_bounds,
        point(px(0.0), px(0.0))
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        opened.window().window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));
    let host_position = target_center_host_position();
    let target_point = screen_position_for_host_position(target_window_bounds, host_position);

    let route = cx.update(|app| {
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_point,
            Some(target_window_bounds),
            DockViewportPlatformSignals::from_app(app).with_hovered_window(opened.window()),
        );
        runtime
            .resolve_payload_drop_delivery(&request, app)
            .route()
            .clone()
    });

    let expected_generation = runtime
        .borrow()
        .adapter()
        .snapshot_facts_generation(&target_space, opened.window().window_id())
        .expect("target viewport snapshot should expose the current facts generation");
    assert_eq!(
        route,
        DockViewportDropRoute::KnownViewport {
            target: crate::DockViewportTargetHit::with_facts_generation(
                target_space.clone(),
                opened.window(),
                host_position,
                expected_generation,
            )
        }
    );
    let status = runtime.runtime_status();
    let target = &status
        .last_route
        .as_ref()
        .expect("runtime status should expose the last resolved route")
        .target;
    assert_eq!(target.space(), Some(&target_space));
    assert_eq!(target.window_id(), Some(opened.window().window_id()));
    assert_eq!(target.host_position(), Some(host_position));
}

#[open_gpui::test]
fn viewport_runtime_handle_drop_route_uses_workspace_platform_policy(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(source_space.clone(), source_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let release_position = point(px(900.0), px(900.0));

    let rejected_request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        release_position,
        None,
        DockViewportTargetContext::new(),
    );
    let rejected = cx.update(|app| {
        runtime
            .resolve_payload_drop_delivery(&rejected_request, app)
            .route()
            .clone()
    });
    assert!(
        matches!(
            rejected,
            DockViewportDropRoute::Rejected(crate::DockPolicyError::PlatformViewportsDisabled)
        ),
        "default workspace policy should reject outside-all-viewports route"
    );
    let status = runtime.runtime_status();
    let target = &status
        .last_route
        .as_ref()
        .expect("runtime status should record the rejected route")
        .target;
    assert_eq!(
        target.rejection_reason(),
        Some(crate::DockPolicyError::PlatformViewportsDisabled)
    );

    cx.update_entity(&controller, |controller, _| {
        controller.policy_mut().set_allow_platform_viewports(true);
    });
    let tear_off_request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        release_position,
        None,
        DockViewportTargetContext::new(),
    );
    let tear_off = cx.update(|app| {
        runtime
            .resolve_payload_drop_delivery(&tear_off_request, app)
            .route()
            .clone()
    });
    assert!(
        matches!(tear_off, DockViewportDropRoute::TearOff),
        "allowed workspace policy should resolve an outside release as tear-off"
    );
    let status = runtime.runtime_status();
    let target = &status
        .last_route
        .as_ref()
        .expect("runtime status should record the tear-off route")
        .target;
    assert_eq!(target.release_position(), Some(release_position));
}

#[open_gpui::test]
fn viewport_runtime_handle_commits_tear_off_drop_route(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
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
            let request = DockViewportDropRouteRequest::from_target_context(
                source_space.clone(),
                source_tabs,
                DockViewportDropPayload::Item(item("a")),
                release_position,
                Some(suggested_window_bounds),
                DockViewportTargetContext::new(),
            );
            let resolution = runtime.resolve_payload_drop_delivery(&request, app);
            runtime.deliver_payload_drop_with_outcome(resolution.expect_delivery().clone(), app)
        })
        .expect("tear-off route should commit through runtime handle");

    let activation = outcome.activation_target();
    let DockViewportDropRouteOutcome::TearOff(tear_off) = outcome else {
        panic!("tear-off route should open a viewport and complete the move");
    };
    let DockViewportTearOffOpenOutcome::Completed(completed) = *tear_off else {
        panic!("tear-off route should open a viewport and complete the move");
    };
    assert_eq!(completed.action(), crate::DockActionOutcome::Changed);
    assert_eq!(
        activation.as_ref().map(|target| target.window()),
        Some(completed.registration().window()),
        "tear-off completion should surface the new viewport activation target"
    );
    assert_eq!(
        completed.pending().request().release_position(),
        release_position
    );
    assert_eq!(
        completed.pending().request().suggested_window_bounds(),
        Some(suggested_window_bounds)
    );
    assert_eq!(
        completed.pending().target_space().as_str(),
        "source:tear-off:a:0"
    );
    assert_eq!(
        runtime
            .borrow()
            .adapter()
            .window_for_space(completed.pending().target_space()),
        Some(completed.registration().window())
    );
    let opened_window = completed
        .registration()
        .window()
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
                .collect_items_in_space(completed.pending().target_space()),
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
        selected: Some(item("c")),
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
            let request = DockViewportDropRouteRequest::from_target_context(
                source_space.clone(),
                source_tabs,
                DockViewportDropPayload::Tabs,
                release_position,
                Some(suggested_window_bounds),
                DockViewportTargetContext::new(),
            );
            let resolution = runtime.resolve_payload_drop_delivery(&request, app);
            runtime.deliver_payload_drop_with_outcome(resolution.expect_delivery().clone(), app)
        })
        .expect("stack tear-off route should commit through runtime handle");

    let activation = outcome.activation_target();
    let DockViewportDropRouteOutcome::TearOff(tear_off) = outcome else {
        panic!("stack tear-off route should open a viewport and complete the move");
    };
    let DockViewportTearOffOpenOutcome::Completed(completed) = *tear_off else {
        panic!("stack tear-off route should open a viewport and complete the move");
    };
    assert_eq!(completed.action(), crate::DockActionOutcome::Changed);
    assert_eq!(
        activation.as_ref().map(|target| target.window()),
        Some(completed.registration().window()),
        "stack tear-off completion should surface the new viewport activation target"
    );
    assert_eq!(
        completed.pending().target_space().as_str(),
        "source:tear-off:tabs:0"
    );
    let opened_window = completed
        .registration()
        .window()
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
            .root(completed.pending().target_space())
            .expect("detached stack should become the target root");
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(detached_root)
            .expect("detached root should exist")
        else {
            panic!("detached root should be tabs");
        };
        assert_eq!(items, &vec![item("a"), item("c")]);
        assert_eq!(selected.as_ref(), items.get(1));
    });
}

#[open_gpui::test]
fn viewport_runtime_handle_rejects_known_viewport_drop_without_host_scene(cx: &mut TestAppContext) {
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
        .window()
        .update(cx, |_, window, _| window.window_bounds())
        .expect("target window should be live");
    let target_window_bounds = WindowBounds::Windowed(target_window_bounds.get_bounds());
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(target_window_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        point(px(0.0), px(0.0))
    ));

    let target_point = point(
        target_window_bounds.get_bounds().origin.x + px(120.0),
        target_window_bounds.get_bounds().origin.y + px(100.0),
    );
    let result = cx.update(|app| {
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_point,
            None,
            DockViewportPlatformSignals::from_app(app).with_hovered_window(opened.window()),
        );
        let resolution = runtime.resolve_payload_drop_delivery(&request, app);
        assert_eq!(
            resolution.route(),
            &DockViewportDropRoute::Unavailable,
            "a registered viewport without host scene facts should not preview as droppable"
        );
        assert!(
            resolution.delivery().is_none(),
            "unavailable viewport routes must not carry a delivery"
        );
        resolution
            .delivery_result()
            .cloned()
            .and_then(|delivery| runtime.deliver_payload_drop_with_outcome(delivery, app))
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
        .window()
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
        .window()
        .update(cx, |_, window, _| window.activate_window())
        .expect("source viewport should be activatable before drop");
    let before_drop_context = opened
        .window()
        .update(cx, |_, _, app| DockViewportPlatformSignals::from_app(app))
        .expect("target window should be live");
    assert_eq!(
        before_drop_context.active_window(),
        Some(source_opened.window().window_id()),
        "source viewport should be active before the routed drop commits"
    );
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(target_window_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        target_center_host_position(),
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        opened.window().window_id(),
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
            DockViewportPlatformSignals::from_app(app).with_hovered_window(opened.window()),
            app,
        );
        let status = runtime.runtime_status();
        let target = &status
            .last_route
            .as_ref()
            .expect("screen release should record the destination viewport route")
            .target;
        assert_eq!(target.window_id(), Some(opened.window().window_id()));
        result
    });

    let DockViewportDropRouteOutcome::Action(action) = result.expect("route should commit") else {
        panic!("known viewport drop should produce a normal action outcome");
    };
    assert_eq!(action.action(), crate::DockActionOutcome::Changed);
    assert_eq!(
        action.activation().map(|activation| activation.window()),
        Some(opened.window()),
        "known viewport drop should request activation of the destination window"
    );
    assert_eq!(
        action
            .activation()
            .and_then(|activation| activation.focus_item().cloned()),
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
        Some(opened.window().window_id()),
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
        .window()
        .update(cx, |_, _, app| DockViewportPlatformSignals::from_app(app))
        .expect("source window should be live");
    assert_eq!(
        after_drop_context.active_window(),
        Some(opened.window().window_id()),
        "successful routed drop should activate the destination viewport"
    );
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should still exist")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a")]);
        assert_eq!(selected.as_ref(), items.get(1));
    });
}

#[open_gpui::test]
fn host_render_drop_consumes_routed_viewport_activation(cx: &mut TestAppContext) {
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
        target_opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(target_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        target_center_host_position(),
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_opened.window().window_id(),
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
        .window()
        .update(cx, |_, window, _| window.activate_window())
        .expect("source viewport should be activatable before host drop");
    let source_window = source_opened
        .window()
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
    cache_known_viewport_preview(
        cx,
        &runtime,
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        release_screen_position,
        target_opened.window(),
        "Panel A",
    );

    cx.set_platform_hovered_window(Some(target_opened.window()));
    let changed = source_window
        .update(cx, |host, window, cx| {
            host.drop_payload_release_from_render(
                DockPayloadDropRelease::source_only(
                    payload.clone(),
                    source_space.clone(),
                    release_in_source_window,
                ),
                window,
                cx,
            )
        })
        .expect("source host should commit the routed render drop");
    assert!(changed, "host render drop should report a graph change");
    cx.run_until_parked();

    let after_drop_context = source_opened
        .window()
        .update(cx, |_, _, app| DockViewportPlatformSignals::from_app(app))
        .expect("source window should be live");
    assert_eq!(
        after_drop_context.active_window(),
        Some(target_opened.window().window_id()),
        "host interaction should consume the routed activation target"
    );
    target_opened
        .window()
        .update(cx, |_, window, cx| {
            assert_eq!(
                window.focused(cx),
                Some(panel_a_focus),
                "target viewport should focus the moved panel after rendered drop"
            );
        })
        .expect("target window should still be live");
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should still exist")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a")]);
        assert_eq!(selected.as_ref(), items.get(1));
    });
}

#[open_gpui::test]
fn hovered_host_release_does_not_consume_cached_delivery_for_another_window(
    cx: &mut TestAppContext,
) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
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
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
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
        target_opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(target_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        target_center_host_position(),
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_opened.window().window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
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
        .window()
        .downcast::<crate::DockHost>()
        .expect("source viewport should render DockHost");
    assert!(runtime.begin_viewport_host_scene(
        source_space.clone(),
        source_opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(source_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        target_center_host_position(),
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &source_space,
        source_opened.window().window_id(),
        leaf_host_scene_fact(source_tabs, source_tabs),
    ));

    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    let target_screen_position =
        screen_position_for_host_position(target_bounds, target_center_host_position());
    let resolution = cache_known_viewport_preview(
        cx,
        &runtime,
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        target_screen_position,
        target_opened.window(),
        "Panel A",
    );

    source_window
        .update(cx, |host, window, cx| {
            host.interaction_mut()
                .update_drop_route_preview(&resolution, target_center_host_position());
            host.drop_payload_release_from_render(
                DockPayloadDropRelease::hovered_host(
                    payload.clone(),
                    source_space.clone(),
                    target_center_host_position(),
                ),
                window,
                cx,
            )
        })
        .expect("source host should handle hovered release");
    cx.run_until_parked();

    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(source_tabs)
            .expect("source tabs should still exist")
        else {
            panic!("source should remain tabs");
        };
        assert_eq!(items, &vec![item("a"), item("c")]);
        assert_eq!(selected.as_ref(), items.first());

        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should still exist")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("b")]);
        assert_eq!(selected.as_ref(), items.get(0));
    });
}

#[open_gpui::test]
fn hovered_host_release_rejects_cached_delivery_when_release_point_misses_target(
    cx: &mut TestAppContext,
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
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

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
    let target_window = target_opened
        .window()
        .downcast::<crate::DockHost>()
        .expect("target viewport should render DockHost");

    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(target_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        target_center_host_position(),
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_opened.window().window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));

    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    let session = runtime.begin_payload_drag(&payload);
    let target_screen_position =
        screen_position_for_host_position(target_bounds, target_center_host_position());
    let resolution = cache_known_viewport_preview(
        cx,
        &runtime,
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        target_screen_position,
        target_opened.window(),
        "Panel A",
    );
    target_window
        .update(cx, |host, window, cx| {
            host.interaction_mut()
                .update_drop_route_preview(&resolution, target_center_host_position());
            window.refresh();
            cx.notify();
        })
        .expect("target host should cache the routed delivery");

    let missed_target_position = point(px(720.0), px(420.0));
    target_window
        .update(cx, |host, window, cx| {
            host.drop_payload_release_from_render(
                DockPayloadDropRelease::hovered_host_with_session(
                    payload.clone(),
                    target_space.clone(),
                    missed_target_position,
                    Some(session.clone()),
                ),
                window,
                cx,
            )
        })
        .expect("target host should handle release outside the cached target");
    cx.run_until_parked();

    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("a")],
            "release outside current target should not commit the stale cached delivery"
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&target_space),
            vec![item("b")]
        );
    });
    let status = runtime.runtime_status();
    assert!(
        !matches!(
            status.last_route.as_ref().map(|record| &record.target),
            Some(crate::DockViewportRouteTarget::KnownViewport { window_id, .. })
                if *window_id == target_opened.window().window_id()
        ),
        "release should be rerouted from the current pointer facts instead of the cached delivery, got {:?}",
        status.last_route
    );
    assert!(
        runtime.active_payload_drag_session(&payload).is_none(),
        "release should still finish the drag session after rejecting stale cached delivery"
    );

    source_opened
        .window()
        .update(cx, |_, _, _| ())
        .expect("source window should remain live");
}

#[open_gpui::test]
fn hovered_host_release_reroutes_when_cached_delivery_scene_is_stale(cx: &mut TestAppContext) {
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
    let target_window = target_opened
        .window()
        .downcast::<crate::DockHost>()
        .expect("target viewport should render DockHost");

    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(target_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        target_center_host_position(),
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_opened.window().window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));

    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    let target_screen_position =
        screen_position_for_host_position(target_bounds, target_center_host_position());
    let resolution = cache_known_viewport_preview(
        cx,
        &runtime,
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        target_screen_position,
        target_opened.window(),
        "Panel A",
    );

    target_window
        .update(cx, |host, window, cx| {
            host.interaction_mut()
                .update_drop_route_preview(&resolution, target_center_host_position());
            window.refresh();
            cx.notify();
        })
        .expect("target host should cache routed delivery");

    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(target_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        target_center_host_position(),
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_opened.window().window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));

    target_window
        .update(cx, |host, window, cx| {
            host.drop_payload_release_from_render(
                DockPayloadDropRelease::hovered_host(
                    payload.clone(),
                    target_space.clone(),
                    target_center_host_position(),
                ),
                window,
                cx,
            )
        })
        .expect("target host should reroute stale cached delivery from current release facts");
    cx.run_until_parked();

    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            Vec::<DockItemId>::new()
        );
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should still exist")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a")]);
        assert_eq!(selected.as_ref(), items.get(1));
    });
    let status = runtime.runtime_status();
    assert_eq!(
        status.last_drop_outcome.as_ref().map(|record| record.kind),
        Some(DockViewportDropOutcomeKind::Action),
        "fallback reroute should record the successful release outcome, not the stale cached attempt"
    );
}

#[open_gpui::test]
fn host_render_route_preview_uses_route_debug_selector(cx: &mut TestAppContext) {
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
        target_opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(target_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        target_center_host_position(),
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_opened.window().window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
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
        .window()
        .downcast::<crate::DockHost>()
        .expect("source viewport should render DockHost");
    let source_host = source_window
        .root(cx)
        .expect("source viewport should expose DockHost root");
    let target_screen_position =
        screen_position_for_host_position(target_bounds, target_center_host_position());
    let resolution = cache_known_viewport_preview(
        cx,
        &runtime,
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        target_screen_position,
        target_opened.window(),
        "Panel A",
    );

    source_window
        .update(cx, |host, window, cx| {
            host.interaction_mut()
                .update_drop_route_preview(&resolution, target_center_host_position());
            window.refresh();
            cx.notify();
        })
        .expect("source host should update route preview");
    cx.run_until_parked();
    let source_visual = VisualTestContext::from_window(source_window.into(), cx);

    assert!(
        selector_for(
            &source_visual,
            &source_host,
            DockDebugRegion::DropRoutePreview {
                kind: DockDropRoutePreviewKind::KnownViewport
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
fn source_hover_over_known_viewport_renders_target_drop_preview(cx: &mut TestAppContext) {
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
        .window()
        .downcast::<crate::DockHost>()
        .expect("source viewport should render DockHost");
    let target_window = target_opened
        .window()
        .downcast::<crate::DockHost>()
        .expect("target viewport should render DockHost");
    let source_host = source_window
        .root(cx)
        .expect("source viewport should expose DockHost root");
    let target_host = target_window
        .root(cx)
        .expect("target viewport should expose DockHost root");
    cx.run_until_parked();

    let target_screen_position =
        screen_position_for_host_position(target_bounds, target_center_host_position());
    let resolution = cache_known_viewport_preview(
        cx,
        &runtime,
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        target_screen_position,
        target_opened.window(),
        "Panel A",
    );

    source_window
        .update(cx, |host, window, cx| {
            host.interaction_mut()
                .update_drop_route_preview(&resolution, target_center_host_position());
            window.refresh();
            cx.notify();
        })
        .expect("source host should update route preview");
    cx.run_until_parked();

    let mut target_visual = VisualTestContext::from_window(target_opened.window(), cx);
    let target_preview = selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview)
        .expect("target viewport should render the routed drop preview");
    let target_preview_bounds = debug_bounds(&mut target_visual, &target_preview);
    assert!(
        target_preview_bounds.size.width > px(0.0) && target_preview_bounds.size.height > px(0.0),
        "target routed drop preview should have visible bounds"
    );
    assert!(
        selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::DropPayloadTabPreview
        )
        .is_some(),
        "target viewport should render the payload tab label inside the routed preview"
    );
    assert!(
        selector_for(
            &VisualTestContext::from_window(source_opened.window(), cx),
            &source_host,
            DockDebugRegion::DropRoutePreview {
                kind: DockDropRoutePreviewKind::KnownViewport
            }
        )
        .is_some(),
        "source viewport can still show the route marker while the target draws the dock overlay"
    );
}

#[open_gpui::test]
fn source_release_prefers_local_target_over_cached_route_delivery(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
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
        .window()
        .downcast::<crate::DockHost>()
        .expect("source viewport should render DockHost");
    let target_window = target_opened
        .window()
        .downcast::<crate::DockHost>()
        .expect("target viewport should render DockHost");
    let source_host = source_window
        .root(cx)
        .expect("source viewport should expose DockHost root");
    let target_host = target_window
        .root(cx)
        .expect("target viewport should expose DockHost root");

    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(target_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        target_center_host_position(),
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_opened.window().window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));

    let target_screen_position = point(
        target_bounds.get_bounds().origin.x + px(120.0),
        target_bounds.get_bounds().origin.y + px(100.0),
    );
    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    let resolution = cache_known_viewport_preview(
        cx,
        &runtime,
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        target_screen_position,
        target_opened.window(),
        "Panel A",
    );

    source_window
        .update(cx, |host, window, cx| {
            host.interaction_mut()
                .update_drop_route_preview(&resolution, target_center_host_position());
            window.refresh();
            cx.notify();
        })
        .expect("source host should update route preview");
    cx.run_until_parked();

    let target_visual = VisualTestContext::from_window(target_opened.window(), cx);
    assert!(
        selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview).is_some(),
        "target viewport should draw the cached routed preview before release"
    );
    let mut source_visual = VisualTestContext::from_window(source_opened.window(), cx);
    assert!(
        selector_for(
            &source_visual,
            &source_host,
            DockDebugRegion::DropRoutePreview {
                kind: DockDropRoutePreviewKind::KnownViewport
            }
        )
        .is_some(),
        "source viewport should cache the routed preview before release"
    );
    let source_tab_selector = selector_for(
        &source_visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let source_tab_center = debug_bounds(&mut source_visual, &source_tab_selector).center();

    source_window
        .update(cx, |host, window, cx| {
            host.drop_payload_release_from_render(
                DockPayloadDropRelease::hovered_host(
                    payload.clone(),
                    source_space.clone(),
                    source_tab_center,
                ),
                window,
                cx,
            )
        })
        .expect("source host should commit the rendered release");
    cx.run_until_parked();

    let final_source_visual = VisualTestContext::from_window(source_opened.window(), cx);
    assert!(
        selector_for(
            &final_source_visual,
            &source_host,
            DockDebugRegion::DropPreview
        )
        .is_none(),
        "release should clear the cached routed preview"
    );
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(source_tabs)
            .expect("source tabs should still exist")
        else {
            panic!("source should remain tabs");
        };
        assert_eq!(items, &vec![item("a"), item("b")]);
        assert_eq!(selected.as_ref(), items.get(0));

        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should still exist")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("c")]);
        assert_eq!(selected.as_ref(), items.get(0));
    });
}

#[open_gpui::test]
fn source_only_release_does_not_consume_cached_route_delivery(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
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
    let source_opened = cx
        .update(|app| {
            runtime.open_viewport(
                source_space.clone(),
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(floating_bounds(
                        520.0, 100.0, 360.0, 220.0,
                    ))),
                    ..Default::default()
                },
                app,
            )
        })
        .expect("source viewport should open");
    let source_window = source_opened
        .window()
        .downcast::<crate::DockHost>()
        .expect("source viewport should render DockHost");

    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(target_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        target_center_host_position(),
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_opened.window().window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));

    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    let session = runtime.begin_payload_drag(&payload);
    let target_screen_position = point(
        target_bounds.get_bounds().origin.x + px(120.0),
        target_bounds.get_bounds().origin.y + px(100.0),
    );
    let preview_request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        target_screen_position,
        None,
        DockViewportTargetContext::new().with_hovered_window(target_opened.window()),
    )
    .with_drag_session(Some(session.clone()));
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
    assert!(
        matches!(
            resolution.route(),
            DockViewportDropRoute::KnownViewport { target }
                if target.window_id() == target_opened.window().window_id()
        ),
        "preview setup should resolve the target viewport"
    );
    source_window
        .update(cx, |host, window, cx| {
            host.interaction_mut()
                .update_drop_route_preview(&resolution, target_center_host_position());
            window.refresh();
            cx.notify();
        })
        .expect("source host should cache the routed delivery");

    source_window
        .update(cx, |host, window, cx| {
            host.drop_payload_release_from_render(
                DockPayloadDropRelease::source_only_with_session(
                    payload.clone(),
                    source_space.clone(),
                    point(px(900.0), px(900.0)),
                    Some(session.clone()),
                ),
                window,
                cx,
            )
        })
        .expect("source host should handle source-only release");
    cx.run_until_parked();

    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(source_tabs)
            .expect("source tabs should still exist")
        else {
            panic!("source should remain tabs");
        };
        assert_eq!(items, &vec![item("a"), item("b")]);
        assert_eq!(selected.as_ref(), items.get(0));

        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should still exist")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("c")]);
        assert_eq!(selected.as_ref(), items.get(0));
    });
}

#[open_gpui::test]
fn runtime_opened_viewports_publish_host_scene_for_cross_window_drop(cx: &mut TestAppContext) {
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
        .window()
        .downcast::<crate::DockHost>()
        .expect("source viewport should render DockHost");
    let target_window = target_opened
        .window()
        .downcast::<crate::DockHost>()
        .expect("target viewport should render DockHost");
    let source_host = source_window
        .root(cx)
        .expect("source viewport should expose DockHost root");
    let target_host = target_window
        .root(cx)
        .expect("target viewport should expose DockHost root");
    cx.run_until_parked();
    let mut source_visual = VisualTestContext::from_window(source_opened.window(), cx);
    let mut target_visual = VisualTestContext::from_window(target_opened.window(), cx);

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
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should still exist")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a")]);
        assert_eq!(selected.as_ref(), items.get(1));
    });
}

#[open_gpui::test]
fn runtime_opened_viewports_dock_back_from_source_only_release(cx: &mut TestAppContext) {
    let target_space = DockSpaceId::from("main");
    let source_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
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
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(floating_bounds(
                        100.0, 100.0, 360.0, 220.0,
                    ))),
                    ..Default::default()
                },
                app,
            )
        })
        .expect("target viewport should open");
    let source_opened = cx
        .update(|app| {
            runtime.open_viewport(
                source_space.clone(),
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(floating_bounds(
                        520.0, 100.0, 360.0, 220.0,
                    ))),
                    ..Default::default()
                },
                app,
            )
        })
        .expect("source viewport should open");
    let target_window = target_opened
        .window()
        .downcast::<crate::DockHost>()
        .expect("target viewport should render DockHost");
    let target_host = target_window
        .root(cx)
        .expect("target viewport should expose DockHost root");
    let source_window = source_opened
        .window()
        .downcast::<crate::DockHost>()
        .expect("source viewport should render DockHost");
    let source_host = source_window
        .root(cx)
        .expect("source viewport should expose DockHost root");
    cx.run_until_parked();

    let mut target_visual = VisualTestContext::from_window(target_opened.window(), cx);
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
        .window()
        .update(cx, |_, window, _| window.window_bounds())
        .expect("target window should still be live")
        .get_bounds();
    let release_screen_position = point(
        target_window_bounds.origin.x + target_position.x,
        target_window_bounds.origin.y + target_position.y,
    );
    let source_release_signals = source_opened
        .window()
        .update(cx, |_, _, app| {
            DockViewportPlatformSignals::from_app_without_hovered_window_authority(app)
        })
        .expect("source window should still be live");
    // TestPlatform normalizes runtime-opened window origins to zero. Override only the source
    // snapshot so this models a native detached window releasing over main, not over itself.
    assert!(runtime.begin_viewport_host_scene(
        source_space.clone(),
        source_opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
            520.0, 0.0, 360.0, 220.0,
        ))),
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
    cx.run_until_parked();
    let target_visual = VisualTestContext::from_window(target_opened.window(), cx);
    let source_visual = VisualTestContext::from_window(source_opened.window(), cx);

    assert!(
        selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview).is_none(),
        "target viewport drop preview should clear after release"
    );
    assert!(
        selector_for(&source_visual, &source_host, DockDebugRegion::DropPreview).is_none(),
        "source viewport drop preview should clear after release"
    );
    let status = runtime.runtime_status();
    let target = &status
        .last_route
        .as_ref()
        .expect("dock-back should route to the target viewport")
        .target;
    assert_eq!(target.space(), Some(&target_space));
    assert_eq!(action.action(), crate::DockActionOutcome::Changed);
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should still exist")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a")]);
        assert_eq!(selected.as_ref(), items.get(1));
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            Vec::<DockItemId>::new(),
            "source viewport should be emptied by a successful dock-back"
        );
    });
}

#[open_gpui::test]
fn runtime_opened_viewports_do_not_reuse_previewed_target_when_source_only_release_leaves_viewport(
    cx: &mut TestAppContext,
) {
    let target_space = DockSpaceId::from("main");
    let source_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
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
    let _target_window = target_opened
        .window()
        .downcast::<crate::DockHost>()
        .expect("target viewport should render DockHost");
    let source_window = source_opened
        .window()
        .downcast::<crate::DockHost>()
        .expect("source viewport should render DockHost");
    cx.run_until_parked();

    let target_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(target_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        target_center_host_position(),
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_opened.window().window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));

    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "A".to_string(),
    );
    let session = runtime.begin_payload_drag(&payload);
    let preview_request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new()
            .with_hovered_window(target_opened.window())
            .with_window_stack([source_opened.window(), target_opened.window()]),
    )
    .with_drag_session(Some(session.clone()));
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
    assert!(
        matches!(
            resolution.route(),
            DockViewportDropRoute::KnownViewport { target }
                if target.window_id() == target_opened.window().window_id()
        ),
        "preview route should target the main viewport"
    );
    cx.update(|app| {
        runtime.update_routed_drop_preview(&resolution, "Panel A", app);
    });
    assert!(
        runtime
            .routed_drop_preview_for(&target_space, target_opened.window().window_id())
            .is_some(),
        "shared runtime should store the routed preview"
    );
    assert!(
        runtime
            .routed_drop_delivery_for_drag_session(Some(&session))
            .is_some(),
        "shared runtime should store a reusable routed delivery"
    );
    assert_eq!(
        runtime
            .active_payload_drag_session(&payload)
            .expect("drag session should still be active before release")
            .id(),
        session.id(),
        "the active session should still match the routed preview session"
    );
    let release = DockPayloadDropRelease::source_only_with_session(
        payload.clone(),
        source_space.clone(),
        point(px(900.0), px(900.0)),
        Some(session.clone()),
    );
    source_window
        .update(cx, |host, window, cx| {
            host.drop_payload_release_from_render(release, window, cx)
        })
        .expect("source host should handle the source-only release");
    cx.run_until_parked();

    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should still exist")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("b")]);
        assert_eq!(selected.as_ref(), items.get(0));
        assert_eq!(
            controller.graph().collect_items_in_space(&target_space),
            vec![item("b")]
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
        selected: Some(item("c")),
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
        .window()
        .downcast::<crate::DockHost>()
        .expect("source viewport should render DockHost");
    let target_window = target_opened
        .window()
        .downcast::<crate::DockHost>()
        .expect("target viewport should render DockHost");
    let source_host = source_window
        .root(cx)
        .expect("source viewport should expose DockHost root");
    let target_host = target_window
        .root(cx)
        .expect("target viewport should expose DockHost root");
    cx.run_until_parked();
    let mut source_visual = VisualTestContext::from_window(source_opened.window(), cx);
    let mut target_visual = VisualTestContext::from_window(target_opened.window(), cx);

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
    let target_visual = VisualTestContext::from_window(target_opened.window(), cx);
    let source_visual = VisualTestContext::from_window(source_opened.window(), cx);

    assert!(
        selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview).is_none(),
        "target viewport drop preview should clear after release"
    );
    assert!(
        selector_for(&source_visual, &source_host, DockDebugRegion::DropPreview).is_none(),
        "source viewport drop preview should clear after release"
    );

    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should still exist")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a"), item("c")]);
        assert_eq!(selected.as_ref(), items.get(2));
    });
}

#[open_gpui::test]
fn viewport_runtime_handle_prevents_platform_close_when_policy_prevents(cx: &mut TestAppContext) {
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

    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("secondary viewport should open through runtime handle");
    let mut visual = VisualTestContext::from_window(opened.window(), cx);

    assert_eq!(
        runtime.close_policy(),
        DockViewportClosePolicy::RetainLayout
    );
    assert_eq!(
        runtime
            .handle_window_should_close(opened.window().window_id())
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
            .handle_window_should_close(opened.window().window_id())
            .status,
        DockViewportShouldCloseStatus::Vetoed
    );
    assert_eq!(
        runtime
            .borrow()
            .adapter()
            .window_for_space(&secondary_space),
        Some(opened.window())
    );
}

#[open_gpui::test]
fn viewport_runtime_handle_vetoes_retain_layout_close_for_non_closable_panel(
    cx: &mut TestAppContext,
) {
    let secondary_space = DockSpaceId::from("secondary");
    let mut graph = DockGraph::new();
    let secondary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(secondary_space.clone(), secondary_tabs);

    let mut workspace = DockWorkspace::new(secondary_space.clone(), graph);
    workspace.register_panel(
        item("b"),
        DockPanel::new("Panel B", test_view(cx, "B")).closable(false),
    );
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
    let mut visual = VisualTestContext::from_window(opened.window(), cx);
    let window_id = opened.window().window_id();

    assert_eq!(
        cx.update(|app| runtime
            .handle_window_should_close_with_app(window_id, app)
            .status),
        DockViewportShouldCloseStatus::Vetoed
    );
    assert!(
        !visual.simulate_close(),
        "RetainLayout should not hide a non-closable panel by closing its viewport"
    );
    assert_eq!(
        runtime
            .borrow()
            .adapter()
            .window_for_space(&secondary_space),
        Some(opened.window())
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
        selected: Some(item("c")),
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
        .window()
        .update(cx, |_, window, _| window.window_bounds())
        .expect("target window should be live");
    let target_window_bounds = WindowBounds::Windowed(target_window_bounds.get_bounds());
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(target_window_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        target_center_host_position(),
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        opened.window().window_id(),
        DockHostDropSceneFact::Leaf(DockLeafDropTarget {
            root: target_tabs,
            target_tabs,
            bounds: floating_bounds(0.0, 0.0, 360.0, 220.0),
            is_central: false,
        }),
    ));

    let result = cx.update(|app| {
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Tabs,
            runtime
                .last_host_scene_screen_position(&target_space)
                .expect("target scene should expose a screen position"),
            None,
            DockViewportPlatformSignals::from_app(app).with_hovered_window(opened.window()),
        );
        let resolution = runtime.resolve_payload_drop_delivery(&request, app);
        runtime
            .deliver_payload_drop_with_outcome(resolution.expect_delivery().clone(), app)
            .and_then(|outcome| outcome.action_result())
    });

    assert_eq!(result, Ok(crate::DockActionOutcome::Changed));
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should still exist")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a"), item("c")]);
        assert_eq!(selected.as_ref(), items.get(2));
    });
}

#[open_gpui::test]
fn viewport_runtime_handle_resolves_rendered_root_edge_scene(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let target_left_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let target_right_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
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
        .window()
        .downcast::<crate::DockHost>()
        .expect("target viewport should render DockHost");
    let target_host = target_window
        .root(cx)
        .expect("target viewport should expose DockHost root");
    cx.run_until_parked();

    let mut target_visual = VisualTestContext::from_window(target_opened.window(), cx);
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
    let target_host_position = outer_edge_drop_position(right_tabs_bounds, DropZone::Right);
    let resolved = cx
        .update(|app| runtime.resolve_host_scene_target(&target_space, target_host_position, app))
        .expect("rendered host scene should resolve the root edge");
    assert_eq!(resolved.source, DockDropResolveSource::RootEdge);
    assert!(matches!(
        resolved.kind,
        DockResolvedDropTargetKind::RootEdge {
            root,
            leaf_tabs: Some(leaf_tabs),
            zone: DropZone::Right,
        } if root == target_root && leaf_tabs == target_right_tabs
    ));

    let target_window_bounds = target_opened
        .window()
        .update(cx, |_, window, _| window.window_bounds())
        .expect("target window should still be live")
        .get_bounds();
    let release_screen_position = point(
        target_window_bounds.origin.x + target_host_position.x,
        target_window_bounds.origin.y + target_host_position.y,
    );
    let source_release_signals = source_opened
        .window()
        .update(cx, |_, _, app| {
            DockViewportPlatformSignals::from_app_without_hovered_window_authority(app)
        })
        .expect("source window should still be live");
    assert!(runtime.begin_viewport_host_scene(
        source_space.clone(),
        source_opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
            520.0, 0.0, 360.0, 220.0,
        ))),
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
    assert_eq!(action.action(), crate::DockActionOutcome::Changed);
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Split { children, .. } = controller
            .graph()
            .node(target_root)
            .expect("target root should still exist")
        else {
            panic!("target root should remain a split");
        };
        assert_eq!(children.len(), 3);
        let DockNode::Tabs { items, selected } = controller
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
        assert_eq!(selected.as_ref(), items.get(0));
    });
}

#[open_gpui::test]
fn viewport_runtime_handle_allows_platform_close_with_retain_policy(cx: &mut TestAppContext) {
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

    let opened = cx
        .update(|app| {
            runtime.open_viewport(secondary_space, viewport_window_options(360.0, 220.0), app)
        })
        .expect("secondary viewport should open through runtime handle");
    let mut visual = VisualTestContext::from_window(opened.window(), cx);

    assert!(
        visual.simulate_close(),
        "RetainLayout policy should allow GPUI should-close to continue"
    );
    assert_eq!(
        runtime
            .handle_window_should_close(opened.window().window_id())
            .status,
        DockViewportShouldCloseStatus::Allowed
    );
}
