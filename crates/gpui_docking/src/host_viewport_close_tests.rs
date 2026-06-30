//! Concern-owned viewport close regression tests.

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
        DockViewportPlatformSyncSkippedReason, DockViewportResolvedDropRoute,
        DockViewportRouteStatus, DockViewportRouteTarget, DockViewportRuntime,
        DockViewportRuntimeHandle, DockViewportShouldCloseStatus, DockViewportTargetContext,
        DockViewportTearOffOpenOutcome, DockViewportTearOffOutcomeKind,
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
            "initial backend focus suppression is independent of closing an unfocused viewport"
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
            "closing a front-most but never platform-focused viewport must not trigger ImGui's destroyed-previous-focus suppression"
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
                    backend_focus: DockViewportActivationBackendFocusObservation::TargetNotFocused,
                    backend_focus_apply: DockViewportActivationBackendFocusApply::default(),
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
                    backend_focus: DockViewportActivationBackendFocusObservation::TargetNotFocused,
                    backend_focus_apply: DockViewportActivationBackendFocusApply::default(),
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
                    DockViewportFocusCommand::platform_activation(DockViewportFocusRequest::panel(
                        "c"
                    ))
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

        let cleanup = cx
            .update(|app| runtime.handle_window_closed_with_app(opened.window().window_id(), app));
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
    fn viewport_runtime_merge_back_close_without_frozen_plan_only_unregisters(
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

        let outcome =
            cx.update(|app| runtime.handle_window_closed_with_app(window.window_id(), app));
        let activation = cx.update(|app| runtime.activation_transaction_after_close(&outcome, app));

        assert_eq!(outcome.status(), DockViewportCloseStatus::Closed);
        assert_eq!(
            runtime.runtime_status().last_close,
            Some(outcome.clone()),
            "close diagnostics should record a plain close when no should-close plan froze merge-back state"
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
                .outcome
        });
        assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);
        let outcome =
            cx.update(|app| runtime.handle_window_closed_with_app(window.window_id(), app));

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
    fn viewport_runtime_merge_back_close_does_not_use_tree_order_for_focus(
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
                .outcome
        });
        assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);
        let outcome =
            cx.update(|app| runtime.handle_window_closed_with_app(window.window_id(), app));

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
                .outcome
        });
        assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);
        let outcome =
            cx.update(|app| runtime.handle_window_closed_with_app(window.window_id(), app));

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
                .outcome
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

        let closed = cx
            .update(|app| runtime.handle_window_closed_with_app(opened.window().window_id(), app));
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
                .outcome
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
        let closed =
            cx.update(|app| runtime.handle_window_closed_with_app(window.window_id(), app));

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
        let closed = cx.update(|app| {
            runtime.handle_window_closed_with_app(detached.window().window_id(), app)
        });
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
    fn viewport_runtime_pending_merge_back_freezes_should_close_target_tabs(
        cx: &mut TestAppContext,
    ) {
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

        let closed = cx.update(|app| {
            runtime.handle_window_closed_with_app(detached.window().window_id(), app)
        });

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
    fn viewport_runtime_pending_merge_back_rejects_stale_frozen_target_tabs(
        cx: &mut TestAppContext,
    ) {
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

        let closed = cx.update(|app| {
            runtime.handle_window_closed_with_app(detached.window().window_id(), app)
        });

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
                .outcome
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

        let request = hovered_window_route_request_for_test(
            detached_space.clone(),
            detached_tabs,
            DockViewportDropPayload::Item(item("a")),
            screen_position_for_host_position(window_bounds, host_position),
            None,
            window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        );
        let pending_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
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
        let fresh_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        assert!(
            matches!(
                fresh_resolution.route(),
                DockViewportDropRoute::Local { .. }
            ),
            "fresh route facts should restore local route selection after auto-cancel"
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
                .outcome
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

        let request = hovered_window_route_request_for_test(
            detached_space.clone(),
            detached_tabs,
            DockViewportDropPayload::Item(item("a")),
            screen_position_for_host_position(window_bounds, host_position),
            None,
            window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        );
        let pending_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
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
        let fresh_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        assert!(
            matches!(
                fresh_resolution.route(),
                DockViewportDropRoute::Local { .. }
            ),
            "fresh route facts should restore local route selection after auto-cancel"
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
                .outcome
        });
        assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);

        let update = runtime.cancel_window_close_request(window.window_id());
        assert!(update.changed());
        assert_eq!(update.into_windows(), vec![window]);
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

        let request = hovered_window_route_request_for_test(
            detached_space.clone(),
            detached_tabs,
            DockViewportDropPayload::Item(item("a")),
            screen_position_for_host_position(window_bounds, host_position),
            None,
            window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        );
        let fresh_resolution =
            cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        let DockViewportDropRoute::Local {
            host_position: routed_position,
            window_id: routed_window,
            source,
            ..
        } = fresh_resolution.route()
        else {
            panic!("fresh route facts should restore local route selection");
        };
        assert_eq!(*routed_position, host_position);
        assert_eq!(*routed_window, window.window_id());
        assert_eq!(
            *source,
            crate::DockViewportRouteSelectionSource::TrustedHoveredWindow
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
                .outcome
        });
        assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);
        assert!(
            runtime
                .cancel_window_close_request(window.window_id())
                .changed()
        );
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
    fn viewport_runtime_cancel_close_plan_without_adapter_request_reports_change(
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
        let window = handle(51);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, detached_space, window);
        let mut runtime = DockViewportRuntime::from_adapter(
            controller,
            adapter,
            DockViewportClosePolicy::MergeBack {
                target_space: main_space,
            },
        );

        let should_close = cx.update(|app| {
            runtime
                .handle_window_should_close_with_app_and_refresh(window.window_id(), app)
                .outcome
        });
        assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);
        assert!(runtime.adapter().window_close_requested(window.window_id()));

        runtime.unregister_adapter_window_for_test(window.window_id());
        assert!(!runtime.adapter().window_close_requested(window.window_id()));

        let update = runtime.cancel_window_close_request(window.window_id());
        assert!(
            update.changed(),
            "clearing a pending close plan is observable even when the adapter close request was already gone"
        );
        assert!(update.into_windows().is_empty());
        assert!(
            !runtime
                .cancel_window_close_request(window.window_id())
                .changed()
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_discarded_pending_close_does_not_mark_reused_window(
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
                .outcome
        });
        assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);
        assert!(runtime.unregister_host_for_space(&detached_space, window.window_id()));
        runtime.register_opened_viewport(inspector_space.clone(), window);

        let closed =
            cx.update(|app| runtime.handle_window_closed_with_app(window.window_id(), app));

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
        let controller =
            cx.new(|_| DockController::new(DockWorkspace::new(space(), DockGraph::new())));
        let secondary_space = DockSpaceId::from("secondary");
        let window: AnyWindowHandle = WindowHandle::<DockHost>::new(WindowId::from(909)).into();
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, secondary_space.clone(), window);

        let mut runtime = DockViewportRuntime::from_adapter(
            controller,
            adapter,
            DockViewportClosePolicy::Prevent,
        );

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
        let request = hovered_window_route_request_for_test(
            source_space,
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            point(px(220.0), px(200.0)),
            None,
            target_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(session.clone()));
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        let update = runtime.update_routed_drop_preview(&resolution, &payload);
        assert!(update.changed());
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
    fn viewport_runtime_late_close_for_replaced_window_keeps_current_viewport_state(
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

        let old_window = handle(71);
        let new_window = handle(72);
        let mut runtime = DockViewportRuntime::new(controller);
        runtime.register_opened_viewport(target_space.clone(), old_window);

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

        let replaced = runtime.register_opened_viewport(target_space.clone(), new_window);
        assert_eq!(replaced, vec![old_window]);
        assert_eq!(
            runtime.adapter().window_for_space(&target_space),
            Some(new_window)
        );

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
        assert!(
            cx.update(|app| runtime.resolve_host_scene_target(&target_space, host_position, app))
                .is_some(),
            "test setup should start with current route facts for the replacement window"
        );

        let outcome = runtime.handle_window_closed(old_window.window_id());

        assert_eq!(outcome.status(), DockViewportCloseStatus::UnknownWindow);
        assert_eq!(outcome.space(), None);
        assert_eq!(
            runtime.adapter().window_for_space(&target_space),
            Some(new_window),
            "a late close notification for a retired window must not unregister the replacement"
        );
        assert!(
            cx.update(|app| runtime.resolve_host_scene_target(&target_space, host_position, app))
                .is_some(),
            "late close cleanup must only forget scenes for the retired window id"
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
    fn viewport_runtime_handle_closed_source_returns_routed_preview_target_for_refresh(
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
        workspace.policy_mut().set_allow_platform_viewports(true);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller);

        let source_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    source_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("source viewport should open through runtime handle");
        let target_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    target_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("target viewport should open through runtime handle");

        let target_screen_position = screen_position_for_host_position(
            WindowBounds::Windowed(floating_bounds(100.0, 100.0, 360.0, 220.0)),
            target_center_host_position(),
        );
        assert!(runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
                100.0, 100.0, 360.0, 220.0,
            ))),
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
        cache_known_viewport_preview(
            cx,
            &runtime,
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_screen_position,
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

        let closed = cx.update(|app| {
            runtime
                .borrow_mut()
                .handle_window_closed_with_app_and_refresh(source_opened.window().window_id(), app)
        });

        assert_eq!(
            closed.outcome.status(),
            crate::DockViewportCloseStatus::Closed
        );
        assert_eq!(closed.outcome.space(), Some(&source_space));
        assert_eq!(
            closed.window_effects().refresh(),
            &[target_opened.window()],
            "closing the drag source should refresh the surviving routed-preview target"
        );
        assert!(
            runtime
                .routed_drop_preview_for(&target_space, target_opened.window().window_id())
                .is_none(),
            "closing the source should clear its routed preview from the target viewport"
        );
        assert_eq!(
            runtime.active_payload_drag_session(&payload),
            None,
            "closing the source should finish the active drag"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_handle_retain_close_clears_scene_and_reopens_layout(
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
            cx.update(
                |app| runtime.handle_window_should_close_with_app(opened.window().window_id(), app)
            )
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
            cx.update(
                |app| runtime.handle_window_should_close_with_app(opened.window().window_id(), app)
            )
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
    fn viewport_runtime_handle_merge_back_close_without_source_focus_blurs_fallback(
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

        let panel_b = test_view(cx, "B");
        let panel_b_focus = cx.read_entity(&panel_b, |panel, cx| panel.focus_handle(cx));
        let mut workspace = DockWorkspace::new(main_space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_focusable_panel_view(item("b"), "Panel B", panel_b);
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

        runtime.record_panel_focus(main_space.clone(), item("a"));
        main_opened
            .window()
            .update(cx, |view, window, cx| {
                view.downcast::<crate::DockHost>()
                    .expect("runtime viewport should render DockHost")
                    .update(cx, |host, cx| {
                        assert!(host.request_viewport_focus_command(
                            DockViewportFocusCommand::viewport_activation(
                                DockViewportFocusRequest::panel(item("b"))
                            )
                        ));
                        cx.notify();
                    });
                assert_ne!(window.focused(cx), Some(panel_b_focus.clone()));
            })
            .expect("main viewport should remain live");
        cx.run_until_parked();
        main_opened
            .window()
            .update(cx, |_, window, cx| {
                assert_eq!(window.focused(cx), Some(panel_b_focus.clone()));
            })
            .expect("main viewport should remain live");
        main_opened
            .window()
            .update(cx, |_, window, _| window.activate_window())
            .expect("main viewport should activate");
        cx.run_until_parked();
        let should_close = cx.update(|app| {
            runtime.handle_window_should_close_with_app(detached_opened.window().window_id(), app)
        });
        assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);

        detached_opened
            .window()
            .update(cx, |_, window, _| window.remove_window())
            .expect("detached viewport should still be live");
        cx.run_until_parked();

        main_opened
            .window()
            .update(cx, |_, window, cx| {
                assert_eq!(
                    window.focused(cx),
                    None,
                    "merge-back close without source focus must not restore the fallback viewport's old panel focus"
                );
            })
            .expect("main viewport should remain live");
    }

    #[open_gpui::test]
    fn viewport_runtime_handle_merge_back_close_focuses_recorded_source_item(
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
        runtime.record_panel_focus(detached_space.clone(), item("c"));

        main_opened
            .window()
            .update(cx, |_, window, _| window.activate_window())
            .expect("main viewport should activate");
        cx.run_until_parked();
        let should_close = cx.update(|app| {
            runtime.handle_window_should_close_with_app(detached_opened.window().window_id(), app)
        });
        assert_eq!(should_close.status, DockViewportShouldCloseStatus::Allowed);

        detached_opened
            .window()
            .update(cx, |_, window, _| window.remove_window())
            .expect("detached viewport should still be live");
        cx.run_until_parked();

        let active_window = main_opened
            .window()
            .update(cx, |_, _, app| app.active_window())
            .expect("main viewport should remain live");
        assert_eq!(
            active_window.map(|window| window.window_id()),
            Some(main_opened.window().window_id())
        );
        main_opened
            .window()
            .update(cx, |_, window, cx| {
                assert_eq!(
                    window.focused(cx),
                    Some(panel_c_focus),
                    "merge-back close should focus the recorded source item in the fallback viewport"
                );
            })
            .expect("main viewport should remain live");
    }

    #[open_gpui::test]
    fn viewport_runtime_handle_closes_vacated_source_viewport_after_single_root_tab_tear_off(
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
        let source_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    primary_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("source root-only viewport should open through runtime");
        let source_window = source_opened.window();

        let outcome = cx
            .update(|app| {
                runtime.open_tear_off_viewport(
                    tear_off_request(primary_space.clone(), source_tabs, item("a")),
                    detached_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("single root tab tear-off should open the detached viewport");

        let DockViewportTearOffOpenOutcome::Completed(completed) = outcome else {
            panic!("single root tab tear-off should complete");
        };
        assert_eq!(
            completed.window_effects().close_after_current_effect(),
            &[source_window]
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&primary_space),
            None,
            "empty source viewport should be unregistered after its only root tab tears off"
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&detached_space),
            Some(completed.registration().window())
        );
        cx.read_entity(&controller, |controller, _| {
            assert_eq!(
                controller.graph().collect_items_in_space(&primary_space),
                Vec::<DockItemId>::new()
            );
            assert_eq!(
                controller.graph().collect_items_in_space(&detached_space),
                vec![item("a")]
            );
        });
        cx.run_until_parked();
        cx.update(|app| app.refresh_windows());
        assert!(
            source_window.update(cx, |_, _, _| ()).is_err(),
            "a vacated single-root source viewport should close after its only tab tears off"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_handle_closes_vacated_source_viewport_after_floating_tear_off(
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
        let source_opened = cx
            .update(|app| {
                runtime.open_viewport(
                    primary_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("source floating-only viewport should open through runtime");
        let source_window = source_opened.window();

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
            .expect("floating-only source tear-off should open the detached viewport");

        let DockViewportTearOffOpenOutcome::Completed(completed) = outcome else {
            panic!("floating-only source tear-off should complete");
        };
        assert_eq!(
            completed.window_effects().close_after_current_effect(),
            &[source_window]
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&primary_space),
            None,
            "empty source viewport should be unregistered after its only floating payload tears off"
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&detached_space),
            Some(completed.registration().window())
        );
        cx.read_entity(&controller, |controller, _| {
            assert!(controller.graph().root(&primary_space).is_none());
            assert!(
                controller
                    .graph()
                    .floating_containers(&primary_space)
                    .is_empty()
            );
            assert_eq!(
                controller.graph().collect_items_in_space(&detached_space),
                vec![item("a")]
            );
        });
        cx.run_until_parked();
        cx.update(|app| app.refresh_windows());
        assert!(
            source_window.update(cx, |_, _, _| ()).is_err(),
            "a vacated floating-only source viewport should close like ImGui's hidden empty host"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_handle_closes_unregistered_window_when_tear_off_commit_fails(
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
        workspace.register_panel_view(item("occupied"), "Occupied", test_view(cx, "Occupied"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let request = tear_off_request(primary_space.clone(), source_tabs, item("a"));
        let pending = cx.update(|app| {
            let DockViewportTearOffBeginOutcome::Pending(pending) = runtime
                .borrow_mut()
                .begin_tear_off_request(request, detached_space.clone(), app)
            else {
                panic!("fresh tear-off request should create pending state");
            };
            pending
        });

        controller.update(cx, |controller, _| {
            let mut graph = controller.graph().clone();
            let blocker_tabs = graph.insert_node(DockNode::Tabs {
                items: vec![item("occupied")],
                selected: Some(item("occupied")),
            });
            graph.set_root(detached_space.clone(), blocker_tabs);
            controller.workspace_mut().set_graph(graph);
        });
        let unregistered_window: open_gpui::AnyWindowHandle = cx
            .open_window(size(px(360.0), px(220.0)), |_, cx| {
                TestPanel::new("unregistered", cx)
            })
            .into();
        let before_finish_windows = cx.windows().len();

        let error = cx
            .update(|app| {
                runtime.complete_opened_tear_off_viewport_for_test(
                    pending,
                    unregistered_window,
                    app,
                )
            })
            .expect_err("commit should fail after target space becomes occupied");
        assert!(
            error.to_string().contains("not empty"),
            "commit failure should report occupied target space, got {error}"
        );
        cx.run_until_parked();
        cx.update(|app| app.refresh_windows());

        assert_eq!(
            runtime.borrow().pending_tear_off_len(),
            0,
            "failed completion must clear pending tear-off state"
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&detached_space),
            None,
            "failed completion must not register the uncommitted viewport"
        );
        assert_eq!(
            cx.windows().len(),
            before_finish_windows.saturating_sub(1),
            "failed completion must close the unregistered platform window"
        );
        cx.read_entity(&controller, |controller, _| {
            assert_eq!(
                controller.graph().collect_items_in_space(&primary_space),
                vec![item("a")],
                "source content must stay in place when tear-off commit fails"
            );
            assert_eq!(
                controller.graph().collect_items_in_space(&detached_space),
                vec![item("occupied")],
                "target space should keep the content that caused the commit failure"
            );
        });
    }

    #[open_gpui::test]
    fn viewport_runtime_handle_closes_unregistered_window_when_tear_off_source_closes(
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
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let request = tear_off_request(primary_space.clone(), source_tabs, item("a"));
        let pending = cx.update(|app| {
            let DockViewportTearOffBeginOutcome::Pending(pending) = runtime
                .borrow_mut()
                .begin_tear_off_request(request, detached_space.clone(), app)
            else {
                panic!("fresh tear-off request should create pending state");
            };
            pending
        });

        controller.update(cx, |controller, _| {
            controller
                .apply_action(&DockAction::CloseItem {
                    space: primary_space.clone(),
                    item: item("a"),
                })
                .expect("source item close should commit before tear-off completion");
        });
        let unregistered_window: open_gpui::AnyWindowHandle = cx
            .open_window(size(px(360.0), px(220.0)), |_, cx| {
                TestPanel::new("unregistered", cx)
            })
            .into();
        let before_finish_windows = cx.windows().len();

        let error = cx
            .update(|app| {
                runtime.complete_opened_tear_off_viewport_for_test(
                    pending,
                    unregistered_window,
                    app,
                )
            })
            .expect_err("completion should cancel when the source item is gone");
        assert!(
            error.to_string().contains("SourceUnavailable"),
            "source close should be reported as SourceUnavailable, got {error}"
        );
        cx.run_until_parked();
        cx.update(|app| app.refresh_windows());

        assert_eq!(
            runtime.borrow().pending_tear_off_len(),
            0,
            "cancelled completion must clear pending tear-off state"
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&detached_space),
            None,
            "cancelled completion must not register the uncommitted viewport"
        );
        assert_eq!(
            cx.windows().len(),
            before_finish_windows.saturating_sub(1),
            "cancelled completion must close the unregistered platform window"
        );
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
    fn viewport_runtime_handle_closes_unregistered_window_when_tear_off_source_moves(
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
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let request = tear_off_request(primary_space.clone(), source_tabs, item("a"));
        let pending = cx.update(|app| {
            let DockViewportTearOffBeginOutcome::Pending(pending) = runtime
                .borrow_mut()
                .begin_tear_off_request(request, detached_space.clone(), app)
            else {
                panic!("fresh tear-off request should create pending state");
            };
            pending
        });

        controller.update(cx, |controller, _| {
            controller
                .workspace_mut()
                .commit_tab_move(
                    &primary_space,
                    source_tabs,
                    &item("a"),
                    &other_space,
                    DockGraphDropTarget::center(other_tabs),
                )
                .expect("source item move should commit before tear-off completion");
        });
        let unregistered_window: open_gpui::AnyWindowHandle = cx
            .open_window(size(px(360.0), px(220.0)), |_, cx| {
                TestPanel::new("unregistered", cx)
            })
            .into();
        let before_finish_windows = cx.windows().len();

        let error = cx
            .update(|app| {
                runtime.complete_opened_tear_off_viewport_for_test(
                    pending,
                    unregistered_window,
                    app,
                )
            })
            .expect_err("completion should cancel when the source item moved");
        assert!(
            error.to_string().contains("SourceUnavailable"),
            "source move should be reported as SourceUnavailable, got {error}"
        );
        cx.run_until_parked();
        cx.update(|app| app.refresh_windows());

        assert_eq!(
            runtime.borrow().pending_tear_off_len(),
            0,
            "cancelled completion must clear pending tear-off state"
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&detached_space),
            None,
            "cancelled completion must not register the uncommitted viewport"
        );
        assert_eq!(
            cx.windows().len(),
            before_finish_windows.saturating_sub(1),
            "cancelled completion must close the unregistered platform window"
        );
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
    fn runtime_opened_cross_window_inner_edge_drag_closes_vacated_source_viewport(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source:single");
        let target_space = DockSpaceId::from("target:single");
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
                    viewport_window_options(420.0, 240.0),
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
        let source_any_window = source_opened.window();
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
        let right_tabs_selector = selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::Tabs {
                node: target_right_tabs,
            },
        )
        .expect("right target tabs selector should be emitted");
        let start = debug_bounds(&mut source_visual, &source_tab).center();
        let threshold = point(start.x + px(24.0), start.y);
        let end = inner_edge_drop_position(
            debug_bounds(&mut target_visual, &right_tabs_selector),
            DropZone::Bottom,
        );

        source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
        target_visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
        target_visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();

        let target_visual = VisualTestContext::from_window(target_opened.window(), cx);
        let source_visual = VisualTestContext::from_window(source_opened.window(), cx);
        assert!(
            selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview).is_none(),
            "target drop preview should clear after release"
        );
        assert!(
            selector_for(&source_visual, &source_host, DockDebugRegion::DropPreview).is_none(),
            "source drop preview should clear after release"
        );
        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        assert_eq!(
            runtime.active_payload_drag_session(&payload),
            None,
            "drag session should finish after release"
        );
        assert_eq!(
            runtime.routed_drop_preview_for(&target_space, target_opened.window().window_id()),
            None,
            "routed preview should clear after release"
        );

        cx.read_entity(&controller, |controller, _| {
            assert_eq!(controller.graph().root(&source_space), None);
            assert_eq!(controller.graph().collect_items_in_space(&source_space), []);
            let DockNode::Split { axis, children, .. } = controller
                .graph()
                .node(target_root)
                .expect("target root should still exist")
            else {
                panic!("target root should remain a split");
            };
            assert_eq!(*axis, SplitAxis::Horizontal);
            assert_eq!(children[0], target_left_tabs);
            let DockNode::Split {
                axis: nested_axis,
                children: nested_children,
                ..
            } = controller
                .graph()
                .node(children[1])
                .expect("right target should be wrapped in a nested split")
            else {
                panic!("right target should become a nested split");
            };
            assert_eq!(*nested_axis, SplitAxis::Vertical);
            assert_eq!(nested_children[0], target_right_tabs);
            assert_tabs_node_items(
                controller.graph(),
                nested_children[1],
                &[item("a")],
                "moved tab should dock below the old right target",
            );
        });

        cx.update(|app| app.refresh_windows());
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&source_space),
            None,
            "vacated source viewport should be unregistered after refresh"
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&target_space),
            Some(target_opened.window()),
            "target viewport should remain registered"
        );
        assert_eq!(
            runtime.registered_viewport_spaces(),
            vec![target_space.clone()],
            "only the target viewport should remain registered after refresh"
        );
        assert!(
            source_any_window.update(cx, |_, _, _| ()).is_err(),
            "vacated source viewport should close after commit effects refresh"
        );
    }

    #[open_gpui::test]
    fn runtime_opened_cross_window_center_tab_merge_closes_vacated_source_viewport(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source:center");
        let target_space = DockSpaceId::from("target:center");
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
                    viewport_window_options(420.0, 240.0),
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
        let source_any_window = source_opened.window();
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

        source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
        target_visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
        target_visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();

        assert_eq!(
            runtime.active_payload_drag_session(&DockDragPayload::new_item(
                source_space.clone(),
                source_tabs,
                item("a"),
                "Panel A".to_string(),
            )),
            None,
            "drag session should finish after center merge release"
        );

        cx.read_entity(&controller, |controller, _| {
            assert_eq!(controller.graph().root(&source_space), None);
            assert_eq!(controller.graph().collect_items_in_space(&source_space), []);
            let DockNode::Tabs { items, selected } = controller
                .graph()
                .node(target_tabs)
                .expect("target tabs should still exist after center merge")
            else {
                panic!("center merge target should remain a tabs node");
            };
            assert_eq!(
                items.as_slice(),
                &[item("b"), item("a")],
                "center merge should append the source tab to the target tab bar"
            );
            assert_eq!(
                *selected,
                Some(item("a")),
                "center merge should select the payload tab after it is merged back"
            );
        });

        cx.update(|app| app.refresh_windows());
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&source_space),
            None,
            "vacated source viewport should be unregistered after center tab merge"
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&target_space),
            Some(target_opened.window()),
            "target viewport should remain registered after center tab merge"
        );
        assert!(
            source_any_window.update(cx, |_, _, _| ()).is_err(),
            "vacated source viewport should close after its last tab merges back"
        );
    }

    #[open_gpui::test]
    fn runtime_opened_cross_window_drag_clears_state_when_source_window_closes_before_release(
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
        let target = debug_bounds(&mut target_visual, &target_tabs_selector).center();
        source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
        target_visual.simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "A".to_string(),
        );
        let session = runtime
            .active_payload_drag_session(&payload)
            .expect("drag session should be active before source close");
        assert_eq!(
            runtime
                .last_hovered_viewport_identity_for_drag_session(Some(&session))
                .as_ref()
                .map(|identity| identity.window_id()),
            Some(target_window.window_id()),
            "dragging into the target viewport should remember the target as the last hovered viewport"
        );

        let close = cx.update(|app| {
            runtime.handle_window_closed_with_app(source_opened.window().window_id(), app)
        });
        assert_eq!(close.status(), DockViewportCloseStatus::Closed);
        cx.run_until_parked();

        let target_visual = VisualTestContext::from_window(target_opened.window(), cx);
        assert!(
            selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview).is_none(),
            "target viewport drop preview should clear when the source window closes"
        );
        assert_eq!(
            runtime.active_payload_drag_session(&payload),
            None,
            "closing the source window should finish the active drag session"
        );
        assert!(
            !runtime.has_routed_drop_preview_for_drag_session(Some(&session)),
            "closing the source window should clear the routed preview"
        );
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
    fn runtime_opened_cross_window_drag_clears_target_preview_when_target_window_closes(
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
        let target = debug_bounds(&mut target_visual, &target_tabs_selector).center();
        source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
        target_visual.simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "A".to_string(),
        );
        let session = runtime
            .active_payload_drag_session(&payload)
            .expect("drag session should remain active before target close");

        let close = cx.update(|app| {
            runtime.handle_window_closed_with_app(target_opened.window().window_id(), app)
        });
        assert_eq!(close.status(), DockViewportCloseStatus::Closed);
        cx.run_until_parked();

        let source_visual = VisualTestContext::from_window(source_opened.window(), cx);
        assert!(
            selector_for(&source_visual, &source_host, DockDebugRegion::DropPreview).is_none(),
            "source viewport drop preview should clear when the target window closes"
        );
        assert_eq!(
            runtime.routed_drop_preview_for(&target_space, target_window.window_id()),
            None,
            "closing the target window should clear its routed preview"
        );
        assert!(
            runtime.active_payload_drag_session(&payload).is_some(),
            "closing the target window should not finish the source drag session"
        );
        assert!(
            !runtime.has_routed_drop_preview_for_drag_session(Some(&session)),
            "closing the target window should clear the routed preview session state"
        );
        cx.read_entity(&controller, |controller, _| {
            let DockNode::Tabs { items, selected } = controller
                .graph()
                .node(source_tabs)
                .expect("source tabs should still exist")
            else {
                panic!("source should remain tabs");
            };
            assert_eq!(items, &vec![item("a")]);
            assert_eq!(selected.as_ref(), items.first());
        });
    }

    #[open_gpui::test]
    fn runtime_opened_cross_window_bottom_edge_drag_clears_state_when_target_window_closes(
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
        let target_tabs_bounds = debug_bounds(&mut target_visual, &target_tabs_selector);
        let end = inner_edge_drop_position(target_tabs_bounds, DropZone::Bottom);
        source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
        target_visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert!(
            selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview).is_some(),
            "bottom-edge target should render a visible drop preview before close"
        );

        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "A".to_string(),
        );
        let session = runtime
            .active_payload_drag_session(&payload)
            .expect("drag session should be active before target close");

        let close = cx.update(|app| {
            runtime.handle_window_closed_with_app(target_opened.window().window_id(), app)
        });
        assert_eq!(close.status(), DockViewportCloseStatus::Closed);
        cx.run_until_parked();

        let source_visual = VisualTestContext::from_window(source_opened.window(), cx);
        assert!(
            selector_for(&source_visual, &source_host, DockDebugRegion::DropPreview).is_none(),
            "source viewport drop preview should clear when the bottom-edge target window closes"
        );
        assert_eq!(
            runtime.routed_drop_preview_for(&target_space, target_window.window_id()),
            None,
            "closing the bottom-edge target window should clear its routed preview"
        );
        assert!(
            runtime.active_payload_drag_session(&payload).is_some(),
            "closing the bottom-edge target window should not finish the source drag session"
        );
        assert!(
            !runtime.has_routed_drop_preview_for_drag_session(Some(&session)),
            "closing the bottom-edge target window should clear the routed preview session state"
        );
        cx.read_entity(&controller, |controller, _| {
            let DockNode::Tabs { items, selected } = controller
                .graph()
                .node(source_tabs)
                .expect("source tabs should still exist")
            else {
                panic!("source should remain tabs");
            };
            assert_eq!(items, &vec![item("a")]);
            assert_eq!(selected.as_ref(), items.first());
        });
    }

    #[open_gpui::test]
    fn viewport_runtime_handle_prevents_platform_close_when_policy_prevents(
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
        runtime.set_close_policy(DockViewportClosePolicy::Prevent);
        assert_eq!(runtime.close_policy(), DockViewportClosePolicy::Prevent);
        assert!(
            !visual.simulate_close(),
            "updated Prevent policy should veto GPUI should-close before the window closes"
        );
        assert_eq!(
            cx.update(
                |app| runtime.handle_window_should_close_with_app(opened.window().window_id(), app)
            )
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
            cx.update(
                |app| runtime.handle_window_should_close_with_app(opened.window().window_id(), app)
            )
            .status,
            DockViewportShouldCloseStatus::Allowed
        );
    }
}
