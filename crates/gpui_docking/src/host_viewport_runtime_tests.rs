use crate::{
    DockActionApplyError, DockActionOutcome, DockClassId, DockController, DockDropDelivery,
    DockFloatingContainer, DockGraph, DockHost, DockItemId, DockNode, DockPanel, DockPolicyError,
    DockSpaceId, DockViewportAdapter, DockViewportClosePolicy, DockViewportCloseStatus,
    DockViewportDropPayload, DockViewportDropRoute, DockViewportDropRouteOutcome,
    DockViewportDropRouteRequest, DockViewportFocusCommand, DockViewportFocusRequest,
    DockViewportInputStatus, DockViewportOpenStatus, DockViewportPlatformSyncAction,
    DockViewportPlatformSyncRequest, DockViewportPlatformSyncSkippedReason,
    DockViewportResolvedDropRoute, DockViewportRouteStatus, DockViewportRouteTarget,
    DockViewportRuntime, DockViewportRuntimeHandle, DockViewportShouldCloseStatus,
    DockViewportTargetContext, DockViewportTearOffOpenOutcome, DockViewportTearOffOutcomeKind,
    DockViewportTearOffPlacementSource, DockViewportTearOffRequest, DockViewportWindowActivation,
    DockViewportWindowFacts, DockWorkspace, SplitAxis,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    drop_runtime::DockHostDropSceneFact,
    drop_target::DockLeafDropTarget,
    host_test_support::*,
    interaction::DockPayloadDropReleaseOrigin,
    viewport_activation::{
        DockViewportActivationApplyOutcome, apply_viewport_activation_transaction,
    },
    viewport_registry::{
        DockViewportInputMask, DockViewportRouteUnavailableReason, DockViewportStaleReason,
    },
    viewport_tear_off::{
        DockViewportTearOffBeginOutcome, DockViewportTearOffCancelReason, DockViewportTearOffTick,
    },
    viewport_test_support::{handle, register_viewport},
};
use open_gpui::{
    AnyWindowHandle, AppContext as _, Focusable, SharedString, TestAppContext, TitlebarOptions,
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

fn viewport_input_status(
    runtime: &DockViewportRuntimeHandle,
    space: &DockSpaceId,
) -> Option<DockViewportInputStatus> {
    runtime
        .runtime_status()
        .viewport_lifecycle
        .iter()
        .find(|record| &record.space == space)
        .map(|record| record.input_status)
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

fn freeze_should_close_plan(
    cx: &mut TestAppContext,
    runtime: &DockViewportRuntimeHandle,
    window_id: WindowId,
) {
    let should_close = cx.update(|app| runtime.handle_window_should_close_with_app(window_id, app));
    assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);
}

#[open_gpui::test]
fn viewport_window_facts_report_native_no_input_windows(cx: &mut TestAppContext) {
    let root = test_view(cx, "A");
    let window = cx
        .update(|app| {
            app.open_window(
                WindowOptions {
                    accepts_pointer_input: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                |_, _| root.clone(),
            )
        })
        .expect("no-input test window should open");

    window
        .update(cx, |_, window, app| {
            assert!(!window.accepts_pointer_input());
            assert_eq!(
                DockViewportWindowFacts::from_window(window, app).input_mask,
                DockViewportInputMask::NoInputPassThrough
            );
        })
        .expect("no-input test window should remain live");
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
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
    )
    .with_drag_session(Some(session.clone()));
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    let (changed, _) = runtime.update_routed_drop_preview(&resolution, "Panel A");
    assert!(changed);
    assert!(runtime.finish_routed_drop_acceptance_pass(target_space, target_window.window_id()));

    session
}

fn accepted_preview_delivery_for_test(
    runtime: &mut DockViewportRuntime,
    request: &DockViewportDropRouteRequest,
    target_space: &DockSpaceId,
    target_window: AnyWindowHandle,
    cx: &mut TestAppContext,
) -> DockViewportResolvedDropRoute {
    let preview_resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(request, app));
    let (changed, _) = runtime.update_routed_drop_preview(&preview_resolution, "Panel A");
    assert!(changed, "preview route should publish a routed preview");
    assert!(
        runtime.finish_routed_drop_acceptance_pass(target_space, target_window.window_id()),
        "target viewport should accept the routed preview"
    );
    cx.update(|app| runtime.resolve_payload_drop_delivery(request, app))
}

fn close_window_quietly_for_test(window: AnyWindowHandle, cx: &mut TestAppContext) {
    let _ = window.update(cx, |_, window, _| window.remove_window());
}

fn focus_backend_window_for_test(window: AnyWindowHandle, cx: &mut TestAppContext) {
    window
        .update(cx, |_, window, _| window.activate_window())
        .expect("test viewport should activate");
}

#[open_gpui::test]
fn viewport_runtime_drag_restores_original_no_input_source_state(cx: &mut TestAppContext) {
    let source = DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("drag")],
        selected: Some(item("drag")),
    });
    graph.set_root(source.clone(), source_tabs);
    let window = handle(1);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, source.clone(), window);
    adapter.update_snapshot(
        &source,
        DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
            0.0, 0.0, 360.0, 220.0,
        )))
        .with_input_mask(DockViewportInputMask::NoInputPassThrough),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
    );
    let controller = cx.new(|_| DockController::new(DockWorkspace::new(source.clone(), graph)));
    let mut runtime =
        DockViewportRuntime::from_adapter(controller, adapter, DockViewportClosePolicy::default());
    let payload = DockDragPayload::new_item(source, source_tabs, item("drag"), "Drag".to_string());

    let (session, begin_sync) =
        runtime.begin_payload_drag_with_pointer_sync_and_focus(&payload, None);
    assert_eq!(
        begin_sync.map(|request| request.requested_accepts_pointer_input()),
        None,
        "an already no-input source window should not be re-requested as click-through"
    );

    let (_, _, finish_sync) = runtime.finish_payload_drag_with_pointer_sync(&session);
    assert_eq!(
        finish_sync.map(|request| (request.window(), request.requested_accepts_pointer_input())),
        Some((window, false)),
        "drag finish should restore the source window's original no-input state"
    );
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
                        0.0, 0.0, 480.0, 260.0,
                    ))),
                    accepts_pointer_input: false,
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
        "same-origin reuse should preserve the live screen origin"
    );
    assert!(
        !reused
            .window()
            .update(cx, |_, window, _| window.accepts_pointer_input())
            .expect("reused viewport should remain live"),
        "reused viewport sync should apply native no-input/click-through state"
    );
    assert_eq!(
        runtime.viewport_route_unavailable_reason(&secondary_space),
        None,
        "native no-input should not invalidate route facts"
    );
    assert_eq!(
        viewport_input_status(&runtime, &secondary_space),
        Some(DockViewportInputStatus::NoInputPassThrough),
        "runtime registry must observe the reused window's live no-input state"
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
    assert!(
        !sync.unsupported_requests.iter().any(|unsupported| matches!(
            unsupported.request,
            DockViewportPlatformSyncRequest::WindowOrigin { .. }
        ))
    );
    assert!(
        sync.applied
            .contains(&DockViewportPlatformSyncAction::PointerInput { enabled: false })
    );
    assert!(!sync.unsupported_requests.iter().any(|unsupported| {
        unsupported.request == DockViewportPlatformSyncRequest::PointerInput { requested: false }
    }));
}

#[open_gpui::test]
fn viewport_runtime_does_not_reverse_sync_size_during_platform_resize(cx: &mut TestAppContext) {
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
                    ..Default::default()
                },
                app,
            )
        })
        .expect("secondary viewport should open through runtime");
    assert!(runtime.begin_viewport_host_scene(
        secondary_space.clone(),
        opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
            0.0, 0.0, 360.0, 220.0,
        ))),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        point(px(180.0), px(110.0)),
    ));
    assert_eq!(
        runtime.viewport_route_unavailable_reason(&secondary_space),
        None
    );

    opened
        .window()
        .update(cx, |_, window, _| {
            window.resize(size(px(520.0), px(300.0)));
        })
        .expect("test viewport window should remain live");
    let platform_facts_applied = cx.update(|app| {
        runtime.apply_platform_window_facts(
            opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
                0.0, 0.0, 520.0, 300.0,
            ))),
            app,
        )
    });
    assert!(
        platform_facts_applied,
        "backend resize facts should update the viewport runtime"
    );
    assert_eq!(
        runtime.viewport_route_unavailable_reason(&secondary_space),
        Some(DockViewportRouteUnavailableReason::Stale(
            DockViewportStaleReason::WindowFactsChanged
        )),
        "platform resize must wait for a fresh host scene before routing again"
    );

    let reused = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(floating_bounds(
                        0.0, 0.0, 360.0, 220.0,
                    ))),
                    ..Default::default()
                },
                app,
            )
        })
        .expect("live viewport should be reused while resize request is pending");

    assert_eq!(reused.status(), DockViewportOpenStatus::Reused);
    assert_eq!(reused.window(), opened.window());
    let bounds = reused
        .window()
        .update(cx, |_, window, _| window.bounds())
        .expect("reused viewport should remain live");
    assert_eq!(
        bounds.size,
        size(px(520.0), px(300.0)),
        "runtime sync must not overwrite an in-flight platform resize"
    );

    let sync = runtime
        .runtime_status()
        .last_platform_sync
        .expect("reuse should record platform sync diagnostics");
    assert!(
        !sync
            .applied
            .iter()
            .any(|action| matches!(action, DockViewportPlatformSyncAction::Resize { .. })),
        "reverse resize must be skipped while backend resize request is pending"
    );
    assert!(sync.skipped_requests.iter().any(|skipped| {
        skipped.reason == DockViewportPlatformSyncSkippedReason::PlatformRequestInProgress
            && matches!(
                &skipped.request,
                DockViewportPlatformSyncRequest::WindowSize { requested }
                    if *requested == size(px(360.0), px(220.0))
            )
    }));

    assert!(runtime.begin_viewport_host_scene(
        secondary_space.clone(),
        reused.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
            0.0, 0.0, 520.0, 300.0,
        ))),
        floating_bounds(0.0, 0.0, 520.0, 300.0),
        point(px(260.0), px(150.0)),
    ));

    let resized_after_fresh_scene = cx
        .update(|app| {
            runtime.open_viewport(
                secondary_space.clone(),
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(floating_bounds(
                        0.0, 0.0, 360.0, 220.0,
                    ))),
                    ..Default::default()
                },
                app,
            )
        })
        .expect("fresh host scene should consume the platform resize request");
    assert_eq!(
        resized_after_fresh_scene.status(),
        DockViewportOpenStatus::Reused
    );
    assert_eq!(
        resized_after_fresh_scene
            .window()
            .update(cx, |_, window, _| window.bounds().size)
            .expect("reused viewport should remain live"),
        size(px(360.0), px(220.0)),
        "after a fresh host scene, programmatic viewport resize can apply again"
    );
    let sync = runtime
        .runtime_status()
        .last_platform_sync
        .expect("second reuse should record platform sync diagnostics");
    assert!(sync.skipped_requests.is_empty());
    assert!(
        sync.applied
            .contains(&DockViewportPlatformSyncAction::Resize {
                size: size(px(360.0), px(220.0)),
            })
    );
}

#[open_gpui::test]
fn viewport_runtime_reuses_window_and_records_origin_sync_diagnostics(cx: &mut TestAppContext) {
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
                    ..Default::default()
                },
                app,
            )
        })
        .expect("origin-changing reopen should reuse the viewport window");

    assert_eq!(reused.status(), DockViewportOpenStatus::Reused);
    assert_eq!(reused.window(), opened.window());
    assert_eq!(
        runtime
            .borrow()
            .adapter()
            .window_for_space(&secondary_space),
        Some(reused.window())
    );
    assert!(
        reused
            .window()
            .update(cx, |_, window, _| window.bounds().size)
            .is_ok(),
        "reused viewport window should remain live"
    );
    let sync = runtime
        .runtime_status()
        .last_platform_sync
        .expect("reuse should record platform sync diagnostics");
    assert!(
        sync.unsupported_requests.iter().any(|unsupported| matches!(
            unsupported.request,
            DockViewportPlatformSyncRequest::WindowOrigin { .. }
        )),
        "origin mismatch should be recorded as unsupported sync, not a replacement trigger"
    );
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
fn viewport_runtime_render_registered_viewport_records_window_binding(cx: &mut TestAppContext) {
    let alpha_space = DockSpaceId::from("alpha");
    let zeta_space = DockSpaceId::from("zeta");
    let mut graph = DockGraph::new();
    let alpha_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let zeta_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("z")],
        selected: Some(item("z")),
    });
    graph.set_root(alpha_space.clone(), alpha_tabs);
    graph.set_root(zeta_space.clone(), zeta_tabs);

    let mut workspace = DockWorkspace::new(alpha_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("z"), "Panel Z", test_view(cx, "Z"));
    let controller = cx.new(|_| DockController::new(workspace));
    let mut runtime = DockViewportRuntime::new(controller);
    let alpha_window = handle(1);
    let zeta_window = handle(2);

    assert!(runtime.register_rendered_host_viewport(alpha_space.clone(), alpha_window));
    assert!(runtime.register_rendered_host_viewport(zeta_space.clone(), zeta_window));

    assert_eq!(
        runtime.adapter().window_for_space(&alpha_space),
        Some(alpha_window)
    );
    assert_eq!(
        runtime.adapter().window_for_space(&zeta_space),
        Some(zeta_window)
    );
}

#[open_gpui::test]
fn viewport_runtime_render_registration_cleans_replaced_space_state(cx: &mut TestAppContext) {
    let alpha_space = DockSpaceId::from("alpha");
    let zeta_space = DockSpaceId::from("zeta");
    let mut graph = DockGraph::new();
    let alpha_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let zeta_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("z")],
        selected: Some(item("z")),
    });
    graph.set_root(alpha_space.clone(), alpha_tabs);
    graph.set_root(zeta_space.clone(), zeta_tabs);

    let mut workspace = DockWorkspace::new(alpha_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("z"), "Panel Z", test_view(cx, "Z"));
    let controller = cx.new(|_| DockController::new(workspace));
    let window = handle(3);
    let mut runtime = DockViewportRuntime::new(controller);

    assert!(runtime.register_rendered_host_viewport(alpha_space.clone(), window));
    assert!(runtime.begin_viewport_host_scene(
        alpha_space.clone(),
        window.window_id(),
        DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
            100.0, 100.0, 360.0, 220.0,
        ))),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        point(px(120.0), px(100.0)),
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &alpha_space,
        window.window_id(),
        leaf_host_scene_fact(alpha_tabs, alpha_tabs),
    ));
    runtime.record_panel_focus(alpha_space.clone(), item("a"));

    assert!(
        runtime
            .last_host_scene_screen_position(&alpha_space)
            .is_some()
    );
    assert_eq!(
        runtime.recorded_had_panel_focus_for_test(&alpha_space),
        Some(true)
    );

    assert!(runtime.register_rendered_host_viewport(zeta_space.clone(), window));

    assert_eq!(runtime.adapter().window_for_space(&alpha_space), None);
    assert_eq!(
        runtime.adapter().window_for_space(&zeta_space),
        Some(window)
    );
    assert_eq!(runtime.last_host_scene_screen_position(&alpha_space), None);
    assert_eq!(
        runtime.recorded_had_panel_focus_for_test(&alpha_space),
        None
    );
    assert!(
        !runtime.push_viewport_host_scene_fact(
            &alpha_space,
            window.window_id(),
            leaf_host_scene_fact(alpha_tabs, alpha_tabs),
        ),
        "replaced rendered-host mapping must reject stale facts for the old space"
    );
}

