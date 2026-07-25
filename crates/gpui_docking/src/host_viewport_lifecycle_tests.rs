//! Concern-owned viewport lifecycle regression tests.

mod runtime_suite {
    #![allow(dead_code, unused_imports)]

    use crate::{
        DockActionApplyError, DockActionOutcome, DockClassId, DockController, DockDropDelivery,
        DockGraph, DockHost, DockItemId, DockNode, DockPanel, DockPolicyError, DockSpaceId,
        DockViewportAdapter, DockViewportClosePolicy, DockViewportCloseStatus,
        DockViewportDropPayload, DockViewportDropRoute, DockViewportDropRouteOutcome,
        DockViewportDropRouteRequest, DockViewportFocusCommand, DockViewportFocusRequest,
        DockViewportInputStatus, DockViewportOpenStatus, DockViewportPlatformSyncAction,
        DockViewportPlatformSyncRequest, DockViewportPlatformSyncSkippedReason,
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
        viewport_activation::{
            DockViewportActivationApplyOutcome, DockViewportActivationBackendFocusApply,
            DockViewportActivationBackendFocusObservation,
            DockViewportActivationBackendFocusRecordEffect,
            DockViewportActivationPendingBackendFocusEffect, apply_viewport_activation_transaction,
        },
        viewport_drop_scene::DockViewportHostSceneSnapshot,
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
    fn viewport_runtime_render_registered_viewport_records_window_binding(cx: &mut TestAppContext) {
        let alpha_space = DockSpaceId::from("alpha");
        let zeta_space = DockSpaceId::from("zeta");
        let fixture = DockViewportRuntimeFixture::builder(alpha_space.clone())
            .space(alpha_space.clone(), ["a"])
            .space(zeta_space.clone(), ["z"])
            .build(cx);
        let runtime = fixture.runtime.clone();
        let alpha_window = handle(1);
        let zeta_window = handle(2);

        assert!(
            runtime
                .borrow_mut()
                .register_rendered_host_viewport(alpha_space.clone(), alpha_window)
        );
        assert!(
            runtime
                .borrow_mut()
                .register_rendered_host_viewport(zeta_space.clone(), zeta_window)
        );

        assert_eq!(
            runtime.borrow().adapter().window_for_space(&alpha_space),
            Some(alpha_window)
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&zeta_space),
            Some(zeta_window)
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_render_registered_viewport_stamps_focus_fallback_order(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(target_space.clone(), ["b"])
            .build(cx);
        let source_tabs = fixture.tabs(&source_space);
        let target_tabs = fixture.tabs(&target_space);
        let runtime = fixture.runtime.clone();
        let target_window = handle(77);
        let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
        let host_position = point(px(120.0), px(100.0));

        assert!(
            runtime
                .borrow_mut()
                .register_rendered_host_viewport(target_space.clone(), target_window)
        );
        assert!(runtime.borrow_mut().begin_viewport_host_scene(
            target_space.clone(),
            target_window.window_id(),
            DockViewportWindowFacts::from_window_bounds(window_bounds),
            host_bounds,
            host_position,
        ));
        assert!(runtime.borrow_mut().push_viewport_host_scene_fact(
            &target_space,
            target_window.window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));
        assert_eq!(
            runtime
                .borrow()
                .adapter()
                .window_can_route_hover_hit(target_window.window_id()),
            Some(true),
            "render-registered viewport should be route-ready before focus-stamp fallback is resolved"
        );

        let platform_signals = cx
            .update(|app| {
                crate::DockViewportPlatformSignals::from_app_without_target_window_signals(app)
            })
            .with_event_receiver_window(target_window);
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source_space,
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            point(px(220.0), px(200.0)),
            None,
            platform_signals,
        );
        let resolution = cx.update(|app| {
            runtime
                .borrow_mut()
                .resolve_payload_drop_delivery(&request, app)
        });

        assert!(
            matches!(
                resolution.route(),
                DockViewportDropRoute::KnownViewport { target, source }
                    if target.space() == &target_space
                        && target.window_id() == target_window.window_id()
                        && *source
                            == crate::DockViewportRouteSelectionSource::FocusStampWindowStackFallback
            ),
            "render-registered viewports should enter ImGui-style z-order fallback, got {:?}",
            resolution.route()
        );
        assert!(resolution.routed_preview_target_snapshot().is_some());
        assert!(
            resolution.delivery().is_some(),
            "focus-stamp fallback should mint delivery from current route facts"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_render_registration_cleans_replaced_space_state(cx: &mut TestAppContext) {
        let alpha_space = DockSpaceId::from("alpha");
        let zeta_space = DockSpaceId::from("zeta");
        let fixture = DockViewportRuntimeFixture::builder(alpha_space.clone())
            .space(alpha_space.clone(), ["a"])
            .space(zeta_space.clone(), ["z"])
            .build(cx);
        let alpha_tabs = fixture.tabs(&alpha_space);
        let runtime = fixture.runtime.clone();
        let window = handle(3);

        assert!(
            runtime
                .borrow_mut()
                .register_rendered_host_viewport(alpha_space.clone(), window)
        );
        assert!(runtime.borrow_mut().begin_viewport_host_scene(
            alpha_space.clone(),
            window.window_id(),
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
                100.0, 100.0, 360.0, 220.0,
            ))),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            point(px(120.0), px(100.0)),
        ));
        assert!(runtime.borrow_mut().push_viewport_host_scene_fact(
            &alpha_space,
            window.window_id(),
            leaf_host_scene_fact(alpha_tabs, alpha_tabs),
        ));
        runtime.record_panel_focus(alpha_space.clone(), item("a"));

        assert!(
            runtime
                .borrow()
                .last_host_scene_screen_position(&alpha_space)
                .is_some()
        );
        assert_eq!(
            runtime
                .borrow()
                .recorded_had_panel_focus_for_test(&alpha_space),
            Some(true)
        );

        assert!(
            runtime
                .borrow_mut()
                .register_rendered_host_viewport(zeta_space.clone(), window)
        );

        assert_eq!(
            runtime.borrow().adapter().window_for_space(&alpha_space),
            None
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&zeta_space),
            Some(window)
        );
        assert_eq!(
            runtime
                .borrow()
                .last_host_scene_screen_position(&alpha_space),
            None
        );
        assert_eq!(
            runtime
                .borrow()
                .recorded_had_panel_focus_for_test(&alpha_space),
            None
        );
        assert!(
            !runtime.borrow_mut().push_viewport_host_scene_fact(
                &alpha_space,
                window.window_id(),
                leaf_host_scene_fact(alpha_tabs, alpha_tabs),
            ),
            "replaced rendered-host mapping must reject stale facts for the old space"
        );
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
        let mut runtime = DockViewportRuntime::from_adapter(
            controller,
            adapter,
            DockViewportClosePolicy::Prevent,
        );

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
        let fixture = DockViewportRuntimeFixture::builder(main_space.clone())
            .space(main_space.clone(), ["a"])
            .build(cx);
        let runtime = fixture.runtime.clone();
        let opened = fixture.open_unfocused_viewport(cx, &main_space);
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
        let fixture = DockViewportRuntimeFixture::builder(main_space.clone())
            .space(main_space.clone(), ["a"])
            .build(cx);
        let runtime = fixture.runtime.clone();
        let first = fixture.open_unfocused_viewport(cx, &main_space);
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

        let second = fixture.open_unfocused_viewport(cx, &main_space);
        focus_backend_window_for_test(second.window(), cx);
        assert_eq!(
            cx.update(|app| {
                runtime.focus_command_for_confirmed_backend_window_focus(
                    &main_space,
                    second.window().window_id(),
                    false,
                    app,
                )
            }),
            None,
            "initial live replacement focus should consume the ImGui-style initial suppression gate"
        );
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
        let fixture = DockViewportRuntimeFixture::builder(alpha_space.clone())
            .space(alpha_space.clone(), ["a"])
            .space(zeta_space.clone(), ["z"])
            .build(cx);
        let runtime = fixture.runtime.clone();
        let alpha = fixture.open_unfocused_viewport(cx, &alpha_space);
        let zeta = fixture.open_unfocused_viewport(cx, &zeta_space);
        runtime.record_panel_focus(alpha_space.clone(), item("a"));
        runtime.record_panel_focus(zeta_space.clone(), item("z"));

