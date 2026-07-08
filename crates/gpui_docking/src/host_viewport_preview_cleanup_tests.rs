//! Routed viewport preview cleanup regression tests.

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

        let registration =
            runtime.register_opened_viewport_with_cleanup(target_space.clone(), new_window);
        let effects = registration.window_effects();

        assert!(
            effects.close_now().is_empty(),
            "adapter-seeded windows are not runtime-owned and should not be closed as replacements"
        );
        assert_eq!(
            effects.refresh(),
            &[new_window],
            "replacement cleanup should refresh the surviving current viewport after clearing the old preview"
        );
        assert_eq!(
            runtime.routed_drop_preview_for(&target_space, old_window.window_id()),
            None
        );
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
    fn viewport_runtime_reusable_stale_source_returns_routed_preview_target_for_refresh(
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

        let source_window = handle(64);
        let target_window = handle(65);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source_space.clone(), source_window);
        register_viewport(&mut adapter, target_space.clone(), target_window);
        let mut runtime = DockViewportRuntime::from_adapter(
            controller,
            adapter,
            DockViewportClosePolicy::RetainLayout,
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
        assert!(
            runtime
                .routed_drop_preview_for(&target_space, target_window.window_id())
                .is_some()
        );

        let reusable =
            cx.update(|app| runtime.reusable_window_for_space_with_cleanup(&source_space, app));
        let (window, effects) = reusable.into_parts();

        assert!(
            matches!(window, crate::DockViewportReusableWindow::Stale),
            "test handle should behave like a stale GPUI window"
        );
        assert_eq!(
            effects.refresh(),
            &[target_window],
            "clearing a stale source drag should refresh the surviving routed preview target"
        );
        assert_eq!(runtime.adapter().window_for_space(&source_space), None);
        assert_eq!(
            runtime.adapter().window_for_space(&target_space),
            Some(target_window)
        );
        assert_eq!(
            runtime.routed_drop_preview_for(&target_space, target_window.window_id()),
            None
        );
        assert!(!runtime.has_routed_drop_preview_for_drag_session(Some(&session)));
    }

    #[open_gpui::test]
    fn viewport_runtime_unregister_source_host_returns_routed_preview_target_for_refresh(
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

        let source_window = handle(92);
        let target_window = handle(93);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source_space.clone(), source_window);
        register_viewport(&mut adapter, target_space.clone(), target_window);
        let mut runtime = DockViewportRuntime::from_adapter(
            controller,
            adapter,
            DockViewportClosePolicy::RetainLayout,
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
        assert!(
            runtime
                .routed_drop_preview_for(&target_space, target_window.window_id())
                .is_some(),
            "preview setup should cache a routed preview for the target"
        );

        let update = runtime
            .unregister_host_for_space_with_pointer_sync(&source_space, source_window.window_id());

        assert!(update.changed());
        assert_eq!(runtime.adapter().window_for_space(&source_space), None);
        assert_eq!(
            runtime.adapter().window_for_space(&target_space),
            Some(target_window)
        );
        let pointer_sync = update.pointer_input_sync();
        assert_eq!(
            update.into_windows(),
            vec![target_window],
            "unregistering the drag source should refresh the surviving routed-preview target"
        );
        assert_eq!(
            pointer_sync.map(|request| request.window().window_id()),
            Some(source_window.window_id()),
            "unregistering the source should still restore its pointer-input state"
        );
        assert_eq!(
            runtime.routed_drop_preview_for(&target_space, target_window.window_id()),
            None
        );
        assert!(!runtime.has_routed_drop_preview_for_drag_session(Some(&session)));
    }
}