#[open_gpui::test]
fn viewport_runtime_host_scene_liveness_expires_unrendered_host_scene(cx: &mut TestAppContext) {
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
    let window = handle(4);
    let mut runtime = DockViewportRuntime::new(controller);
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let host_position = center_drop_position(host_bounds);
    let window_facts = DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(
        floating_bounds(100.0, 100.0, 360.0, 220.0),
    ));

    assert!(runtime.register_rendered_host_viewport(target_space.clone(), window));
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        window.window_id(),
        window_facts,
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        window.window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));
    let identity = crate::DockViewportIdentity::new(target_space.clone(), window.window_id());
    let token = runtime.lease_rendered_viewport_host_scene(identity.clone());

    let (changed, windows) = runtime.expire_viewport_host_scene_if_unrendered(token);

    assert!(changed);
    assert_eq!(windows, Vec::<AnyWindowHandle>::new());
    assert_eq!(
        runtime.adapter().route_unavailable_reason(&target_space),
        Some(DockViewportRouteUnavailableReason::Stale(
            DockViewportStaleReason::WindowFactsChanged
        ))
    );
    assert_eq!(runtime.last_host_scene_screen_position(&target_space), None);
}

#[open_gpui::test]
fn viewport_runtime_host_scene_liveness_preserves_scene_after_new_render(cx: &mut TestAppContext) {
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
    let window = handle(5);
    let mut runtime = DockViewportRuntime::new(controller);
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let host_position = center_drop_position(host_bounds);
    let window_facts = DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(
        floating_bounds(100.0, 100.0, 360.0, 220.0),
    ));

    assert!(runtime.register_rendered_host_viewport(target_space.clone(), window));
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        window.window_id(),
        window_facts,
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        window.window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));
    let identity = crate::DockViewportIdentity::new(target_space.clone(), window.window_id());
    let stale_token = runtime.lease_rendered_viewport_host_scene(identity.clone());
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        window.window_id(),
        window_facts,
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        window.window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));
    let _current_token = runtime.lease_rendered_viewport_host_scene(identity.clone());

    let (changed, windows) = runtime.expire_viewport_host_scene_if_unrendered(stale_token);

    assert!(!changed);
    assert_eq!(windows, Vec::<AnyWindowHandle>::new());
    assert_eq!(
        runtime.adapter().route_unavailable_reason(&target_space),
        None
    );
    assert!(
        runtime
            .last_host_scene_screen_position(&target_space)
            .is_some()
    );
}

#[open_gpui::test]
fn viewport_runtime_host_scene_liveness_token_is_bound_to_viewport_identity(
    cx: &mut TestAppContext,
) {
    let old_space = DockSpaceId::from("old");
    let new_space = DockSpaceId::from("new");
    let mut graph = DockGraph::new();
    let old_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("old")],
        selected: Some(item("old")),
    });
    let new_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("new")],
        selected: Some(item("new")),
    });
    graph.set_root(old_space.clone(), old_tabs);
    graph.set_root(new_space.clone(), new_tabs);

    let mut workspace = DockWorkspace::new(old_space.clone(), graph);
    workspace.register_panel_view(item("old"), "Old", test_view(cx, "Old"));
    workspace.register_panel_view(item("new"), "New", test_view(cx, "New"));
    let controller = cx.new(|_| DockController::new(workspace));
    let window = handle(6);
    let mut runtime = DockViewportRuntime::new(controller);
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let host_position = center_drop_position(host_bounds);
    let window_facts = DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(
        floating_bounds(100.0, 100.0, 360.0, 220.0),
    ));

    assert!(runtime.register_rendered_host_viewport(old_space.clone(), window));
    assert!(runtime.begin_viewport_host_scene(
        old_space.clone(),
        window.window_id(),
        window_facts,
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &old_space,
        window.window_id(),
        leaf_host_scene_fact(old_tabs, old_tabs),
    ));
    let old_identity = crate::DockViewportIdentity::new(old_space.clone(), window.window_id());
    let stale_old_token = runtime.lease_rendered_viewport_host_scene(old_identity);

    assert!(runtime.register_rendered_host_viewport(new_space.clone(), window));
    assert!(runtime.begin_viewport_host_scene(
        new_space.clone(),
        window.window_id(),
        window_facts,
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &new_space,
        window.window_id(),
        leaf_host_scene_fact(new_tabs, new_tabs),
    ));

    let (changed, windows) = runtime.expire_viewport_host_scene_if_unrendered(stale_old_token);

    assert!(!changed);
    assert_eq!(windows, Vec::<AnyWindowHandle>::new());
    assert_eq!(runtime.last_host_scene_screen_position(&old_space), None);
    assert!(
        runtime
            .last_host_scene_screen_position(&new_space)
            .is_some(),
        "an old-space liveness token for the same window must not expire the replacement viewport"
    );
    assert_eq!(runtime.adapter().route_unavailable_reason(&new_space), None);
}

#[open_gpui::test]
fn viewport_runtime_reconciles_backend_focus_without_route_order_shadow_state(
    cx: &mut TestAppContext,
) {
    let alpha_space = DockSpaceId::from("alpha");
    let zeta_space = DockSpaceId::from("zeta");
    let mut graph = DockGraph::new();
    let alpha_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let zeta_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("z")],
        selected: Some(item("z")),
    });
    graph.set_root(alpha_space.clone(), alpha_tabs);
    graph.set_root(zeta_space.clone(), zeta_tabs);

    let mut workspace = DockWorkspace::new(alpha_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("z"), "Panel Z", test_view(cx, "Z"));
    let controller = cx.new(|_| DockController::new(workspace));
    let (alpha_window, _alpha_host, mut alpha_visual) = open_controller_space(
        cx,
        controller.clone(),
        alpha_space.clone(),
        size(px(320.0), px(240.0)),
    );
    let (zeta_window, _zeta_host, _zeta_visual) = open_controller_space(
        cx,
        controller.clone(),
        zeta_space.clone(),
        size(px(320.0), px(240.0)),
    );
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, alpha_space.clone(), alpha_window);
    register_viewport(&mut adapter, zeta_space.clone(), zeta_window);
    let mut runtime =
        DockViewportRuntime::from_adapter(controller, adapter, DockViewportClosePolicy::Prevent);

    alpha_window
        .update(cx, |_, window, _| window.activate_window())
        .expect("alpha viewport should activate");
    cx.run_until_parked();
    assert!(cx.update(|app| runtime.reconcile_backend_window_focus(app)));
    assert!(
        !cx.update(|app| runtime.reconcile_backend_window_focus(app)),
        "reconciling the same focused window twice should not churn backend focus state"
    );

    alpha_visual.deactivate_window();
    assert!(!cx.update(|app| runtime.reconcile_backend_window_focus(app)));

    zeta_window
        .update(cx, |_, window, _| window.activate_window())
        .expect("zeta viewport should activate");
    cx.run_until_parked();
    assert!(cx.update(|app| runtime.reconcile_backend_window_focus(app)));

    cx.set_platform_focused_window_available(false);
    alpha_window
        .update(cx, |_, window, _| window.activate_window())
        .expect("alpha viewport should activate while backend focus is unavailable");
    cx.run_until_parked();
    assert!(
        !cx.update(|app| runtime.reconcile_backend_window_focus(app)),
        "unavailable backend focus must not overwrite the last trusted backend focus"
    );
}

#[open_gpui::test]
fn unavailable_backend_focus_reconcile_preserves_pending_viewport_activation(
    cx: &mut TestAppContext,
) {
    let main_space = DockSpaceId::from("main");
    let mut graph = DockGraph::new();
    let tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(main_space.clone(), tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                main_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("viewport should open through runtime");
    assert!(
        runtime.record_pending_activation(crate::DockViewportActivationTransaction::new(
            main_space.clone(),
            opened.window(),
            DockViewportFocusRequest::panel("a"),
        ),)
    );
    cx.set_platform_focused_window_available(false);

    assert!(
        !cx.update(|app| runtime.reconcile_backend_window_focus(app)),
        "unavailable backend focus should be treated as unknown, not as a clear signal"
    );
    assert!(runtime.pending_activation().is_some());
}

#[open_gpui::test]
fn platform_activation_focus_request_requires_live_runtime_binding(cx: &mut TestAppContext) {
    let main_space = DockSpaceId::from("main");
    let mut graph = DockGraph::new();
    let tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(main_space.clone(), tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let first = cx
        .update(|app| {
            runtime.open_viewport(
                main_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("first viewport should open through runtime");
    runtime.record_panel_focus(main_space.clone(), item("a"));
    runtime
        .borrow_mut()
        .unregister_adapter_window_for_test(first.window().window_id());
    focus_backend_window_for_test(first.window(), cx);

    assert_eq!(
        cx.update(|app| {
            runtime.focus_command_for_confirmed_backend_window_focus(
                &main_space,
                first.window().window_id(),
                false,
                app,
            )
        }),
        None,
        "stale replaced windows must not restore focus from space history"
    );

    let second = cx
        .update(|app| {
            runtime.open_viewport(
                main_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("replacement viewport should open through runtime");
    focus_backend_window_for_test(second.window(), cx);
    let command = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &main_space,
            second.window().window_id(),
            false,
            app,
        )
    });
    assert_eq!(
        command.as_ref().map(DockViewportFocusCommand::request),
        Some(&DockViewportFocusRequest::panel("a"))
    );

    assert_eq!(
        cx.update(|app| {
            runtime.focus_command_for_confirmed_backend_window_focus(
                &main_space,
                second.window().window_id(),
                true,
                app,
            )
        }),
        None,
        "mouse-down platform activation must update window focus without restoring internal panel focus"
    );
}

#[open_gpui::test]
fn platform_activation_only_mouse_down_suppresses_focus_restore(cx: &mut TestAppContext) {
    let alpha_space = DockSpaceId::from("alpha");
    let zeta_space = DockSpaceId::from("zeta");
    let mut graph = DockGraph::new();
    let alpha_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let zeta_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("z")],
        selected: Some(item("z")),
    });
    graph.set_root(alpha_space.clone(), alpha_tabs);
    graph.set_root(zeta_space.clone(), zeta_tabs);

    let mut workspace = DockWorkspace::new(alpha_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("z"), "Panel Z", test_view(cx, "Z"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let alpha = cx
        .update(|app| {
            runtime.open_viewport(
                alpha_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("alpha viewport should open through runtime");
    let zeta = cx
        .update(|app| {
            runtime.open_viewport(
                zeta_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("zeta viewport should open through runtime");
    runtime.record_panel_focus(alpha_space.clone(), item("a"));
    runtime.record_panel_focus(zeta_space.clone(), item("z"));

    focus_backend_window_for_test(alpha.window(), cx);
    let command = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &alpha_space,
            alpha.window().window_id(),
            false,
            app,
        )
    });
    assert_eq!(
        command.as_ref().map(DockViewportFocusCommand::request),
        Some(&DockViewportFocusRequest::panel("a"))
    );

    focus_backend_window_for_test(zeta.window(), cx);
    assert_eq!(
        cx.update(|app| {
            runtime.focus_command_for_confirmed_backend_window_focus(
                &zeta_space,
                zeta.window().window_id(),
                true,
                app,
            )
        }),
        None,
        "mouse-down platform activation should update backend focus without restoring panel focus"
    );

    focus_backend_window_for_test(alpha.window(), cx);
    let command = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &alpha_space,
            alpha.window().window_id(),
            false,
            app,
        )
    });
    assert_eq!(
        command.as_ref().map(DockViewportFocusCommand::request),
        Some(&DockViewportFocusRequest::panel("a")),
        "backend-confirmed platform activation should restore dock focus when no mouse button is down"
    );
}

#[open_gpui::test]
fn platform_activation_after_destroyed_previous_focused_viewport_does_not_restore_panel_focus(
    cx: &mut TestAppContext,
) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let main = cx
        .update(|app| {
            runtime.open_viewport(
                main_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("main viewport should open through runtime");
    let detached = cx
        .update(|app| {
            runtime.open_viewport(
                detached_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("detached viewport should open through runtime");
    runtime.record_panel_focus(main_space.clone(), item("a"));
    runtime.record_panel_focus(detached_space.clone(), item("a"));
    focus_backend_window_for_test(main.window(), cx);
    let _ = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &main_space,
            main.window().window_id(),
            true,
            app,
        )
    });
    focus_backend_window_for_test(detached.window(), cx);
    let _ = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &detached_space,
            detached.window().window_id(),
            true,
            app,
        )
    });
    let closed = runtime
        .borrow_mut()
        .handle_window_closed(detached.window().window_id());
    assert_eq!(closed.status(), DockViewportCloseStatus::Closed);

    focus_backend_window_for_test(main.window(), cx);
    assert_eq!(
        cx.update(|app| {
            runtime.focus_command_for_confirmed_backend_window_focus(
                &main_space,
                main.window().window_id(),
                false,
                app,
            )
        }),
        None,
        "OS fallback activation after destroying the previous focused viewport must not restore internal panel focus"
    );
    focus_backend_window_for_test(main.window(), cx);
    let command = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &main_space,
            main.window().window_id(),
            false,
            app,
        )
    });
    assert_eq!(
        command.as_ref().map(DockViewportFocusCommand::request),
        Some(&DockViewportFocusRequest::panel("a")),
        "the destroyed-previous gate is consumed after one platform activation"
    );
}

