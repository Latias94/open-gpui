use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockClassId, DockController,
    DockFloatingContainer, DockGraph, DockGraphDropTarget, DockGraphMutationError, DockHost,
    DockItemId, DockNode, DockPanel, DockPolicyError, DockSpaceId, DockViewportAdapter,
    DockViewportClosePolicy, DockViewportCloseStatus, DockViewportDropPayload,
    DockViewportDropRoute, DockViewportDropRouteRequest, DockViewportFocusRequest,
    DockViewportOpenStatus, DockViewportPlatformSyncAction, DockViewportPlatformSyncRequest,
    DockViewportPlatformSyncUnsupportedReason, DockViewportResolvedDropRoute,
    DockViewportRouteStatus, DockViewportRouteTarget, DockViewportRuntime,
    DockViewportRuntimeHandle, DockViewportShouldCloseStatus, DockViewportStaleStatusReason,
    DockViewportTargetContext, DockViewportTearOffOpenOutcome, DockViewportTearOffOutcomeKind,
    DockViewportTearOffPlacementSource, DockViewportTearOffRequest, DockViewportWindowFacts,
    DockWorkspace, SplitAxis,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    drop_runtime::DockHostDropSceneFact,
    drop_target::DockLeafDropTarget,
    host_test_support::*,
    viewport_activation::apply_viewport_activation,
    viewport_tear_off::{
        DockViewportTearOffBeginOutcome, DockViewportTearOffCancelReason,
        DockViewportTearOffCompletionOutcome, DockViewportTearOffTick,
    },
    viewport_test_support::handle,
};
use open_gpui::{
    AnyWindowHandle, AppContext as _, SharedString, TestAppContext, TitlebarOptions,
    VisualTestContext, WindowBounds, WindowHandle, WindowId, WindowOptions, point, px, size,
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

fn cache_known_viewport_preview_for_test(
    runtime: &mut DockViewportRuntime,
    source_space: DockSpaceId,
    source_tabs: crate::DockNodeId,
    target_space: &DockSpaceId,
    target_window: AnyWindowHandle,
    target_tabs: crate::DockNodeId,
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
    let request = DockViewportDropRouteRequest::from_target_context(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_hovered_window(target_window),
    )
    .with_drag_session(Some(session.clone()));
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    let (changed, _) = runtime.update_routed_drop_preview(&resolution, "Panel A");
    assert!(changed);
    session
}

fn close_window_quietly_for_test(window: AnyWindowHandle, cx: &mut TestAppContext) {
    let _ = window.update(cx, |_, window, _| window.remove_window());
}

#[open_gpui::test]
fn viewport_runtime_opens_and_reuses_controller_backed_window(cx: &mut TestAppContext) {
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
fn viewport_runtime_syncs_supported_options_when_reusing_window(cx: &mut TestAppContext) {
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

    let mut workspace = DockWorkspace::new(primary_space, graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);

    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(floating_bounds(
                        0.0, 0.0, 360.0, 220.0,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some(SharedString::from("Initial")),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                app,
            )
        })
        .expect("secondary viewport should open through runtime");

    let reused = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(floating_bounds(
                        24.0, 32.0, 480.0, 260.0,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some(SharedString::from("Retitled")),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                app,
            )
        })
        .expect("live viewport should be reused through runtime");

    assert_eq!(reused.status(), DockViewportOpenStatus::Reused);
    assert_eq!(reused.window(), opened.window());
    let bounds = reused
        .window()
        .update(cx, |_, window, _| window.bounds())
        .expect("reused viewport should remain live");
    assert_eq!(bounds.size, size(px(480.0), px(260.0)));
    assert_eq!(
        bounds.origin,
        point(px(0.0), px(0.0)),
        "GPUI exposes live resize but not live screen-origin updates"
    );

    let sync = runtime
        .runtime_status()
        .last_platform_sync
        .expect("reuse should record platform sync diagnostics");
    assert_eq!(sync.window_id, reused.window().window_id());
    assert!(
        sync.applied
            .contains(&DockViewportPlatformSyncAction::Activate)
    );
    assert!(
        sync.applied
            .contains(&DockViewportPlatformSyncAction::Title {
                title: "Retitled".to_string(),
            })
    );
    assert!(
        sync.applied
            .contains(&DockViewportPlatformSyncAction::Resize {
                size: size(px(480.0), px(260.0)),
            })
    );
    assert!(sync.unsupported_requests.iter().any(|unsupported| {
        unsupported.request
            == DockViewportPlatformSyncRequest::WindowOrigin {
                requested: point(px(24.0), px(32.0)),
            }
            && unsupported.reason
                == DockViewportPlatformSyncUnsupportedReason::UnsupportedByWindowApi
    }));
}

#[open_gpui::test]
fn viewport_runtime_reuse_respects_focus_option(cx: &mut TestAppContext) {
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

    let primary = cx
        .update(|app| {
            runtime.open_viewport(primary_space, viewport_window_options(360.0, 220.0), app)
        })
        .expect("primary viewport should open");
    let secondary = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("secondary viewport should open");
    primary
        .window()
        .update(cx, |_, window, _| window.activate_window())
        .expect("primary viewport should be activatable");
    cx.run_until_parked();
    assert_eq!(cx.update(|app| app.active_window()), Some(primary.window()));

    let reused = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(420.0, 240.0)
                },
                app,
            )
        })
        .expect("secondary viewport should be reused");
    cx.run_until_parked();

    assert_eq!(reused.status(), DockViewportOpenStatus::Reused);
    assert_eq!(reused.window(), secondary.window());
    assert_eq!(
        cx.update(|app| app.active_window()),
        Some(primary.window()),
        "reusing a viewport with focus=false should not raise it during stale probing"
    );
    let sync = runtime
        .runtime_status()
        .last_platform_sync
        .expect("reuse should record platform sync diagnostics");
    assert!(
        !sync
            .applied
            .contains(&DockViewportPlatformSyncAction::Activate)
    );
    assert!(
        sync.applied
            .contains(&DockViewportPlatformSyncAction::Resize {
                size: size(px(420.0), px(240.0)),
            })
    );
}

#[open_gpui::test]
fn viewport_runtime_tracks_recent_activation_for_diagnostic_hit_order(cx: &mut TestAppContext) {
    let alpha_space = DockSpaceId::from("alpha");
    let zeta_space = DockSpaceId::from("zeta");
    let mut graph = DockGraph::new();
    let alpha_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let zeta_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
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
        runtime.borrow().adapter().spaces_by_diagnostic_hit_order(),
        vec![zeta_space, alpha_space]
    );
}

#[open_gpui::test]
fn viewport_runtime_refreshes_focus_stamp_for_close_activation_before_render(
    cx: &mut TestAppContext,
) {
    let main_space = DockSpaceId::from("main");
    let inspector_space = DockSpaceId::from("inspector");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let inspector_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(inspector_space.clone(), inspector_tabs);
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

    let open_options = || WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(floating_bounds(
            0.0, 0.0, 360.0, 220.0,
        ))),
        focus: false,
        ..Default::default()
    };
    let _main = cx
        .update(|app| runtime.open_viewport(main_space.clone(), open_options(), app))
        .expect("main viewport should open");
    let _inspector = cx
        .update(|app| runtime.open_viewport(inspector_space.clone(), open_options(), app))
        .expect("inspector viewport should open");
    let detached = cx
        .update(|app| runtime.open_viewport(detached_space.clone(), open_options(), app))
        .expect("detached viewport should open");
    runtime.record_panel_focus(detached_space.clone(), item("c"));

    let outcome = cx.update(|app| {
        let outcome = runtime
            .borrow_mut()
            .handle_window_closed_with_app(detached.window().window_id(), app);
        let activation = runtime
            .borrow_mut()
            .activation_target_after_close(&outcome, app);
        assert_eq!(
            activation
                .as_ref()
                .map(|target| target.focus_request().clone()),
            Some(DockViewportFocusRequest::panel(item("c"))),
            "close activation should restore focus to the source viewport's recorded focus item"
        );
        assert!(apply_viewport_activation(activation, app));
        outcome
    });

    assert_eq!(outcome.status(), DockViewportCloseStatus::MergedBack);
    assert_eq!(
        runtime.borrow().adapter().spaces_by_diagnostic_hit_order(),
        vec![main_space, inspector_space]
    );
}

