//! Concern-owned viewport placement regression tests.

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
        workspace.policy_mut().set_allow_platform_viewports(true);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
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
                runtime.open_viewport_unchecked_policy(
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
    fn viewport_runtime_open_viewport_fails_closed_when_policy_disables_platform_viewports(
        cx: &mut TestAppContext,
    ) {
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
        let before_windows = cx.windows().len();

        let error = cx
            .update(|app| {
                runtime.open_viewport(
                    secondary_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect_err("policy-disabled runtime open should fail before window creation");

        assert_eq!(
            error.downcast_ref::<DockPolicyError>(),
            Some(&DockPolicyError::PlatformViewportsDisabled)
        );
        assert_eq!(
            runtime
                .borrow()
                .adapter()
                .window_for_space(&secondary_space),
            None
        );
        assert_eq!(cx.windows().len(), before_windows);
    }

    #[open_gpui::test]
    fn viewport_runtime_open_viewport_fails_closed_when_platform_viewport_windows_unsupported(
        cx: &mut TestAppContext,
    ) {
        cx.set_platform_viewport_windows(false);

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
        workspace.policy_mut().set_allow_platform_viewports(true);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller);
        let before_windows = cx.windows().len();

        let error = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
                    secondary_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect_err("unsupported backend should reject viewport open before window creation");

        let io_error = error
            .downcast_ref::<std::io::Error>()
            .expect("unsupported viewport open should be reported as an io error");
        assert_eq!(io_error.kind(), std::io::ErrorKind::Unsupported);
        assert_eq!(
            runtime
                .borrow()
                .adapter()
                .window_for_space(&secondary_space),
            None
        );
        assert_eq!(cx.windows().len(), before_windows);
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
    fn viewport_runtime_tear_off_fails_closed_when_platform_viewport_windows_unsupported(
        cx: &mut TestAppContext,
    ) {
        cx.set_platform_viewport_windows(false);

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
            .expect_err("unsupported backend should reject tear-off before window creation");

        let io_error = error
            .downcast_ref::<std::io::Error>()
            .expect("unsupported tear-off should be reported as an io error");
        assert_eq!(io_error.kind(), std::io::ErrorKind::Unsupported);
        assert_eq!(
            runtime.borrow().pending_tear_off_len(),
            0,
            "failed tear-off should cancel pending state"
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&detached_space),
            None
        );
        cx.run_until_parked();
        cx.update(|app| app.refresh_windows());
        assert_eq!(cx.windows().len(), before_windows);
        cx.read_entity(&controller, |controller, _| {
            assert_eq!(
                controller.graph().collect_items_in_space(&primary_space),
                vec![item("a"), item("b")]
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

        let first = runtime_core.begin_tear_off_request_with_focus(
            tear_off_request(primary_space.clone(), source_tabs, item("a")),
            detached_space.clone(),
            None,
        );
        let second = runtime_core.begin_tear_off_request_with_focus(
            tear_off_request(primary_space.clone(), source_tabs, item("a")),
            DockSpaceId::from("other"),
            None,
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
    fn viewport_runtime_tear_off_cancellation_clears_pending_without_graph_mutation(
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

        let request = tear_off_request(primary_space.clone(), source_tabs, item("a"));
        let key = request.key();
        runtime.begin_tear_off_request_with_focus(request, detached_space.clone(), None);
        let cancelled = runtime
            .cancel_tear_off_request(&key, DockViewportTearOffCancelReason::Cancelled)
            .expect("pending tear-off request should cancel");

        assert_eq!(
            cancelled.reason(),
            DockViewportTearOffCancelReason::Cancelled
        );
        assert_eq!(cancelled.pending().target_space(), &detached_space);
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
    fn viewport_runtime_replacement_preserves_logical_space_panel_focus(cx: &mut TestAppContext) {
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
                runtime.open_viewport_unchecked_policy(
                    secondary_space.clone(),
                    WindowOptions {
                        focus: false,
                        ..viewport_window_options(360.0, 220.0)
                    },
                    app,
                )
            })
            .expect("secondary viewport should open through runtime");
        runtime.record_panel_focus(secondary_space.clone(), item("b"));
        assert_eq!(
            runtime
                .borrow()
                .recorded_had_panel_focus_for_test(&secondary_space),
            Some(true)
        );

        let replacement = open_controller_space(
            cx,
            runtime.borrow().controller_entity(),
            secondary_space.clone(),
            size(px(360.0), px(220.0)),
        )
        .0;
        let replacement: AnyWindowHandle = replacement.into();
        assert_eq!(
            runtime
                .borrow_mut()
                .register_opened_viewport(secondary_space.clone(), replacement),
            vec![opened.window()]
        );

        assert_eq!(
            runtime
                .borrow()
                .recorded_had_panel_focus_for_test(&secondary_space),
            Some(true),
            "replacing the platform window must not discard logical dock-space focus history"
        );
        focus_backend_window_for_test(replacement, cx);
        assert_eq!(
            cx.update(|app| {
                runtime.focus_command_for_confirmed_backend_window_focus(
                    &secondary_space,
                    replacement.window_id(),
                    false,
                    app,
                )
            }),
            None,
            "replacement viewport first consumes initial/destroyed focus suppression"
        );
        let command = cx.update(|app| {
            runtime.focus_command_for_confirmed_backend_window_focus(
                &secondary_space,
                replacement.window_id(),
                false,
                app,
            )
        });
        assert_eq!(
            command.as_ref().map(DockViewportFocusCommand::request),
            Some(&DockViewportFocusRequest::panel("b"))
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_replacement_preserves_logical_space_no_panel_focus(
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
                runtime.open_viewport_unchecked_policy(
                    secondary_space.clone(),
                    WindowOptions {
                        focus: false,
                        ..viewport_window_options(360.0, 220.0)
                    },
                    app,
                )
            })
            .expect("secondary viewport should open through runtime");
        runtime.record_no_panel_focus(&secondary_space);
        assert_eq!(
            runtime
                .borrow()
                .recorded_had_panel_focus_for_test(&secondary_space),
            Some(false)
        );

        let replacement = open_controller_space(
            cx,
            runtime.borrow().controller_entity(),
            secondary_space.clone(),
            size(px(360.0), px(220.0)),
        )
        .0;
        let replacement: AnyWindowHandle = replacement.into();
        assert_eq!(
            runtime
                .borrow_mut()
                .register_opened_viewport(secondary_space.clone(), replacement),
            vec![opened.window()]
        );

        assert_eq!(
            runtime
                .borrow()
                .recorded_had_panel_focus_for_test(&secondary_space),
            Some(false),
            "replacing the platform window must preserve explicit no-panel-focus history"
        );
        focus_backend_window_for_test(replacement, cx);
        assert_eq!(
            cx.update(|app| {
                runtime.focus_command_for_confirmed_backend_window_focus(
                    &secondary_space,
                    replacement.window_id(),
                    false,
                    app,
                )
            }),
            None,
            "replacement viewport first consumes initial/destroyed focus suppression"
        );
        let command = cx.update(|app| {
            runtime.focus_command_for_confirmed_backend_window_focus(
                &secondary_space,
                replacement.window_id(),
                false,
                app,
            )
        });
        assert_eq!(
            command.as_ref().map(DockViewportFocusCommand::request),
            Some(&DockViewportFocusRequest::no_panel_focus())
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
                runtime.open_viewport_unchecked_policy(
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
    fn viewport_runtime_open_registration_rebinds_one_runtime_window_across_two_spaces_and_keeps_target_state(
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

        let source_opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
                    source_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("source viewport should open through runtime");
        let target_opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
                    target_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("target viewport should open through runtime");

        runtime.record_panel_focus(source_space.clone(), item("a"));
        runtime.record_panel_focus(target_space.clone(), item("b"));
        seed_runtime_host_scene_for_test(
            &runtime,
            &source_space,
            source_opened.window(),
            source_tabs,
        );
        seed_runtime_host_scene_for_test(
            &runtime,
            &target_space,
            target_opened.window(),
            target_tabs,
        );

        let registration = runtime
            .borrow_mut()
            .register_opened_viewport_with_cleanup(target_space.clone(), source_opened.window());
        let effects = registration.window_effects();

        assert_eq!(registration.outcome.space(), &target_space);
        assert_eq!(registration.outcome.window(), source_opened.window());
        assert_eq!(registration.outcome.replaced().len(), 2);
        assert!(
            registration
                .outcome
                .replaced()
                .contains(&crate::DockViewportUnregisterOutcome {
                    space: target_space.clone(),
                    window: target_opened.window(),
                    reason: crate::DockViewportUnregisterReason::Replaced,
                })
        );
        assert!(
            registration
                .outcome
                .replaced()
                .contains(&crate::DockViewportUnregisterOutcome {
                    space: source_space.clone(),
                    window: source_opened.window(),
                    reason: crate::DockViewportUnregisterReason::Replaced,
                })
        );
        assert_eq!(effects.close_now(), &[target_opened.window()]);
        assert!(effects.refresh().is_empty());
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&source_space),
            None
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&target_space),
            Some(source_opened.window())
        );
        assert_eq!(
            runtime.recorded_had_panel_focus_for_test(&source_space),
            None,
            "moving the window away from its old space should retire that space focus state"
        );
        assert_eq!(
            runtime.recorded_had_panel_focus_for_test(&target_space),
            Some(true),
            "same-space replacement must preserve the surviving target focus state"
        );
        assert_eq!(runtime.last_host_scene_screen_position(&source_space), None);
        assert_eq!(runtime.last_host_scene_screen_position(&target_space), None);
        assert_eq!(
            runtime.borrow().adapter().spaces(),
            vec![target_space.clone()]
        );

        assert!(
            !runtime
                .borrow_mut()
                .unregister_host_for_space(&source_space, source_opened.window().window_id())
        );
        assert_eq!(
            runtime.recorded_had_panel_focus_for_test(&target_space),
            Some(true),
            "stale source-space cleanup must not disturb the surviving target focus state"
        );

        let stale_outcome = runtime
            .borrow_mut()
            .handle_window_closed(target_opened.window().window_id());
        assert_eq!(
            stale_outcome.status(),
            DockViewportCloseStatus::UnknownWindow
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&target_space),
            Some(source_opened.window())
        );
        assert_eq!(
            runtime.recorded_had_panel_focus_for_test(&target_space),
            Some(true),
            "late close for the retired window must not affect the live target mapping"
        );

        let closed_outcome = runtime
            .borrow_mut()
            .handle_window_closed(source_opened.window().window_id());
        assert_eq!(closed_outcome.status(), DockViewportCloseStatus::Closed);
        assert_eq!(closed_outcome.space(), Some(&target_space));
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&target_space),
            None
        );
        assert_eq!(
            runtime.recorded_had_panel_focus_for_test(&target_space),
            None,
            "closing the live rehomed window should retire the target focus state"
        );
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

        let payload =
            DockDragPayload::new_tabs(source_space.clone(), source_tabs, "Stack".to_string());
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
            prepared.focus_item(),
            Some(&item("c")),
            "prepared tear-off should freeze focus from the delivery snapshot"
        );

        controller.update(cx, |controller, _| {
            controller
                .select_tab(source_tabs, item("a"))
                .expect("test should be able to change selected tab after preparation");
        });

        assert_eq!(
            prepared.focus_item(),
            Some(&item("c")),
            "later selected-tab changes must not rewrite prepared tear-off focus"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_prepared_tear_off_does_not_infer_selected_tab_focus(
        cx: &mut TestAppContext,
    ) {
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

        let payload =
            DockDragPayload::new_tabs(source_space.clone(), source_tabs, "Stack".to_string());
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
            prepared.focus_item(),
            None,
            "selected tab alone is not a recorded focus identity"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_tear_off_without_geometry_rejects_release_point_only(
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

        let placement = runtime.tear_off_window_placement(&request).expect(
            "host-suggested bounds should authorize tear-off placement without global release",
        );
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

    #[open_gpui::test]
    fn viewport_runtime_tear_off_large_drag_bounds_limit_to_undock_work_area(
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
            floating_bounds(0.0, 0.0, 1200.0, 900.0),
            point(px(600.0), px(450.0)),
        )
        .with_preferred_size(size(px(1200.0), px(900.0)))
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
            WindowBounds::Windowed(floating_bounds(100.0, 80.0, 900.0, 720.0))
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_tear_off_suggested_bounds_use_undock_limited_drag_size(
        cx: &mut TestAppContext,
    ) {
        let source_window_bounds =
            WindowBounds::Windowed(floating_bounds(100.0, 200.0, 1200.0, 900.0));
        let geometry = DockDragTearOffGeometry::from_source_bounds(
            floating_bounds(0.0, 0.0, 1200.0, 900.0),
            point(px(600.0), px(450.0)),
        )
        .with_preferred_size(size(px(1200.0), px(900.0)))
        .with_display_work_area(floating_bounds(0.0, 0.0, 1000.0, 800.0));
        let suggested = crate::viewport_runtime::suggested_tear_off_window_bounds(
            source_window_bounds,
            point(px(1100.0), px(780.0)),
            geometry,
        );

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
            .expect("host-suggested bounds should authorize tear-off placement");
        assert_eq!(
            placement.source(),
            DockViewportTearOffPlacementSource::Suggested
        );
        assert_eq!(
            placement.window_bounds(),
            WindowBounds::Windowed(floating_bounds(100.0, 80.0, 900.0, 720.0))
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
        let error = cx
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
            .expect_err("invalid floating source node should be rejected before opening a window");

        assert!(
            error.to_string().contains("did not match"),
            "invalid floating source node should fail preflight, got {error}"
        );
        assert_eq!(
            runtime.borrow().pending_tear_off_len(),
            0,
            "preflight rejection must not create pending tear-off state"
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&detached_space),
            None,
            "preflight rejection must never register a routeable dock viewport"
        );
        cx.run_until_parked();
        cx.update(|app| app.refresh_windows());
        assert_eq!(
            cx.windows().len(),
            before_windows,
            "preflight rejection must not open a platform window"
        );
        cx.read_entity(&controller, |controller, _| {
            assert_eq!(
                controller.graph().collect_items_in_space(&primary_space),
                vec![item("a")]
            );
        });
    }

    #[open_gpui::test]
    fn viewport_runtime_handle_rejects_cancelled_tear_off_pending_completion(
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
        assert_eq!(runtime.borrow().pending_tear_off_len(), 1);
        assert!(
            runtime
                .borrow_mut()
                .cancel_tear_off_request(
                    &pending.request().key(),
                    DockViewportTearOffCancelReason::Cancelled,
                )
                .is_some()
        );
        assert_eq!(runtime.borrow().pending_tear_off_len(), 0);

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
            .expect_err(
                "cancelled pending tear-off requests must not commit from a stale pending value",
            );
        assert!(
            error
                .to_string()
                .contains("dock drop target is not currently available"),
            "cancelled pending tear-off should report unavailable target, got {error}"
        );
        cx.run_until_parked();
        cx.update(|app| app.refresh_windows());

        assert_eq!(
            runtime.borrow().pending_tear_off_len(),
            0,
            "stale completion must not recreate pending tear-off state"
        );
        assert_eq!(
            runtime.borrow().adapter().window_for_space(&detached_space),
            None,
            "stale completion must not register the uncommitted viewport"
        );
        assert_eq!(
            cx.windows().len(),
            before_finish_windows.saturating_sub(1),
            "stale completion must close the unregistered platform window"
        );
        cx.read_entity(&controller, |controller, _| {
            assert_eq!(
                controller.graph().collect_items_in_space(&primary_space),
                vec![item("a"), item("b")],
                "source content must stay in place when a pending tear-off has expired"
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
                DockViewportPlatformSignals::from_app(app)
                    .with_trusted_hovered_window(detached_window),
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
                DockViewportPlatformSignals::from_app(app)
                    .with_trusted_hovered_window(detached_window),
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
    fn viewport_runtime_handle_publishes_minimized_window_as_not_routable(cx: &mut TestAppContext) {
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
        let runtime = DockViewportRuntimeHandle::new(controller);
        let opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
                    target_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("target viewport should open");
        let target_window = opened
            .window()
            .downcast::<crate::DockHost>()
            .expect("target viewport should render DockHost");
        let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);

        target_window
            .update(cx, |host, window, cx| {
                host.publish_viewport_host_scene_interaction(
                    host_bounds,
                    target_center_host_position(),
                    window,
                    cx,
                );
            })
            .expect("target host should publish a live scene");
        assert!(runtime.viewport_route_ready(&target_space));

        target_window
            .update(cx, |host, window, cx| {
                window.minimize_window();
                assert!(window.is_minimized());
                host.publish_viewport_host_scene_interaction(
                    host_bounds,
                    target_center_host_position(),
                    window,
                    cx,
                );
            })
            .expect("target host should publish minimized window facts");

        assert!(!runtime.viewport_route_ready(&target_space));
        assert_eq!(
            runtime.viewport_route_unavailable_reason(&target_space),
            Some(DockViewportRouteUnavailableReason::Minimized)
        );
        assert_eq!(
            runtime
                .runtime_status()
                .viewport_lifecycle
                .iter()
                .find(|record| record.space == target_space)
                .map(|record| record.route_status),
            Some(DockViewportRouteStatus::Minimized)
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_rechecks_minimized_state_before_route_without_render(
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

        let opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
                    target_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("target viewport should open through runtime handle");
        let target_window = opened
            .window()
            .downcast::<crate::DockHost>()
            .expect("target viewport should render DockHost");
        let target_window_bounds = target_window
            .update(cx, |_, window, _| window.window_bounds())
            .expect("target window should be live");
        let target_window_bounds = WindowBounds::Windowed(target_window_bounds.get_bounds());
        let host_bounds = floating_bounds(0.0, 0.0, 360.0, 220.0);
        target_window
            .update(cx, |host, window, cx| {
                host.publish_viewport_host_scene_interaction(
                    host_bounds,
                    target_center_host_position(),
                    window,
                    cx,
                );
            })
            .expect("target host should publish live route facts");
        assert!(runtime.push_viewport_host_scene_fact(
            &target_space,
            opened.window().window_id(),
            leaf_host_scene_fact(target_tabs, target_tabs),
        ));
        assert!(runtime.viewport_route_ready(&target_space));

        target_window
            .update(cx, |_, window, _| {
                window.minimize_window();
                assert!(window.is_minimized());
            })
            .expect("target window should still be live after minimize");

        let target_point =
            screen_position_for_host_position(target_window_bounds, target_center_host_position());
        let resolution = cx.update(|app| {
            let request = DockViewportDropRouteRequest::from_platform_signals(
                source_space.clone(),
                source_tabs,
                DockViewportDropPayload::Item(item("a")),
                target_point,
                Some(target_window_bounds),
                DockViewportPlatformSignals::from_app(app)
                    .with_trusted_hovered_window(opened.window()),
            );
            runtime.resolve_payload_drop_delivery(&request, app)
        });

        assert_eq!(resolution.route(), &DockViewportDropRoute::Unavailable);
        assert_eq!(resolution.delivery(), None);
        assert!(!runtime.viewport_route_ready(&target_space));
        assert_eq!(
            runtime.viewport_route_unavailable_reason(&target_space),
            Some(DockViewportRouteUnavailableReason::Minimized)
        );
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
        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);

        let outcome = cx
            .update(|app| {
                let request = DockViewportDropRouteRequest::from_target_context(
                    source_space.clone(),
                    source_tabs,
                    DockViewportDropPayload::Item(item("a")),
                    release_position,
                    Some(suggested_window_bounds),
                    DockViewportTargetContext::new(),
                )
                .with_drag_session(Some(session.clone()));
                let resolution = runtime.resolve_payload_drop_delivery(&request, app);
                runtime.deliver_drop_commit_delivery(
                    DockDropDelivery::from_resolution(resolution)?,
                    app,
                )
            })
            .expect("tear-off route should commit through runtime handle");

        let activation = outcome.activation_transaction();
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
            "tear-off completion should surface the new viewport activation transaction"
        );
        let active_window_before_activation = completed
            .registration()
            .window()
            .update(cx, |_, _, app| app.active_window())
            .expect("tear-off viewport should be live");
        assert_eq!(
            active_window_before_activation.map(|window| window.window_id()),
            None,
            "tear-off registration must only create an activation transaction, not directly activate the platform window"
        );
        cx.update(|app| {
            assert!(
                apply_viewport_activation_transaction(activation.clone(), app).changed(),
                "applying the tear-off activation transaction should activate the new viewport"
            );
        });
        let active_window_after_activation = completed
            .registration()
            .window()
            .update(cx, |_, _, app| app.active_window())
            .expect("tear-off viewport should remain live after activation");
        assert_eq!(
            active_window_after_activation.map(|window| window.window_id()),
            Some(completed.registration().window().window_id()),
            "platform focus should be written only by the activation transaction apply path"
        );
        assert_eq!(
            completed.pending().request().release_position(),
            Some(release_position)
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
            assert_eq!(
                host.viewport_runtime()
                    .window_id_for_space(completed.pending().target_space()),
                Some(completed.registration().window().window_id()),
                "tear-off viewport should keep the runtime-backed host path for dock-back"
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
        let payload =
            DockDragPayload::new_tabs(source_space.clone(), source_tabs, "Stack".to_string());
        let session = runtime.begin_payload_drag(&payload);

        let outcome = cx
            .update(|app| {
                let request = DockViewportDropRouteRequest::from_target_context(
                    source_space.clone(),
                    source_tabs,
                    DockViewportDropPayload::Tabs,
                    release_position,
                    Some(suggested_window_bounds),
                    DockViewportTargetContext::new(),
                )
                .with_drag_session(Some(session.clone()));
                let resolution = runtime.resolve_payload_drop_delivery(&request, app);
                runtime.deliver_drop_commit_delivery(
                    DockDropDelivery::from_resolution(resolution)?,
                    app,
                )
            })
            .expect("stack tear-off route should commit through runtime handle");

        let activation = outcome.activation_transaction();
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
            "stack tear-off completion should surface the new viewport activation transaction"
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
            assert_eq!(
                host.viewport_runtime()
                    .window_id_for_space(completed.pending().target_space()),
                Some(completed.registration().window().window_id()),
                "stack tear-off viewport should keep the runtime-backed host path for dock-back"
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
                runtime.open_viewport_unchecked_policy(
                    source_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("source viewport should open");
        let target_opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
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

        activate_window_for_pointer_input(&mut source_visual);
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
        cx.set_platform_hovered_window(Some(target_opened.window()));
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
        cx.set_platform_hovered_window(None);

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
    fn runtime_opened_viewports_reject_source_only_dock_back_without_current_route_facts(
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
                runtime.open_viewport_unchecked_policy(
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
                runtime.open_viewport_unchecked_policy(
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
        cx.set_platform_window_stack(Some(vec![source_opened.window(), target_opened.window()]));
        let source_release_signals = source_opened
            .window()
            .update(cx, |_, _, app| {
                DockViewportPlatformSignals::from_app_without_hovered_window_signal(app)
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
        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let session = runtime.begin_payload_drag(&payload);

        let result = cx.update(|app| {
            let request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
                source_space.clone(),
                source_tabs,
                DockViewportDropPayload::Item(item("a")),
                release_screen_position,
                None,
                source_release_signals,
                DockPayloadDropReleaseOrigin::SourceOnly,
            )
            .with_drag_session(Some(session.clone()));
            runtime.commit_payload_drop_from_screen(&request, app)
        });

        assert_eq!(result, Err(DockActionApplyError::DropTargetUnavailable));
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
        assert!(matches!(
            status
                .last_route
                .as_ref()
                .expect("source-only dock-back attempt should record a route")
                .target,
            crate::DockViewportRouteTarget::Unavailable
        ));
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
            assert_eq!(
                controller.graph().collect_items_in_space(&source_space),
                vec![item("a")],
                "without current route facts, source-only dock-back must leave the source payload in place"
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
                runtime.open_viewport_unchecked_policy(
                    source_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("source viewport should open");
        let target_opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
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

        activate_window_for_pointer_input(&mut source_visual);
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
        cx.set_platform_hovered_window(Some(target_opened.window()));
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
        cx.set_platform_hovered_window(None);
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
}