#[open_gpui::test]
fn unfocused_new_viewport_close_does_not_suppress_next_platform_focus_restore(
    cx: &mut TestAppContext,
) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let main = cx
        .update(|app| {
            runtime.open_viewport(
                main_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("main viewport should open through runtime");
    let detached = cx
        .update(|app| {
            runtime.open_viewport(
                detached_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("detached viewport should open through runtime");
    runtime.record_panel_focus(main_space.clone(), item("a"));

    let closed = runtime
        .borrow_mut()
        .handle_window_closed(detached.window().window_id());
    assert_eq!(closed.status(), DockViewportCloseStatus::Closed);

    focus_backend_window_for_test(main.window(), cx);
    let command = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &main_space,
            main.window().window_id(),
            false,
            app,
        )
    });

    assert_eq!(
        command.as_ref().map(DockViewportFocusCommand::request),
        Some(&DockViewportFocusRequest::panel("a")),
        "closing a front-most but never platform-focused viewport must not trigger ImGui's destroyed-previous-focus suppression"
    );
}

#[open_gpui::test]
fn closing_non_last_confirmed_backend_focused_viewport_does_not_suppress_platform_focus_restore(
    cx: &mut TestAppContext,
) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let main = cx
        .update(|app| {
            runtime.open_viewport(
                main_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("main viewport should open through runtime");
    let detached = cx
        .update(|app| {
            runtime.open_viewport(
                detached_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("detached viewport should open through runtime");
    runtime.record_panel_focus(main_space.clone(), item("a"));
    runtime.record_panel_focus(detached_space.clone(), item("c"));

    focus_backend_window_for_test(detached.window(), cx);
    let _ = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &detached_space,
            detached.window().window_id(),
            false,
            app,
        )
    });
    focus_backend_window_for_test(main.window(), cx);
    let _ = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &main_space,
            main.window().window_id(),
            false,
            app,
        )
    });
    let closed = runtime
        .borrow_mut()
        .handle_window_closed(detached.window().window_id());
    assert_eq!(closed.status(), DockViewportCloseStatus::Closed);

    focus_backend_window_for_test(main.window(), cx);
    let command = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &main_space,
            main.window().window_id(),
            false,
            app,
        )
    });
    assert_eq!(
        command.as_ref().map(DockViewportFocusCommand::request),
        Some(&DockViewportFocusRequest::panel("a")),
        "closing a non-most-recent viewport should not suppress the next platform focus restore"
    );
}

#[open_gpui::test]
fn closing_last_confirmed_backend_focused_viewport_suppresses_platform_focus_restore_once(
    cx: &mut TestAppContext,
) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let main = cx
        .update(|app| {
            runtime.open_viewport(
                main_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("main viewport should open through runtime");
    let detached = cx
        .update(|app| {
            runtime.open_viewport(
                detached_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("detached viewport should open through runtime");
    runtime.record_panel_focus(main_space.clone(), item("a"));
    runtime.record_panel_focus(detached_space.clone(), item("c"));

    focus_backend_window_for_test(detached.window(), cx);
    let _ = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &detached_space,
            detached.window().window_id(),
            false,
            app,
        )
    });
    focus_backend_window_for_test(main.window(), cx);
    let _ = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &main_space,
            main.window().window_id(),
            false,
            app,
        )
    });

    let closed = runtime
        .borrow_mut()
        .handle_window_closed(main.window().window_id());
    assert_eq!(closed.status(), DockViewportCloseStatus::Closed);

    focus_backend_window_for_test(detached.window(), cx);
    assert_eq!(
        cx.update(|app| {
            runtime.focus_command_for_confirmed_backend_window_focus(
                &detached_space,
                detached.window().window_id(),
                false,
                app,
            )
        }),
        None,
        "closing the last platform-focused viewport should suppress the next platform focus restore"
    );

    focus_backend_window_for_test(detached.window(), cx);
    let command = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &detached_space,
            detached.window().window_id(),
            false,
            app,
        )
    });
    assert_eq!(
        command.as_ref().map(DockViewportFocusCommand::request),
        Some(&DockViewportFocusRequest::panel("c")),
        "the close-recovery suppression gate should be consumed after one platform activation"
    );
}

#[open_gpui::test]
fn reconcile_before_focus_command_keeps_destroyed_previous_focus_suppression(
    cx: &mut TestAppContext,
) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let main = cx
        .update(|app| {
            runtime.open_viewport(
                main_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("main viewport should open through runtime");
    let detached = cx
        .update(|app| {
            runtime.open_viewport(
                detached_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("detached viewport should open through runtime");
    runtime.record_panel_focus(main_space.clone(), item("a"));
    runtime.record_panel_focus(detached_space.clone(), item("c"));

    focus_backend_window_for_test(main.window(), cx);
    let _ = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &main_space,
            main.window().window_id(),
            false,
            app,
        )
    });
    let closed = runtime
        .borrow_mut()
        .handle_window_closed(main.window().window_id());
    assert_eq!(closed.status(), DockViewportCloseStatus::Closed);

    focus_backend_window_for_test(detached.window(), cx);
    assert!(
        cx.update(|app| runtime.reconcile_backend_window_focus(app)),
        "reconcile should record backend focus without consuming the destroyed-previous focus gate"
    );
    assert_eq!(
        cx.update(|app| {
            runtime.focus_command_for_confirmed_backend_window_focus(
                &detached_space,
                detached.window().window_id(),
                false,
                app,
            )
        }),
        None,
        "focus restore suppression must survive an earlier backend-focus reconcile"
    );

    focus_backend_window_for_test(detached.window(), cx);
    let command = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &detached_space,
            detached.window().window_id(),
            false,
            app,
        )
    });
    assert_eq!(
        command.as_ref().map(DockViewportFocusCommand::request),
        Some(&DockViewportFocusRequest::panel("c"))
    );
}

#[open_gpui::test]
fn pending_activation_overrides_destroyed_previous_focus_suppression(cx: &mut TestAppContext) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let main = cx
        .update(|app| {
            runtime.open_viewport(
                main_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("main viewport should open through runtime");
    let detached = cx
        .update(|app| {
            runtime.open_viewport(
                detached_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("detached viewport should open through runtime");
    runtime.record_panel_focus(main_space.clone(), item("a"));
    runtime.record_panel_focus(detached_space.clone(), item("c"));

    focus_backend_window_for_test(main.window(), cx);
    let _ = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &main_space,
            main.window().window_id(),
            false,
            app,
        )
    });
    let closed = runtime
        .borrow_mut()
        .handle_window_closed(main.window().window_id());
    assert_eq!(closed.status(), DockViewportCloseStatus::Closed);
    assert!(
        runtime.record_pending_activation(crate::DockViewportActivationTransaction::new(
            detached_space.clone(),
            detached.window(),
            DockViewportFocusRequest::panel("c"),
        ),)
    );

    focus_backend_window_for_test(detached.window(), cx);
    let command = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &detached_space,
            detached.window().window_id(),
            false,
            app,
        )
    });
    assert_eq!(
        command.as_ref().map(DockViewportFocusCommand::request),
        Some(&DockViewportFocusRequest::panel("c")),
        "explicit pending viewport activation should win over destroyed-previous platform focus suppression"
    );
    assert_eq!(
        command.as_ref().map(DockViewportFocusCommand::source),
        Some(crate::DockViewportFocusCommandSource::ViewportActivation)
    );
}

#[open_gpui::test]
fn pending_activation_is_not_suppressed_by_mouse_down(cx: &mut TestAppContext) {
    let main_space = DockSpaceId::from("main");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(main_space.clone(), main_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let main = cx
        .update(|app| {
            runtime.open_viewport(
                main_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("main viewport should open through runtime");
    assert!(
        runtime.record_pending_activation(crate::DockViewportActivationTransaction::new(
            main_space.clone(),
            main.window(),
            DockViewportFocusRequest::panel("a"),
        ))
    );

    focus_backend_window_for_test(main.window(), cx);
    let command = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &main_space,
            main.window().window_id(),
            true,
            app,
        )
    });

    assert_eq!(
        command.as_ref().map(DockViewportFocusCommand::request),
        Some(&DockViewportFocusRequest::panel("a")),
        "mouse-down backend focus should not suppress an explicit viewport activation transaction"
    );
    assert_eq!(
        command.as_ref().map(DockViewportFocusCommand::source),
        Some(crate::DockViewportFocusCommandSource::ViewportActivation)
    );
}

#[open_gpui::test]
fn non_docking_backend_focus_does_not_overwrite_last_confirmed_backend_focused_viewport(
    cx: &mut TestAppContext,
) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let open_options = || WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(floating_bounds(
            0.0, 0.0, 360.0, 220.0,
        ))),
        focus: false,
        ..Default::default()
    };
    let main = cx
        .update(|app| runtime.open_viewport(main_space.clone(), open_options(), app))
        .expect("main viewport should open through runtime");
    let detached = cx
        .update(|app| runtime.open_viewport(detached_space.clone(), open_options(), app))
        .expect("detached viewport should open through runtime");
    let plain_root = test_view(cx, "Plain");
    let non_docking = cx
        .update(|app| {
            let plain_root = plain_root.clone();
            app.open_window(open_options(), move |_, _| plain_root)
        })
        .expect("plain GPUI window should open");
    runtime.record_panel_focus(main_space.clone(), item("a"));
    runtime.record_panel_focus(detached_space.clone(), item("c"));

    focus_backend_window_for_test(main.window(), cx);
    let _ = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &main_space,
            main.window().window_id(),
            false,
            app,
        )
    });

    focus_backend_window_for_test(non_docking.into(), cx);
    assert!(
        !cx.update(|app| runtime.reconcile_backend_window_focus(app)),
        "non-docking GPUI focus must not update ImGui-style platform viewport focus history"
    );

    let closed = runtime
        .borrow_mut()
        .handle_window_closed(main.window().window_id());
    assert_eq!(closed.status(), DockViewportCloseStatus::Closed);

    focus_backend_window_for_test(detached.window(), cx);
    assert_eq!(
        cx.update(|app| {
            runtime.focus_command_for_confirmed_backend_window_focus(
                &detached_space,
                detached.window().window_id(),
                false,
                app,
            )
        }),
        None,
        "closing the last focused docking viewport should still suppress restore after a non-docking window was focused"
    );
}

#[open_gpui::test]
fn backend_focus_command_consumes_pending_viewport_activation(cx: &mut TestAppContext) {
    let main_space = DockSpaceId::from("main");
    let mut graph = DockGraph::new();
    let tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(main_space.clone(), tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                main_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("viewport should open through runtime");
    runtime.record_panel_focus(main_space.clone(), item("a"));
    assert!(
        runtime.record_pending_activation(crate::DockViewportActivationTransaction::new(
            main_space.clone(),
            opened.window(),
            DockViewportFocusRequest::panel("a"),
        ),)
    );
    opened
        .window()
        .update(cx, |_, window, _| window.activate_window())
        .expect("viewport should activate");

    let command = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &main_space,
            opened.window().window_id(),
            false,
            app,
        )
    });

    assert_eq!(
        command.as_ref().map(DockViewportFocusCommand::request),
        Some(&DockViewportFocusRequest::panel("a"))
    );
    assert_eq!(
        command.as_ref().map(DockViewportFocusCommand::source),
        Some(crate::DockViewportFocusCommandSource::ViewportActivation)
    );
    assert_eq!(runtime.pending_activation(), None);
}

#[open_gpui::test]
fn backend_focus_unavailable_does_not_consume_pending_viewport_activation(cx: &mut TestAppContext) {
    let main_space = DockSpaceId::from("main");
    let mut graph = DockGraph::new();
    let tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(main_space.clone(), tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                main_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("viewport should open through runtime");
    runtime.record_panel_focus(main_space.clone(), item("a"));
    assert!(
        runtime.record_pending_activation(crate::DockViewportActivationTransaction::new(
            main_space.clone(),
            opened.window(),
            DockViewportFocusRequest::panel("a"),
        ),)
    );
    cx.set_platform_focused_window_available(false);

    let command = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &main_space,
            opened.window().window_id(),
            false,
            app,
        )
    });

    assert_eq!(command, None);
    assert!(runtime.pending_activation().is_some());
}