#[open_gpui::test]
fn viewport_runtime_tear_off_opens_viewport_then_moves_item(cx: &mut TestAppContext) {
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
        selected: Some(item("a")),
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
        selected: Some(item("a")),
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
        selected: Some(item("a")),
    });
    let other_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
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
            .workspace_mut()
            .commit_tab_move(
                &primary_space,
                source_tabs,
                &item("a"),
                &other_space,
                DockGraphDropTarget::center(other_tabs),
            )
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
        selected: Some(item("a")),
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
        selected: Some(item("a")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
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
fn viewport_runtime_tear_off_commit_failure_preserves_existing_target_window(
    cx: &mut TestAppContext,
) {
    let primary_space = DockSpaceId::from("primary");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(primary_space.clone(), source_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(primary_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let mut runtime = DockViewportRuntime::new(controller);
    let old_window = handle(933);
    let new_window = handle(934);

    runtime.register_opened_viewport(detached_space.clone(), old_window);
    runtime.begin_tear_off_request_at(
        tear_off_request(primary_space.clone(), source_tabs, item("a")),
        detached_space.clone(),
        None,
        DockViewportTearOffTick::new(1),
    );
    let outcome = cx.update(|app| {
        let key = item_tear_off_key(&primary_space, source_tabs, item("a"));
        runtime.complete_tear_off_viewport_at(
            &key,
            new_window,
            DockViewportTearOffTick::new(2),
            app,
        )
    });

    let DockViewportTearOffCompletionOutcome::CommitFailed(failure) = outcome else {
        panic!("non-empty destination space should fail the tear-off move transaction");
    };
    assert_eq!(failure.window(), new_window);
    assert_eq!(failure.replaced_windows(), &[]);
    assert_eq!(
        runtime.adapter().window_for_space(&detached_space),
        Some(old_window),
        "failed tear-off completion must not replace an existing routeable target window"
    );
}

#[open_gpui::test]
fn viewport_runtime_tear_off_commit_failure_closes_opened_window(cx: &mut TestAppContext) {
    let primary_space = DockSpaceId::from("primary");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
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
fn viewport_runtime_replacement_closes_superseded_runtime_window(cx: &mut TestAppContext) {
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
        .expect("secondary viewport should open through runtime");
    let replacement = open_controller_space(
        cx,
        runtime.borrow().controller_entity(),
        secondary_space.clone(),
        size(px(360.0), px(220.0)),
    )
    .0;
    let replacement: AnyWindowHandle = replacement.into();
    let window_count_with_both = cx.windows().len();

    let superseded = runtime
        .borrow_mut()
        .register_opened_viewport(secondary_space.clone(), replacement);
    assert_eq!(superseded, vec![opened.window()]);
    close_window_quietly_for_test(opened.window(), cx);
    cx.run_until_parked();
    cx.update(|app| app.refresh_windows());

    assert_eq!(
        runtime
            .borrow()
            .adapter()
            .window_for_space(&secondary_space),
        Some(replacement)
    );
    assert!(
        cx.windows().len() < window_count_with_both,
        "replacing a runtime-owned viewport should not leave the old window alive"
    );
}

#[open_gpui::test]
fn viewport_runtime_tear_off_completion_reports_replaced_runtime_windows(cx: &mut TestAppContext) {
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
    let mut runtime = DockViewportRuntime::new(controller.clone());

    runtime.begin_tear_off_request_at(
        tear_off_request(primary_space.clone(), source_tabs, item("a")),
        detached_space.clone(),
        Some(item("a")),
        DockViewportTearOffTick::new(1),
    );

    let old_window = open_controller_space(
        cx,
        controller.clone(),
        detached_space.clone(),
        size(px(360.0), px(220.0)),
    )
    .0;
    let old_window: AnyWindowHandle = old_window.into();
    let new_window = open_controller_space(
        cx,
        controller.clone(),
        detached_space.clone(),
        size(px(360.0), px(220.0)),
    )
    .0;
    let new_window: AnyWindowHandle = new_window.into();
    runtime.register_opened_viewport(detached_space.clone(), old_window);

    let outcome = cx.update(|app| {
        let key = item_tear_off_key(&primary_space, source_tabs, item("a"));
        runtime.complete_tear_off_viewport_at(
            &key,
            new_window,
            DockViewportTearOffTick::new(2),
            app,
        )
    });

    let DockViewportTearOffCompletionOutcome::Completed(completed) = outcome else {
        panic!("tear-off should complete after replacing the pending viewport window");
    };
    assert_eq!(completed.replaced_windows(), &[old_window]);
    assert_eq!(completed.registration().window(), new_window);
    assert_eq!(
        runtime.adapter().window_for_space(&detached_space),
        Some(new_window)
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
fn viewport_runtime_tear_off_rejects_already_open_target_space_without_reuse(
    cx: &mut TestAppContext,
) {
    let primary_space = DockSpaceId::from("primary");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
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
fn viewport_runtime_floating_payload_focus_requires_unique_selected_item(cx: &mut TestAppContext) {
    let primary_space = DockSpaceId::from("primary");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let left_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let right_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let floating_split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left_tabs, right_tabs],
        fractions: vec![0.5, 0.5],
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_split,
    });
    graph
        .floating_containers_mut(primary_space.clone())
        .push(DockFloatingContainer {
            node: floating,
            bounds: floating_bounds(10.0, 20.0, 260.0, 150.0),
        });

    let workspace = DockWorkspace::new(primary_space.clone(), graph);
    let controller = cx.new(|_| DockController::new(workspace));
    let mut runtime = DockViewportRuntime::new(controller);
    let outcome = cx.update(|app| {
        runtime.begin_tear_off_request(
            DockViewportTearOffRequest::new(
                primary_space,
                floating,
                DockViewportDropPayload::Floating(floating),
                point(px(900.0), px(900.0)),
                None,
            ),
            detached_space,
            app,
        )
    });

    let DockViewportTearOffBeginOutcome::Pending(pending) = outcome else {
        panic!("floating tear-off should begin");
    };
    assert_eq!(
        pending.focus_item(),
        None,
        "split floating payload focus must not be inferred from depth-first selected tab order"
    );
}

#[open_gpui::test]
fn viewport_runtime_unregister_space_clears_recorded_panel_focus(cx: &mut TestAppContext) {
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(detached_space.clone(), tabs);

    let mut workspace = DockWorkspace::new(detached_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let window = handle(149);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(detached_space.clone(), window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller,
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );
    runtime.record_panel_focus(detached_space.clone(), item("a"));

    assert_eq!(
        runtime.recorded_panel_focus(&detached_space),
        Some(&item("a"))
    );
    assert!(runtime.unregister_host_for_space(&detached_space, window.window_id()));
    assert_eq!(runtime.adapter().window_for_space(&detached_space), None);
    assert_eq!(runtime.recorded_panel_focus(&detached_space), None);
}

#[open_gpui::test]
fn viewport_runtime_should_close_observes_policy_changes_after_open(cx: &mut TestAppContext) {
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
        .expect("secondary viewport should open through runtime");
    let mut visual = VisualTestContext::from_window(opened.window(), cx);
    runtime.set_close_policy(DockViewportClosePolicy::Prevent);
    assert!(
        !visual.simulate_close(),
        "updated Prevent policy should veto the already-open window"
    );
    assert_eq!(
        cx.update(
            |app| runtime.handle_window_should_close_with_app(opened.window().window_id(), app)
        )
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
        cx.update(
            |app| runtime.handle_window_should_close_with_app(opened.window().window_id(), app)
        )
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
        selected: Some(item("b")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
        selected: Some(item("c")),
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
    assert_eq!(
        outcome.focus_item().cloned(),
        None,
        "merge-back close should not infer panel focus from selected tabs without a recorded GPUI focus"
    );
    assert_eq!(runtime.adapter().window_for_space(&detached_space), None);
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
    });
}

#[open_gpui::test]
fn viewport_runtime_merge_back_close_uses_recorded_source_focus_before_tree_order(
    cx: &mut TestAppContext,
) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let detached_root = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let detached_floating_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let detached_floating = graph.insert_node(DockNode::Floating {
        child: detached_floating_tabs,
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(detached_space.clone(), detached_root);
    graph
        .floating_containers_mut(detached_space.clone())
        .push(DockFloatingContainer {
            node: detached_floating,
            bounds: floating_bounds(10.0, 20.0, 220.0, 140.0),
        });

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let window = handle(47);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(detached_space.clone(), window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller.clone(),
        adapter,
        DockViewportClosePolicy::MergeBack {
            target_space: main_space.clone(),
        },
    );
    runtime.record_panel_focus(detached_space.clone(), item("c"));

    let outcome = cx.update(|app| runtime.handle_window_closed_with_app(window.window_id(), app));

    assert_eq!(outcome.status(), DockViewportCloseStatus::MergedBack);
    assert_eq!(
        outcome.focus_item().cloned(),
        Some(item("c")),
        "merge-back close should use the source viewport's recorded GPUI panel focus before root tree order"
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&main_space),
            vec![item("b"), item("a"), item("c")]
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            Vec::<DockItemId>::new()
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_merge_back_close_does_not_guess_between_multiple_selected_items(
    cx: &mut TestAppContext,
) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let detached_root = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let detached_floating_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let detached_floating = graph.insert_node(DockNode::Floating {
        child: detached_floating_tabs,
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(detached_space.clone(), detached_root);
    graph
        .floating_containers_mut(detached_space.clone())
        .push(DockFloatingContainer {
            node: detached_floating,
            bounds: floating_bounds(10.0, 20.0, 220.0, 140.0),
        });

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let window = handle(48);
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
    assert_eq!(
        outcome.focus_item().cloned(),
        None,
        "merge-back close should not infer focus from root tree order when multiple selected panels are visible"
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&main_space),
            vec![item("b"), item("a"), item("c")]
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_merge_back_should_close_rejects_non_unique_target_tabs(
    cx: &mut TestAppContext,
) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_left = graph.insert_node(DockNode::Tabs {
        items: vec![item("left")],
        selected: Some(item("left")),
    });
    let main_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("right")],
        selected: Some(item("right")),
    });
    let main_root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![main_left, main_right],
        fractions: vec![0.5, 0.5],
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(main_space.clone(), main_root);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("left"), "Left", test_view(cx, "Left"));
    workspace.register_panel_view(item("right"), "Right", test_view(cx, "Right"));
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let window = handle(148);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(detached_space.clone(), window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller.clone(),
        adapter,
        DockViewportClosePolicy::MergeBack {
            target_space: main_space.clone(),
        },
    );
    runtime.record_panel_focus(detached_space.clone(), item("a"));

    let should_close = cx.update(|app| {
        runtime
            .handle_window_should_close_with_app_and_refresh(window.window_id(), app)
            .0
    });

    assert_eq!(should_close.status, DockViewportShouldCloseStatus::Vetoed);
    assert_eq!(
        runtime.adapter().window_for_space(&detached_space),
        Some(window)
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().root(&detached_space),
            Some(detached_tabs)
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&main_space),
            vec![item("left"), item("right")]
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_merge_back_should_close_is_idempotent_after_precommit(cx: &mut TestAppContext) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
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

    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                detached_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("detached viewport should open through runtime");

    let first_should_close = cx.update(|app| {
        runtime.handle_window_should_close_with_app(opened.window().window_id(), app)
    });
    assert_eq!(
        first_should_close.status,
        DockViewportShouldCloseStatus::Allowed
    );

    controller.update(cx, |controller, _| {
        let mut graph = controller.workspace().graph().clone();
        let reinjected_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("c")],
            selected: Some(item("c")),
        });
        graph.set_root(detached_space.clone(), reinjected_tabs);
        controller.workspace_mut().set_graph(graph);
    });

    let second_should_close = cx.update(|app| {
        runtime.handle_window_should_close_with_app(opened.window().window_id(), app)
    });
    assert_eq!(
        second_should_close.status,
        DockViewportShouldCloseStatus::Allowed
    );

    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&main_space),
            vec![item("b"), item("a")]
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            vec![item("c")]
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_merge_back_should_close_vetoes_invalid_target(cx: &mut TestAppContext) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let fallback_space = DockSpaceId::from("fallback");
    let mut graph = DockGraph::new();
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space, graph);
    workspace.policy_mut().set_allow_platform_viewports(false);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::with_close_policy(
        controller,
        DockViewportClosePolicy::MergeBack {
            target_space: fallback_space,
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
        .expect("detached viewport should open through runtime");
    let mut visual = VisualTestContext::from_window(opened.window(), cx);

    assert!(
        !visual.simulate_close(),
        "merge-back should veto close when commit would require a disabled platform viewport"
    );
    assert_eq!(
        runtime.borrow().adapter().window_for_space(&detached_space),
        Some(opened.window())
    );
    let should_close = cx.update(|app| {
        runtime.handle_window_should_close_with_app(opened.window().window_id(), app)
    });
    assert_eq!(should_close.status, DockViewportShouldCloseStatus::Vetoed);
}

#[open_gpui::test]
fn viewport_runtime_merge_back_commits_during_should_close(cx: &mut TestAppContext) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_descriptor(
        item("a"),
        crate::DockPanelDescriptor::new("Panel A").with_dock_class("editor"),
    );
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace
        .policy_mut()
        .allow_dock_class_in_space(main_space.clone(), "editor");
    let controller = cx.new(|_| DockController::new(workspace));
    let window = handle(45);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(detached_space.clone(), window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller.clone(),
        adapter,
        DockViewportClosePolicy::MergeBack {
            target_space: main_space.clone(),
        },
    );
    runtime.record_panel_focus(detached_space.clone(), item("a"));

    let should_close = cx.update(|app| {
        runtime
            .handle_window_should_close_with_app_and_refresh(window.window_id(), app)
            .0
    });
    assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&main_space),
            vec![item("b"), item("a")]
        );
        assert!(
            controller
                .graph()
                .collect_items_in_space(&detached_space)
                .is_empty()
        );
    });

    controller.update(cx, |controller, _| {
        controller
            .policy_mut()
            .set_allowed_dock_classes_for_space(main_space.clone(), ["inspector"]);
    });
    let closed = cx.update(|app| runtime.handle_window_closed_with_app(window.window_id(), app));

    assert_eq!(closed.status(), DockViewportCloseStatus::MergedBack);
    assert_eq!(
        closed.focus_item().cloned(),
        Some(item("a")),
        "precommitted merge-back close should preserve the source focus item"
    );
    assert_eq!(runtime.adapter().window_for_space(&detached_space), None);
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&main_space),
            vec![item("b"), item("a")]
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_precommitted_merge_back_activation_uses_committed_target(
    cx: &mut TestAppContext,
) {
    let main_space = DockSpaceId::from("main");
    let inspector_space = DockSpaceId::from("inspector");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let inspector_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(inspector_space.clone(), inspector_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_descriptor(
        item("a"),
        crate::DockPanelDescriptor::new("Panel A").with_dock_class("editor"),
    );
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    workspace
        .policy_mut()
        .allow_dock_class_in_space(main_space.clone(), "editor");
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::with_close_policy(
        controller.clone(),
        DockViewportClosePolicy::MergeBack {
            target_space: main_space.clone(),
        },
    );
    let open_options = || WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(floating_bounds(
            0.0, 0.0, 360.0, 220.0,
        ))),
        focus: false,
        ..Default::default()
    };
    let _main = cx
        .update(|app| runtime.open_viewport(main_space.clone(), open_options(), app))
        .expect("main viewport should open");
    let _inspector = cx
        .update(|app| runtime.open_viewport(inspector_space.clone(), open_options(), app))
        .expect("inspector viewport should open");
    let detached = cx
        .update(|app| runtime.open_viewport(detached_space.clone(), open_options(), app))
        .expect("detached viewport should open");
    runtime.record_panel_focus(detached_space.clone(), item("a"));

    let should_close = cx.update(|app| {
        runtime.handle_window_should_close_with_app(detached.window().window_id(), app)
    });
    assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);

    runtime.set_close_policy(DockViewportClosePolicy::MergeBack {
        target_space: inspector_space.clone(),
    });
    let closed =
        cx.update(|app| runtime.handle_window_closed_with_app(detached.window().window_id(), app));
    let activation = cx.update(|app| {
        runtime
            .borrow_mut()
            .activation_target_after_close(&closed, app)
    });

    assert_eq!(closed.status(), DockViewportCloseStatus::MergedBack);
    assert_eq!(closed.merge_target_space(), Some(&main_space));
    assert_eq!(closed.focus_item().cloned(), Some(item("a")));
    assert_eq!(
        activation.as_ref().map(|target| target.space()),
        Some(&main_space),
        "activation must use the precommitted merge-back target, not a later close policy"
    );
    assert_eq!(
        activation
            .as_ref()
            .map(|target| target.focus_request().clone()),
        Some(DockViewportFocusRequest::panel(item("a")))
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&main_space),
            vec![item("b"), item("a")]
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&inspector_space),
            vec![item("c")]
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_merge_back_should_close_disables_pending_window_routing(
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
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_descriptor(
        item("a"),
        crate::DockPanelDescriptor::new("Panel A").with_dock_class("editor"),
    );
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace
        .policy_mut()
        .allow_dock_class_in_space(main_space.clone(), "editor");
    let controller = cx.new(|_| DockController::new(workspace));
    let window = handle(47);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(detached_space.clone(), window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller.clone(),
        adapter,
        DockViewportClosePolicy::MergeBack {
            target_space: main_space.clone(),
        },
    );
    let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let host_position = center_drop_position(host_bounds);
    assert!(runtime.begin_viewport_host_scene(
        detached_space.clone(),
        window.window_id(),
        DockViewportWindowFacts::from_window_bounds(window_bounds),
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &detached_space,
        window.window_id(),
        leaf_host_scene_fact(detached_tabs, detached_tabs),
    ));

    let should_close = cx.update(|app| {
        runtime
            .handle_window_should_close_with_app_and_refresh(window.window_id(), app)
            .0
    });

    assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);
    assert_eq!(
        runtime.adapter().window_for_space(&detached_space),
        Some(window)
    );
    let lifecycle = runtime.runtime_status().viewport_lifecycle;
    let detached_lifecycle = lifecycle
        .iter()
        .find(|record| record.space == detached_space)
        .expect("pending close should keep the mapping for the close callback");
    assert_eq!(
        detached_lifecycle.route_status,
        DockViewportRouteStatus::Stale {
            reason: DockViewportStaleStatusReason::PlatformCloseRequested,
        }
    );
    assert_eq!(
        runtime.last_host_scene_screen_position(&detached_space),
        None
    );
    assert!(
        !runtime.begin_viewport_host_scene(
            detached_space.clone(),
            window.window_id(),
            DockViewportWindowFacts::from_window_bounds(window_bounds),
            host_bounds,
            host_position,
        ),
        "pending close windows must not republish route facts before the close callback"
    );

    let request = DockViewportDropRouteRequest::from_target_context(
        detached_space.clone(),
        detached_tabs,
        DockViewportDropPayload::Item(item("a")),
        screen_position_for_host_position(window_bounds, host_position),
        None,
        DockViewportTargetContext::new().with_hovered_window(window),
    );
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert_eq!(resolution.route(), &DockViewportDropRoute::Unavailable);
    assert!(resolution.delivery().is_none());

    let closed = cx.update(|app| runtime.handle_window_closed_with_app(window.window_id(), app));

    assert_eq!(closed.status(), DockViewportCloseStatus::MergedBack);
    assert_eq!(runtime.adapter().window_for_space(&detached_space), None);
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&main_space),
            vec![item("b"), item("a")]
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_retain_should_close_disables_pending_window_routing(cx: &mut TestAppContext) {
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(detached_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let window = handle(48);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(detached_space.clone(), window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller,
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let host_position = center_drop_position(host_bounds);
    assert!(runtime.begin_viewport_host_scene(
        detached_space.clone(),
        window.window_id(),
        DockViewportWindowFacts::from_window_bounds(window_bounds),
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &detached_space,
        window.window_id(),
        leaf_host_scene_fact(detached_tabs, detached_tabs),
    ));

    let should_close = cx.update(|app| {
        runtime
            .handle_window_should_close_with_app_and_refresh(window.window_id(), app)
            .0
    });

    assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);
    assert_eq!(
        runtime.adapter().window_for_space(&detached_space),
        Some(window),
        "pending close keeps the mapping for close callback attribution"
    );
    let lifecycle = runtime.runtime_status().viewport_lifecycle;
    let detached_lifecycle = lifecycle
        .iter()
        .find(|record| record.space == detached_space)
        .expect("pending retain close should keep lifecycle diagnostics");
    assert_eq!(
        detached_lifecycle.route_status,
        DockViewportRouteStatus::Stale {
            reason: DockViewportStaleStatusReason::PlatformCloseRequested,
        }
    );
    assert_eq!(
        runtime.last_host_scene_screen_position(&detached_space),
        None
    );
    assert!(
        !runtime.begin_viewport_host_scene(
            detached_space.clone(),
            window.window_id(),
            DockViewportWindowFacts::from_window_bounds(window_bounds),
            host_bounds,
            host_position,
        ),
        "retain close pending windows must not republish route facts before close callback"
    );

    let request = DockViewportDropRouteRequest::from_target_context(
        detached_space.clone(),
        detached_tabs,
        DockViewportDropPayload::Item(item("a")),
        screen_position_for_host_position(window_bounds, host_position),
        None,
        DockViewportTargetContext::new().with_hovered_window(window),
    );
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert_eq!(resolution.route(), &DockViewportDropRoute::Unavailable);
    assert!(resolution.delivery().is_none());
}

#[open_gpui::test]
fn viewport_runtime_discarded_precommitted_close_does_not_mark_reused_window(
    cx: &mut TestAppContext,
) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let inspector_space = DockSpaceId::from("inspector");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let inspector_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);
    graph.set_root(inspector_space.clone(), inspector_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_descriptor(
        item("a"),
        crate::DockPanelDescriptor::new("Panel A").with_dock_class("editor"),
    );
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_descriptor(
        item("c"),
        crate::DockPanelDescriptor::new("Panel C").with_dock_class("inspector"),
    );
    workspace
        .policy_mut()
        .allow_dock_class_in_space(main_space.clone(), "editor");
    let controller = cx.new(|_| DockController::new(workspace));
    let window = handle(46);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(detached_space.clone(), window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller.clone(),
        adapter,
        DockViewportClosePolicy::MergeBack {
            target_space: main_space.clone(),
        },
    );

    let should_close = cx.update(|app| {
        runtime
            .handle_window_should_close_with_app_and_refresh(window.window_id(), app)
            .0
    });
    assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);
    assert!(runtime.unregister_host_for_space(&detached_space, window.window_id()));
    runtime.register_opened_viewport(inspector_space.clone(), window);

    let closed = cx.update(|app| runtime.handle_window_closed_with_app(window.window_id(), app));

    assert_eq!(closed.status(), DockViewportCloseStatus::MergeBackFailed);
    assert_eq!(runtime.adapter().window_for_space(&inspector_space), None);
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&main_space),
            vec![item("b"), item("a")]
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&inspector_space),
            vec![item("c")]
        );
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
        selected: Some(item("b")),
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
fn viewport_runtime_window_closed_clears_live_window_diagnostics(cx: &mut TestAppContext) {
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
    let mut runtime = DockViewportRuntime::new(controller);
    let source_window = handle(50);
    let target_window = handle(51);
    runtime.register_opened_viewport(source_space.clone(), source_window);
    runtime.register_opened_viewport(target_space.clone(), target_window);

    let session = cache_known_viewport_preview_for_test(
        &mut runtime,
        source_space,
        source_tabs,
        &target_space,
        target_window,
        target_tabs,
        cx,
    );
    let status = runtime.runtime_status();
    assert!(
        matches!(
            status.last_route.as_ref().map(|record| &record.target),
            Some(DockViewportRouteTarget::KnownViewport { window_id, .. })
                if *window_id == target_window.window_id()
        ),
        "test setup should record a route into the target window"
    );
    assert!(
        runtime
            .routed_drop_delivery_for_drag_session(Some(&session))
            .is_some()
    );

    let outcome = runtime.handle_window_closed(target_window.window_id());

    assert_eq!(outcome.status(), DockViewportCloseStatus::Closed);
    assert_eq!(runtime.runtime_status().last_route, None);
    assert_eq!(
        runtime.routed_drop_delivery_for_drag_session(Some(&session)),
        None
    );
}

