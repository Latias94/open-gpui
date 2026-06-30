use crate::{
    DockViewportPlatformFlagRequests, DockViewportPlatformSyncAction,
    DockViewportPlatformSyncRequest, DockViewportPlatformSyncUnsupportedReason,
    host_test_support::{test_view, viewport_window_options},
    sync_reused_viewport_window, unsupported_viewport_platform_flag_requests,
    viewport_registry::DockViewportPlatformRequests,
};
use open_gpui::{
    PlatformViewportCapabilities, PlatformViewportFlagCapabilities, TestAppContext,
    WindowBackgroundAppearance, WindowOptions,
};

#[test]
fn viewport_flag_requests_default_to_unsupported_without_backend_capability() {
    let unsupported = unsupported_viewport_platform_flag_requests(
        DockViewportPlatformFlagRequests::default()
            .with_no_focus_on_appearing(true)
            .with_no_focus_on_click(true)
            .with_alpha(Some(0.6))
            .with_topmost(true)
            .with_no_taskbar(true),
        PlatformViewportFlagCapabilities::default(),
    );

    let requests = unsupported
        .iter()
        .map(|unsupported| &unsupported.request)
        .collect::<Vec<_>>();
    assert!(requests.contains(
        &&DockViewportPlatformSyncRequest::ViewportFlagNoFocusOnAppearing { requested: true }
    ));
    assert!(requests.contains(
        &&DockViewportPlatformSyncRequest::ViewportFlagNoFocusOnClick { requested: true }
    ));
    assert!(
        requests.contains(&&DockViewportPlatformSyncRequest::ViewportFlagAlpha { requested: 0.6 })
    );
    assert!(
        requests
            .contains(&&DockViewportPlatformSyncRequest::ViewportFlagTopMost { requested: true })
    );
    assert!(
        requests
            .contains(&&DockViewportPlatformSyncRequest::ViewportFlagNoTaskbar { requested: true })
    );
    assert!(
        unsupported.iter().all(|unsupported| {
            unsupported.reason == DockViewportPlatformSyncUnsupportedReason::UnsupportedByWindowApi
        }),
        "unsupported viewport flags should be explicit diagnostics, not silently treated as applied"
    );
}

#[test]
fn viewport_flag_requests_respect_advertised_backend_capabilities() {
    let unsupported = unsupported_viewport_platform_flag_requests(
        DockViewportPlatformFlagRequests::default()
            .with_no_focus_on_appearing(true)
            .with_no_focus_on_click(true)
            .with_alpha(Some(0.6))
            .with_topmost(true)
            .with_no_taskbar(true),
        PlatformViewportFlagCapabilities {
            no_focus_on_appearing_windows: true,
            no_focus_on_click_windows: true,
            alpha_windows: true,
            topmost_windows: true,
            no_taskbar_windows: true,
        },
    );

    assert!(
        unsupported.is_empty(),
        "backend-supported viewport flags should leave no unsupported diagnostics"
    );
}

#[open_gpui::test]
fn reused_viewport_no_input_request_records_unsupported_without_backend_capability(
    cx: &mut TestAppContext,
) {
    let root = test_view(cx, "viewport");
    let window = cx
        .update(|app| app.open_window(viewport_window_options(320.0, 240.0), |_, _| root.clone()))
        .expect("test window should open");

    let sync_record = window
        .update(cx, |_, window, _| {
            sync_reused_viewport_window(
                window,
                WindowOptions {
                    focus: false,
                    accepts_pointer_input: false,
                    ..viewport_window_options(320.0, 240.0)
                },
                DockViewportPlatformRequests::default(),
                PlatformViewportCapabilities::default(),
                PlatformViewportFlagCapabilities::default(),
            )
        })
        .expect("test window should stay live");

    assert_eq!(sync_record.applied, Vec::new());
    assert!(
        sync_record
            .unsupported_requests
            .iter()
            .any(|unsupported| unsupported.request
                == DockViewportPlatformSyncRequest::PointerInput { requested: false })
    );
    assert!(
        sync_record
            .unsupported_requests
            .iter()
            .any(|unsupported| unsupported.request
                == DockViewportPlatformSyncRequest::ViewportFlagNoInputs { requested: true })
    );
}