#[open_gpui::test]
fn backend_focus_on_another_docking_window_clears_pending_viewport_activation(
    cx: &mut TestAppContext,
) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let main = cx
        .update(|app| {
            runtime.open_viewport(
                main_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("main viewport should open through runtime");
    let detached = cx
        .update(|app| {
            runtime.open_viewport(
                detached_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("detached viewport should open through runtime");
    assert!(
        runtime.record_pending_activation(crate::DockViewportActivationTransaction::new(
            detached_space.clone(),
            detached.window(),
            DockViewportFocusRequest::panel("c"),
        ))
    );

    focus_backend_window_for_test(main.window(), cx);
    assert!(
        cx.update(|app| runtime.reconcile_backend_window_focus(app)),
        "backend focus on another docking viewport should cancel stale activation intent"
    );
    assert_eq!(
        runtime.pending_activation(),
        None,
        "explicit activation intent must not survive confirmed backend focus on another docking viewport"
    );

    focus_backend_window_for_test(detached.window(), cx);
    assert_eq!(
        cx.update(|app| {
            runtime.focus_command_for_confirmed_backend_window_focus(
                &detached_space,
                detached.window().window_id(),
                false,
                app,
            )
        }),
        None,
        "later ordinary focus of the original target must not replay the stale activation"
    );
}

#[open_gpui::test]
fn backend_confirmed_activation_consumes_pending_viewport_activation(cx: &mut TestAppContext) {
    let main_space = DockSpaceId::from("main");
    let mut graph = DockGraph::new();
    let tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(main_space.clone(), tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                main_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("viewport should open through runtime");
    let host = opened
        .window()
        .downcast::<DockHost>()
        .expect("runtime viewport should render DockHost")
        .root(cx)
        .expect("runtime viewport should expose DockHost root");
    let mut visual = VisualTestContext::from_window(opened.window(), cx);
    let host_selector = selector_for(&visual, &host, crate::debug::DockDebugRegion::Host)
        .expect("host selector should be available");
    assert!(debug_bounds(&mut visual, &host_selector).size.width > px(0.0));

    host.update(cx, |host, _| {
        assert!(host.request_viewport_focus_command(
            DockViewportFocusCommand::platform_activation(DockViewportFocusRequest::panel("a"))
        ));
    });
    runtime.record_pending_activation(crate::DockViewportActivationTransaction::new(
        main_space.clone(),
        opened.window(),
        DockViewportFocusRequest::panel("a"),
    ));
    assert_eq!(
        runtime
            .pending_activation()
            .map(|activation| activation.focus_request().clone()),
        Some(DockViewportFocusRequest::panel("a"))
    );

    opened
        .window()
        .update(cx, |_, window, _| window.activate_window())
        .expect("viewport should activate");
    cx.run_until_parked();

    assert_eq!(runtime.pending_activation(), None);
}

#[open_gpui::test]
fn backend_confirmed_activation_while_mouse_is_pressed_preserves_pending_viewport_activation(
    cx: &mut TestAppContext,
) {
    let main_space = DockSpaceId::from("main");
    let mut graph = DockGraph::new();
    let tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(main_space.clone(), tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                main_space.clone(),
                WindowOptions {
                    focus: false,
                    ..viewport_window_options(360.0, 220.0)
                },
                app,
            )
        })
        .expect("viewport should open through runtime");

    runtime.record_panel_focus(main_space.clone(), item("a"));
    assert!(
        runtime.record_pending_activation(crate::DockViewportActivationTransaction::new(
            main_space.clone(),
            opened.window(),
            DockViewportFocusRequest::panel("a"),
        ),)
    );

    cx.set_platform_mouse_button_is_pressed(open_gpui::MouseButton::Left, Some(true));
    opened
        .window()
        .update(cx, |_, window, _| window.activate_window())
        .expect("viewport should activate");

    let command = cx.update(|app| {
        runtime.focus_command_for_confirmed_backend_window_focus(
            &main_space,
            opened.window().window_id(),
            true,
            app,
        )
    });

    assert_eq!(
        command.as_ref().map(DockViewportFocusCommand::request),
        Some(&DockViewportFocusRequest::panel("a"))
    );
    assert_eq!(
        command.as_ref().map(DockViewportFocusCommand::source),
        Some(crate::DockViewportFocusCommandSource::ViewportActivation)
    );
    assert_eq!(
        runtime.pending_activation(),
        None,
        "mouse-down suppresses platform focus restore, not explicit pending viewport activation"
    );
}

#[open_gpui::test]
fn close_recovery_does_not_steal_activation_from_another_active_docking_window(
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
    workspace.register_focusable_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::with_close_policy(
        controller,
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

    let main = cx
        .update(|app| runtime.open_viewport(main_space.clone(), open_options(), app))
        .expect("main viewport should open");
    let inspector = cx
        .update(|app| runtime.open_viewport(inspector_space.clone(), open_options(), app))
        .expect("inspector viewport should open");
    let detached = cx
        .update(|app| runtime.open_viewport(detached_space.clone(), open_options(), app))
        .expect("detached viewport should open");
    runtime.record_panel_focus(detached_space.clone(), item("c"));

    inspector
        .window()
        .update(cx, |_, window, _| window.activate_window())
        .expect("inspector viewport should activate");
    cx.run_until_parked();
    let _ = cx.update(|app| runtime.reconcile_backend_window_focus(app));
    assert_eq!(
        cx.update(|app| app.active_window())
            .map(|window| window.window_id()),
        Some(inspector.window().window_id())
    );
    freeze_should_close_plan(cx, &runtime, detached.window().window_id());

    let closed = cx.update(|app| {
        let closed = runtime
            .borrow_mut()
            .handle_window_closed_with_app(detached.window().window_id(), app);
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&detached_space),
            None,
            "closed detached viewport should be unregistered before close recovery activation"
        );
        let activation = runtime
            .borrow_mut()
            .activation_transaction_after_close(&closed, app)
            .expect("merge-back close should request close recovery activation");
        assert_eq!(
            activation.window_activation(),
            DockViewportWindowActivation::DoNotRequest
        );
        assert_eq!(
            apply_viewport_activation_transaction(Some(activation), app),
            DockViewportActivationApplyOutcome::Applied {
                changed: false,
                focus_command_queued: false,
                window_activation_requested: false,
                backend_focus_confirmed: false,
                pending_backend_focus: false,
            }
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&detached_space),
            None,
            "close recovery activation must not recreate the closed detached viewport binding"
        );
        closed
    });

    assert_eq!(closed.status(), DockViewportCloseStatus::MergedBack);
    assert_eq!(
        cx.update(|app| app.active_window())
            .map(|window| window.window_id()),
        Some(inspector.window().window_id()),
        "close recovery must not bring the merge target forward over another active docking window"
    );
    cx.run_until_parked();
    assert_eq!(
        main.window()
            .update(cx, |_, window, cx| window.focused(cx))
            .expect("main viewport should remain live"),
        None,
        "close recovery must not move GPUI focus inside a viewport that did not become platform-active"
    );
}

#[open_gpui::test]
fn close_recovery_does_not_steal_activation_from_active_non_docking_window(
    cx: &mut TestAppContext,
) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_focusable_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::with_close_policy(
        controller,
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

    let main = cx
        .update(|app| runtime.open_viewport(main_space.clone(), open_options(), app))
        .expect("main viewport should open");
    let detached = cx
        .update(|app| runtime.open_viewport(detached_space.clone(), open_options(), app))
        .expect("detached viewport should open");
    let plain_root = test_view(cx, "Plain");
    let non_docking = cx
        .update(|app| {
            let plain_root = plain_root.clone();
            app.open_window(open_options(), move |_, _| plain_root)
        })
        .expect("plain GPUI window should open");
    let non_docking: AnyWindowHandle = non_docking.into();
    runtime.record_panel_focus(detached_space.clone(), item("c"));

    non_docking
        .update(cx, |_, window, _| window.activate_window())
        .expect("plain GPUI window should activate");
    cx.run_until_parked();
    let _ = cx.update(|app| runtime.reconcile_backend_window_focus(app));
    assert_eq!(
        cx.update(|app| app.active_window())
            .map(|window| window.window_id()),
        Some(non_docking.window_id())
    );
    freeze_should_close_plan(cx, &runtime, detached.window().window_id());

    let closed = cx.update(|app| {
        let closed = runtime
            .borrow_mut()
            .handle_window_closed_with_app(detached.window().window_id(), app);
        let activation = runtime
            .borrow_mut()
            .activation_transaction_after_close(&closed, app)
            .expect("merge-back close should request close recovery activation");
        assert_eq!(
            activation.window_activation(),
            DockViewportWindowActivation::DoNotRequest
        );
        assert_eq!(
            apply_viewport_activation_transaction(Some(activation), app),
            DockViewportActivationApplyOutcome::Applied {
                changed: false,
                focus_command_queued: false,
                window_activation_requested: false,
                backend_focus_confirmed: false,
                pending_backend_focus: false,
            }
        );
        closed
    });

    assert_eq!(closed.status(), DockViewportCloseStatus::MergedBack);
    assert_eq!(
        cx.update(|app| app.active_window())
            .map(|window| window.window_id()),
        Some(non_docking.window_id()),
        "close recovery must not bring the merge target forward over a non-docking active window"
    );
    cx.run_until_parked();
    assert_eq!(
        main.window()
            .update(cx, |_, window, cx| window.focused(cx))
            .expect("main viewport should remain live"),
        None,
        "close recovery must not move GPUI focus inside a viewport that did not become platform-active"
    );
}

#[open_gpui::test]
fn close_recovery_without_source_focus_clears_target_panel_focus(cx: &mut TestAppContext) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(main_space.clone(), main_tabs);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::with_close_policy(
        controller,
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

    let main = cx
        .update(|app| runtime.open_viewport(main_space.clone(), open_options(), app))
        .expect("main viewport should open");
    let detached = cx
        .update(|app| runtime.open_viewport(detached_space.clone(), open_options(), app))
        .expect("detached viewport should open");
    runtime.record_panel_focus(main_space.clone(), item("a"));

    main.window()
        .update(cx, |_, window, _| window.activate_window())
        .expect("main viewport should activate");
    cx.run_until_parked();
    freeze_should_close_plan(cx, &runtime, detached.window().window_id());
    let closed = cx.update(|app| {
        let closed = runtime
            .borrow_mut()
            .handle_window_closed_with_app(detached.window().window_id(), app);
        let activation = runtime
            .borrow_mut()
            .activation_transaction_after_close(&closed, app)
            .expect("merge-back close should request activation");
        assert_eq!(
            activation.focus_request(),
            &DockViewportFocusRequest::no_panel_focus()
        );
        assert!(apply_viewport_activation_transaction(Some(activation), app).changed());
        closed
    });

    assert_eq!(closed.status(), DockViewportCloseStatus::MergedBack);
    cx.run_until_parked();
    assert_eq!(
        main.window()
            .update(cx, |_, window, cx| window.focused(cx))
            .expect("main viewport should remain live"),
        None,
        "close recovery without source focus should not restore the target viewport's focus history"
    );
}

#[open_gpui::test]
fn viewport_runtime_close_activation_before_backend_focus_does_not_raise_window(
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

    let panel_a = test_view(cx, "A");
    let panel_b = test_view(cx, "B");
    let panel_c = test_view(cx, "C");
    let panel_c_focus = cx.read_entity(&panel_c, |panel, cx| panel.focus_handle(cx));
    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel_a);
    workspace.register_focusable_panel_view(item("b"), "Panel B", panel_b);
    workspace.register_focusable_panel_view(item("c"), "Panel C", panel_c);
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
    let main = cx
        .update(|app| runtime.open_viewport(main_space.clone(), open_options(), app))
        .expect("main viewport should open");
    let main_host = main
        .window()
        .downcast::<DockHost>()
        .expect("main viewport should render DockHost")
        .root(cx)
        .expect("main viewport should expose DockHost root");
    let _inspector = cx
        .update(|app| runtime.open_viewport(inspector_space.clone(), open_options(), app))
        .expect("inspector viewport should open");
    let detached = cx
        .update(|app| runtime.open_viewport(detached_space.clone(), open_options(), app))
        .expect("detached viewport should open");
    runtime.record_panel_focus(detached_space.clone(), item("c"));

    main.window()
        .update(cx, |_, window, _| window.activate_window())
        .expect("main viewport should activate");
    cx.run_until_parked();
    let _ = cx.update(|app| runtime.reconcile_backend_window_focus(app));
    freeze_should_close_plan(cx, &runtime, detached.window().window_id());
    let outcome = cx.update(|app| {
        let outcome = runtime
            .borrow_mut()
            .handle_window_closed_with_app(detached.window().window_id(), app);
        let activation = runtime
            .borrow_mut()
            .activation_transaction_after_close(&outcome, app);
        assert_eq!(
            activation
                .as_ref()
                .map(|target| target.focus_request().clone()),
            Some(DockViewportFocusRequest::panel(item("c"))),
            "close activation should restore focus to the source viewport's recorded focus item"
        );
        main_host.update(app, |host, _| {
            assert!(host.request_viewport_focus_command(
                DockViewportFocusCommand::platform_activation(DockViewportFocusRequest::panel("c"))
            ));
            assert_eq!(
                host.pending_focus_command()
                    .map(DockViewportFocusCommand::request),
                Some(&DockViewportFocusRequest::panel("c"))
            );
        });
        assert!(apply_viewport_activation_transaction(activation, app).changed());
        outcome
    });

    assert_eq!(outcome.status(), DockViewportCloseStatus::MergedBack);
    cx.run_until_parked();
    assert_eq!(
        main.window()
            .update(cx, |_, window, cx| window.focused(cx))
            .expect("main viewport should remain live"),
        Some(panel_c_focus),
        "close recovery focus must override an earlier platform activation restore request"
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
fn viewport_runtime_tear_off_preflight_failure_does_not_open_window(cx: &mut TestAppContext) {
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
    let error = cx
        .update(|app| {
            runtime.open_tear_off_viewport(
                tear_off_request(primary_space.clone(), source_tabs, item("a")),
                detached_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect_err("non-empty destination space should fail before opening a tear-off window");

    assert!(
        error
            .to_string()
            .contains("target dock space detached is not empty"),
        "non-empty target should fail preflight, got {error}"
    );
    assert_eq!(
        runtime.borrow().pending_tear_off_len(),
        0,
        "preflight failure must not create pending tear-off state"
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
fn viewport_runtime_floating_payload_focus_requires_recorded_focus(cx: &mut TestAppContext) {
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

    let mut workspace = DockWorkspace::new(primary_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
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
        "floating payload focus must not be inferred from selected tabs"
    );
}

#[open_gpui::test]
fn viewport_runtime_floating_payload_without_focus_activates_with_no_panel_focus(
    cx: &mut TestAppContext,
) {
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

    let mut workspace = DockWorkspace::new(primary_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let request = DockViewportTearOffRequest::new(
        primary_space.clone(),
        floating,
        DockViewportDropPayload::Floating(floating),
        point(px(900.0), px(900.0)),
        None,
    );
    let outcome = cx
        .update(|app| {
            runtime.open_tear_off_viewport(
                request,
                detached_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("floating tear-off should complete through runtime handle");
    let DockViewportTearOffOpenOutcome::Completed(completed) = outcome else {
        panic!("floating tear-off should complete");
    };
    assert_eq!(completed.pending().focus_item(), None);
    let route_outcome = crate::DockViewportDropRouteOutcome::tear_off(
        DockViewportTearOffOpenOutcome::Completed(completed),
    );

    assert_eq!(
        route_outcome
            .activation_transaction()
            .map(|target| target.focus_request().clone()),
        Some(DockViewportFocusRequest::no_panel_focus()),
        "payloads without explicit focus provenance must clear panel focus instead of restoring history"
    );
}

#[open_gpui::test]
fn viewport_runtime_unregister_space_clears_had_panel_focus_fact(cx: &mut TestAppContext) {
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
    register_viewport(&mut adapter, detached_space.clone(), window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller,
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );
    runtime.record_panel_focus(detached_space.clone(), item("c"));

    assert_eq!(
        runtime.recorded_had_panel_focus_for_test(&detached_space),
        Some(true)
    );
    assert!(runtime.unregister_host_for_space(&detached_space, window.window_id()));
    assert_eq!(runtime.adapter().window_for_space(&detached_space), None);
    assert_eq!(
        runtime.recorded_had_panel_focus_for_test(&detached_space),
        None
    );
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
fn viewport_runtime_merge_back_close_without_frozen_plan_only_unregisters(cx: &mut TestAppContext) {
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
    let main_window = open_controller_space(
        cx,
        controller.clone(),
        main_space.clone(),
        size(px(360.0), px(220.0)),
    )
    .0;
    let main_window: AnyWindowHandle = main_window.into();
    let window = handle(44);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, main_space.clone(), main_window);
    register_viewport(&mut adapter, detached_space.clone(), window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller.clone(),
        adapter,
        DockViewportClosePolicy::MergeBack {
            target_space: main_space.clone(),
        },
    );

    let outcome = cx.update(|app| runtime.handle_window_closed_with_app(window.window_id(), app));
    let activation = cx.update(|app| runtime.activation_transaction_after_close(&outcome, app));

    assert_eq!(outcome.status(), DockViewportCloseStatus::Closed);
    assert_eq!(
        runtime.runtime_status().last_close,
        Some(outcome.clone()),
        "close diagnostics should record a plain close when no should-close plan froze merge-back authority"
    );
    assert_eq!(
        outcome.focus_item().cloned(),
        None,
        "plain close has no merge-back focus item"
    );
    assert_eq!(
        activation, None,
        "plain close without a frozen merge-back plan must not request close recovery activation"
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
        assert_eq!(items, &vec![item("b")]);
        assert_eq!(selected.as_ref(), items.first());
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            vec![item("a"), item("c")],
            "window cleanup must not move graph content without a frozen should-close merge-back plan"
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_merge_back_close_uses_recorded_source_focus_item(cx: &mut TestAppContext) {
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
    let controller = cx.new(|_| DockController::new(workspace));
    let window = handle(47);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, detached_space.clone(), window);
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
    let outcome = cx.update(|app| runtime.handle_window_closed_with_app(window.window_id(), app));

    assert_eq!(outcome.status(), DockViewportCloseStatus::MergedBack);
    assert_eq!(
        outcome.focus_item().cloned(),
        Some(item("a")),
        "merge-back close may restore focus only from the closing viewport's recorded panel focus"
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&main_space),
            vec![item("b"), item("a")]
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            Vec::<DockItemId>::new()
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_merge_back_close_does_not_use_tree_order_for_focus(cx: &mut TestAppContext) {
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
    let window = handle(470);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, detached_space.clone(), window);
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
    let outcome = cx.update(|app| runtime.handle_window_closed_with_app(window.window_id(), app));

    assert_eq!(outcome.status(), DockViewportCloseStatus::MergedBack);
    assert_eq!(
        outcome.focus_item().cloned(),
        None,
        "merge-back close must not infer a concrete focus item from root/floating tree order without recorded focus"
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
    register_viewport(&mut adapter, detached_space.clone(), window);
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
    register_viewport(&mut adapter, detached_space.clone(), window);
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
fn viewport_runtime_merge_back_should_close_records_pending_plan_without_graph_mutation(
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
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&main_space),
            vec![item("b")]
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            vec![item("a")]
        );
    });

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

    let closed =
        cx.update(|app| runtime.handle_window_closed_with_app(opened.window().window_id(), app));
    assert_eq!(closed.status(), DockViewportCloseStatus::MergedBack);
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&main_space),
            vec![item("b"), item("c")]
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            Vec::<DockItemId>::new()
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
fn viewport_runtime_merge_back_commits_on_window_closed_after_should_close(
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
    let window = handle(45);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, detached_space.clone(), window);
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
            vec![item("b")]
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            vec![item("a")]
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
        "pending merge-back close should preserve the source focus item captured at should-close"
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
fn viewport_runtime_pending_merge_back_activation_uses_should_close_target(
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
            .activation_transaction_after_close(&closed, app)
    });

    assert_eq!(closed.status(), DockViewportCloseStatus::MergedBack);
    assert_eq!(closed.merge_target_space(), Some(&main_space));
    assert_eq!(closed.focus_item().cloned(), Some(item("a")));
    assert_eq!(
        activation.as_ref().map(|target| target.space()),
        Some(&main_space),
        "activation must use the pending should-close merge-back target, not a later close policy"
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
fn viewport_runtime_pending_merge_back_freezes_should_close_target_tabs(cx: &mut TestAppContext) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let target_left = graph.insert_node(DockNode::Tabs {
        items: vec![item("left-a"), item("left-b")],
        selected: Some(item("left-b")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("x")],
        selected: Some(item("x")),
    });
    graph.set_root(main_space.clone(), target_left);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    for id in ["left-a", "left-b", "right-a", "right-b", "x"] {
        workspace.register_panel_view(item(id), id, test_view(cx, id));
    }
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace
        .select_tab(target_left, item("left-b"))
        .expect("selected target tabs should be observed before should-close");
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
    let detached = cx
        .update(|app| runtime.open_viewport(detached_space.clone(), open_options(), app))
        .expect("detached viewport should open");

    let should_close = cx.update(|app| {
        runtime.handle_window_should_close_with_app(detached.window().window_id(), app)
    });
    assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);
    cx.update_entity(&controller, |controller, _| {
        let mut graph = controller.graph().clone();
        let target_right = graph.insert_node(DockNode::Tabs {
            items: vec![item("right-a"), item("right-b")],
            selected: Some(item("right-b")),
        });
        let main_root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![target_left, target_right],
            fractions: vec![0.5, 0.5],
        });
        graph.set_root(main_space.clone(), main_root);
        controller.workspace_mut().set_graph(graph);
        controller
            .workspace_mut()
            .select_tab(target_right, item("right-b"))
            .expect("post-validation target tabs should still be selectable");
    });

    let closed =
        cx.update(|app| runtime.handle_window_closed_with_app(detached.window().window_id(), app));

    assert_eq!(closed.status(), DockViewportCloseStatus::MergedBack);
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs {
            items: left_items,
            selected: left_selected,
        } = controller
            .graph()
            .node(target_left)
            .expect("left tabs should remain")
        else {
            panic!("left target should be tabs");
        };
        assert_eq!(left_items, &vec![item("left-a"), item("left-b"), item("x")]);
        assert_eq!(left_selected.as_ref(), left_items.get(2));

        let (right_tabs, _) = controller
            .graph()
            .find_item_in_space(&main_space, &item("right-a"))
            .expect("right tabs should remain in the target space");
        let DockNode::Tabs {
            items: right_items,
            selected: right_selected,
        } = controller
            .graph()
            .node(right_tabs)
            .expect("right tabs should remain")
        else {
            panic!("right target should be tabs");
        };
        assert_eq!(right_items, &vec![item("right-a"), item("right-b")]);
        assert_eq!(right_selected.as_ref(), right_items.get(1));
    });
}

#[open_gpui::test]
fn viewport_runtime_pending_merge_back_rejects_stale_frozen_target_tabs(cx: &mut TestAppContext) {
    let main_space = DockSpaceId::from("main");
    let detached_space = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let target_left = graph.insert_node(DockNode::Tabs {
        items: vec![item("left-a"), item("left-b")],
        selected: Some(item("left-b")),
    });
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("x")],
        selected: Some(item("x")),
    });
    graph.set_root(main_space.clone(), target_left);
    graph.set_root(detached_space.clone(), detached_tabs);

    let mut workspace = DockWorkspace::new(main_space.clone(), graph);
    for id in ["left-a", "left-b", "right-a", "right-b", "x"] {
        workspace.register_panel_view(item(id), id, test_view(cx, id));
    }
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace
        .select_tab(target_left, item("left-b"))
        .expect("selected target tabs should be observed before should-close");
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
    let detached = cx
        .update(|app| runtime.open_viewport(detached_space.clone(), open_options(), app))
        .expect("detached viewport should open");

    let should_close = cx.update(|app| {
        runtime.handle_window_should_close_with_app(detached.window().window_id(), app)
    });
    assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);
    cx.update_entity(&controller, |controller, _| {
        let mut graph = controller.graph().clone();
        let target_right = graph.insert_node(DockNode::Tabs {
            items: vec![item("right-a"), item("right-b")],
            selected: Some(item("right-b")),
        });
        let main_root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![target_left, target_right],
            fractions: vec![0.5, 0.5],
        });
        graph.set_root(main_space.clone(), main_root);
        controller.workspace_mut().set_graph(graph);
        controller
            .workspace_mut()
            .close_item(main_space.clone(), item("left-b"))
            .expect("first frozen-target item should close");
        controller
            .workspace_mut()
            .close_item(main_space.clone(), item("left-a"))
            .expect("stale frozen target tabs should be removed before close");
        controller
            .workspace_mut()
            .select_tab(target_right, item("right-b"))
            .expect("another merge target should be available");
    });

    let closed =
        cx.update(|app| runtime.handle_window_closed_with_app(detached.window().window_id(), app));

    assert_eq!(closed.status(), DockViewportCloseStatus::MergeBackFailed);
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&main_space),
            vec![item("right-a"), item("right-b")],
            "stale frozen target must not reroute merge-back into another target tabs"
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            vec![item("x")],
            "failed merge-back should leave source layout available for retain/reopen diagnostics"
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_merge_back_should_close_auto_cancels_when_window_renders_again(
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
    register_viewport(&mut adapter, detached_space.clone(), window);
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
        DockViewportRouteStatus::RouteReady,
        "pending close is a platform request flag, not stale route facts"
    );
    assert!(detached_lifecycle.platform_request_status.close_requested);
    assert_eq!(
        runtime.last_host_scene_screen_position(&detached_space),
        None
    );

    let request = DockViewportDropRouteRequest::from_target_context(
        detached_space.clone(),
        detached_tabs,
        DockViewportDropPayload::Item(item("a")),
        screen_position_for_host_position(window_bounds, host_position),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(window),
    );
    let pending_resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert_eq!(
        pending_resolution.route(),
        &DockViewportDropRoute::Unavailable
    );
    assert!(pending_resolution.delivery().is_none());

    assert!(
        runtime.begin_viewport_host_scene(
            detached_space.clone(),
            window.window_id(),
            DockViewportWindowFacts::from_window_bounds(window_bounds),
            host_bounds,
            host_position,
        ),
        "a live render frame means the accepted platform close request was not completed"
    );
    assert!(runtime.push_viewport_host_scene_fact(
        &detached_space,
        window.window_id(),
        leaf_host_scene_fact(detached_tabs, detached_tabs),
    ));
    let lifecycle = runtime.runtime_status().viewport_lifecycle;
    let detached_lifecycle = lifecycle
        .iter()
        .find(|record| record.space == detached_space)
        .expect("live frame should keep the detached viewport registered");
    assert_eq!(
        detached_lifecycle.route_status,
        DockViewportRouteStatus::RouteReady
    );
    assert!(!detached_lifecycle.platform_request_status.close_requested);
    let fresh_resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert!(
        matches!(
            fresh_resolution.route(),
            DockViewportDropRoute::Local { .. }
        ),
        "fresh route facts should restore local route authority after auto-cancel"
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&main_space),
            vec![item("b")]
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            vec![item("a")]
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_retain_should_close_auto_cancels_when_window_renders_again(
    cx: &mut TestAppContext,
) {
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
    register_viewport(&mut adapter, detached_space.clone(), window);
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
        DockViewportRouteStatus::RouteReady,
        "pending retain close is a platform request flag, not stale route facts"
    );
    assert!(detached_lifecycle.platform_request_status.close_requested);
    assert_eq!(
        runtime.last_host_scene_screen_position(&detached_space),
        None
    );

    let request = DockViewportDropRouteRequest::from_target_context(
        detached_space.clone(),
        detached_tabs,
        DockViewportDropPayload::Item(item("a")),
        screen_position_for_host_position(window_bounds, host_position),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(window),
    );
    let pending_resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert_eq!(
        pending_resolution.route(),
        &DockViewportDropRoute::Unavailable
    );
    assert!(pending_resolution.delivery().is_none());

    assert!(
        runtime.begin_viewport_host_scene(
            detached_space.clone(),
            window.window_id(),
            DockViewportWindowFacts::from_window_bounds(window_bounds),
            host_bounds,
            host_position,
        ),
        "a live render frame means the accepted retain close request was not completed"
    );
    assert!(runtime.push_viewport_host_scene_fact(
        &detached_space,
        window.window_id(),
        leaf_host_scene_fact(detached_tabs, detached_tabs),
    ));
    let lifecycle = runtime.runtime_status().viewport_lifecycle;
    let detached_lifecycle = lifecycle
        .iter()
        .find(|record| record.space == detached_space)
        .expect("live frame should keep the detached viewport registered");
    assert_eq!(
        detached_lifecycle.route_status,
        DockViewportRouteStatus::RouteReady
    );
    assert!(!detached_lifecycle.platform_request_status.close_requested);
    let fresh_resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert!(
        matches!(
            fresh_resolution.route(),
            DockViewportDropRoute::Local { .. }
        ),
        "fresh route facts should restore local route authority after auto-cancel"
    );
}