#[open_gpui::test]
fn viewport_runtime_window_closed_clears_host_scene_without_adapter_mapping(
    cx: &mut TestAppContext,
) {
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
    let mut runtime = DockViewportRuntime::new(controller);
    let target_window = handle(49);
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let host_position = center_drop_position(host_bounds);

    runtime.register_opened_viewport(target_space.clone(), target_window);
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
            100.0, 100.0, 360.0, 220.0,
        ))),
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_window.window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));
    assert!(
        cx.update(|app| runtime.resolve_host_scene_target(&target_space, host_position, app))
            .is_some(),
        "test setup should start with a resolvable current host scene"
    );

    runtime.unregister_adapter_window_for_test(target_window.window_id());
    assert!(
        cx.update(|app| runtime.resolve_host_scene_target(&target_space, host_position, app))
            .is_none(),
        "host scene target resolution must not bypass the runtime window mapping"
    );
    assert!(
        runtime
            .last_host_scene_screen_position(&target_space)
            .is_some(),
        "test setup should leave behind a host scene after the adapter mapping is gone"
    );
    let outcome = runtime.handle_window_closed(target_window.window_id());

    assert_eq!(outcome.status(), DockViewportCloseStatus::UnknownWindow);
    assert!(
        runtime
            .last_host_scene_screen_position(&target_space)
            .is_none(),
        "closed window notifications must clear host scenes even after adapter mapping is gone"
    );
}