#[open_gpui::test]
fn reused_transparent_background_records_alpha_unsupported_without_backend_capability(
    cx: &mut TestAppContext,
) {
    let root = test_view(cx, "viewport");
    let window = cx
        .update(|app| app.open_window(viewport_window_options(320.0, 240.0), |_, _| root.clone()))
        .expect("test window should open");

    let sync_record = window
        .update(cx, |_, window, _| {
            sync_reused_viewport_window(
                window,
                WindowOptions {
                    window_background: WindowBackgroundAppearance::Transparent,
                    ..viewport_window_options(320.0, 240.0)
                },
                DockViewportPlatformRequests::default(),
                PlatformViewportCapabilities::default(),
                PlatformViewportFlagCapabilities::default(),
            )
        })
        .expect("test window should stay live");

    assert!(!sync_record.applied.iter().any(|action| matches!(
        action,
        DockViewportPlatformSyncAction::BackgroundAppearance { .. }
    )));
    assert!(
        sync_record
            .unsupported_requests
            .iter()
            .any(|unsupported| matches!(
                unsupported.request,
                DockViewportPlatformSyncRequest::ViewportFlagAlpha { .. }
            ))
    );
}

// Mechanical migration: platform_capability viewport runtime suites.
mod runtime_suite {
    #![allow(dead_code, unused_imports)]

    use crate::{
        DockActionApplyError, DockActionOutcome, DockClassId, DockController, DockDropDelivery,
        DockFloatingContainer, DockGraph, DockHost, DockItemId, DockNode, DockPanel,
        DockPolicyError, DockSpaceId, DockViewportAdapter, DockViewportClosePolicy,
        DockViewportCloseStatus, DockViewportDropPayload, DockViewportDropRoute,
        DockViewportDropRouteOutcome, DockViewportDropRouteRequest, DockViewportFocusCommand,
        DockViewportFocusRequest, DockViewportInputStatus, DockViewportOpenStatus,
        DockViewportPlatformSyncAction, DockViewportPlatformSyncRequest,
        DockViewportPlatformSyncSkippedReason, DockViewportPlatformSyncUnsupportedReason,
        DockViewportResolvedDropRoute, DockViewportRouteStatus, DockViewportRouteTarget,
        DockViewportRuntime, DockViewportRuntimeHandle, DockViewportShouldCloseStatus,
        DockViewportTargetContext, DockViewportTearOffOpenOutcome, DockViewportTearOffOutcomeKind,
        DockViewportTearOffPlacementSource, DockViewportTearOffRequest,
        DockViewportWindowActivation, DockViewportWindowFacts, DockWorkspace, SplitAxis,
        drag::{DockDragPayload, DockDragTearOffGeometry},
        drop_runtime::DockHostDropSceneFact,
        drop_target::DockLeafDropTarget,
        host_test_support::*,
        interaction::DockPayloadDropReleaseOrigin,
        unavailable_reused_viewport_window_sync,
        viewport_activation::{
            DockViewportActivationApplyOutcome, DockViewportActivationBackendFocusApply,
            DockViewportActivationBackendFocusObservation,
            DockViewportActivationBackendFocusRecordEffect,
            DockViewportActivationPendingBackendFocusEffect, apply_viewport_activation_transaction,
        },
        viewport_registry::{
            DockViewportInputMask, DockViewportRouteUnavailableReason, DockViewportStaleReason,
        },
        viewport_tear_off::{DockViewportTearOffBeginOutcome, DockViewportTearOffCancelReason},
        viewport_test_support::{handle, register_viewport},
    };
    use open_gpui::{
        AnyWindowHandle, AppContext as _, Focusable, SharedString, TestAppContext, TitlebarOptions,
        VisualTestContext, WindowBounds, WindowHandle, WindowId, WindowOptions, point, px, size,
    };

    use crate::host_viewport_runtime_test_support::*;

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
        let mut runtime = DockViewportRuntime::from_adapter(
            controller,
            adapter,
            DockViewportClosePolicy::default(),
        );
        let payload =
            DockDragPayload::new_item(source, source_tabs, item("drag"), "Drag".to_string());