#[open_gpui::test]
fn viewport_runtime_cancel_retain_should_close_restores_current_route_facts(
    cx: &mut TestAppContext,
) {
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
    let window = handle(49);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, detached_space.clone(), window);
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

    let (cancelled, windows) = runtime.cancel_window_close_request(window.window_id());
    assert!(cancelled);
    assert_eq!(windows, vec![window]);
    let lifecycle = runtime.runtime_status().viewport_lifecycle;
    let detached_lifecycle = lifecycle
        .iter()
        .find(|record| record.space == detached_space)
        .expect("cancelled close should keep the viewport registered");
    assert_eq!(
        detached_lifecycle.route_status,
        DockViewportRouteStatus::RouteReady,
        "cancel clears only the close request flag and restores otherwise-current route facts"
    );
    assert!(!detached_lifecycle.platform_request_status.close_requested);

    let request = DockViewportDropRouteRequest::from_target_context(
        detached_space.clone(),
        detached_tabs,
        DockViewportDropPayload::Item(item("a")),
        screen_position_for_host_position(window_bounds, host_position),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(window),
    );
    let fresh_resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    let DockViewportDropRoute::Local {
        host_position: routed_position,
        window_id: routed_window,
        authority,
        ..
    } = fresh_resolution.route()
    else {
        panic!("fresh route facts should restore local route authority");
    };
    assert_eq!(*routed_position, host_position);
    assert_eq!(*routed_window, window.window_id());
    assert_eq!(
        *authority,
        crate::DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow
    );
}

#[open_gpui::test]
fn viewport_runtime_cancel_merge_back_should_close_restores_current_route_facts(
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
    let window = handle(50);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, detached_space.clone(), window);
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

    let should_close = cx.update(|app| {
        runtime
            .handle_window_should_close_with_app_and_refresh(window.window_id(), app)
            .0
    });
    assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);
    assert!(runtime.cancel_window_close_request(window.window_id()).0);
    let lifecycle = runtime.runtime_status().viewport_lifecycle;
    let detached_lifecycle = lifecycle
        .iter()
        .find(|record| record.space == detached_space)
        .expect("cancelled close should keep the viewport registered");
    assert_eq!(
        detached_lifecycle.route_status,
        DockViewportRouteStatus::RouteReady,
        "cancel clears only the close request flag and restores otherwise-current route facts"
    );
    assert!(!detached_lifecycle.platform_request_status.close_requested);
}