#[open_gpui::test]
fn viewport_runtime_window_closed_clears_routed_preview(cx: &mut TestAppContext) {
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

    let target_window = handle(51);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(target_space.clone(), target_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller,
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

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
    let session = runtime.begin_payload_drag(&payload);
    let request = DockViewportDropRouteRequest::from_target_context(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_hovered_window(target_window),
    )
    .with_drag_session(Some(session.clone()));
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    let (changed, _) = runtime.update_routed_drop_preview(&resolution, "Panel A");
    assert!(changed);
    assert!(
        runtime
            .routed_drop_preview_for(&target_space, target_window.window_id())
            .is_some()
    );

    let outcome = runtime.handle_window_closed(target_window.window_id());
    assert_eq!(outcome.status(), DockViewportCloseStatus::Closed);
    assert_eq!(
        runtime.routed_drop_preview_for(&target_space, target_window.window_id()),
        None
    );
    assert_eq!(
        runtime.routed_drop_delivery_for_drag_session(Some(&session)),
        None
    );
}

#[open_gpui::test]
fn viewport_runtime_replacement_clears_routed_preview_for_old_window(cx: &mut TestAppContext) {
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

    let old_window = handle(61);
    let new_window = handle(62);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(target_space.clone(), old_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller,
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let session = cache_known_viewport_preview_for_test(
        &mut runtime,
        source_space,
        source_tabs,
        &target_space,
        old_window,
        target_tabs,
        cx,
    );

    runtime.register_opened_viewport(target_space.clone(), new_window);

    assert_eq!(
        runtime.routed_drop_preview_for(&target_space, old_window.window_id()),
        None
    );
    assert_eq!(
        runtime.routed_drop_delivery_for_drag_session(Some(&session)),
        None
    );
}

#[open_gpui::test]
fn viewport_runtime_window_closed_finishes_source_drag_session(cx: &mut TestAppContext) {
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
    let mut runtime = DockViewportRuntime::new(controller);
    let source_window = handle(41);
    let target_window = handle(42);
    runtime.register_opened_viewport(source_space.clone(), source_window);
    runtime.register_opened_viewport(target_space.clone(), target_window);

    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    let session = cache_known_viewport_preview_for_test(
        &mut runtime,
        source_space.clone(),
        source_tabs,
        &target_space,
        target_window,
        target_tabs,
        cx,
    );
    let geometry = DockDragTearOffGeometry::from_source_bounds(
        floating_bounds(10.0, 10.0, 120.0, 40.0),
        point(px(24.0), px(18.0)),
    );
    assert!(runtime.update_payload_drag_tear_off_geometry(&session, geometry));
    assert!(runtime.active_payload_drag_session(&payload).is_some());
    assert_eq!(
        runtime.active_payload_drag_tear_off_geometry(Some(&session)),
        Some(geometry)
    );
    assert!(
        runtime
            .routed_drop_delivery_for_drag_session(Some(&session))
            .is_some()
    );

    let outcome = runtime.handle_window_closed(source_window.window_id());

    assert_eq!(outcome.status(), DockViewportCloseStatus::Closed);
    assert_eq!(outcome.space(), Some(&source_space));
    assert_eq!(runtime.active_payload_drag_session(&payload), None);
    assert_eq!(
        runtime.active_payload_drag_tear_off_geometry(Some(&session)),
        None
    );
    assert_eq!(
        runtime.routed_drop_delivery_for_drag_session(Some(&session)),
        None
    );
}

#[open_gpui::test]
fn viewport_runtime_host_release_finishes_source_drag_session(cx: &mut TestAppContext) {
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
    let mut runtime = DockViewportRuntime::new(controller);
    let source_window = handle(51);
    let target_window = handle(52);
    runtime.register_opened_viewport(source_space.clone(), source_window);
    runtime.register_opened_viewport(target_space.clone(), target_window);

    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    let session = cache_known_viewport_preview_for_test(
        &mut runtime,
        source_space.clone(),
        source_tabs,
        &target_space,
        target_window,
        target_tabs,
        cx,
    );
    assert!(runtime.active_payload_drag_session(&payload).is_some());
    assert!(
        runtime
            .routed_drop_delivery_for_drag_session(Some(&session))
            .is_some()
    );

    assert!(runtime.unregister_host_for_space(&source_space, source_window.window_id()));

    assert_eq!(runtime.active_payload_drag_session(&payload), None);
    assert_eq!(
        runtime.routed_drop_delivery_for_drag_session(Some(&session)),
        None
    );
}

#[open_gpui::test]
fn viewport_runtime_reusable_stale_window_clears_routed_preview(cx: &mut TestAppContext) {
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

    let stale_window = handle(63);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(target_space.clone(), stale_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller,
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let session = cache_known_viewport_preview_for_test(
        &mut runtime,
        source_space,
        source_tabs,
        &target_space,
        stale_window,
        target_tabs,
        cx,
    );
    assert!(
        runtime
            .routed_drop_preview_for(&target_space, stale_window.window_id())
            .is_some()
    );

    cx.update(|app| {
        assert!(
            matches!(
                runtime.reusable_window_for_space(&target_space, app),
                crate::DockViewportReusableWindow::Stale
            ),
            "test handle should behave like a stale GPUI window"
        );
    });

    assert_eq!(
        runtime.routed_drop_preview_for(&target_space, stale_window.window_id()),
        None
    );
    assert_eq!(
        runtime.routed_drop_delivery_for_drag_session(Some(&session)),
        None
    );
}

#[open_gpui::test]
fn viewport_runtime_unregister_host_for_space_clears_runtime_state(cx: &mut TestAppContext) {
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

    let target_window = handle(93);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(target_space.clone(), target_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller,
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let session = cache_known_viewport_preview_for_test(
        &mut runtime,
        source_space,
        source_tabs,
        &target_space,
        target_window,
        target_tabs,
        cx,
    );
    assert!(
        runtime
            .last_host_scene_screen_position(&target_space)
            .is_some()
    );
    assert!(
        runtime
            .routed_drop_preview_for(&target_space, target_window.window_id())
            .is_some()
    );

    assert!(
        !runtime.unregister_host_for_space(&target_space, WindowId::from(999)),
        "release cleanup must not clear a space that has already rebound to another window"
    );
    assert_eq!(
        runtime.adapter().window_for_space(&target_space),
        Some(target_window)
    );
    assert!(
        runtime
            .routed_drop_delivery_for_drag_session(Some(&session))
            .is_some()
    );

    assert!(runtime.unregister_host_for_space(&target_space, target_window.window_id()));
    assert_eq!(runtime.adapter().window_for_space(&target_space), None);
    assert_eq!(runtime.last_host_scene_screen_position(&target_space), None);
    assert_eq!(
        runtime.routed_drop_preview_for(&target_space, target_window.window_id()),
        None
    );
    assert_eq!(
        runtime.routed_drop_delivery_for_drag_session(Some(&session)),
        None
    );
}

#[open_gpui::test]
fn viewport_runtime_rejects_stale_known_viewport_delivery_after_target_rebind(
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
    let host_position = center_drop_position(host_bounds);
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
        screen_position_for_host_position(window_bounds, host_position),
        None,
        DockViewportTargetContext::new().with_hovered_window(old_window),
    );
    let stale_delivery = cx
        .update(|app| runtime.resolve_payload_drop_delivery(&request, app))
        .expect_delivery()
        .clone();

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

    let result = cx.update(|app| runtime.deliver_payload_drop_with_outcome(stale_delivery, app));
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
    let host_position = center_drop_position(host_bounds);
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
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert!(
        matches!(resolution.route(), DockViewportDropRoute::KnownViewport { target, .. }
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
        runtime.deliver_payload_drop_with_outcome(resolution.expect_delivery().clone(), app)
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
fn viewport_runtime_rejects_resolved_target_snapshot_after_window_facts_go_stale(
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

    let target_window = handle(29);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(target_space.clone(), target_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller.clone(),
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

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
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert!(
        matches!(resolution.route(), DockViewportDropRoute::KnownViewport { target, .. }
            if target.window_id() == target_window.window_id()),
        "fresh viewport facts should produce a known viewport route"
    );
    assert!(
        resolution
            .expect_delivery()
            .routed_preview_target()
            .is_some(),
        "fresh route should capture the resolved host scene target"
    );

    let (changed, _) = runtime.mark_viewport_window_snapshot_stale(target_window.window_id());
    assert!(changed);
    let result = cx.update(|app| {
        runtime.deliver_payload_drop_with_outcome(resolution.expect_delivery().clone(), app)
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
fn viewport_runtime_rejects_local_geometry_route_without_platform_authority(
    cx: &mut TestAppContext,
) {
    let source_space = DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: crate::SplitAxis::Horizontal,
        children: vec![source_tabs, target_tabs],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(source_space.clone(), root);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let mut runtime = DockViewportRuntime::new(controller);
    let source_window = handle(33);
    let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let host_position = center_drop_position(host_bounds);

    runtime.register_opened_viewport(source_space.clone(), source_window);
    assert!(runtime.begin_viewport_host_scene(
        source_space.clone(),
        source_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(window_bounds),
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &source_space,
        source_window.window_id(),
        leaf_host_scene_fact(root, target_tabs),
    ));

    let request = DockViewportDropRouteRequest::from_target_context(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        screen_position_for_host_position(window_bounds, host_position),
        None,
        DockViewportTargetContext::new(),
    );
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));

    assert_eq!(resolution.route(), &DockViewportDropRoute::Unavailable);
    assert!(resolution.delivery().is_none());
}

#[open_gpui::test]
fn viewport_runtime_rejects_cached_local_delivery_after_window_facts_go_stale(
    cx: &mut TestAppContext,
) {
    let source_space = DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: crate::SplitAxis::Horizontal,
        children: vec![source_tabs, target_tabs],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(source_space.clone(), root);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));

    let source_window = handle(32);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(source_space.clone(), source_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller.clone(),
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let host_position = center_drop_position(host_bounds);
    assert!(runtime.begin_viewport_host_scene(
        source_space.clone(),
        source_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(window_bounds),
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &source_space,
        source_window.window_id(),
        leaf_host_scene_fact(root, target_tabs),
    ));

    let request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        screen_position_for_host_position(window_bounds, host_position),
        None,
        DockViewportTargetContext::new().with_hovered_window(source_window),
    );
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert!(
        matches!(resolution.route(), DockViewportDropRoute::Local { .. }),
        "fresh source viewport facts should resolve a local route, got {:?}",
        resolution.route()
    );
    let delivery = resolution.expect_delivery().clone();

    let (changed, _) = runtime.mark_viewport_window_snapshot_stale(source_window.window_id());
    assert!(changed);

    let validation = cx.update(|app| runtime.validate_payload_drop_delivery(&delivery, app));
    assert_eq!(validation, Err(DockActionApplyError::DropTargetUnavailable));
    let result = cx.update(|app| runtime.deliver_payload_drop_with_outcome(delivery, app));
    assert_eq!(result, Err(DockActionApplyError::DropTargetUnavailable));
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("a"), item("b")]
        );
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(source_tabs)
            .expect("source tabs should still exist")
        else {
            panic!("source should remain tabs");
        };
        assert_eq!(items, &vec![item("a")]);
        assert_eq!(selected.as_ref(), items.first());
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should still exist")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("b")]);
        assert_eq!(selected.as_ref(), items.first());
    });
}

#[open_gpui::test]
fn viewport_runtime_rejects_host_scene_resolution_after_window_facts_go_stale(
    cx: &mut TestAppContext,
) {
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
    let mut runtime = DockViewportRuntime::new(controller);
    let target_window = handle(31);
    let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let host_position = center_drop_position(host_bounds);

    runtime.register_opened_viewport(target_space.clone(), target_window);
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
    assert!(runtime.viewport_route_ready(&target_space));
    assert!(
        cx.update(|app| runtime.resolve_host_scene_target(&target_space, host_position, app))
            .is_some(),
        "fresh viewport facts should allow host scene target resolution"
    );

    let (changed, _) = runtime.mark_viewport_window_snapshot_stale(target_window.window_id());
    assert!(changed);
    assert!(!runtime.viewport_route_ready(&target_space));
    assert!(
        runtime
            .last_host_scene_screen_position(&target_space)
            .is_some(),
        "stale window facts should not delete the last rendered scene"
    );
    assert!(
        cx.update(|app| runtime.resolve_host_scene_target(&target_space, host_position, app))
            .is_none(),
        "stale window facts must block direct host scene target resolution"
    );

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
    assert!(
        cx.update(|app| runtime.resolve_host_scene_target(&target_space, host_position, app))
            .is_some(),
        "the next rendered host-scene frame should restore resolution"
    );
}

#[open_gpui::test]
fn viewport_runtime_known_viewport_without_scene_is_unavailable(cx: &mut TestAppContext) {
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

    let target_window = handle(31);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(target_space.clone(), target_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller.clone(),
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(window_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        point(px(120.0), px(100.0)),
    ));

    let request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_hovered_window(target_window),
    );
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert_eq!(
        resolution.route(),
        &DockViewportDropRoute::Unavailable,
        "viewport hit without a current host scene target should be unavailable"
    );
    assert!(
        resolution.delivery().is_none(),
        "unavailable route must not carry a delivery"
    );
    let (changed, windows) = runtime.update_routed_drop_preview(&resolution, "Panel A");
    assert!(!changed);
    assert!(windows.is_empty());

    let result = cx.update(|app| {
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
fn viewport_runtime_revalidates_resolved_target_snapshot_against_current_policy(
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
    let host_position = center_drop_position(host_bounds);
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
        screen_position_for_host_position(window_bounds, host_position),
        None,
        DockViewportTargetContext::new().with_hovered_window(target_window),
    );
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert!(
        resolution
            .expect_delivery()
            .routed_preview_target()
            .is_some(),
        "preview should capture the accepted central target"
    );
    let (_, _, resolved_target) = resolution
        .expect_delivery()
        .routed_preview_target()
        .expect("preview target should be captured");
    assert!(
        matches!(
            resolved_target.kind,
            crate::drop_target::DockResolvedDropTargetKind::LeafCenter { .. }
        ),
        "resolved target snapshot should be the central leaf body, got {resolved_target:?}"
    );
    assert!(
        resolved_target.is_central_region,
        "resolved target snapshot should retain the central-region marker"
    );

    controller.update(cx, |controller, _| {
        controller
            .policy_mut()
            .set_allow_central_region_dock_over(false);
    });

    let result = cx.update(|app| {
        runtime.deliver_payload_drop_with_outcome(resolution.expect_delivery().clone(), app)
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
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
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
    let host_position = center_drop_position(host_bounds);
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
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert!(
        matches!(
            resolution.route(),
            DockViewportDropRoute::Rejected(DockPolicyError::DockClassRejected { .. })
        ),
        "policy-rejected cross-viewport targets should render as rejected routes"
    );
    assert!(
        resolution.delivery().is_none(),
        "policy-rejected cross-viewport targets must not carry a delivery"
    );

    let result = cx.update(|app| {
        resolution
            .delivery_result()
            .cloned()
            .and_then(|delivery| runtime.deliver_payload_drop_with_outcome(delivery, app))
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
fn viewport_runtime_source_only_release_does_not_use_last_hovered_viewport(
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
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
    cx.update(|app| {
        runtime.update_routed_drop_preview(&resolution, "Panel A", app);
    });
    assert!(
        runtime
            .routed_drop_preview_for(&target_space, target_opened.window().window_id())
            .is_some(),
        "preview should store routed delivery for the hovered target"
    );

    let release_position = point(px(220.0), px(200.0));
    let request_without_hovered = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        release_position,
        None,
        DockViewportTargetContext::new()
            .with_window_stack([source_opened.window(), target_opened.window()]),
    );
    let baseline_route = cx.update(|app| {
        runtime
            .resolve_payload_drop_delivery(&request_without_hovered, app)
            .route()
            .clone()
    });
    assert_eq!(
        baseline_route,
        DockViewportDropRoute::Unavailable,
        "window-stack fallback must not authorize a release without current hover authority"
    );

    let request_with_hovered = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        release_position,
        None,
        DockViewportTargetContext::new()
            .with_hovered_window(target_opened.window())
            .with_window_stack([source_opened.window(), target_opened.window()]),
    );
    let hovered_route = cx.update(|app| {
        runtime
            .resolve_payload_drop_delivery(&request_with_hovered, app)
            .route()
            .clone()
    });
    assert!(
        matches!(
            hovered_route,
            DockViewportDropRoute::KnownViewport { target, .. }
                if target.window_id() == target_opened.window().window_id()
        ),
        "a current hovered signal should still authorize the target viewport"
    );

    cx.update(|app| {
        assert!(runtime.clear_routed_drop_preview(app));
    });
    assert_eq!(
        runtime.routed_drop_preview_for(&target_space, target_opened.window().window_id()),
        None
    );

    let source_only_request = DockViewportDropRouteRequest::from_platform_signals(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        release_position,
        None,
        crate::DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new()
                .with_window_stack([source_opened.window(), target_opened.window()]),
        ),
    );
    let source_only_route = cx.update(|app| {
        runtime
            .resolve_payload_drop_delivery(&source_only_request, app)
            .route()
            .clone()
    });
    assert_eq!(
        source_only_route, baseline_route,
        "source-only release must not promote last-hovered preview state into current hover authority"
    );
}

#[open_gpui::test]
fn viewport_runtime_source_only_release_retargets_current_position(cx: &mut TestAppContext) {
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
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(floating_bounds(
                        0.0, 0.0, 360.0, 220.0,
                    ))),
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
                    window_bounds: Some(WindowBounds::Windowed(floating_bounds(
                        100.0, 100.0, 360.0, 220.0,
                    ))),
                    ..Default::default()
                },
                app,
            )
        })
        .expect("target viewport should open");

    let target_window_bounds = target_opened
        .window()
        .update(cx, |_, window, _| window.window_bounds())
        .expect("target window should be live");
    let target_window_bounds = WindowBounds::Windowed(target_window_bounds.get_bounds());
    let target_leaf_bounds = floating_bounds(0.0, 0.0, 180.0, 120.0);
    let preview_host_position = center_drop_position(target_leaf_bounds);
    let preview_screen_position = point(
        target_window_bounds.get_bounds().origin.x + preview_host_position.x,
        target_window_bounds.get_bounds().origin.y + preview_host_position.y,
    );
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(target_window_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        preview_host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_opened.window().window_id(),
        DockHostDropSceneFact::Leaf(DockLeafDropTarget {
            root: target_tabs,
            target_tabs,
            bounds: target_leaf_bounds,
            is_central: false,
        }),
    ));

    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    let session = runtime.begin_payload_drag(&payload);
    let preview_request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        preview_screen_position,
        None,
        DockViewportTargetContext::new().with_hovered_window(target_opened.window()),
    )
    .with_drag_session(Some(session.clone()));
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
    cx.update(|app| {
        runtime.update_routed_drop_preview(&resolution, "Panel A", app);
    });
    assert!(
        runtime
            .routed_drop_delivery_for_drag_session(Some(&session))
            .is_some(),
        "preview should cache a routed delivery before release"
    );
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(target_window_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        preview_host_position,
    ));

    let release_request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        screen_position_for_host_position(
            target_window_bounds,
            center_drop_position(floating_bounds(180.0, 120.0, 180.0, 100.0)),
        ),
        None,
        DockViewportTargetContext::new()
            .with_window_stack([target_opened.window(), source_opened.window()]),
    )
    .with_drag_session(Some(session.clone()));
    let release_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&release_request, app));
    assert_eq!(
        release_resolution.route(),
        &DockViewportDropRoute::Unavailable,
        "release should be retargeted to the current point instead of reusing cached host_position"
    );
    let result = cx.update(|app| {
        release_resolution
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
fn viewport_runtime_scopes_routed_preview_delivery_to_drag_session(cx: &mut TestAppContext) {
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
    let target_window = handle(77);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(target_space.clone(), target_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller,
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let target_position = center_drop_position(host_bounds);
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
            100.0, 100.0, 360.0, 220.0,
        ))),
        host_bounds,
        target_position,
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
        "A".to_string(),
    );
    let session = runtime.begin_payload_drag(&payload);
    let request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_hovered_window(target_window),
    )
    .with_drag_session(Some(session.clone()));
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert!(
        matches!(
            resolution.route(),
            DockViewportDropRoute::KnownViewport { target, .. }
                if target.window_id() == target_window.window_id()
        ),
        "preview setup should resolve a known target viewport"
    );
    let delivery = resolution.expect_delivery().clone();

    runtime.update_routed_drop_preview(&resolution, "Panel A");
    assert_eq!(
        runtime.routed_drop_delivery_for_drag_session(Some(&session)),
        Some(delivery.clone())
    );
    assert_eq!(runtime.routed_drop_delivery_for_drag_session(None), None);

    let local_resolution = DockViewportResolvedDropRoute::new(
        DockViewportDropRoute::Local {
            host_position: target_position,
            authority: crate::DockViewportRouteAuthority::TrustedHoveredWindow,
        },
        None,
    );
    runtime.update_routed_drop_preview(&local_resolution, "Panel A");
    assert_eq!(
        runtime.routed_drop_delivery_for_drag_session(Some(&session)),
        None
    );

    runtime.update_routed_drop_preview(&resolution, "Panel A");
    let (changed, _) = runtime.finish_payload_drag(&session);
    assert!(changed);
    assert_eq!(
        runtime.routed_drop_delivery_for_drag_session(Some(&session)),
        None
    );

    let next_session = runtime.begin_payload_drag(&payload);
    assert_ne!(next_session.id(), session.id());
    let next_request = DockViewportDropRouteRequest::from_target_context(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_hovered_window(target_window),
    )
    .with_drag_session(Some(next_session.clone()));
    let next_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&next_request, app));
    let next_delivery = next_resolution.expect_delivery().clone();
    runtime.update_routed_drop_preview(&next_resolution, "Panel A");
    assert_eq!(
        runtime.routed_drop_delivery_for_drag_session(Some(&next_session)),
        Some(next_delivery)
    );
    assert_eq!(
        runtime.routed_drop_delivery_for_drag_session(Some(&session)),
        None
    );
}