        focus_backend_window_for_test(alpha.window(), cx);
        assert_eq!(
            cx.update(|app| {
                runtime.focus_command_for_confirmed_backend_window_focus(
                    &alpha_space,
                    alpha.window().window_id(),
                    false,
                    app,
                )
            }),
            None,
            "initial backend focus suppresses ordinary platform restore once"
        );
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
        let fixture = DockViewportRuntimeFixture::builder(main_space.clone())
            .space(main_space.clone(), ["a"])
            .space(detached_space.clone(), ["c"])
            .build(cx);
        let runtime = fixture.runtime.clone();
        let main = fixture.open_unfocused_viewport(cx, &main_space);
        let detached = fixture.open_unfocused_viewport(cx, &detached_space);
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
    fn closing_non_last_confirmed_backend_focused_viewport_does_not_suppress_platform_focus_restore(
        cx: &mut TestAppContext,
    ) {
        let main_space = DockSpaceId::from("main");
        let detached_space = DockSpaceId::from("detached");
        let fixture = DockViewportRuntimeFixture::builder(main_space.clone())
            .space(main_space.clone(), ["a"])
            .space(detached_space.clone(), ["c"])
            .build(cx);
        let runtime = fixture.runtime.clone();
        let main = fixture.open_unfocused_viewport(cx, &main_space);
        let detached = fixture.open_unfocused_viewport(cx, &detached_space);
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
        let fixture = DockViewportRuntimeFixture::builder(main_space.clone())
            .space(main_space.clone(), ["a"])
            .space(detached_space.clone(), ["c"])
            .build(cx);
        let runtime = fixture.runtime.clone();
        let main = fixture.open_unfocused_viewport(cx, &main_space);
        let detached = fixture.open_unfocused_viewport(cx, &detached_space);
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
        let fixture = DockViewportRuntimeFixture::builder(main_space.clone())
            .space(main_space.clone(), ["a"])
            .space(detached_space.clone(), ["c"])
            .build(cx);
        let runtime = fixture.runtime.clone();
        let main = fixture.open_unfocused_viewport(cx, &main_space);
        let detached = fixture.open_unfocused_viewport(cx, &detached_space);
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
        let fixture = DockViewportRuntimeFixture::builder(main_space.clone())
            .space(main_space.clone(), ["a"])
            .space(detached_space.clone(), ["c"])
            .build(cx);
        let runtime = fixture.runtime.clone();
        let main = fixture.open_unfocused_viewport(cx, &main_space);
        let detached = fixture.open_unfocused_viewport(cx, &detached_space);
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
        let fixture = DockViewportRuntimeFixture::builder(main_space.clone())
            .space(main_space.clone(), ["a"])
            .build(cx);
        let runtime = fixture.runtime.clone();
        let main = fixture.open_unfocused_viewport(cx, &main_space);
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
        let fixture = DockViewportRuntimeFixture::builder(main_space.clone())
            .space(main_space.clone(), ["a"])
            .space(detached_space.clone(), ["c"])
            .build(cx);
        let runtime = fixture.runtime.clone();
        let main = fixture.open_unfocused_viewport(cx, &main_space);
        let detached = fixture.open_unfocused_viewport(cx, &detached_space);
        let plain_root = test_view(cx, "Plain");
        let non_docking = cx
            .update(|app| {
                let plain_root = plain_root.clone();
                app.open_window(unfocused_viewport_window_options(), move |_, _| plain_root)
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
        let fixture = DockViewportRuntimeFixture::builder(main_space.clone())
            .space(main_space.clone(), ["a"])
            .build(cx);
        let runtime = fixture.runtime.clone();
        let opened = fixture.open_unfocused_viewport(cx, &main_space);
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
    fn backend_focus_unavailable_does_not_consume_pending_viewport_activation(
        cx: &mut TestAppContext,
    ) {
        let main_space = DockSpaceId::from("main");
        let fixture = DockViewportRuntimeFixture::builder(main_space.clone())
            .space(main_space.clone(), ["a"])
            .build(cx);
        let runtime = fixture.runtime.clone();
        let opened = fixture.open_unfocused_viewport(cx, &main_space);
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
        let fixture = DockViewportRuntimeFixture::builder(main_space.clone())
            .space(main_space.clone(), ["a"])
            .space(detached_space.clone(), ["c"])
            .build(cx);
        let runtime = fixture.runtime.clone();
        let main = fixture.open_unfocused_viewport(cx, &main_space);
        let detached = fixture.open_unfocused_viewport(cx, &detached_space);
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
        let fixture = DockViewportRuntimeFixture::builder(main_space.clone())
            .space(main_space.clone(), ["a"])
            .build(cx);
        let runtime = fixture.runtime.clone();
        let opened = fixture.open_unfocused_viewport(cx, &main_space);
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
        let fixture = DockViewportRuntimeFixture::builder(main_space.clone())
            .space(main_space.clone(), ["a"])
            .build(cx);
        let runtime = fixture.runtime.clone();
        let opened = fixture.open_unfocused_viewport(cx, &main_space);

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
    fn viewport_runtime_rebinding_window_to_new_space_discards_old_space_focus(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(target_space.clone(), ["b"])
            .build(cx);
        let runtime = fixture.runtime.clone();
        let opened = fixture.open_unfocused_viewport(cx, &source_space);
        runtime.record_panel_focus(source_space.clone(), item("a"));
        runtime.record_panel_focus(target_space.clone(), item("b"));

        assert_eq!(
            runtime
                .borrow_mut()
                .register_opened_viewport(target_space.clone(), opened.window()),
            Vec::new()
        );

        assert_eq!(
            runtime.borrow().adapter().window_for_space(&source_space),
            None
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&target_space),
            Some(opened.window())
        );
        assert_eq!(
            runtime
                .borrow()
                .recorded_had_panel_focus_for_test(&source_space),
            None,
            "moving a native window to another logical space must retire the old space focus state"
        );
        assert_eq!(
            runtime
                .borrow()
                .recorded_had_panel_focus_for_test(&target_space),
            Some(true),
            "rebind cleanup must not discard the target space focus state"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_floating_payload_focus_requires_recorded_focus(cx: &mut TestAppContext) {
        let primary_space = DockSpaceId::from("primary");
        let detached_space = DockSpaceId::from("detached");
        let (graph, floating) = horizontal_split_floating_graph(primary_space.clone(), None);

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
        let (graph, floating) = horizontal_split_floating_graph(primary_space.clone(), None);

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
        let fixture = DockViewportRuntimeFixture::builder(detached_space.clone())
            .space(detached_space.clone(), ["a"])
            .build_controller(cx);
        let window = handle(149);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, detached_space.clone(), window);
        let mut runtime = DockViewportRuntime::from_adapter(
            fixture.controller.clone(),
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
    fn viewport_runtime_render_registration_rebinds_same_window_to_new_space_and_clears_old_state(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(target_space.clone(), ["b"])
            .build_controller(cx);
        let source_tabs = fixture.tabs(&source_space);
        let mut runtime = DockViewportRuntime::new(fixture.controller.clone());

        let window = handle(73);
        assert!(runtime.register_rendered_host_viewport(source_space.clone(), window));
        DockViewportHostSceneSeed::new(source_space.clone(), window, source_tabs)
            .publish_runtime(&mut runtime);
        runtime.record_panel_focus(source_space.clone(), item("a"));

        let replaced = runtime.register_rendered_host_viewport(target_space.clone(), window);
        assert!(replaced);

        assert_eq!(runtime.adapter().window_for_space(&source_space), None);
        assert_eq!(
            runtime.adapter().window_for_space(&target_space),
            Some(window)
        );
        assert_eq!(runtime.last_host_scene_screen_position(&source_space), None);
        assert_eq!(
            runtime.recorded_had_panel_focus_for_test(&source_space),
            None
        );
        assert!(
            !runtime.push_viewport_host_scene_fact(
                &source_space,
                window.window_id(),
                leaf_host_scene_fact(source_tabs, source_tabs),
            ),
            "rebound rendered-host mapping must reject stale facts for the old space"
        );
        assert_eq!(
            runtime.recorded_had_panel_focus_for_test(&target_space),
            None,
            "rebinding a window to a new space should not invent target focus history"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_host_release_finishes_source_drag_session(cx: &mut TestAppContext) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(target_space.clone(), ["b"])
            .build_controller(cx);
        let source_tabs = fixture.tabs(&source_space);
        let target_tabs = fixture.tabs(&target_space);
        let mut runtime = DockViewportRuntime::new(fixture.controller.clone());
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
    fn viewport_runtime_unregister_host_for_space_clears_runtime_state(cx: &mut TestAppContext) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(target_space.clone(), ["b"])
            .build_controller(cx);
        let source_tabs = fixture.tabs(&source_space);
        let target_tabs = fixture.tabs(&target_space);

        let target_window = handle(93);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        let mut runtime = DockViewportRuntime::from_adapter(
            fixture.controller.clone(),
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
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(target_space.clone(), ["b"])
            .build_controller(cx);
        let source_tabs = fixture.tabs(&source_space);
        let target_tabs = fixture.tabs(&target_space);
        let controller = fixture.controller.clone();

        let old_window = handle(10);
        let new_window = handle(11);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), old_window);
        let mut runtime = DockViewportRuntime::from_adapter(
            controller.clone(),
            adapter,
            DockViewportClosePolicy::RetainLayout,
        );

        let old_scene =
            DockViewportHostSceneSeed::new(target_space.clone(), old_window, target_tabs);
        let release_position = old_scene.screen_position();
        old_scene.publish_runtime(&mut runtime);
        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);

        let request = hovered_window_route_request_for_test(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            None,
            old_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(session.clone()));
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        let stale_plan = DockDropDelivery::from_resolution(resolution)
            .expect("fresh route should mint a commit plan");

        runtime.register_opened_viewport(target_space.clone(), new_window);
        DockViewportHostSceneSeed::new(target_space.clone(), new_window, target_tabs)
            .publish_runtime(&mut runtime);

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
    fn viewport_runtime_rejects_delivery_after_current_host_scene_frame_changes(
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
        let replacement_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("c")],
            selected: Some(item("c")),
        });
        let target_root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![target_tabs, replacement_tabs],
            fractions: vec![0.5, 0.5],
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_root);

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
        let controller = cx.new(|_| DockController::new(workspace));

        let target_window = handle(122);
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
            leaf_host_scene_fact(target_root, target_tabs),
        ));

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let request = hovered_window_route_request_for_test(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            screen_position_for_host_position(window_bounds, host_position),
            None,
            target_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(session.clone()));
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        let commit_plan =
            DockDropDelivery::from_resolution(resolution).expect("fresh route should mint a plan");

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
            leaf_host_scene_fact(target_root, replacement_tabs),
        ));
        let current_target = cx
            .update(|app| runtime.resolve_host_scene_target(&target_space, host_position, app))
            .expect("new frame should still resolve a valid host-scene target");
        assert!(
            matches!(
                current_target.kind,
                crate::drop_target::DockResolvedDropTargetKind::LeafCenter { target_tabs, .. }
                    if target_tabs == replacement_tabs
            ),
            "test setup should replace the current host-scene target before committing stale delivery"
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
                vec![item("b"), item("c")]
            );
        });
    }

    #[open_gpui::test]
    fn viewport_runtime_tabs_drop_uses_recorded_payload_focus(cx: &mut TestAppContext) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space_selected(source_space.clone(), ["a", "c"], "c")
            .space(target_space.clone(), ["b"])
            .build(cx);
        let source_tabs = fixture.tabs(&source_space);
        let target_tabs = fixture.tabs(&target_space);
        let controller = fixture.controller.clone();
        let runtime = fixture.runtime.clone();
        runtime.record_panel_focus(source_space.clone(), item("a"));
        let opened =
            fixture.open_viewport(cx, &target_space, viewport_window_options(360.0, 220.0));
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

        let payload =
            DockDragPayload::new_tabs(source_space.clone(), source_tabs, "Stack".to_string());
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
        let preview_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        cx.update(|app| {
            runtime.update_routed_drop_preview(&preview_resolution, &payload, app);
        });
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
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(target_space.clone(), ["b"])
            .build_controller(cx);
        let source_tabs = fixture.tabs(&source_space);
        let target_tabs = fixture.tabs(&target_space);
        let controller = fixture.controller.clone();

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

        let request = hovered_window_route_request_for_test(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            point(px(220.0), px(200.0)),
            None,
            target_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
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
            resolution.delivery().is_some(),
            "fresh route should mint delivery from current route facts"
        );
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        let commit_plan =
            DockDropDelivery::from_resolution(resolution).expect("fresh route should mint a plan");
        assert!(
            runtime
                .mark_viewport_window_snapshot_stale(target_window.window_id())
                .changed()
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
    fn viewport_runtime_rejects_host_scene_resolution_after_window_facts_go_stale(
        cx: &mut TestAppContext,
    ) {
        let target_space = DockSpaceId::from("target");
        let fixture = DockViewportRuntimeFixture::builder(target_space.clone())
            .space(target_space.clone(), ["b"])
            .build_controller(cx);
        let target_tabs = fixture.tabs(&target_space);
        let mut runtime = DockViewportRuntime::new(fixture.controller.clone());
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

        assert!(
            runtime
                .mark_viewport_window_snapshot_stale(target_window.window_id())
                .changed()
        );
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
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(target_space.clone(), ["b"])
            .build_controller(cx);
        let source_tabs = fixture.tabs(&source_space);
        let controller = fixture.controller.clone();

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
        let update = runtime.update_routed_drop_preview(&resolution, &payload);
        assert!(!update.changed());
        assert!(update.into_windows().is_empty());

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
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(target_space.clone(), ["b"])
            .build_controller(cx);
        let source_tabs = fixture.tabs(&source_space);
        let target_tabs = fixture.tabs(&target_space);
        let controller = fixture.controller.clone();

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

        let request = hovered_window_route_request_for_test(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            screen_position_for_host_position(window_bounds, host_position),
            None,
            target_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(session.clone()));
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        assert!(
            resolution.routed_preview_target_snapshot().is_some(),
            "preview should capture the current central target"
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
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        let commit_plan =
            DockDropDelivery::from_resolution(resolution).expect("fresh route should mint a plan");

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
    fn viewport_runtime_hovered_host_release_uses_backend_focus_stamp_when_stack_unavailable(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let decoy_space = DockSpaceId::from("decoy");
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(target_space.clone(), ["b"])
            .space(decoy_space.clone(), ["c"])
            .build(cx);
        let source_tabs = fixture.tabs(&source_space);
        let target_tabs = fixture.tabs(&target_space);
        let decoy_tabs = fixture.tabs(&decoy_space);
        let runtime = fixture.runtime.clone();

        let source_window_bounds =
            WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let target_window_bounds =
            WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let decoy_window_bounds =
            WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let _ = fixture.open_viewport(
            cx,
            &source_space,
            WindowOptions {
                window_bounds: Some(source_window_bounds),
                focus: false,
                ..Default::default()
            },
        );
        let target_opened = fixture.open_viewport(
            cx,
            &target_space,
            WindowOptions {
                window_bounds: Some(target_window_bounds),
                focus: false,
                ..Default::default()
            },
        );
        let decoy_opened = fixture.open_viewport(
            cx,
            &decoy_space,
            WindowOptions {
                window_bounds: Some(decoy_window_bounds),
                focus: false,
                ..Default::default()
            },
        );

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
        assert!(runtime.begin_viewport_host_scene(
            decoy_space.clone(),
            decoy_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(decoy_window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            point(px(120.0), px(100.0)),
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &decoy_space,
            decoy_opened.window().window_id(),
            leaf_host_scene_fact(decoy_tabs, decoy_tabs),
        ));
        focus_backend_window_for_test(target_opened.window(), cx);

        let platform_signals = cx.update(|app| {
            crate::DockViewportPlatformSignals::from_app_without_target_window_signals(app)
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

        let DockViewportDropRoute::KnownViewport { target, source } = resolution.route() else {
            panic!(
                "route resolution should sample current backend focus before using ImGui-style focus stamps, got {:?}",
                resolution.route()
            );
        };
        assert_eq!(target.space(), &target_space);
        assert_eq!(target.window_id(), target_opened.window().window_id());
        assert_eq!(target.host_position(), point(px(120.0), px(100.0)));
        assert_eq!(
            *source,
            crate::DockViewportRouteSelectionSource::FocusStampWindowStackFallback
        );
        assert!(
            resolution.routed_preview_target_snapshot().is_some(),
            "focus-stamp fallback should resolve the target preview"
        );
        assert!(
            resolution.delivery().is_some(),
            "focus-stamp fallback should mint delivery from current route facts"
        );
    }

    #[open_gpui::test]
    fn viewport_activation_confirmed_backend_focus_updates_focus_stamp_fallback(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let decoy_space = DockSpaceId::from("decoy");
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(target_space.clone(), ["b"])
            .space(decoy_space.clone(), ["c"])
            .build(cx);
        let source_tabs = fixture.tabs(&source_space);
        let target_tabs = fixture.tabs(&target_space);
        let decoy_tabs = fixture.tabs(&decoy_space);
        let runtime = fixture.runtime.clone();
        let open_options = || WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(floating_bounds(
                100.0, 100.0, 360.0, 220.0,
            ))),
            focus: false,
            ..Default::default()
        };

        let _source_opened = fixture.open_viewport(cx, &source_space, open_options());
        let target_opened = fixture.open_viewport(cx, &target_space, open_options());
        let decoy_opened = fixture.open_viewport(cx, &decoy_space, open_options());
        let shared_window_bounds =
            WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let host_position = point(px(120.0), px(100.0));
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(shared_window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            host_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_opened.window().window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));
        assert!(runtime.begin_viewport_host_scene(
            decoy_space.clone(),
            decoy_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(shared_window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            host_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &decoy_space,
            decoy_opened.window().window_id(),
            leaf_host_scene_fact(decoy_tabs, decoy_tabs),
        ));

        focus_backend_window_for_test(decoy_opened.window(), cx);
        assert!(cx.update(|app| runtime.reconcile_backend_window_focus(app)));
        focus_backend_window_for_test(target_opened.window(), cx);
        let activation = crate::DockViewportActivationTransaction::new(
            target_space.clone(),
            target_opened.window(),
            DockViewportFocusRequest::panel(item("b")),
        );
        let outcome = cx.update(|app| apply_viewport_activation_transaction(Some(activation), app));
        let expected_backend_focus_apply = DockViewportActivationBackendFocusApply::new(
            DockViewportActivationBackendFocusRecordEffect::RecordedTargetFocus,
            DockViewportActivationPendingBackendFocusEffect::Unchanged,
        );
        assert!(
            matches!(
                outcome,
                DockViewportActivationApplyOutcome::Applied {
                    backend_focus: DockViewportActivationBackendFocusObservation::TargetFocused,
                    window_activation_requested: false,
                    backend_focus_apply,
                    ..
                } if backend_focus_apply == expected_backend_focus_apply
            ),
            "activation should apply while backend focus is already confirmed, got {outcome:?}"
        );

        let platform_signals = cx.update(|app| {
            crate::DockViewportPlatformSignals::from_app_without_target_window_signals(app)
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

        assert!(
            matches!(
                resolution.route(),
                DockViewportDropRoute::KnownViewport { target, source }
                    if target.space() == &target_space
                        && target.window_id() == target_opened.window().window_id()
                        && *source
                            == crate::DockViewportRouteSelectionSource::FocusStampWindowStackFallback
            ),
            "confirmed activation should stamp target focus for backend focus-stamp fallback, got {:?}",
            resolution.route()
        );
        assert!(
            resolution.delivery().is_some(),
            "focus-stamp fallback should mint delivery from current route facts"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_new_viewport_creation_stamps_focus_fallback_order(cx: &mut TestAppContext) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(target_space.clone(), ["b"])
            .build(cx);
        let source_tabs = fixture.tabs(&source_space);
        let target_tabs = fixture.tabs(&target_space);
        let runtime = fixture.runtime.clone();

        let source_window_bounds =
            WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let target_window_bounds =
            WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let _source_opened = fixture.open_viewport(
            cx,
            &source_space,
            WindowOptions {
                window_bounds: Some(source_window_bounds),
                focus: false,
                ..Default::default()
            },
        );
        let target_opened = fixture.open_viewport(
            cx,
            &target_space,
            WindowOptions {
                window_bounds: Some(target_window_bounds),
                focus: false,
                ..Default::default()
            },
        );

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

        let platform_signals = cx.update(|app| {
            crate::DockViewportPlatformSignals::from_app_without_target_window_signals(app)
        });
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            point(px(220.0), px(200.0)),
            None,
            platform_signals,
        );
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));

        let DockViewportDropRoute::KnownViewport { target, source } = resolution.route() else {
            panic!(
                "newly-created viewports should enter ImGui-style z-order fallback even before backend focus confirmation, got {:?}",
                resolution.route()
            );
        };
        assert_eq!(target.space(), &target_space);
        assert_eq!(target.window_id(), target_opened.window().window_id());
        assert_eq!(
            *source,
            crate::DockViewportRouteSelectionSource::FocusStampWindowStackFallback
        );
        assert!(resolution.routed_preview_target_snapshot().is_some());
        assert!(resolution.delivery().is_some());

        cx.set_platform_focused_window_available(false);
        let unavailable_platform_signals = cx.update(|app| {
            crate::DockViewportPlatformSignals::from_app_without_target_window_signals(app)
        });
        let unavailable_request = DockViewportDropRouteRequest::from_platform_signals(
            source_space,
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            point(px(220.0), px(200.0)),
            None,
            unavailable_platform_signals,
        );
        let unavailable_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&unavailable_request, app));

        assert_eq!(
            unavailable_resolution.route(),
            &DockViewportDropRoute::Unavailable,
            "focus-stamp fallback requires a focused-window backend signal; stale creation stamps alone must not select a viewport"
        );
        assert!(unavailable_resolution.delivery().is_none());
    }

    #[open_gpui::test]
    fn viewport_runtime_backend_focus_unavailable_clears_stale_focus_stamp_context(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(target_space.clone(), ["b"])
            .build_controller(cx);
        let source_tabs = fixture.tabs(&source_space);
        let target_tabs = fixture.tabs(&target_space);
        let mut runtime = DockViewportRuntime::new(fixture.controller.clone());
        let target_window = handle(77);
        let target_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let host_position = point(px(120.0), px(100.0));

        assert!(runtime.register_rendered_host_viewport(target_space.clone(), target_window));
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_window.window_id(),
            DockViewportWindowFacts::from_window_bounds(target_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            host_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_window.window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));
        cx.set_platform_focused_window_available(false);

        let request = DockViewportDropRouteRequest::from_target_context(
            source_space,
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            point(px(220.0), px(200.0)),
            None,
            DockViewportTargetContext::new()
                .with_focus_stamp_window_stack([target_window.window_id()]),
        );
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));

        assert_eq!(
            resolution.route(),
            &DockViewportDropRoute::Unavailable,
            "backend focus Unavailable must clear stale focus-stamp context before route selection is resolved"
        );
        assert!(resolution.delivery().is_none());
    }

    #[open_gpui::test]
    fn viewport_runtime_rebinding_window_stamps_focus_fallback_order(cx: &mut TestAppContext) {
        let source_space = DockSpaceId::from("source");
        let old_space = DockSpaceId::from("old");
        let target_space = DockSpaceId::from("target");
        let decoy_space = DockSpaceId::from("decoy");
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(old_space.clone(), ["old"])
            .space(target_space.clone(), ["b"])
            .space(decoy_space.clone(), ["c"])
            .build(cx);
        let source_tabs = fixture.tabs(&source_space);
        let target_tabs = fixture.tabs(&target_space);
        let decoy_tabs = fixture.tabs(&decoy_space);
        let runtime = fixture.runtime.clone();

        let window_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let open_options = || WindowOptions {
            window_bounds: Some(window_bounds),
            focus: false,
            ..Default::default()
        };

        let _source_opened = fixture.open_viewport(cx, &source_space, open_options());
        let rebound_window = fixture.open_viewport(cx, &old_space, open_options());
        let decoy_opened = fixture.open_viewport(cx, &decoy_space, open_options());

        assert_eq!(
            runtime
                .borrow_mut()
                .register_opened_viewport(target_space.clone(), rebound_window.window()),
            Vec::new()
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&old_space),
            None
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&target_space),
            Some(rebound_window.window())
        );

        let host_position = point(px(120.0), px(100.0));
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            rebound_window.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            host_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            rebound_window.window().window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));
        assert!(runtime.begin_viewport_host_scene(
            decoy_space.clone(),
            decoy_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(window_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            host_position,
        ));
        assert!(runtime.push_viewport_host_scene_fact(
            &decoy_space,
            decoy_opened.window().window_id(),
            leaf_host_scene_fact(decoy_tabs, decoy_tabs),
        ));

        let platform_signals = cx.update(|app| {
            crate::DockViewportPlatformSignals::from_app_without_target_window_signals(app)
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

        assert!(
            matches!(
                resolution.route(),
                DockViewportDropRoute::KnownViewport { target, source }
                    if target.space() == &target_space
                        && target.window_id() == rebound_window.window().window_id()
                        && *source
                            == crate::DockViewportRouteSelectionSource::FocusStampWindowStackFallback
            ),
            "rebinding a live window to a new logical viewport should stamp it as front-most fallback, got {:?}",
            resolution.route()
        );
        assert!(resolution.routed_preview_target_snapshot().is_some());
        assert!(resolution.delivery().is_some());
    }

    #[open_gpui::test]
    fn viewport_runtime_drag_geometry_is_bound_to_active_drag_session(cx: &mut TestAppContext) {
        let source_space = DockSpaceId::from("source");
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .build_controller(cx);
        let source_tabs = fixture.tabs(&source_space);
        let mut runtime = DockViewportRuntime::new(fixture.controller.clone());
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
        assert!(runtime.finish_payload_drag(&active_session).changed());
        assert_eq!(
            runtime.active_payload_drag_tear_off_geometry(Some(&active_session)),
            None,
            "finishing a drag must discard its geometry"
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
        viewport_drop_scene::DockViewportHostSceneSnapshot,
        viewport_registry::{DockViewportRouteUnavailableReason, DockViewportStaleReason},
    };
    use open_gpui::{
        AppContext as _, Focusable, Modifiers, MouseButton, TestAppContext, VisualTestContext,
        WindowBounds, WindowOptions, point, px, size,
    };
    use slotmap::Key;

    use crate::host_viewport_runtime_test_support::*;

    #[open_gpui::test]
    fn viewport_runtime_handle_tracks_payload_drag_session(cx: &mut TestAppContext) {
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
                runtime.open_viewport_unchecked_policy(
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

        let session = cx.update(|app| runtime.begin_payload_drag_with_app(&payload, app));
        assert_eq!(session.id(), 1);
        assert!(
            opened
                .window()
                .update(cx, |_, window, _| window.accepts_pointer_input())
                .expect("drag test window should remain live"),
            "payload drag should preserve source viewport pointer input"
        );
        assert_eq!(
            runtime.viewport_route_unavailable_reason(&source),
            None,
            "native no-input should not invalidate route facts"
        );
        assert_eq!(
            viewport_input_status(&runtime, &source),
            Some(DockViewportInputStatus::ReceivesInput),
            "payload drag should not rewrite the registered viewport input state"
        );
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

        assert!(cx.update(|app| runtime.finish_payload_drag_with_app(&session, app)));
        assert!(
            opened
                .window()
                .update(cx, |_, window, _| window.accepts_pointer_input())
                .expect("drag test window should remain live"),
            "drag finish should preserve the source viewport pointer input"
        );
        assert_eq!(
            runtime.viewport_route_unavailable_reason(&source),
            None,
            "drag finish should publish routable pointer state again"
        );
        assert_eq!(runtime.active_payload_drag_session(&payload), None);
        assert!(!cx.update(|app| runtime.finish_payload_drag_with_app(&session, app)));
    }

    #[open_gpui::test]
    fn viewport_runtime_handle_unregister_source_retires_drag_without_input_mutation(
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
                runtime.open_viewport_unchecked_policy(
                    source.clone(),
                    WindowOptions {
                        accepts_pointer_input: true,
                        ..viewport_window_options(360.0, 220.0)
                    },
                    app,
                )
            })
            .expect("drag source viewport should open");
        assert!(runtime.begin_viewport_host_scene(
            source.clone(),
            opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
                0.0, 0.0, 360.0, 220.0,
            ))),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            center_drop_position(floating_bounds(0.0, 0.0, 360.0, 220.0)),
        ));

        let session = cx.update(|app| runtime.begin_payload_drag_with_app(&payload, app));
        assert!(
            opened
                .window()
                .update(cx, |_, window, _| window.accepts_pointer_input())
                .expect("drag source viewport should remain live"),
            "payload drag should preserve the original source window input state"
        );

        assert!(cx.update(|app| {
            runtime.unregister_host_for_space_with_app(&source, opened.window().window_id(), app)
        }));
        assert!(
            opened
                .window()
                .update(cx, |_, window, _| window.accepts_pointer_input())
                .expect("drag source viewport should remain live"),
            "source unregister must not mutate the retired drag window's input state"
        );
        assert_eq!(runtime.active_payload_drag_session(&payload), None);
        assert!(!cx.update(|app| runtime.finish_payload_drag_with_app(&session, app)));
    }

    #[open_gpui::test]
    fn viewport_runtime_handle_rejects_stale_host_scene_frame_facts(cx: &mut TestAppContext) {
        let target_space = DockSpaceId::from("target");
        let fixture = DockViewportRuntimeFixture::builder(target_space.clone())
            .space(target_space.clone(), ["b"])
            .build(cx);
        let target_tabs = fixture.tabs(&target_space);
        let runtime = fixture.runtime.clone();
        let opened =
            fixture.open_viewport(cx, &target_space, viewport_window_options(360.0, 220.0));
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
                crate::DockDropGuideMetrics::default(),
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
                crate::DockDropGuideMetrics::default(),
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
    fn viewport_runtime_handle_tears_off_split_floating_from_floating_root(
        cx: &mut TestAppContext,
    ) {
        let primary_space = DockSpaceId::from("primary");
        let detached_space = DockSpaceId::from("detached");
        let (graph, floating) =
            horizontal_split_floating_graph(primary_space.clone(), Some(&["b"]));

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
    fn viewport_runtime_handle_render_prepaint_sync_refreshes_other_viewport_facts(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(target_space.clone(), ["b"])
            .allow_platform_viewports(true)
            .build(cx);
        let source_tabs = fixture.tabs(&source_space);
        let target_tabs = fixture.tabs(&target_space);
        let runtime = fixture.runtime.clone();

        let source_opened =
            fixture.open_viewport(cx, &source_space, viewport_window_options(360.0, 220.0));
        let source_window = source_opened
            .window()
            .downcast::<crate::DockHost>()
            .expect("source viewport should render DockHost");
        source_window
            .update(cx, |host, window, cx| {
                host.publish_viewport_host_scene_interaction(
                    floating_bounds(0.0, 0.0, 360.0, 220.0),
                    target_center_host_position(),
                    window,
                    cx,
                );
            })
            .expect("source host should publish live route facts");
        assert!(runtime.push_viewport_host_scene_fact(
            &source_space,
            source_opened.window().window_id(),
            leaf_host_scene_fact(source_tabs, source_tabs),
        ));

        let target_opened =
            fixture.open_viewport(cx, &target_space, viewport_window_options(360.0, 220.0));
        let target_window = target_opened
            .window()
            .downcast::<crate::DockHost>()
            .expect("target viewport should render DockHost");
        let target_bounds = target_window
            .update(cx, |_, window, _| window.window_bounds())
            .expect("target window should be live");
        let target_bounds = WindowBounds::Windowed(target_bounds.get_bounds());
        target_window
            .update(cx, |host, window, cx| {
                host.publish_viewport_host_scene_interaction(
                    floating_bounds(0.0, 0.0, 360.0, 220.0),
                    target_center_host_position(),
                    window,
                    cx,
                );
            })
            .expect("target host should publish live route facts");
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            target_opened.window().window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));
        assert!(runtime.viewport_route_ready(&target_space));

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);
        let release_position =
            screen_position_for_host_position(target_bounds, target_center_host_position());
        cache_known_viewport_preview(
            cx,
            &runtime,
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            target_opened.window(),
            Some(session),
            "Panel A",
        );
        assert!(
            runtime
                .routed_drop_preview_for(&target_space, target_opened.window().window_id())
                .is_some(),
            "preview setup should cache a routed preview for the target"
        );

        target_window
            .update(cx, |_, window, _| {
                window.minimize_window();
                assert!(window.is_minimized());
            })
            .expect("target window should still be live after minimize");

        let preparation = source_window
            .update(cx, |_, window, cx| {
                let snapshot = DockViewportHostSceneSnapshot::new_with_facts(
                    source_space.clone(),
                    window.window_handle().window_id(),
                    DockViewportWindowFacts::from_window(window, cx).current_bounds,
                    floating_bounds(0.0, 0.0, 360.0, 220.0),
                    target_center_host_position(),
                    crate::DockDropGuideMetrics::default(),
                    Vec::new(),
                );
                runtime.commit_rendered_viewport_host_scene_snapshot(snapshot, window, cx, false)
            })
            .expect("source render commit should run");
        assert!(
            preparation.changed,
            "render commit sync should report runtime changes when it clears stale routed previews"
        );

        assert!(!runtime.viewport_route_ready(&target_space));
        assert_eq!(
            runtime.viewport_route_unavailable_reason(&target_space),
            Some(DockViewportRouteUnavailableReason::Minimized)
        );
        assert!(
            runtime
                .routed_drop_preview_for(&target_space, target_opened.window().window_id())
                .is_none(),
            "render commit sync should clear previews targeting a now-unroutable viewport"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_handle_rejects_known_viewport_drop_without_host_scene(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(target_space.clone(), ["b"])
            .build(cx);
        let source_tabs = fixture.tabs(&source_space);
        let controller = fixture.controller.clone();
        let runtime = fixture.runtime.clone();

        let opened =
            fixture.open_viewport(cx, &target_space, viewport_window_options(360.0, 220.0));
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
                DockViewportPlatformSignals::from_app(app)
                    .with_trusted_hovered_window(opened.window()),
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
            DockDropDelivery::from_resolution(resolution)
                .and_then(|plan| runtime.deliver_drop_commit_delivery(plan, app))
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
    fn viewport_runtime_handle_commits_known_viewport_drop_through_host_scene(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(target_space.clone(), ["b"])
            .build(cx);
        let source_tabs = fixture.tabs(&source_space);
        let target_tabs = fixture.tabs(&target_space);
        let controller = fixture.controller.clone();
        let runtime = fixture.runtime.clone();

        let opened =
            fixture.open_viewport(cx, &target_space, viewport_window_options(360.0, 220.0));
        let target_window_bounds = opened
            .window()
            .update(cx, |_, window, _| window.window_bounds())
            .expect("target window should be live");
        let target_window_bounds = WindowBounds::Windowed(target_window_bounds.get_bounds());
        let source_opened =
            fixture.open_viewport(cx, &source_space, viewport_window_options(360.0, 220.0));
        source_opened
            .window()
            .update(cx, |_, window, _| window.activate_window())
            .expect("source viewport should be activatable before drop");
        let active_window_before_drop = opened
            .window()
            .update(cx, |_, _, app| app.active_window())
            .expect("target window should be live");
        assert_eq!(
            active_window_before_drop.map(|window| window.window_id()),
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
        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);

        let release_position = runtime
            .last_host_scene_screen_position(&target_space)
            .expect("target scene should expose a screen position");
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(opened.window()),
        )
        .with_drag_session(Some(session.clone()));
        let plan = fresh_delivery_for_request(cx, &runtime, &request);
        let result = cx.update(|app| {
            let result = runtime.deliver_drop_commit_delivery(plan, app);
            let status = runtime.runtime_status();
            let target = &status
                .last_route
                .as_ref()
                .expect("screen release should record the destination viewport route")
                .target;
            assert_eq!(target.window_id(), Some(opened.window().window_id()));
            result
        });

        let DockViewportDropRouteOutcome::Action(action) = result.expect("route should commit")
        else {
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
                .map(|activation| activation.focus_request().clone()),
            Some(DockViewportFocusRequest::panel(item("a"))),
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
                .map(|activation| activation.focus_request.clone()),
            Some(DockViewportFocusRequest::panel(item("a"))),
            "runtime status should record the destination focus request"
        );
        cx.update(|app| {
            assert!(
                apply_viewport_activation_transaction(action.activation().cloned(), app).changed(),
                "host finish should apply the routed activation transaction"
            );
        });
        cx.run_until_parked();
        let active_window_after_drop = opened
            .window()
            .update(cx, |_, _, app| app.active_window())
            .expect("target window should be live");
        assert_eq!(
            active_window_after_drop.map(|window| window.window_id()),
            Some(opened.window().window_id()),
            "successful routed drop should activate the destination viewport"
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&source_space),
            None,
            "empty source viewport should be unregistered after routed drop"
        );
        assert!(
            source_opened.window().update(cx, |_, _, _| ()).is_err(),
            "empty source viewport should close after routed drop"
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
                runtime.open_viewport_unchecked_policy(
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
                runtime.open_viewport_unchecked_policy(
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
        let release_screen_position = point(
            target_bounds.get_bounds().origin.x + target_center_host_position().x,
            target_bounds.get_bounds().origin.y + target_center_host_position().y,
        );
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
        let session = runtime.begin_payload_drag(&payload);
        cache_known_viewport_preview(
            cx,
            &runtime,
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_screen_position,
            target_opened.window(),
            Some(session.clone()),
            "Panel A",
        );

        cx.set_platform_hovered_window(Some(target_opened.window()));
        let changed = source_window
            .update(cx, |host, window, cx| {
                let changed = host.drop_payload_release_from_render(
                    DockPayloadDropRelease::hovered_host_with_session(
                        payload.clone(),
                        source_space.clone(),
                        release_in_source_window,
                        Some(session.clone()),
                    ),
                    window,
                    cx,
                );
                cx.stop_active_drag(window);
                changed
            })
            .expect("source host should commit the routed render drop");
        assert!(changed, "host render drop should report a graph change");
        cx.set_platform_hovered_window(None);
        let status = runtime.runtime_status();
        assert_eq!(
            status
                .last_route
                .as_ref()
                .and_then(|route| route.target.window_id()),
            Some(target_opened.window().window_id()),
            "routed drop should target the destination viewport"
        );
        assert_eq!(
            status
                .last_drop_outcome
                .as_ref()
                .map(|outcome| outcome.kind),
            Some(DockViewportDropOutcomeKind::Action),
            "routed drop should resolve into a workspace action"
        );
        assert_eq!(
            status
                .last_activation
                .as_ref()
                .map(|activation| activation.window_id),
            Some(target_opened.window().window_id()),
            "routed drop should record an activation transaction for the destination viewport"
        );
        cx.run_until_parked();

        let active_window_after_drop = target_opened
            .window()
            .update(cx, |_, _, app| app.active_window())
            .expect("target window should be live");
        assert_eq!(
            active_window_after_drop.map(|window| window.window_id()),
            Some(target_opened.window().window_id()),
            "host interaction should consume the routed activation transaction"
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&source_space),
            None,
            "empty source viewport should be unregistered after rendered routed drop"
        );
        assert!(
            source_opened.window().update(cx, |_, _, _| ()).is_err(),
            "empty source viewport should close after rendered routed drop"
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
    fn release_delivery_resamples_platform_target_context_after_reconcile(cx: &mut TestAppContext) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(target_space.clone(), ["b"])
            .build(cx);
        let source_tabs = fixture.tabs(&source_space);
        let target_tabs = fixture.tabs(&target_space);
        let controller = fixture.controller.clone();
        let runtime = fixture.runtime.clone();

        let source_bounds = WindowBounds::Windowed(floating_bounds(520.0, 100.0, 360.0, 220.0));
        let target_bounds = WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0));
        let source_opened = fixture.open_viewport(
            cx,
            &source_space,
            WindowOptions {
                window_bounds: Some(source_bounds),
                ..Default::default()
            },
        );
        let target_opened = fixture.open_viewport(
            cx,
            &target_space,
            WindowOptions {
                window_bounds: Some(target_bounds),
                ..Default::default()
            },
        );
        assert!(runtime.begin_viewport_host_scene(
            source_space.clone(),
            source_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(source_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            target_center_host_position(),
        ));
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

        let release_position =
            screen_position_for_host_position(target_bounds, target_center_host_position());

        let stale_release_signals =
            cx.update(|app| crate::DockViewportPlatformSignals::from_app(app));
        assert_eq!(
            stale_release_signals
                .target_context()
                .trusted_hovered_window(),
            None,
            "test setup should capture a stale release snapshot without hovered route selection"
        );
        let stale_release_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            None,
            stale_release_signals.clone(),
            DockPayloadDropReleaseOrigin::HoveredHost,
        );
        let stale_route_request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            None,
            stale_release_signals,
            DockPayloadDropReleaseOrigin::HoveredHost,
        );
        let stale_route =
            cx.update(|app| runtime.resolve_payload_drop_route_for_test(&stale_route_request, app));
        assert_eq!(
            stale_route,
            DockViewportDropRoute::Unavailable,
            "without current backend route selection, the stale snapshot has no viewport route selection"
        );

        cx.set_platform_hovered_window(Some(target_opened.window()));
        let refreshed_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&stale_release_request, app));
        assert!(
            matches!(
                refreshed_resolution.route(),
                DockViewportDropRoute::KnownViewport { target, source }
                    if target.window_id() == target_opened.window().window_id()
                        && *source
                            == crate::DockViewportRouteSelectionSource::TrustedHoveredWindow
            ),
            "release delivery should resample current backend target context before resolving a route, got {:?}",
            refreshed_resolution.route()
        );
        assert!(
            refreshed_resolution.delivery().is_some(),
            "a freshly resolved backend route should mint delivery from current target facts"
        );
        assert!(
            refreshed_resolution
                .routed_preview_target_snapshot()
                .is_some(),
            "fresh backend route should publish a preview target for the target viewport to accept"
        );
        cx.set_platform_hovered_window(None);

        cx.read_entity(&controller, |controller, _| {
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
    fn runtime_opened_cross_window_inner_edge_drag_docks_nested_vertical_split(
        cx: &mut TestAppContext,
    ) {
        for zone in [DropZone::Top, DropZone::Bottom] {
            let source_space = DockSpaceId::from(format!("source:{zone:?}"));
            let target_space = DockSpaceId::from(format!("target:{zone:?}"));
            let mut graph = DockGraph::new();
            let source_tabs = graph.insert_node(DockNode::Tabs {
                items: vec![item("a"), item("d")],
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
            workspace.register_panel_view(item("d"), "Panel D", test_view(cx, "D"));
            let controller = cx.new(|_| DockController::new(workspace));
            let runtime = DockViewportRuntimeHandle::new(controller.clone());
            let visual = DockCrossWindowVisualDragFixture::open(
                cx,
                &runtime,
                source_space.clone(),
                viewport_window_options(360.0, 220.0),
                target_space.clone(),
                viewport_window_options(420.0, 240.0),
                &format!("{zone:?}"),
            );
            visual.drag_source_tab_to_target_inner_edge(
                cx,
                source_tabs,
                item("a"),
                target_right_tabs,
                zone,
                DockCrossWindowDragRelease::Release,
                &format!("{zone:?}"),
            );
            visual.assert_drop_previews_cleared(cx, &format!("{zone:?}: after release"));

            assert_eq!(
                runtime.borrow().adapter().window_for_space(&source_space),
                Some(visual.source.window()),
                "{zone:?}: source viewport should stay registered"
            );
            assert_eq!(
                runtime.borrow().adapter().window_for_space(&target_space),
                Some(visual.target.window()),
                "{zone:?}: target viewport should stay registered"
            );
            let registered = runtime.registered_viewport_spaces();
            assert_eq!(registered.len(), 2, "{zone:?}");
            assert!(registered.contains(&source_space), "{zone:?}");
            assert!(registered.contains(&target_space), "{zone:?}");

            let payload = DockDragPayload::new_item(
                source_space.clone(),
                source_tabs,
                item("a"),
                "Panel A".to_string(),
            );
            assert_eq!(
                runtime.active_payload_drag_session(&payload),
                None,
                "{zone:?}: drag session should finish after release"
            );
            assert_eq!(
                runtime.routed_drop_preview_for(&target_space, visual.target.window().window_id()),
                None,
                "{zone:?}: routed preview should clear after release"
            );

            cx.read_entity(&controller, |controller, _| {
                assert_tabs_node_items(
                    controller.graph(),
                    target_left_tabs,
                    &[item("b")],
                    &format!("{zone:?}: left target tabs should stay intact"),
                );
                assert_tabs_node_items(
                    controller.graph(),
                    source_tabs,
                    &[item("d")],
                    &format!("{zone:?}: source tabs should retain the remaining item"),
                );
                let DockNode::Split { axis, children, .. } = controller
                    .graph()
                    .node(target_root)
                    .unwrap_or_else(|| panic!("{zone:?}: target root should still exist"))
                else {
                    panic!("{zone:?}: target root should remain a split");
                };
                assert_eq!(*axis, SplitAxis::Horizontal, "{zone:?}");
                assert_eq!(children.len(), 2, "{zone:?}");
                assert_eq!(children[0], target_left_tabs, "{zone:?}");
                let nested_vertical = children[1];
                let DockNode::Split {
                    axis: nested_axis,
                    children: nested_children,
                    ..
                } = controller
                    .graph()
                    .node(nested_vertical)
                    .unwrap_or_else(|| panic!("{zone:?}: nested vertical split should exist"))
                else {
                    panic!("{zone:?}: nested child should be a split");
                };
                assert_eq!(*nested_axis, SplitAxis::Vertical, "{zone:?}");
                assert_eq!(nested_children.len(), 2, "{zone:?}");
                let (moved_index, old_index) = match zone {
                    DropZone::Top => (0, 1),
                    DropZone::Bottom => (1, 0),
                    _ => unreachable!(),
                };
                assert_eq!(nested_children[old_index], target_right_tabs, "{zone:?}");
                assert_tabs_node_items(
                    controller.graph(),
                    nested_children[moved_index],
                    &[item("a")],
                    &format!("{zone:?}: moved tab should become the new nested child"),
                );
            });
        }
    }

    #[open_gpui::test]
    fn runtime_opened_cross_window_inner_edge_drag_docks_nested_horizontal_split(
        cx: &mut TestAppContext,
    ) {
        for zone in [DropZone::Left, DropZone::Right] {
            let source_space = DockSpaceId::from(format!("source-horizontal:{zone:?}"));
            let target_space = DockSpaceId::from(format!("target-horizontal:{zone:?}"));
            let mut graph = DockGraph::new();
            let source_tabs = graph.insert_node(DockNode::Tabs {
                items: vec![item("a"), item("d")],
                selected: Some(item("a")),
            });
            let target_top_tabs = graph.insert_node(DockNode::Tabs {
                items: vec![item("b")],
                selected: Some(item("b")),
            });
            let target_bottom_tabs = graph.insert_node(DockNode::Tabs {
                items: vec![item("c")],
                selected: Some(item("c")),
            });
            let target_root = graph.insert_node(DockNode::Split {
                axis: SplitAxis::Vertical,
                children: vec![target_top_tabs, target_bottom_tabs],
                fractions: vec![0.5, 0.5],
            });
            graph.set_root(source_space.clone(), source_tabs);
            graph.set_root(target_space.clone(), target_root);

            let mut workspace = DockWorkspace::new(source_space.clone(), graph);
            workspace.policy_mut().set_allow_platform_viewports(true);
            workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
            workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
            workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
            workspace.register_panel_view(item("d"), "Panel D", test_view(cx, "D"));
            let controller = cx.new(|_| DockController::new(workspace));
            let runtime = DockViewportRuntimeHandle::new(controller.clone());
            let visual = DockCrossWindowVisualDragFixture::open(
                cx,
                &runtime,
                source_space.clone(),
                viewport_window_options(360.0, 220.0),
                target_space.clone(),
                viewport_window_options(420.0, 300.0),
                &format!("{zone:?}"),
            );
            visual.drag_source_tab_to_target_inner_edge(
                cx,
                source_tabs,
                item("a"),
                target_bottom_tabs,
                zone,
                DockCrossWindowDragRelease::Release,
                &format!("{zone:?}"),
            );
            visual.assert_drop_previews_cleared(cx, &format!("{zone:?}: after release"));

            assert_eq!(
                runtime.borrow().adapter().window_for_space(&source_space),
                Some(visual.source.window()),
                "{zone:?}: source viewport should stay registered"
            );
            assert_eq!(
                runtime.borrow().adapter().window_for_space(&target_space),
                Some(visual.target.window()),
                "{zone:?}: target viewport should stay registered"
            );
            let registered = runtime.registered_viewport_spaces();
            assert_eq!(registered.len(), 2, "{zone:?}");
            assert!(registered.contains(&source_space), "{zone:?}");
            assert!(registered.contains(&target_space), "{zone:?}");

            let payload = DockDragPayload::new_item(
                source_space.clone(),
                source_tabs,
                item("a"),
                "Panel A".to_string(),
            );
            assert_eq!(
                runtime.active_payload_drag_session(&payload),
                None,
                "{zone:?}: drag session should finish after release"
            );
            assert_eq!(
                runtime.routed_drop_preview_for(&target_space, visual.target.window().window_id()),
                None,
                "{zone:?}: routed preview should clear after release"
            );

            cx.read_entity(&controller, |controller, _| {
                assert_tabs_node_items(
                    controller.graph(),
                    target_top_tabs,
                    &[item("b")],
                    &format!("{zone:?}: top target tabs should stay intact"),
                );
                assert_tabs_node_items(
                    controller.graph(),
                    source_tabs,
                    &[item("d")],
                    &format!("{zone:?}: source tabs should retain the remaining item"),
                );
                let DockNode::Split { axis, children, .. } = controller
                    .graph()
                    .node(target_root)
                    .unwrap_or_else(|| panic!("{zone:?}: target root should still exist"))
                else {
                    panic!("{zone:?}: target root should remain a split");
                };
                assert_eq!(*axis, SplitAxis::Vertical, "{zone:?}");
                assert_eq!(children.len(), 2, "{zone:?}");
                assert_eq!(children[0], target_top_tabs, "{zone:?}");
                let nested_horizontal = children[1];
                let DockNode::Split {
                    axis: nested_axis,
                    children: nested_children,
                    ..
                } = controller
                    .graph()
                    .node(nested_horizontal)
                    .unwrap_or_else(|| panic!("{zone:?}: nested horizontal split should exist"))
                else {
                    panic!("{zone:?}: nested child should be a split");
                };
                assert_eq!(*nested_axis, SplitAxis::Horizontal, "{zone:?}");
                assert_eq!(nested_children.len(), 2, "{zone:?}");
                let (moved_index, old_index) = match zone {
                    DropZone::Left => (0, 1),
                    DropZone::Right => (1, 0),
                    _ => unreachable!(),
                };
                assert_eq!(nested_children[old_index], target_bottom_tabs, "{zone:?}");
                assert_tabs_node_items(
                    controller.graph(),
                    nested_children[moved_index],
                    &[item("a")],
                    &format!("{zone:?}: moved tab should become the new nested child"),
                );
            });
        }
    }

    #[open_gpui::test]
    fn runtime_opened_cross_window_inner_edge_drag_then_re_docks_nested_mixed_axes(
        cx: &mut TestAppContext,
    ) {
        for first_zone in [DropZone::Top, DropZone::Bottom] {
            let second_zone = match first_zone {
                DropZone::Top => DropZone::Left,
                DropZone::Bottom => DropZone::Right,
                _ => unreachable!(),
            };
            let source_space = DockSpaceId::from(format!("source-mixed:{first_zone:?}"));
            let target_space = DockSpaceId::from(format!("target-mixed:{first_zone:?}"));
            let mut graph = DockGraph::new();
            let source_tabs = graph.insert_node(DockNode::Tabs {
                items: vec![item("a"), item("d")],
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
            workspace.register_panel_view(item("d"), "Panel D", test_view(cx, "D"));
            let controller = cx.new(|_| DockController::new(workspace));
            let runtime = DockViewportRuntimeHandle::new(controller.clone());
            let visual = DockCrossWindowVisualDragFixture::open(
                cx,
                &runtime,
                source_space.clone(),
                viewport_window_options(360.0, 220.0),
                target_space.clone(),
                viewport_window_options(420.0, 240.0),
                &format!("{first_zone:?}"),
            );
            let source_any_window = visual.source.window();

            visual.drag_source_tab_to_target_inner_edge(
                cx,
                source_tabs,
                item("a"),
                target_right_tabs,
                first_zone,
                DockCrossWindowDragRelease::Release,
                &format!("{first_zone:?}: first drag"),
            );

            let moved_tabs = cx.read_entity(&controller, |controller, _| {
                let DockNode::Split { axis, children, .. } = controller
                    .graph()
                    .node(target_root)
                    .unwrap_or_else(|| panic!("{first_zone:?}: target root should still exist"))
                else {
                    panic!("{first_zone:?}: target root should remain a split");
                };
                assert_eq!(*axis, SplitAxis::Horizontal, "{first_zone:?}");
                assert_eq!(children.len(), 2, "{first_zone:?}");
                assert_eq!(children[0], target_left_tabs, "{first_zone:?}");
                let nested_vertical = children[1];
                let DockNode::Split {
                    axis: nested_axis,
                    children: nested_children,
                    ..
                } = controller.graph().node(nested_vertical).unwrap_or_else(|| {
                    panic!("{first_zone:?}: nested vertical split should exist")
                })
                else {
                    panic!("{first_zone:?}: nested child should be a split");
                };
                assert_eq!(*nested_axis, SplitAxis::Vertical, "{first_zone:?}");
                assert_eq!(nested_children.len(), 2, "{first_zone:?}");
                let (moved_index, old_index) = match first_zone {
                    DropZone::Top => (0, 1),
                    DropZone::Bottom => (1, 0),
                    _ => unreachable!(),
                };
                assert_eq!(
                    nested_children[old_index], target_right_tabs,
                    "{first_zone:?}"
                );
                let moved_tabs = nested_children[moved_index];
                assert_tabs_node_items(
                    controller.graph(),
                    moved_tabs,
                    &[item("a")],
                    &format!("{first_zone:?}: moved tab should become the new nested child"),
                );
                moved_tabs
            });
            let target_window_id = visual.target.window().window_id();
            let hover_runtime = runtime.clone();
            let hover_target_space = target_space.clone();

            visual.drag_source_tab_to_target_inner_edge_with_hover(
                cx,
                source_tabs,
                item("d"),
                target_right_tabs,
                second_zone,
                &format!("{first_zone:?}: second drag"),
                |_, _| {
                    assert!(
                        hover_runtime
                            .routed_drop_preview_for(&hover_target_space, target_window_id)
                            .is_some(),
                        "{first_zone:?}: second-stage nested hover should publish an allowed routed preview"
                    );
                },
            );
            visual
                .assert_drop_previews_cleared(cx, &format!("{first_zone:?}: after second release"));

            let second_payload = DockDragPayload::new_item(
                source_space.clone(),
                source_tabs,
                item("d"),
                "Panel D".to_string(),
            );
            assert_eq!(
                runtime.active_payload_drag_session(&second_payload),
                None,
                "{first_zone:?}: second drag session should finish after release"
            );
            assert_eq!(
                runtime.routed_drop_preview_for(&target_space, visual.target.window().window_id()),
                None,
                "{first_zone:?}: routed preview should clear after the second release"
            );

            cx.read_entity(&controller, |controller, _| {
                assert_eq!(controller.graph().root(&source_space), None);
                assert_eq!(controller.graph().collect_items_in_space(&source_space), []);
                assert_tabs_node_items(
                    controller.graph(),
                    target_left_tabs,
                    &[item("b")],
                    &format!("{first_zone:?}: left target tabs should stay intact"),
                );
                let DockNode::Split { axis, children, .. } = controller
                    .graph()
                    .node(target_root)
                    .unwrap_or_else(|| panic!("{first_zone:?}: target root should still exist"))
                else {
                    panic!("{first_zone:?}: target root should remain a split");
                };
                assert_eq!(*axis, SplitAxis::Horizontal, "{first_zone:?}");
                assert_eq!(children.len(), 2, "{first_zone:?}");
                assert_eq!(children[0], target_left_tabs, "{first_zone:?}");
                let split_child = children[1];
                let DockNode::Split {
                    axis: nested_axis,
                    children: nested_children,
                    ..
                } = controller
                    .graph()
                    .node(split_child)
                    .unwrap_or_else(|| panic!("{first_zone:?}: nested split should still exist"))
                else {
                    panic!("{first_zone:?}: nested child should remain a split");
                };
                assert_eq!(*nested_axis, SplitAxis::Vertical, "{first_zone:?}");
                assert_eq!(nested_children.len(), 2, "{first_zone:?}");
                let (moved_index, wrapped_index) = match first_zone {
                    DropZone::Top => (0, 1),
                    DropZone::Bottom => (1, 0),
                    _ => unreachable!(),
                };
                assert_eq!(nested_children[moved_index], moved_tabs, "{first_zone:?}");
                assert_tabs_node_items(
                    controller.graph(),
                    moved_tabs,
                    &[item("a")],
                    &format!(
                        "{first_zone:?}: first-stage moved tab should stay in the nested split"
                    ),
                );
                let wrapped_leaf = nested_children[wrapped_index];
                let DockNode::Split {
                    axis: wrapped_axis,
                    children: wrapped_children,
                    ..
                } = controller.graph().node(wrapped_leaf).unwrap_or_else(|| {
                    panic!("{first_zone:?}: target leaf should be wrapped inside the nested split")
                })
                else {
                    panic!("{first_zone:?}: target leaf should become a horizontal split");
                };
                assert_eq!(*wrapped_axis, SplitAxis::Horizontal, "{first_zone:?}");
                assert_eq!(wrapped_children.len(), 2, "{first_zone:?}");
                let (inserted_index, old_index) = match second_zone {
                    DropZone::Left => (0, 1),
                    DropZone::Right => (1, 0),
                    _ => unreachable!(),
                };
                assert_eq!(
                    wrapped_children[old_index], target_right_tabs,
                    "{first_zone:?}"
                );
                assert_tabs_node_items(
                    controller.graph(),
                    wrapped_children[inserted_index],
                    &[item("d")],
                    &format!(
                        "{first_zone:?}: second-stage moved tab should dock inside the target leaf"
                    ),
                );
            });

            cx.update(|app| app.refresh_windows());
            assert_eq!(
                runtime.borrow().adapter().window_for_space(&source_space),
                None,
                "{first_zone:?}: vacated source viewport should be unregistered after refresh"
            );
            assert_eq!(
                runtime.borrow().adapter().window_for_space(&target_space),
                Some(visual.target.window()),
                "{first_zone:?}: target viewport should remain registered"
            );
            assert_eq!(
                runtime.registered_viewport_spaces(),
                vec![target_space.clone()],
                "{first_zone:?}: only the target viewport should remain registered after refresh"
            );
            assert!(
                source_any_window.update(cx, |_, _, _| ()).is_err(),
                "{first_zone:?}: vacated source viewport should close after the second commit refresh"
            );
        }
    }

    #[open_gpui::test]
    fn viewport_runtime_handle_commits_known_viewport_stack_drop_through_host_scene(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space_selected(source_space.clone(), ["a", "c"], "c")
            .space(target_space.clone(), ["b"])
            .build(cx);
        let source_tabs = fixture.tabs(&source_space);
        let target_tabs = fixture.tabs(&target_space);
        let controller = fixture.controller.clone();
        let runtime = fixture.runtime.clone();

        let opened =
            fixture.open_viewport(cx, &target_space, viewport_window_options(360.0, 220.0));
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
        let payload =
            DockDragPayload::new_tabs(source_space.clone(), source_tabs, "Stack".to_string());
        let session = runtime.begin_payload_drag(&payload);

        let request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Tabs,
            runtime
                .last_host_scene_screen_position(&target_space)
                .expect("target scene should expose a screen position"),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(opened.window()),
        )
        .with_drag_session(Some(session.clone()));
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        let result = cx.update(|app| {
            runtime
                .deliver_drop_commit_delivery(DockDropDelivery::from_resolution(resolution)?, app)
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
                runtime.open_viewport_unchecked_policy(
                    target_space.clone(),
                    viewport_window_options(420.0, 240.0),
                    app,
                )
            })
            .expect("target viewport should open");
        let source_opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
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
            .update(|app| {
                runtime.resolve_host_scene_target(&target_space, target_host_position, app)
            })
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
        let target_release_signals = source_opened
            .window()
            .update(cx, |_, _, app| {
                DockViewportPlatformSignals::from_app(app)
                    .with_trusted_hovered_window(target_opened.window())
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
        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);

        let request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_screen_position,
            None,
            target_release_signals,
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(session.clone()));
        let result = cx.update(|app| runtime.commit_payload_drop_from_screen(&request, app));

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
}