#[open_gpui::test]
fn viewport_runtime_discarded_pending_close_does_not_mark_reused_window(cx: &mut TestAppContext) {
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
    register_viewport(&mut adapter, detached_space.clone(), window);
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
            vec![item("b")]
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            vec![item("a")]
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
    register_viewport(&mut adapter, secondary_space.clone(), window);
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
    register_viewport(&mut adapter, secondary_space.clone(), window);

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
    assert!(runtime.has_routed_drop_preview_for_drag_session(Some(&session)));

    let outcome = runtime.handle_window_closed(target_window.window_id());

    assert_eq!(outcome.status(), DockViewportCloseStatus::Closed);
    assert_eq!(runtime.runtime_status().last_route, None);
    assert!(!runtime.has_routed_drop_preview_for_drag_session(Some(&session)));
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
    register_viewport(&mut adapter, target_space.clone(), target_window);
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
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
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
    assert!(!runtime.has_routed_drop_preview_for_drag_session(Some(&session)));
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
    register_viewport(&mut adapter, target_space.clone(), old_window);
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
    assert!(!runtime.has_routed_drop_preview_for_drag_session(Some(&session)));
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
    assert!(runtime.has_routed_drop_preview_for_drag_session(Some(&session)));

    let outcome = runtime.handle_window_closed(source_window.window_id());

    assert_eq!(outcome.status(), DockViewportCloseStatus::Closed);
    assert_eq!(outcome.space(), Some(&source_space));
    assert_eq!(runtime.active_payload_drag_session(&payload), None);
    assert_eq!(
        runtime.active_payload_drag_tear_off_geometry(Some(&session)),
        None
    );
    assert!(!runtime.has_routed_drop_preview_for_drag_session(Some(&session)));
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
    assert!(runtime.has_routed_drop_preview_for_drag_session(Some(&session)));

    assert!(runtime.unregister_host_for_space(&source_space, source_window.window_id()));

    assert_eq!(runtime.active_payload_drag_session(&payload), None);
    assert!(!runtime.has_routed_drop_preview_for_drag_session(Some(&session)));
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
    register_viewport(&mut adapter, target_space.clone(), stale_window);
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
    assert!(!runtime.has_routed_drop_preview_for_drag_session(Some(&session)));
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
    register_viewport(&mut adapter, target_space.clone(), target_window);
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
    assert!(runtime.has_routed_drop_preview_for_drag_session(Some(&session)));

    assert!(runtime.unregister_host_for_space(&target_space, target_window.window_id()));
    assert_eq!(runtime.adapter().window_for_space(&target_space), None);
    assert_eq!(runtime.last_host_scene_screen_position(&target_space), None);
    assert_eq!(
        runtime.routed_drop_preview_for(&target_space, target_window.window_id()),
        None
    );
    assert!(!runtime.has_routed_drop_preview_for_drag_session(Some(&session)));
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
    register_viewport(&mut adapter, target_space.clone(), old_window);
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
    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    let session = runtime.begin_payload_drag(&payload);

    let request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        screen_position_for_host_position(window_bounds, host_position),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(old_window),
    )
    .with_drag_session(Some(session.clone()));
    let accepted_resolution =
        accepted_preview_delivery_for_test(&mut runtime, &request, &target_space, old_window, cx);
    let stale_plan = DockDropDelivery::from_resolution(accepted_resolution)
        .expect("accepted preview should mint a commit plan");

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

    let result =
        cx.update(|app| runtime.deliver_drop_commit_delivery_with_outcome(stale_plan, app));
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
    register_viewport(&mut adapter, target_space.clone(), target_window);
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
    let session = runtime.begin_payload_drag(&payload);

    let request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
    )
    .with_drag_session(Some(session.clone()));
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert!(
        matches!(resolution.route(), DockViewportDropRoute::KnownViewport { target, .. }
            if target.window_id() == target_window.window_id()),
        "preview route should target the registered viewport"
    );
    assert!(
        resolution.routed_preview_target_snapshot().is_some(),
        "fresh route should carry a preview target"
    );
    assert!(
        resolution.delivery().is_none(),
        "fresh route must not mint delivery before target acceptance"
    );
    let accepted_resolution = accepted_preview_delivery_for_test(
        &mut runtime,
        &request,
        &target_space,
        target_window,
        cx,
    );
    let commit_plan = DockDropDelivery::from_resolution(accepted_resolution)
        .expect("accepted preview should mint a plan");

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

    let result =
        cx.update(|app| runtime.deliver_drop_commit_delivery_with_outcome(commit_plan, app));
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
fn viewport_runtime_tabs_drop_uses_recorded_payload_focus(cx: &mut TestAppContext) {
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
    runtime.record_panel_focus(source_space.clone(), item("a"));
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
    let host_position = center_drop_position(host_bounds);
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(window_bounds),
        host_bounds,
        host_position,
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        opened.window().window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));

    let payload = DockDragPayload::new_tabs(source_space.clone(), source_tabs, "Stack".to_string());
    let session = cx.update(|app| runtime.begin_payload_drag_with_app(&payload, app));
    let request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Tabs,
        screen_position_for_host_position(window_bounds, host_position),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(opened.window()),
    )
    .with_drag_session(Some(session.clone()));
    let preview_resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    cx.update(|app| {
        runtime.update_routed_drop_preview(&preview_resolution, "Stack", app);
    });
    assert!(runtime.finish_routed_drop_acceptance_pass(&target_space, opened.window().window_id()));
    let outcome = cx
        .update(|app| runtime.commit_payload_drop_from_screen(&request, app))
        .expect("recorded-focus tabs drop should commit");
    let DockViewportDropRouteOutcome::Action(action) = outcome else {
        panic!("tabs drop should produce an action outcome");
    };
    assert_eq!(action.action(), DockActionOutcome::Changed);
    assert_eq!(
        action
            .activation()
            .map(|activation| activation.focus_request().clone()),
        Some(DockViewportFocusRequest::panel(item("a"))),
        "tabs payload activation should use the recorded drag focus, not selected tab"
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(controller.graph().collect_items_in_space(&source_space), []);
        assert_eq!(
            controller.graph().collect_items_in_space(&target_space),
            vec![item("b"), item("a"), item("c")]
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
    register_viewport(&mut adapter, target_space.clone(), target_window);
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
    let session = runtime.begin_payload_drag(&payload);

    let request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
    )
    .with_drag_session(Some(session.clone()));
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert!(
        matches!(resolution.route(), DockViewportDropRoute::KnownViewport { target, .. }
            if target.window_id() == target_window.window_id()),
        "fresh viewport facts should produce a known viewport route"
    );
    assert!(
        resolution.routed_preview_target_snapshot().is_some(),
        "fresh route should capture the resolved host scene target"
    );

    assert!(
        resolution.delivery().is_none(),
        "fresh route must not mint delivery before target acceptance"
    );
    let accepted_resolution = accepted_preview_delivery_for_test(
        &mut runtime,
        &request,
        &target_space,
        target_window,
        cx,
    );
    let commit_plan = DockDropDelivery::from_resolution(accepted_resolution)
        .expect("accepted preview should mint a plan");
    let (changed, _) = runtime.mark_viewport_window_snapshot_stale(target_window.window_id());
    assert!(changed);
    let result =
        cx.update(|app| runtime.deliver_drop_commit_delivery_with_outcome(commit_plan, app));
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
fn viewport_runtime_requires_backend_route_authority_for_drop(cx: &mut TestAppContext) {
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
    let runtime = DockViewportRuntimeHandle::new(controller);
    let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let host_position = center_drop_position(host_bounds);
    let opened = cx
        .update(|app| {
            runtime.open_viewport(
                source_space.clone(),
                WindowOptions {
                    window_bounds: Some(window_bounds),
                    focus: false,
                    ..Default::default()
                },
                app,
            )
        })
        .expect("source viewport should open through runtime");
    let source_window = opened.window();

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
        DockViewportTargetContext::new(),
    );
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));

    assert_eq!(
        resolution.route(),
        &DockViewportDropRoute::Unavailable,
        "fresh live-window facts must not route without backend authority"
    );
    assert!(
        resolution.routed_preview_target_snapshot().is_none(),
        "authority-free route must not carry a routed preview target"
    );
    assert!(
        resolution.delivery().is_none(),
        "fallback route must not mint delivery before target acceptance"
    );

    let trusted_request = DockViewportDropRouteRequest::from_target_context(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        screen_position_for_host_position(window_bounds, host_position),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(source_window),
    );
    let trusted_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&trusted_request, app));
    assert!(
        matches!(
            trusted_resolution.route(),
            DockViewportDropRoute::Local {
                host_position: route_host_position,
                window_id,
                authority: crate::DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
                ..
            } if *route_host_position == host_position && *window_id == source_window.window_id()
        ),
        "trusted hovered live-window facts should route with trusted-hovered authority"
    );
    assert!(
        trusted_resolution
            .routed_preview_target_snapshot()
            .is_some(),
        "trusted route should carry a preview target"
    );
    assert!(
        trusted_resolution.delivery().is_none(),
        "trusted route must not mint delivery before target acceptance"
    );
}

#[open_gpui::test]
fn viewport_runtime_rejected_preview_records_last_routed_viewport_identity(
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
    workspace.policy_mut().set_allow_edge_split(false);
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let source_window_bounds = WindowBounds::Windowed(floating_bounds(520.0, 100.0, 360.0, 220.0));
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
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_opened.window()),
    )
    .with_drag_session(Some(session.clone()));
    let preview_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
    assert!(matches!(
        preview_resolution.route(),
        DockViewportDropRoute::Rejected(_)
    ));
    assert!(
        preview_resolution.preview_target().is_some(),
        "rejected hover should still expose the viewport target for routed-preview bookkeeping"
    );
    cx.update(|app| {
        runtime.update_routed_drop_preview(&preview_resolution, "Panel A", app);
    });
    assert_eq!(
        runtime
            .last_routed_viewport_identity_for_drag_session(Some(&session))
            .map(|identity| identity.window_id()),
        Some(target_opened.window().window_id()),
        "policy-rejected hover should still record the last routed viewport during the drag"
    );
    let _ = source_opened;
}

#[open_gpui::test]
fn viewport_runtime_unavailable_preview_does_not_clear_last_routed_viewport_identity(
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

    let source_window_bounds = WindowBounds::Windowed(floating_bounds(520.0, 100.0, 360.0, 220.0));
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
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_opened.window()),
    )
    .with_drag_session(Some(session.clone()));
    let preview_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
    cx.update(|app| {
        runtime.update_routed_drop_preview(&preview_resolution, "Panel A", app);
    });
    assert_eq!(
        runtime
            .last_routed_viewport_identity_for_drag_session(Some(&session))
            .map(|identity| identity.window_id()),
        Some(target_opened.window().window_id())
    );

    let unavailable_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        crate::DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new())
            .with_event_receiver_window(source_opened.window())
            .with_global_window_bounds(false),
        DockPayloadDropReleaseOrigin::HoveredHost,
    )
    .with_drag_session(Some(session.clone()));
    let unavailable_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&unavailable_request, app));
    assert_eq!(
        unavailable_resolution.route(),
        &DockViewportDropRoute::Unavailable
    );
    cx.update(|app| {
        runtime.update_routed_drop_preview(&unavailable_resolution, "Panel A", app);
    });
    assert_eq!(
        runtime
            .last_routed_viewport_identity_for_drag_session(Some(&session))
            .map(|identity| identity.window_id()),
        Some(target_opened.window().window_id()),
        "unavailable preview should not erase the previously recorded routed viewport identity"
    );
    let _ = source_opened;
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
    register_viewport(&mut adapter, source_space.clone(), source_window);
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
    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    let session = runtime.begin_payload_drag(&payload);

    let request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        screen_position_for_host_position(window_bounds, host_position),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(source_window),
    )
    .with_drag_session(Some(session.clone()));
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert!(
        matches!(resolution.route(), DockViewportDropRoute::Local { .. }),
        "fresh source viewport facts should resolve a local route, got {:?}",
        resolution.route()
    );
    assert!(
        resolution.routed_preview_target_snapshot().is_some(),
        "fresh local route should carry a preview target"
    );
    assert!(
        resolution.delivery().is_none(),
        "fresh local route must not mint delivery before target acceptance"
    );
    let accepted_resolution = accepted_preview_delivery_for_test(
        &mut runtime,
        &request,
        &source_space,
        source_window,
        cx,
    );
    let delivery = accepted_resolution.expect_delivery().clone();
    let commit_plan = DockDropDelivery::from_resolution(accepted_resolution)
        .expect("accepted preview should mint a plan");

    let (changed, _) = runtime.mark_viewport_window_snapshot_stale(source_window.window_id());
    assert!(changed);

    let validation = cx.update(|app| runtime.validate_payload_drop_delivery(&delivery, app));
    assert_eq!(validation, Err(DockActionApplyError::DropTargetUnavailable));
    let result =
        cx.update(|app| runtime.deliver_drop_commit_delivery_with_outcome(commit_plan, app));
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
    register_viewport(&mut adapter, target_space.clone(), target_window);
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

    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    let session = runtime.begin_payload_drag(&payload);
    let request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
    )
    .with_drag_session(Some(session.clone()));
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

    let result = DockDropDelivery::from_resolution(resolution);
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
    register_viewport(&mut adapter, target_space.clone(), target_window);
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
    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    let session = runtime.begin_payload_drag(&payload);

    let request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        screen_position_for_host_position(window_bounds, host_position),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
    )
    .with_drag_session(Some(session.clone()));
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert!(
        resolution.routed_preview_target_snapshot().is_some(),
        "preview should capture the accepted central target"
    );
    let resolved_target = resolution
        .routed_preview_target_snapshot()
        .map(|snapshot| snapshot.target())
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
    let accepted_resolution = accepted_preview_delivery_for_test(
        &mut runtime,
        &request,
        &target_space,
        target_window,
        cx,
    );
    let commit_plan = DockDropDelivery::from_resolution(accepted_resolution)
        .expect("accepted preview should mint a plan");

    controller.update(cx, |controller, _| {
        controller
            .policy_mut()
            .set_allow_central_region_dock_over(false);
    });

    let result =
        cx.update(|app| runtime.deliver_drop_commit_delivery_with_outcome(commit_plan, app));
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
fn viewport_runtime_revalidates_accepted_preview_release_against_current_policy(
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

    let target_window = handle(24);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, target_space.clone(), target_window);
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
        screen_position_for_host_position(window_bounds, host_position),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
    )
    .with_drag_session(Some(session));

    let preview_resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert!(
        matches!(
            preview_resolution.route(),
            DockViewportDropRoute::KnownViewport { target, .. }
                if target.window_id() == target_window.window_id()
        ),
        "preview setup should resolve the target viewport before policy changes"
    );
    let (changed, _) = runtime.update_routed_drop_preview(&preview_resolution, "Panel A");
    assert!(changed);
    assert!(runtime.finish_routed_drop_acceptance_pass(&target_space, target_window.window_id()));

    controller.update(cx, |controller, _| {
        controller
            .policy_mut()
            .set_allow_central_region_dock_over(false);
    });

    let release_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery_for_request(&request, app));
    assert_eq!(
        release_resolution.route(),
        &DockViewportDropRoute::Rejected(DockPolicyError::CentralRegionDockOverDisabled),
        "accepted preview release must not reuse a stale KnownViewport route after policy changes"
    );
    assert!(
        release_resolution.delivery().is_none(),
        "policy-rejected release must not carry a commit delivery"
    );
    assert!(
        release_resolution.preview_target().is_some(),
        "policy-rejected release should retain a preview target for rejected feedback"
    );
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
    register_viewport(&mut adapter, target_space.clone(), target_window);
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
    let session = runtime.begin_payload_drag(&payload);
    let request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
    )
    .with_drag_session(Some(session.clone()));
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
    let (preview_changed, preview_windows) =
        runtime.update_routed_drop_preview(&resolution, "Panel A");
    assert!(preview_changed);
    assert_eq!(preview_windows, vec![target_window]);
    let preview = runtime
        .routed_drop_preview_for(&target_space, target_window.window_id())
        .expect("policy-rejected route should render a target-window preview");
    assert!(preview.preview.rejected);
    assert!(!preview.preview.payload_tab);
    assert!(
        runtime.has_routed_drop_preview_for_drag_session(Some(&session)),
        "rejected routed previews should stay scoped to the active drag session"
    );

    let result = DockDropDelivery::from_resolution(resolution);
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
    let (finished, _) = runtime.finish_payload_drag(&session);
    assert!(finished);
    assert_eq!(
        runtime.routed_drop_preview_for(&target_space, target_window.window_id()),
        None,
        "finishing the drag must clear rejected routed previews even though they are not commit-capable"
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
fn viewport_runtime_hovered_host_release_uses_backend_focus_stamp_when_stack_unavailable(
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
    let runtime = DockViewportRuntimeHandle::new(controller);

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
    focus_backend_window_for_test(target_opened.window(), cx);
    assert!(
        cx.update(|app| runtime.reconcile_backend_window_focus(app)),
        "backend focus reconciliation should stamp the focused target viewport"
    );

    let platform_signals = cx.update(|app| {
        crate::DockViewportPlatformSignals::from_app_without_hovered_window_authority(app)
    });
    let request = DockViewportDropRouteRequest::from_platform_signals(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        platform_signals,
    );
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));

    let DockViewportDropRoute::KnownViewport { target, authority } = resolution.route() else {
        panic!(
            "when hovered-window and platform stack are unavailable, ImGui-style focus stamps should provide a known-viewport route, got {:?}",
            resolution.route()
        );
    };
    assert_eq!(target.space(), &target_space);
    assert_eq!(target.window_id(), target_opened.window().window_id());
    assert_eq!(target.host_position(), point(px(120.0), px(100.0)));
    assert_eq!(
        *authority,
        crate::DockViewportAuthorizedRouteAuthority::FrontToBackWindowStackFallback
    );
    assert!(
        resolution.routed_preview_target_snapshot().is_some(),
        "focus-stamp fallback should resolve the target preview without minting release delivery"
    );
    assert!(
        resolution.delivery().is_none(),
        "fallback preview authority must still wait for target render acceptance before delivery"
    );
    let _ = source_opened;
}