#[open_gpui::test]
fn viewport_runtime_begin_payload_drag_clears_previous_routed_preview(cx: &mut TestAppContext) {
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
    let target_window = handle(78);
    let mut adapter = DockViewportAdapter::new();
    adapter.register_viewport(target_space.clone(), target_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller,
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let target_position = center_drop_position(host_bounds);
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
            100.0, 100.0, 360.0, 220.0,
        ))),
        host_bounds,
        target_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_window.window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));

    let first_payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "A".to_string(),
    );
    let first_session = runtime.begin_payload_drag(&first_payload);
    let request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_hovered_window(target_window),
    )
    .with_drag_session(Some(first_session.clone()));
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    runtime.update_routed_drop_preview(&resolution, "Panel A");
    assert!(
        runtime
            .routed_drop_preview_for(&target_space, target_window.window_id())
            .is_some()
    );
    assert!(
        runtime
            .routed_drop_delivery_for_drag_session(Some(&first_session))
            .is_some()
    );

    let second_payload =
        DockDragPayload::new_item(source_space, source_tabs, item("c"), "C".to_string());
    let second_session = runtime.begin_payload_drag(&second_payload);
    assert_ne!(second_session.id(), first_session.id());
    assert_eq!(
        runtime.routed_drop_preview_for(&target_space, target_window.window_id()),
        None
    );
    assert_eq!(
        runtime.routed_drop_delivery_for_drag_session(Some(&first_session)),
        None
    );
}