        let begin = runtime.begin_payload_drag_with_pointer_sync_and_focus(&payload, None);
        assert_eq!(
            begin
                .pointer_input_sync
                .map(|request| request.requested_accepts_pointer_input()),
            None,
            "an already no-input source window should not be re-requested as click-through"
        );

        let finish_update = runtime.finish_payload_drag_with_pointer_sync(&begin.session);
        assert_eq!(
            finish_update
                .pointer_input_sync()
                .map(|request| (request.window(), request.requested_accepts_pointer_input())),
            Some((window, false)),
            "drag finish should restore the source window's original no-input state"
        );
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
        let runtime = DockViewportRuntimeHandle::new(controller.clone());

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
        assert!(
            sync.applied
                .contains(&DockViewportPlatformSyncAction::ViewportFlagNoInputs { enabled: true })
        );
        assert!(!sync.unsupported_requests.iter().any(|unsupported| {
            unsupported.request
                == DockViewportPlatformSyncRequest::PointerInput { requested: false }
        }));
        assert!(!sync.unsupported_requests.iter().any(|unsupported| {
            unsupported.request
                == DockViewportPlatformSyncRequest::ViewportFlagNoInputs { requested: true }
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
                DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(
                    floating_bounds(0.0, 0.0, 520.0, 300.0),
                )),
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

    #[test]
    fn unavailable_reused_viewport_window_sync_records_diagnostic() {
        let window = crate::viewport_test_support::handle(42);
        let sync = unavailable_reused_viewport_window_sync(window.window_id());

        assert_eq!(sync.window_id, window.window_id());
        assert!(sync.applied.is_empty());
        assert!(sync.skipped_requests.is_empty());
        assert_eq!(sync.unsupported_requests.len(), 1);
        assert_eq!(
            sync.unsupported_requests[0].request,
            DockViewportPlatformSyncRequest::WindowUnavailable
        );
        assert_eq!(
            sync.unsupported_requests[0].reason,
            DockViewportPlatformSyncUnsupportedReason::WindowUnavailable
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
}

mod handle_suite {
    #![allow(dead_code, unused_imports)]

    use crate::{
        DockAction, DockActionApplyError, DockController, DockDropDelivery, DockGraph,
        DockGraphDropTarget, DockItemId, DockNode, DockNodeId, DockPanel, DockPolicy, DockSpaceId,
        DockViewportClosePolicy, DockViewportCloseStatus, DockViewportDropOutcomeKind,
        DockViewportDropPayload, DockViewportDropRoute, DockViewportDropRouteOutcome,
        DockViewportDropRouteRequest, DockViewportFocusCommand, DockViewportFocusRequest,
        DockViewportInputStatus, DockViewportOpenStatus, DockViewportPlatformSignals,
        DockViewportRouteStatus, DockViewportRuntimeHandle, DockViewportShouldCloseStatus,
        DockViewportStaleStatusReason, DockViewportTargetContext, DockViewportTearOffBeginOutcome,
        DockViewportTearOffCancelReason, DockViewportTearOffOpenOutcome,
        DockViewportTearOffRequest, DockViewportWindowFacts, DockWorkspace, DropZone, SplitAxis,
        debug::DockDebugRegion,
        drag::DockDragPayload,
        drop_preview::DockDropRoutePreviewKind,
        drop_runtime::{DockHostDropScene, DockHostDropSceneFact},
        drop_target::{DockDropResolveSource, DockLeafDropTarget, DockResolvedDropTargetKind},
        host_test_support::*,
        interaction::{
            DockPayloadDropRelease, DockPayloadDropReleaseOrigin, DockRuntimeDragSession,
        },
        viewport_activation::apply_viewport_activation_transaction,
        viewport_registry::{DockViewportRouteUnavailableReason, DockViewportStaleReason},
    };
    use open_gpui::{
        AppContext as _, Focusable, Modifiers, MouseButton, TestAppContext, VisualTestContext,
        WindowBounds, WindowOptions, point, px, size,
    };
    use slotmap::Key;

    use crate::host_viewport_runtime_test_support::*;

    #[open_gpui::test]
    fn viewport_pointer_input_sync_request_does_not_change_route_facts_until_observed(
        cx: &mut TestAppContext,
    ) {
        let source = DockSpaceId::from("source");
        let source_tabs = DockNodeId::null();
        let mut workspace = DockWorkspace::new(source.clone(), DockGraph::new());
        workspace.register_panel_view(item("drag"), "Drag", test_view(cx, "Drag"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let payload = DockDragPayload::new_item(
            source.clone(),
            source_tabs,
            item("drag"),
            "Drag".to_string(),
        );
        let opened = cx
            .update(|app| {
                runtime.open_viewport(
                    source.clone(),
                    WindowOptions {
                        accepts_pointer_input: true,
                        ..viewport_window_options(360.0, 220.0)
                    },
                    app,
                )
            })
            .expect("drag source viewport should open");
        let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
        assert!(runtime.begin_viewport_host_scene(
            source.clone(),
            opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
                0.0, 0.0, 360.0, 220.0,
            ))),
            host_bounds,
            center_drop_position(host_bounds),
        ));
        assert_eq!(runtime.viewport_route_unavailable_reason(&source), None);

        let begin = runtime
            .borrow_mut()
            .begin_payload_drag_with_pointer_sync_and_focus(&payload, None);

        assert_eq!(
            begin
                .pointer_input_sync
                .map(|request| (request.window(), request.requested_accepts_pointer_input())),
            Some((opened.window(), false)),
            "drag begin should request source-window click-through without treating the request as observed state"
        );
        assert_eq!(
            runtime.viewport_route_unavailable_reason(&source),
            None,
            "route facts should remain routable until a refreshed window fact observes native no-input"
        );
        assert!(
            runtime
                .borrow_mut()
                .finish_payload_drag(&begin.session)
                .changed()
        );
    }

    #[open_gpui::test]
    fn viewport_drag_preserves_no_input_source_window(cx: &mut TestAppContext) {
        let source = DockSpaceId::from("source");
        let source_tabs = DockNodeId::null();
        let mut workspace = DockWorkspace::new(source.clone(), DockGraph::new());
        workspace.register_panel_view(item("drag"), "Drag", test_view(cx, "Drag"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller);
        let payload = DockDragPayload::new_item(
            source.clone(),
            source_tabs,
            item("drag"),
            "Drag".to_string(),
        );

        let opened = cx
            .update(|app| {
                runtime.open_viewport(
                    source.clone(),
                    WindowOptions {
                        accepts_pointer_input: false,
                        ..viewport_window_options(360.0, 220.0)
                    },
                    app,
                )
            })
            .expect("no-input source viewport should open");
        assert!(
            runtime.begin_viewport_host_scene(
                source.clone(),
                opened.window().window_id(),
                DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(
                    floating_bounds(0.0, 0.0, 360.0, 220.0,)
                ))
                .with_input_mask(
                    crate::viewport_registry::DockViewportInputMask::NoInputPassThrough
                ),
                floating_bounds(0.0, 0.0, 360.0, 220.0),
                center_drop_position(floating_bounds(0.0, 0.0, 360.0, 220.0)),
            )
        );
        assert_eq!(runtime.viewport_route_unavailable_reason(&source), None);
        assert_eq!(
            viewport_input_status(&runtime, &source),
            Some(DockViewportInputStatus::NoInputPassThrough)
        );

        let session = cx.update(|app| runtime.begin_payload_drag_with_app(&payload, app));
        assert!(
            !opened
                .window()
                .update(cx, |_, window, _| window.accepts_pointer_input())
                .expect("source viewport should remain live"),
            "drag begin must not enable or re-toggle an already no-input source window"
        );

        assert!(cx.update(|app| runtime.finish_payload_drag_with_app(&session, app)));
        assert!(
            !opened
                .window()
                .update(cx, |_, window, _| window.accepts_pointer_input())
                .expect("source viewport should remain live"),
            "drag finish must restore the source window's original no-input state"
        );
        assert_eq!(runtime.viewport_route_unavailable_reason(&source), None);
        assert_eq!(
            viewport_input_status(&runtime, &source),
            Some(DockViewportInputStatus::NoInputPassThrough)
        );
    }
}