#[open_gpui::test]
fn viewport_runtime_source_only_release_uses_current_backend_fallback_not_last_routed_viewport(
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
        DockViewportTargetContext::new().with_trusted_hovered_window(target_opened.window()),
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
    let request_without_hovered_or_stack = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        release_position,
        None,
        DockViewportTargetContext::new(),
    );
    let geometry_only_route = cx.update(|app| {
        runtime
            .resolve_payload_drop_delivery(&request_without_hovered_or_stack, app)
            .route()
            .clone()
    });
    let request_with_stack = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        release_position,
        None,
        DockViewportTargetContext::new()
            .with_window_stack([source_opened.window(), target_opened.window()]),
    );
    let stack_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&request_with_stack, app));
    assert_eq!(
        geometry_only_route,
        DockViewportDropRoute::Unavailable,
        "empty target context must not use geometry or reuse preview state as route authority"
    );

    assert_eq!(
        stack_resolution.route(),
        &DockViewportDropRoute::Local {
            host_position: point(px(120.0), px(100.0)),
            window_id: source_opened.window().window_id(),
            facts_generation: 1,
            authority: crate::DockViewportAuthorizedRouteAuthority::FrontToBackWindowStackFallback,
        },
        "window-stack fallback must use the current stack instead of reusing the previewed target"
    );
    assert!(
        stack_resolution.routed_preview_target_snapshot().is_some(),
        "current stack fallback should still resolve a preview target"
    );
    assert!(
        stack_resolution.delivery().is_none(),
        "current stack fallback must not mint delivery before target acceptance"
    );

    let request_with_hovered = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        release_position,
        None,
        DockViewportTargetContext::new()
            .with_trusted_hovered_window(target_opened.window())
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
        source_only_route,
        stack_resolution.route().clone(),
        "source-only release must use the current window-stack fallback route, not last-routed preview state"
    );
}

#[open_gpui::test]
fn viewport_runtime_hovered_host_release_uses_last_hovered_viewport_when_hover_backend_unavailable(
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
    let source_window = handle(1);
    let target_window = handle(2);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, source_space.clone(), source_window);
    register_viewport(&mut adapter, target_space.clone(), target_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller,
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let source_window_bounds = WindowBounds::Windowed(floating_bounds(0.0, 0.0, 360.0, 220.0));
    let target_window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));

    let target_host_position = point(px(120.0), px(100.0));
    let target_screen_position = point(
        target_window_bounds.get_bounds().origin.x + target_host_position.x,
        target_window_bounds.get_bounds().origin.y + target_host_position.y,
    );
    assert!(runtime.begin_viewport_host_scene(
        source_space.clone(),
        source_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(source_window_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        point(px(0.0), px(0.0)),
    ));
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(target_window_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        target_host_position,
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
    let preview_request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        target_screen_position,
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
    )
    .with_drag_session(Some(session.clone()));
    let preview_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
    runtime.update_routed_drop_preview(&preview_resolution, "Panel A");

    let release_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        target_screen_position,
        None,
        crate::DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new())
            .with_event_receiver_window(target_window),
        DockPayloadDropReleaseOrigin::HoveredHost,
    )
    .with_drag_session(Some(session));
    let release_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery_for_request(&release_request, app));

    assert!(
        matches!(
            release_resolution.route(),
            DockViewportDropRoute::KnownViewport { target, authority }
                if target.window_id() == target_window.window_id()
                    && target.host_position() == target_host_position
                    && *authority
                        == crate::DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow
        ),
        "when hovered-window authority is unavailable, active drag should reuse the last hovered viewport as mouse reference; got {:?}",
        release_resolution.route()
    );
    assert!(
        release_resolution.delivery().is_none(),
        "last hovered viewport authority selects a preview route but must not mint delivery without accepted preview"
    );
}

#[open_gpui::test]
fn viewport_runtime_source_only_release_does_not_use_last_hovered_viewport_as_delivery_authority(
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
    let source_window = handle(1);
    let target_window = handle(2);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, source_space.clone(), source_window);
    register_viewport(&mut adapter, target_space.clone(), target_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller,
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let target_window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let target_host_position = point(px(120.0), px(100.0));
    let target_screen_position = point(
        target_window_bounds.get_bounds().origin.x + target_host_position.x,
        target_window_bounds.get_bounds().origin.y + target_host_position.y,
    );
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(target_window_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        target_host_position,
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
    let preview_request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        target_screen_position,
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
    )
    .with_drag_session(Some(session.clone()));
    let preview_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
    runtime.update_routed_drop_preview(&preview_resolution, "Panel A");

    let release_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        target_screen_position,
        None,
        crate::DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new()),
        DockPayloadDropReleaseOrigin::SourceOnly,
    )
    .with_drag_session(Some(session));
    let release_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery_for_request(&release_request, app));

    assert_eq!(
        release_resolution.route(),
        &DockViewportDropRoute::Unavailable,
        "source-only releases must not treat last hovered viewport as fresh hovered-window authority"
    );
    assert!(
        release_resolution.delivery().is_none(),
        "source-only last-hovered fallback must not mint cross-viewport delivery"
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
        DockViewportTargetContext::new().with_trusted_hovered_window(target_opened.window()),
    )
    .with_drag_session(Some(session.clone()));
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
    cx.update(|app| {
        runtime.update_routed_drop_preview(&resolution, "Panel A", app);
    });
    assert!(
        runtime.has_routed_drop_preview_for_drag_session(Some(&session)),
        "preview should cache a routed delivery before release"
    );
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_opened.window().window_id(),
        DockViewportWindowFacts::from_window_bounds(target_window_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        preview_host_position,
    ));

    let release_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        screen_position_for_host_position(
            target_window_bounds,
            center_drop_position(floating_bounds(180.0, 120.0, 180.0, 100.0)),
        ),
        None,
        crate::DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new()
                .with_window_stack([target_opened.window(), source_opened.window()]),
        ),
        DockPayloadDropReleaseOrigin::SourceOnly,
    )
    .with_drag_session(Some(session.clone()));
    let release_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&release_request, app));
    assert_eq!(
        release_resolution.route(),
        &DockViewportDropRoute::Unavailable,
        "release should be retargeted to the current point instead of reusing cached host_position"
    );
    let result = DockDropDelivery::from_resolution(release_resolution);
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
fn viewport_runtime_source_only_release_does_not_replay_unaccepted_routed_preview(
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

    let target_window = handle(90);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, target_space.clone(), target_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller,
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let target_window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(target_window_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        point(px(120.0), px(100.0)),
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
    let preview_request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
    )
    .with_drag_session(Some(session.clone()));
    let preview_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
    let (changed, _) = runtime.update_routed_drop_preview(&preview_resolution, "Panel A");
    assert!(changed);

    let release_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        crate::DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new()),
        DockPayloadDropReleaseOrigin::SourceOnly,
    )
    .with_drag_session(Some(session));
    let release_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&release_request, app));

    assert_eq!(
        release_resolution.route(),
        &DockViewportDropRoute::Unavailable,
        "unaccepted routed preview must not authorize source-only replay"
    );
    assert!(release_resolution.delivery().is_none());
}

#[open_gpui::test]
fn viewport_runtime_source_only_release_requires_current_routed_preview_acceptance(
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

    let target_window = handle(91);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, target_space.clone(), target_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller,
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let target_window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(target_window_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        point(px(120.0), px(100.0)),
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
    let preview_request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
    )
    .with_drag_session(Some(session.clone()));
    let preview_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
    let (changed, windows) = runtime.update_routed_drop_preview(&preview_resolution, "Panel A");
    assert!(changed);
    assert_eq!(windows, vec![target_window]);
    assert!(runtime.finish_routed_drop_acceptance_pass(&target_space, target_window.window_id()));

    let (changed, windows) = runtime.update_routed_drop_preview(&preview_resolution, "Panel A");
    assert!(
        !changed,
        "re-publishing the same preview should not report a visual preview change"
    );
    assert_eq!(
        windows,
        vec![target_window],
        "a new acceptance pass must refresh the target window even when the preview is visually unchanged"
    );

    let release_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        crate::DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new()),
        DockPayloadDropReleaseOrigin::SourceOnly,
    )
    .with_drag_session(Some(session));
    let release_without_current_acceptance =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&release_request, app));

    assert_eq!(
        release_without_current_acceptance.route(),
        &DockViewportDropRoute::Unavailable,
        "a stale routed-preview acceptance must not authorize source-only replay"
    );
    assert!(release_without_current_acceptance.delivery().is_none());

    assert!(runtime.finish_routed_drop_acceptance_pass(&target_space, target_window.window_id()));
    let release_after_current_acceptance =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&release_request, app));
    assert!(
        matches!(
            release_after_current_acceptance.route(),
            DockViewportDropRoute::KnownViewport { target, authority }
                if target.window_id() == target_window.window_id()
                    && *authority
                        == crate::DockViewportAuthorizedRouteAuthority::AcceptedRoutedPreview
        ),
        "the target may replay only after accepting the current routed-preview pass"
    );
    assert!(release_after_current_acceptance.delivery().is_some());
}

#[open_gpui::test]
fn viewport_runtime_source_only_release_replays_after_explicit_acceptance_despite_scene_fact_refresh(
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

    let target_window = handle(94);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, target_space.clone(), target_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller,
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let target_window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
    let host_position = point(px(120.0), px(100.0));
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(target_window_bounds),
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
    let preview_request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
    )
    .with_drag_session(Some(session.clone()));
    let preview_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
    let (changed, _) = runtime.update_routed_drop_preview(&preview_resolution, "Panel A");
    assert!(changed);
    let preview_frame = preview_resolution
        .routed_preview_target_snapshot()
        .expect("preview should resolve a target snapshot")
        .frame()
        .clone();
    let updated_frame = runtime
        .push_viewport_host_scene_frame_fact(
            &preview_frame,
            leaf_host_scene_fact(target_tabs, target_tabs),
        )
        .expect("a fresh target render should advance the host-scene frame generation");
    assert_ne!(
        preview_frame, updated_frame,
        "host-scene fact generation should advance independently from drag-drop acceptance"
    );

    let release_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        crate::DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new()),
        DockPayloadDropReleaseOrigin::SourceOnly,
    )
    .with_drag_session(Some(session.clone()));
    let release_without_render_acceptance =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&release_request, app));

    assert_eq!(
        release_without_render_acceptance.route(),
        &DockViewportDropRoute::Unavailable,
        "pushing host-scene facts must not accept a routed preview without the target render path"
    );
    assert!(release_without_render_acceptance.delivery().is_none());

    assert!(
        runtime.finish_routed_drop_acceptance_pass(&target_space, target_window.window_id()),
        "the target render path should accept the current routed-preview pass"
    );
    let release_after_render_acceptance =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&release_request, app));

    assert!(
        matches!(
            release_after_render_acceptance.route(),
            DockViewportDropRoute::KnownViewport { target, authority }
                if target.window_id() == target_window.window_id()
                    && *authority
                        == crate::DockViewportAuthorizedRouteAuthority::AcceptedRoutedPreview
        ),
        "accepted routed preview should replay through the stable target key even if scene fact generation changed"
    );
    assert!(release_after_render_acceptance.delivery().is_some());
}

#[open_gpui::test]
fn viewport_runtime_source_only_known_empty_hover_does_not_replay_accepted_routed_preview(
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

    let source_window_bounds = WindowBounds::Windowed(floating_bounds(520.0, 100.0, 360.0, 220.0));
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
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_opened.window()),
    )
    .with_drag_session(Some(session.clone()));
    let preview_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
    cx.update(|app| {
        runtime.update_routed_drop_preview(&preview_resolution, "Panel A", app);
    });
    assert!(
        runtime
            .finish_routed_drop_acceptance_pass(&target_space, target_opened.window().window_id())
    );

    let release_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        crate::DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new()
                .with_trusted_hovered_window_known_empty()
                .with_window_stack([source_opened.window()]),
        ),
        DockPayloadDropReleaseOrigin::SourceOnly,
    )
    .with_drag_session(Some(session));
    let release_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&release_request, app));

    assert_eq!(
        release_resolution.route(),
        &DockViewportDropRoute::Unavailable,
        "trusted hovered=None is authoritative and must not replay an old routed preview target"
    );
    assert!(
        release_resolution.delivery().is_none(),
        "trusted hovered=None must not mint delivery from an accepted preview"
    );
}

#[open_gpui::test]
fn viewport_runtime_hovered_host_known_empty_hover_does_not_replay_accepted_routed_preview(
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

    let target_window = handle(91);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, target_space.clone(), target_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller,
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let target_window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(target_window_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        point(px(120.0), px(100.0)),
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
    let target_screen_position = point(px(220.0), px(200.0));
    let preview_request = DockViewportDropRouteRequest::from_target_context(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        target_screen_position,
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
    )
    .with_drag_session(Some(session.clone()));
    let preview_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
    let (changed, _) = runtime.update_routed_drop_preview(&preview_resolution, "Panel A");
    assert!(changed);
    assert!(runtime.finish_routed_drop_acceptance_pass(&target_space, target_window.window_id()));

    let release_request = DockViewportDropRouteRequest::from_target_context(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        target_screen_position,
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
    )
    .with_drag_session(Some(session));
    let release_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&release_request, app));

    assert_eq!(
        release_resolution.route(),
        &DockViewportDropRoute::Unavailable,
        "trusted hovered=None is authoritative on hovered-host release and must not replay an old accepted target"
    );
    assert!(
        release_resolution.delivery().is_none(),
        "trusted hovered=None must not mint delivery from an accepted preview"
    );
}

#[open_gpui::test]
fn viewport_runtime_source_only_unavailable_hover_replays_accepted_routed_preview(
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
    let runtime = DockViewportRuntimeHandle::new(controller);

    let target_window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
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
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_opened.window()),
    )
    .with_drag_session(Some(session.clone()));
    let preview_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
    cx.update(|app| {
        runtime.update_routed_drop_preview(&preview_resolution, "Panel A", app);
    });
    assert!(
        runtime
            .finish_routed_drop_acceptance_pass(&target_space, target_opened.window().window_id())
    );

    let release_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        crate::DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new()),
        DockPayloadDropReleaseOrigin::SourceOnly,
    )
    .with_drag_session(Some(session));
    let release_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&release_request, app));

    assert!(
        matches!(
            release_resolution.route(),
            DockViewportDropRoute::KnownViewport { target, authority }
                if target.window_id() == target_opened.window().window_id()
                    && *authority
                        == crate::DockViewportAuthorizedRouteAuthority::AcceptedRoutedPreview
        ),
        "active drag with unavailable hovered-window data should replay the accepted routed preview target"
    );
    assert!(release_resolution.delivery().is_some());
}