#[open_gpui::test]
fn viewport_runtime_rejects_known_viewport_delivery_from_stale_drag_session(
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
    let host_position = center_drop_position(host_bounds);
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
        screen_position_for_host_position(window_bounds, host_position),
        None,
        DockViewportTargetContext::new().with_hovered_window(target_window),
    )
    .with_drag_session(Some(stale_session.clone()));
    let stale_delivery = cx
        .update(|app| runtime.resolve_payload_drop_delivery(&request, app))
        .expect_delivery()
        .clone();

    let _replacement = runtime.begin_payload_drag(&payload);
    let result = cx.update(|app| runtime.deliver_payload_drop_with_outcome(stale_delivery, app));
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
fn viewport_runtime_rejects_tear_off_delivery_from_stale_drag_session(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
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

    let result = cx.update(|app| runtime.prepare_tear_off_drop_delivery(request, app));
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
fn viewport_runtime_drag_geometry_is_bound_to_active_drag_session(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(source_space.clone(), source_tabs);
    let workspace = DockWorkspace::new(source_space.clone(), graph);
    let controller = cx.new(|_| DockController::new(workspace));
    let mut runtime = DockViewportRuntime::new(controller);
    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "A".to_string(),
    );
    let geometry = DockDragTearOffGeometry::from_source_bounds(
        floating_bounds(200.0, 120.0, 480.0, 300.0),
        point(px(260.0), px(150.0)),
    );

    let stale_session = runtime.begin_payload_drag(&payload);
    assert!(runtime.update_payload_drag_tear_off_geometry(&stale_session, geometry));
    assert_eq!(
        runtime.active_payload_drag_tear_off_geometry(Some(&stale_session)),
        Some(geometry)
    );

    let active_session = runtime.begin_payload_drag(&payload);
    assert_eq!(
        runtime.active_payload_drag_tear_off_geometry(Some(&stale_session)),
        None,
        "starting a new drag must not expose the previous session's source geometry"
    );
    assert_eq!(
        runtime.active_payload_drag_tear_off_geometry(Some(&active_session)),
        None
    );
    assert!(
        !runtime.update_payload_drag_tear_off_geometry(&stale_session, geometry),
        "stale drag sessions must not update tear-off geometry"
    );
    assert!(runtime.update_payload_drag_tear_off_geometry(&active_session, geometry));
    let (changed, _) = runtime.finish_payload_drag(&active_session);
    assert!(changed);
    assert_eq!(
        runtime.active_payload_drag_tear_off_geometry(Some(&active_session)),
        None,
        "finishing a drag must discard its geometry"
    );
}

#[open_gpui::test]
fn viewport_runtime_tear_off_bounds_fallback_is_marked_degraded(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
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

    let placement = runtime.tear_off_window_placement(&request);
    assert_eq!(
        placement.source(),
        DockViewportTearOffPlacementSource::Fallback
    );
    assert_eq!(
        placement.window_bounds(),
        WindowBounds::Windowed(floating_bounds(876.0, 882.0, 360.0, 240.0))
    );
    if let WindowBounds::Windowed(bounds) = placement.window_bounds() {
        assert!(bounds.contains(&release_position));
        assert_ne!(bounds.origin, release_position);
    } else {
        panic!("tear-off should use windowed bounds");
    }
}

#[open_gpui::test]
fn viewport_runtime_tear_off_bounds_preserve_drag_cursor_offset(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(source_space.clone(), source_tabs);

    let workspace = DockWorkspace::new(source_space.clone(), graph);
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntime::new(controller);
    let release_position = point(px(900.0), px(900.0));
    let geometry = DockDragTearOffGeometry::from_source_bounds(
        floating_bounds(200.0, 120.0, 480.0, 300.0),
        point(px(260.0), px(150.0)),
    );
    let request = DockViewportTearOffRequest::new(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        release_position,
        None,
    )
    .with_tear_off_geometry(Some(geometry));

    let placement = runtime.tear_off_window_placement(&request);
    assert_eq!(
        placement.source(),
        DockViewportTearOffPlacementSource::DragGeometry
    );
    assert_eq!(
        placement.window_bounds(),
        WindowBounds::Windowed(floating_bounds(840.0, 870.0, 480.0, 300.0))
    );
}