#[open_gpui::test]
fn viewport_runtime_accepted_preview_does_not_replay_through_front_viewport_window(
    cx: &mut TestAppContext,
) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let front_space = DockSpaceId::from("front");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let front_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_tabs);
    graph.set_root(front_space.clone(), front_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));

    let target_window = handle(92);
    let front_window = handle(93);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, target_space.clone(), target_window);
    register_viewport(&mut adapter, front_space.clone(), front_window);
    let mut runtime = DockViewportRuntime::from_adapter(
        controller,
        adapter,
        DockViewportClosePolicy::RetainLayout,
    );

    let shared_window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
    assert!(runtime.begin_viewport_host_scene(
        target_space.clone(),
        target_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(shared_window_bounds),
        floating_bounds(0.0, 0.0, 360.0, 220.0),
        point(px(120.0), px(100.0)),
    ));
    assert!(runtime.push_viewport_host_scene_fact(
        &target_space,
        target_window.window_id(),
        leaf_host_scene_fact(target_tabs, target_tabs),
    ));
    assert!(runtime.begin_viewport_host_scene(
        front_space.clone(),
        front_window.window_id(),
        DockViewportWindowFacts::from_window_bounds(shared_window_bounds),
        floating_bounds(320.0, 180.0, 20.0, 20.0),
        point(px(0.0), px(0.0)),
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
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
    )
    .with_drag_session(Some(session.clone()));
    let preview_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&preview_request, app));
    let (changed, _) = runtime.update_routed_drop_preview(&preview_resolution, "Panel A");
    assert!(changed);
    assert!(runtime.finish_routed_drop_acceptance_pass(&target_space, target_window.window_id()));

    let release_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        crate::DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new().with_window_stack([front_window, target_window]),
        ),
        DockPayloadDropReleaseOrigin::SourceOnly,
    )
    .with_drag_session(Some(session));
    let release_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&release_request, app));

    assert_eq!(
        release_resolution.route(),
        &DockViewportDropRoute::Unavailable,
        "accepted routed preview must not replay through a front viewport window that contains the pointer but has no host target"
    );
    assert!(release_resolution.delivery().is_none());
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
    register_viewport(&mut adapter, target_space.clone(), target_window);
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
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
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
    runtime.update_routed_drop_preview(&resolution, "Panel A");
    assert!(runtime.has_routed_drop_preview_for_drag_session(Some(&session)));
    assert!(!runtime.has_routed_drop_preview_for_drag_session(None));

    let local_resolution = DockViewportResolvedDropRoute::new(
        DockViewportDropRoute::Local {
            host_position: target_position,
            window_id: target_window.window_id(),
            facts_generation: 1,
            authority: crate::DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
        },
        None,
    );
    runtime.update_routed_drop_preview(&local_resolution, "Panel A");
    assert!(!runtime.has_routed_drop_preview_for_drag_session(Some(&session)));

    runtime.update_routed_drop_preview(&resolution, "Panel A");

    let (changed, _) = runtime.finish_payload_drag(&session);
    assert!(changed);
    assert!(!runtime.has_routed_drop_preview_for_drag_session(Some(&session)));

    let next_session = runtime.begin_payload_drag(&payload);
    assert_ne!(next_session.id(), session.id());
    let next_request = DockViewportDropRouteRequest::from_target_context(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(220.0), px(200.0)),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
    )
    .with_drag_session(Some(next_session.clone()));
    let next_resolution =
        cx.update(|app| runtime.resolve_payload_drop_delivery(&next_request, app));
    runtime.update_routed_drop_preview(&next_resolution, "Panel A");
    assert!(runtime.has_routed_drop_preview_for_drag_session(Some(&next_session)));
    assert!(!runtime.has_routed_drop_preview_for_drag_session(Some(&session)));
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
    register_viewport(&mut adapter, target_space.clone(), target_window);
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
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
    )
    .with_drag_session(Some(first_session.clone()));
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    runtime.update_routed_drop_preview(&resolution, "Panel A");

    assert!(
        runtime
            .routed_drop_preview_for(&target_space, target_window.window_id())
            .is_some()
    );
    assert!(runtime.has_routed_drop_preview_for_drag_session(Some(&first_session)));

    let second_payload =
        DockDragPayload::new_item(source_space, source_tabs, item("c"), "C".to_string());
    let second_session = runtime.begin_payload_drag(&second_payload);
    assert_ne!(second_session.id(), first_session.id());
    assert_eq!(
        runtime.routed_drop_preview_for(&target_space, target_window.window_id()),
        None
    );
    assert!(!runtime.has_routed_drop_preview_for_drag_session(Some(&first_session)));
}

#[open_gpui::test]
fn viewport_runtime_rejects_known_viewport_delivery_without_drag_session(cx: &mut TestAppContext) {
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

    let target_window = handle(20);
    let mut adapter = DockViewportAdapter::new();
    register_viewport(&mut adapter, target_space.clone(), target_window);
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
        screen_position_for_host_position(window_bounds, host_position),
        None,
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
    );
    let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
    assert!(
        resolution.routed_preview_target_snapshot().is_some(),
        "fresh known viewport route should carry a preview target"
    );
    assert_eq!(
        DockDropDelivery::from_resolution(resolution),
        Err(DockActionApplyError::DropTargetUnavailable),
        "fresh known viewport route must not mint delivery without accepted preview"
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
    register_viewport(&mut adapter, target_space.clone(), target_window);
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
        DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
    )
    .with_drag_session(Some(stale_session.clone()));
    let accepted_resolution = accepted_preview_delivery_for_test(
        &mut runtime,
        &request,
        &target_space,
        target_window,
        cx,
    );
    let stale_plan = DockDropDelivery::from_resolution(accepted_resolution)
        .expect("accepted preview should mint a plan");

    let _replacement = runtime.begin_payload_drag(&payload);
    let result =
        cx.update(|app| runtime.deliver_drop_commit_delivery_with_outcome(stale_plan, app));
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
fn viewport_runtime_rejects_tear_off_delivery_without_drag_session(cx: &mut TestAppContext) {
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

    let request = DockViewportTearOffRequest::new(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(900.0), px(900.0)),
        None,
    );

    let result = cx.update(|app| runtime.prepare_tear_off_drop_delivery(request, app));
    assert!(matches!(
        result,
        Err(DockActionApplyError::DropDragSessionMissing)
    ));
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("a")]
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
fn viewport_runtime_rejects_tear_off_delivery_without_authoritative_placement(
    cx: &mut TestAppContext,
) {
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
    let session = runtime.begin_payload_drag(&payload);
    let request = DockViewportTearOffRequest::new(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        None,
        None,
    )
    .with_drag_session(Some(session));

    let result = cx.update(|app| runtime.prepare_tear_off_drop_delivery(request, app));
    assert_eq!(
        result.expect_err("tear-off without authoritative placement must be rejected"),
        DockActionApplyError::TearOffViewportPlacementUnavailable
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("a")]
        );
    });
}

#[open_gpui::test]
fn viewport_runtime_rejects_prepared_tear_off_when_target_policy_rejects_payload(
    cx: &mut TestAppContext,
) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("source:tear-off:a:0");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(source_space.clone(), source_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace
        .policy_mut()
        .set_allowed_dock_classes_for_space(target_space.clone(), ["inspector"]);
    workspace.register_panel_descriptor(
        item("a"),
        crate::DockPanelDescriptor::new("Panel A").with_dock_class("editor"),
    );
    let controller = cx.new(|_| DockController::new(workspace));
    let mut runtime = DockViewportRuntime::new(controller.clone());
    let geometry = DockDragTearOffGeometry::from_source_bounds(
        floating_bounds(200.0, 120.0, 480.0, 300.0),
        point(px(260.0), px(150.0)),
    );

    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    let session = runtime.begin_payload_drag(&payload);
    let request = DockViewportTearOffRequest::new(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        point(px(900.0), px(900.0)),
        None,
    )
    .with_tear_off_geometry(Some(geometry))
    .with_drag_session(Some(session));

    let err = cx
        .update(|app| runtime.prepare_tear_off_drop_delivery(request, app))
        .expect_err("dock class policy should reject prepared tear-off");
    assert_eq!(
        err,
        DockActionApplyError::Policy(crate::DockPolicyError::DockClassRejected {
            space: target_space.clone(),
            item: item("a"),
            dock_class: Some(DockClassId::from("editor")),
        })
    );
    assert_eq!(
        runtime.pending_tear_off_len(),
        0,
        "preflight rejection must not create pending tear-off state"
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("a")]
        );
        assert_eq!(controller.graph().root(&target_space), None);
    });
}

#[open_gpui::test]
fn viewport_runtime_prepared_tear_off_freezes_focus_item(cx: &mut TestAppContext) {
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
    let geometry = DockDragTearOffGeometry::from_source_bounds(
        floating_bounds(200.0, 120.0, 480.0, 300.0),
        point(px(260.0), px(150.0)),
    );
    runtime
        .borrow_mut()
        .record_panel_focus(source_space.clone(), item("c"));

    let payload = DockDragPayload::new_tabs(source_space.clone(), source_tabs, "Stack".to_string());
    let session = cx.update(|app| runtime.begin_payload_drag_with_app(&payload, app));
    let request = DockViewportTearOffRequest::new(
        source_space.clone(),
        source_tabs,
        DockViewportDropPayload::Tabs,
        point(px(900.0), px(900.0)),
        None,
    )
    .with_tear_off_geometry(Some(geometry))
    .with_drag_session(Some(session));

    let prepared = cx
        .update(|app| {
            runtime
                .borrow_mut()
                .prepare_tear_off_drop_delivery(request, app)
        })
        .expect("active drag session should prepare tear-off delivery");
    assert_eq!(
        prepared.focus_item,
        Some(item("c")),
        "prepared tear-off should freeze focus from the delivery snapshot"
    );

    controller.update(cx, |controller, _| {
        controller
            .select_tab(source_tabs, item("a"))
            .expect("test should be able to change selected tab after preparation");
    });

    assert_eq!(
        prepared.focus_item,
        Some(item("c")),
        "later selected-tab changes must not rewrite prepared tear-off focus"
    );
}

#[open_gpui::test]
fn viewport_runtime_prepared_tear_off_does_not_infer_selected_tab_focus(cx: &mut TestAppContext) {
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
    let runtime = DockViewportRuntimeHandle::new(controller);
    let geometry = DockDragTearOffGeometry::from_source_bounds(
        floating_bounds(200.0, 120.0, 480.0, 300.0),
        point(px(260.0), px(150.0)),
    );

    let payload = DockDragPayload::new_tabs(source_space.clone(), source_tabs, "Stack".to_string());
    let session = cx.update(|app| runtime.begin_payload_drag_with_app(&payload, app));
    let request = DockViewportTearOffRequest::new(
        source_space,
        source_tabs,
        DockViewportDropPayload::Tabs,
        point(px(900.0), px(900.0)),
        None,
    )
    .with_tear_off_geometry(Some(geometry))
    .with_drag_session(Some(session));

    let prepared = cx
        .update(|app| {
            runtime
                .borrow_mut()
                .prepare_tear_off_drop_delivery(request, app)
        })
        .expect("active drag session should prepare tear-off delivery");
    assert_eq!(
        prepared.focus_item, None,
        "selected tab alone is not a recorded focus identity"
    );
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
fn viewport_runtime_tear_off_without_geometry_rejects_release_point_only(cx: &mut TestAppContext) {
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

    assert_eq!(runtime.tear_off_window_placement(&request), None);
    assert_eq!(
        runtime.tear_off_window_options(&request).expect_err(
            "missing authoritative tear-off placement should be rejected before opening a window"
        ),
        DockActionApplyError::TearOffViewportPlacementUnavailable
    );
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

    let placement = runtime
        .tear_off_window_placement(&request)
        .expect("global release point and drag geometry should produce tear-off placement");
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
fn viewport_runtime_tear_off_preserves_small_drag_geometry_without_minimum_size(
    cx: &mut TestAppContext,
) {
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
        floating_bounds(200.0, 120.0, 120.0, 90.0),
        point(px(40.0), px(30.0)),
    );
    let request = DockViewportTearOffRequest::new(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        release_position,
        None,
    )
    .with_tear_off_geometry(Some(geometry));

    let placement = runtime
        .tear_off_window_placement(&request)
        .expect("small drag geometry should still produce tear-off placement");
    assert_eq!(
        placement.source(),
        DockViewportTearOffPlacementSource::DragGeometry
    );
    assert_eq!(
        placement.window_bounds(),
        WindowBounds::Windowed(floating_bounds(900.0, 900.0, 120.0, 90.0))
    );
}

#[open_gpui::test]
fn viewport_runtime_tear_off_without_global_release_point_does_not_use_drag_position(
    cx: &mut TestAppContext,
) {
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
    );
    let request = DockViewportTearOffRequest::new(
        source_space,
        source_tabs,
        DockViewportDropPayload::Item(item("a")),
        None,
        None,
    )
    .with_tear_off_geometry(Some(geometry));

    assert_eq!(
        runtime.tear_off_window_placement(&request),
        None,
        "host-local/receiver-local release positions must not be used as screen coordinates"
    );
    assert_eq!(
        runtime.tear_off_window_options(&request).expect_err(
            "missing authoritative platform-window placement must reject before opening"
        ),
        DockActionApplyError::TearOffViewportPlacementUnavailable
    );
}

#[open_gpui::test]
fn viewport_runtime_tear_off_suggested_bounds_authorize_missing_global_release_point(
    cx: &mut TestAppContext,
) {
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
        None,
        Some(suggested),
    )
    .with_tear_off_geometry(Some(geometry));

    let placement = runtime
        .tear_off_window_placement(&request)
        .expect("host-suggested bounds should authorize tear-off placement without global release");
    assert_eq!(
        placement.source(),
        DockViewportTearOffPlacementSource::Suggested
    );
    assert_eq!(placement.window_bounds(), suggested);
    let options = runtime
        .tear_off_window_options(&request)
        .expect("suggested bounds should produce window options");
    assert_eq!(options.window_bounds, Some(suggested));
    assert!(
        !options.focus,
        "tear-off windows must not take focus before graph commit and runtime activation"
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

    let placement = runtime
        .tear_off_window_placement(&request)
        .expect("suggested bounds should produce tear-off placement");
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

    let placement = runtime
        .tear_off_window_placement(&request)
        .expect("global release point and drag geometry should produce tear-off placement");
    assert_eq!(
        placement.source(),
        DockViewportTearOffPlacementSource::DragGeometry
    );
    assert_eq!(
        placement.window_bounds(),
        WindowBounds::Windowed(floating_bounds(520.0, 500.0, 480.0, 300.0))
    );
}