#[open_gpui::test]
fn viewport_runtime_tear_off_suggested_bounds_override_drag_geometry(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(source_space.clone(), source_tabs);

    let workspace = DockWorkspace::new(source_space.clone(), graph);
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntime::new(controller);
    let suggested = WindowBounds::Windowed(floating_bounds(700.0, 710.0, 420.0, 260.0));
    let geometry = DockDragTearOffGeometry::from_source_bounds(
        floating_bounds(200.0, 120.0, 480.0, 300.0),
        point(px(260.0), px(150.0)),
    );
    let request = DockViewportTearOffRequest::new(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(900.0), px(900.0)),
        Some(suggested),
    )
    .with_tear_off_geometry(Some(geometry));

    let placement = runtime.tear_off_window_placement(&request);
    assert_eq!(
        placement.source(),
        DockViewportTearOffPlacementSource::Suggested
    );
    assert_eq!(placement.window_bounds(), suggested);
}

#[open_gpui::test]
fn viewport_runtime_tear_off_drag_bounds_clamp_to_work_area(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(source_space.clone(), source_tabs);

    let workspace = DockWorkspace::new(source_space.clone(), graph);
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntime::new(controller);
    let geometry = DockDragTearOffGeometry::from_source_bounds(
        floating_bounds(200.0, 120.0, 480.0, 300.0),
        point(px(260.0), px(150.0)),
    )
    .with_display_work_area(floating_bounds(0.0, 0.0, 1000.0, 800.0));
    let request = DockViewportTearOffRequest::new(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(980.0), px(790.0)),
        None,
    )
    .with_tear_off_geometry(Some(geometry));

    let placement = runtime.tear_off_window_placement(&request);
    assert_eq!(
        placement.source(),
        DockViewportTearOffPlacementSource::DragGeometry
    );
    assert_eq!(
        placement.window_bounds(),
        WindowBounds::Windowed(floating_bounds(520.0, 500.0, 480.0, 300.0))
    );
}
