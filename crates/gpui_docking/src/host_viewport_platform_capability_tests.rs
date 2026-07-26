use crate::{
    DockViewportPlatformSyncDispatch, DockViewportPlatformSyncRequest,
    DockViewportPlatformSyncUnsupportedReason,
    host_test_support::{test_view, viewport_window_options},
    sync_reused_viewport_window,
    viewport_registry::DockViewportPlatformRequests,
};
use open_gpui::{TestAppContext, WindowBackgroundAppearance, WindowKind, WindowOptions};

#[open_gpui::test]
fn reused_viewport_no_input_request_queues_typed_pointer_dispatch(cx: &mut TestAppContext) {
    let root = test_view(cx, "viewport");
    let window = cx
        .update(|app| app.open_window(viewport_window_options(320.0, 240.0), |_, _| root.clone()))
        .expect("test window should open");

    let sync_result = window
        .update(cx, |_, window, _| {
            sync_reused_viewport_window(
                window,
                &WindowKind::Normal,
                WindowOptions {
                    focus: false,
                    accepts_pointer_input: false,
                    ..viewport_window_options(320.0, 240.0)
                },
                DockViewportPlatformRequests::default(),
            )
        })
        .expect("test window should stay live");
    let sync_record = sync_result.record();

    assert!(sync_record.dispatches.iter().any(|dispatch| matches!(
        dispatch,
        DockViewportPlatformSyncDispatch::Queued {
            request: DockViewportPlatformSyncRequest::PointerInput { requested: false },
            ..
        }
    )));
}

#[open_gpui::test]
fn reused_transparent_background_reports_creation_only_without_mutating_facts(
    cx: &mut TestAppContext,
) {
    let root = test_view(cx, "viewport");
    let window = cx
        .update(|app| app.open_window(viewport_window_options(320.0, 240.0), |_, _| root.clone()))
        .expect("test window should open");

    let sync_result = window
        .update(cx, |_, window, _| {
            sync_reused_viewport_window(
                window,
                &WindowKind::Normal,
                WindowOptions {
                    window_background: WindowBackgroundAppearance::Transparent,
                    ..viewport_window_options(320.0, 240.0)
                },
                DockViewportPlatformRequests::default(),
            )
        })
        .expect("test window should stay live");
    let sync_record = sync_result.record();

    assert!(sync_record.dispatches.iter().any(|dispatch| matches!(
        dispatch,
        DockViewportPlatformSyncDispatch::Unsupported(unsupported)
            if unsupported.request
                == DockViewportPlatformSyncRequest::BackgroundAppearance {
                    requested: WindowBackgroundAppearance::Transparent
                }
                && unsupported.reason
                    == DockViewportPlatformSyncUnsupportedReason::CreationOnly
    )));
    assert_eq!(
        window
            .update(cx, |_, window, _| {
                window.platform_facts().background_appearance
            })
            .expect("test window should stay live"),
        WindowBackgroundAppearance::Opaque
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
        DockViewportPlatformSyncAction, DockViewportPlatformSyncDispatch,
        DockViewportPlatformSyncObservationOutcome, DockViewportPlatformSyncRejectedReason,
        DockViewportPlatformSyncRequest, DockViewportPlatformSyncUnsupportedReason,
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
        AnyWindowHandle, AppContext as _, Focusable, PlatformWindowMutationTerminal, SharedString,
        TestAppContext, TitlebarOptions, VisualTestContext, WindowBackgroundAppearance,
        WindowBounds, WindowHandle, WindowId, WindowKind, WindowMutationDomain, WindowOptions,
        point, px, size,
    };

    use crate::host_viewport_runtime_test_support::*;

    fn viewport_facts_generation(runtime: &DockViewportRuntimeHandle, space: &DockSpaceId) -> u64 {
        runtime
            .runtime_status()
            .viewport_lifecycle
            .into_iter()
            .find(|record| &record.space == space)
            .expect("viewport lifecycle should contain the requested space")
            .facts_generation
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

    #[open_gpui::test]
    fn runtime_status_projects_each_viewports_actual_window_kind_profile(cx: &mut TestAppContext) {
        let space = DockSpaceId::from("floating");
        let mut graph = DockGraph::new();
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        graph.set_root(space.clone(), tabs);
        let mut workspace = DockWorkspace::new(space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller);

        let opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
                    space.clone(),
                    WindowOptions {
                        kind: WindowKind::Floating,
                        ..viewport_window_options(360.0, 220.0)
                    },
                    app,
                )
            })
            .expect("floating viewport should open through runtime");
        let status = cx.update(|app| runtime.runtime_status_for_app(app));

        assert_eq!(status.window_mutation_capabilities.len(), 1);
        let profile = &status.window_mutation_capabilities[0];
        assert_eq!(profile.space, space);
        assert_eq!(profile.window_id, opened.window().window_id());
        assert_eq!(profile.window_kind, WindowKind::Floating);
        assert_eq!(
            profile.capabilities,
            opened
                .window()
                .update(cx, |_, window, _| window.window_mutation_capabilities())
                .expect("floating viewport should remain live")
        );
    }

    #[open_gpui::test]
    fn reused_viewport_compares_requested_kind_with_actual_window_kind(cx: &mut TestAppContext) {
        let space = DockSpaceId::from("floating");
        let mut graph = DockGraph::new();
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        graph.set_root(space.clone(), tabs);
        let mut workspace = DockWorkspace::new(space.clone(), graph);
        workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller);

        let opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
                    space.clone(),
                    WindowOptions {
                        kind: WindowKind::Floating,
                        ..viewport_window_options(360.0, 220.0)
                    },
                    app,
                )
            })
            .expect("floating viewport should open through runtime");

        let reused = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
                    space.clone(),
                    WindowOptions {
                        kind: WindowKind::Floating,
                        ..viewport_window_options(360.0, 220.0)
                    },
                    app,
                )
            })
            .expect("floating viewport should be reused through runtime");
        assert_eq!(reused.status(), DockViewportOpenStatus::Reused);
        assert_eq!(reused.window(), opened.window());
        assert!(
            !runtime
                .runtime_status()
                .last_platform_dispatch
                .expect("same-kind reuse should record platform dispatch diagnostics")
                .dispatches
                .iter()
                .any(|dispatch| matches!(
                    dispatch,
                    DockViewportPlatformSyncDispatch::Unsupported(unsupported)
                        if unsupported.request == DockViewportPlatformSyncRequest::WindowKind
                )),
            "same-kind floating reuse must not be compared with an assumed Normal kind"
        );

        let reused = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
                    space,
                    WindowOptions {
                        kind: WindowKind::Normal,
                        ..viewport_window_options(360.0, 220.0)
                    },
                    app,
                )
            })
            .expect("different-kind request should still reuse the existing viewport");
        assert_eq!(reused.status(), DockViewportOpenStatus::Reused);
        assert!(
            runtime
                .runtime_status()
                .last_platform_dispatch
                .expect("different-kind reuse should record platform dispatch diagnostics")
                .dispatches
                .iter()
                .any(|dispatch| matches!(
                    dispatch,
                    DockViewportPlatformSyncDispatch::Unsupported(unsupported)
                        if unsupported.request == DockViewportPlatformSyncRequest::WindowKind
                            && unsupported.reason
                                == DockViewportPlatformSyncUnsupportedReason::CreationOnly
                )),
            "changing a reused viewport's immutable kind must be reported as creation-only"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_drag_tracks_source_without_mutating_no_input_state(
        cx: &mut TestAppContext,
    ) {
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

        let session = runtime.begin_payload_drag_with_focus(&payload, None);
        assert_eq!(
            runtime.active_payload_drag_source_window_id(&payload),
            Some(window.window_id()),
            "the immutable source window should remain available for cross-window routing"
        );

        assert!(runtime.finish_payload_drag(&session).changed());
        assert_eq!(runtime.active_payload_drag_source_window_id(&payload), None);
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
                runtime.open_viewport_unchecked_policy(
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
                runtime.open_viewport_unchecked_policy(
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
        assert_eq!(bounds.size, size(px(360.0), px(220.0)));
        assert_eq!(
            bounds.origin,
            point(px(0.0), px(0.0)),
            "queued placement must preserve the last committed screen origin"
        );
        assert!(
            reused
                .window()
                .update(cx, |_, window, _| window
                    .platform_facts()
                    .accepts_pointer_input)
                .expect("reused viewport should remain live"),
            "queued pointer input must not overwrite the committed fact"
        );
        assert_eq!(
            runtime.viewport_route_unavailable_reason(&secondary_space),
            None,
            "queued pointer intent does not invalidate observed route facts"
        );
        assert_eq!(
            viewport_input_status(&runtime, &secondary_space),
            Some(DockViewportInputStatus::ReceivesInput),
            "runtime registry must retain committed pointer-input facts until observation"
        );

        let sync = runtime
            .runtime_status()
            .last_platform_dispatch
            .expect("reuse should record platform dispatch diagnostics");
        assert_eq!(sync.window_id, reused.window().window_id());
        assert!(sync.dispatches.iter().any(|dispatch| matches!(
            dispatch,
            DockViewportPlatformSyncDispatch::Immediate {
                action: DockViewportPlatformSyncAction::Activate
            }
        )));
        assert!(sync.dispatches.iter().any(|dispatch| matches!(
            dispatch,
            DockViewportPlatformSyncDispatch::Immediate {
                action: DockViewportPlatformSyncAction::Title { title }
            } if title == "Retitled"
        )));
        assert!(
            sync.dispatches.iter().any(|dispatch| matches!(
                dispatch,
                DockViewportPlatformSyncDispatch::Queued {
                    request: DockViewportPlatformSyncRequest::Placement { .. },
                    ..
                }
            )),
            "placement is one queued conflict domain, not separate resize/origin/state facts"
        );
        assert!(
            sync.dispatches.iter().any(|dispatch| matches!(
                dispatch,
                DockViewportPlatformSyncDispatch::Queued {
                    request: DockViewportPlatformSyncRequest::PointerInput { requested: false },
                    ..
                }
            )),
            "pointer input remains independently dispatchable from placement"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_commits_adjusted_terminal_placement_after_queued_dispatch(
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

        let opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
                    secondary_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("secondary viewport should open through runtime");
        cx.run_until_parked();
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
            None,
            "the rendered host must publish initial committed facts before reuse"
        );

        let requested_bounds = floating_bounds(24.0, 32.0, 480.0, 260.0);
        let reused = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
                    secondary_space.clone(),
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(requested_bounds)),
                        ..Default::default()
                    },
                    app,
                )
            })
            .expect("live viewport should be reused");

        assert_eq!(reused.status(), DockViewportOpenStatus::Reused);
        assert_eq!(reused.window(), opened.window());
        assert_eq!(
            reused
                .window()
                .update(cx, |_, window, _| window.platform_facts().bounds)
                .expect("reused viewport should remain live"),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            "queued placement must not overwrite committed GPUI facts"
        );
        assert_eq!(
            runtime.viewport_route_unavailable_reason(&secondary_space),
            None,
            "queued placement must not advance Dock route facts"
        );
        let queued = runtime
            .runtime_status()
            .last_platform_dispatch
            .expect("reuse should record a queued dispatch");
        assert!(
            queued.observations.is_empty(),
            "dispatch status must remain separate from a terminal observation"
        );
        assert!(queued.dispatches.iter().any(|dispatch| matches!(
            dispatch,
            DockViewportPlatformSyncDispatch::Queued {
                request: DockViewportPlatformSyncRequest::Placement { .. },
                ..
            }
        )));

        let adjusted_bounds = floating_bounds(30.0, 40.0, 460.0, 250.0);
        let mut adjusted_facts = reused
            .window()
            .update(cx, |_, window, _| window.platform_facts().clone())
            .expect("reused viewport should remain live");
        adjusted_facts.bounds = adjusted_bounds;
        adjusted_facts.content_size = adjusted_bounds.size;
        adjusted_facts.window_bounds = WindowBounds::Windowed(adjusted_bounds);
        adjusted_facts.inner_window_bounds = WindowBounds::Windowed(adjusted_bounds);
        let facts_generation_before_observation =
            viewport_facts_generation(&runtime, &secondary_space);

        assert!(
            cx.simulate_window_mutation_observation(
                reused.window(),
                WindowMutationDomain::Placement,
                adjusted_facts,
            ),
            "test platform should emit one coherent placement terminal observation"
        );
        assert_eq!(
            reused
                .window()
                .update(cx, |_, window, _| window.platform_facts().bounds)
                .expect("reused viewport should remain live"),
            adjusted_bounds,
            "only the terminal observation may replace committed GPUI facts"
        );
        assert_eq!(
            runtime.viewport_route_unavailable_reason(&secondary_space),
            None,
            "the invalidated Dock host scene should be republished during the platform event"
        );
        assert!(
            viewport_facts_generation(&runtime, &secondary_space)
                > facts_generation_before_observation,
            "committed adjusted placement must invalidate and republish Dock route facts"
        );

        let status = runtime.runtime_status();
        let dispatch = status
            .last_platform_dispatch
            .expect("terminal observation should remain attached to its dispatch record");
        assert!(dispatch.observations.iter().any(|observation| {
            observation.outcome == DockViewportPlatformSyncObservationOutcome::Adjusted
        }));
        assert!(status.recent_platform_observations.iter().any(|record| {
            record.window_id == reused.window().window_id()
                && record.observation.domain == crate::DockViewportPlatformSyncDomain::Placement
                && record.observation.outcome
                    == DockViewportPlatformSyncObservationOutcome::Adjusted
        }));
    }

    #[open_gpui::test]
    fn reused_viewport_does_not_repeat_pending_or_terminal_window_mutations(
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

        let opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
                    secondary_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("secondary viewport should open through runtime");
        cx.run_until_parked();
        assert!(runtime.begin_viewport_host_scene(
            secondary_space.clone(),
            opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
                0.0, 0.0, 360.0, 220.0,
            ))),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            point(px(180.0), px(110.0)),
        ));

        let requested_bounds = floating_bounds(24.0, 32.0, 480.0, 260.0);
        let requested_options = || WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(requested_bounds)),
            accepts_pointer_input: false,
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        };
        let first = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
                    secondary_space.clone(),
                    requested_options(),
                    app,
                )
            })
            .expect("first reuse should dispatch changed window mutation domains");
        assert_eq!(first.status(), DockViewportOpenStatus::Reused);

        let first_dispatch = runtime
            .runtime_status()
            .last_platform_dispatch
            .expect("first reuse should record platform dispatches");
        assert!(first_dispatch.dispatches.iter().any(|dispatch| matches!(
            dispatch,
            DockViewportPlatformSyncDispatch::Queued {
                request: DockViewportPlatformSyncRequest::Placement { .. },
                ..
            }
        )));
        assert!(first_dispatch.dispatches.iter().any(|dispatch| matches!(
            dispatch,
            DockViewportPlatformSyncDispatch::Queued {
                request: DockViewportPlatformSyncRequest::PointerInput { requested: false },
                ..
            }
        )));
        assert!(first_dispatch.dispatches.iter().any(|dispatch| matches!(
            dispatch,
            DockViewportPlatformSyncDispatch::Unsupported(unsupported)
                if matches!(
                    unsupported.request,
                    DockViewportPlatformSyncRequest::BackgroundAppearance {
                        requested: WindowBackgroundAppearance::Transparent
                    }
                )
        )));

        cx.update(|app| {
            runtime
                .open_viewport_unchecked_policy(secondary_space.clone(), requested_options(), app)
                .expect("identical pending reuse should remain a valid no-op");
        });
        let pending_repeat = runtime
            .runtime_status()
            .last_platform_dispatch
            .expect("the no-op reuse may still record non-mutation diagnostics");
        assert!(
            pending_repeat.dispatches.iter().all(|dispatch| !matches!(
                dispatch,
                DockViewportPlatformSyncDispatch::Queued {
                    request: DockViewportPlatformSyncRequest::Placement { .. }
                        | DockViewportPlatformSyncRequest::PointerInput { .. }
                        | DockViewportPlatformSyncRequest::BackgroundAppearance { .. },
                    ..
                } | DockViewportPlatformSyncDispatch::Unchanged {
                    request: DockViewportPlatformSyncRequest::Placement { .. }
                        | DockViewportPlatformSyncRequest::PointerInput { .. }
                        | DockViewportPlatformSyncRequest::BackgroundAppearance { .. },
                } | DockViewportPlatformSyncDispatch::Unsupported(
                    crate::DockViewportPlatformSyncUnsupported {
                        request: DockViewportPlatformSyncRequest::Placement { .. }
                            | DockViewportPlatformSyncRequest::PointerInput { .. }
                            | DockViewportPlatformSyncRequest::BackgroundAppearance { .. },
                        ..
                    }
                ) | DockViewportPlatformSyncDispatch::Rejected(
                    crate::DockViewportPlatformSyncRejected {
                        request: DockViewportPlatformSyncRequest::Placement { .. }
                            | DockViewportPlatformSyncRequest::PointerInput { .. }
                            | DockViewportPlatformSyncRequest::BackgroundAppearance { .. },
                        ..
                    }
                )
            )),
            "pending placement and pointer requests plus terminal alpha failure must not redispatch"
        );
        assert!(
            runtime
                .runtime_status()
                .recent_platform_observations
                .is_empty(),
            "an identical pending reuse must not supersede either queued generation"
        );

        let current_facts = first
            .window()
            .update(cx, |_, window, _| window.platform_facts().clone())
            .expect("reused viewport should remain live");
        assert!(cx.simulate_window_mutation_terminal(
            first.window(),
            WindowMutationDomain::PointerInput,
            PlatformWindowMutationTerminal::Rejected,
            current_facts,
        ));
        let adjusted_bounds = floating_bounds(30.0, 40.0, 460.0, 250.0);
        let mut adjusted_facts = first
            .window()
            .update(cx, |_, window, _| window.platform_facts().clone())
            .expect("reused viewport should remain live");
        adjusted_facts.bounds = adjusted_bounds;
        adjusted_facts.content_size = adjusted_bounds.size;
        adjusted_facts.window_bounds = WindowBounds::Windowed(adjusted_bounds);
        adjusted_facts.inner_window_bounds = WindowBounds::Windowed(adjusted_bounds);
        assert!(cx.simulate_window_mutation_observation(
            first.window(),
            WindowMutationDomain::Placement,
            adjusted_facts,
        ));
        let observation_count = runtime.runtime_status().recent_platform_observations.len();
        assert_eq!(observation_count, 2);

        cx.update(|app| {
            runtime
                .open_viewport_unchecked_policy(secondary_space.clone(), requested_options(), app)
                .expect("identical terminal reuse should remain a valid no-op");
        });
        let terminal_repeat = runtime
            .runtime_status()
            .last_platform_dispatch
            .expect("the no-op reuse may still record non-mutation diagnostics");
        assert!(
            terminal_repeat.dispatches.iter().all(|dispatch| !matches!(
                dispatch,
                DockViewportPlatformSyncDispatch::Queued {
                    request: DockViewportPlatformSyncRequest::Placement { .. }
                        | DockViewportPlatformSyncRequest::PointerInput { .. }
                        | DockViewportPlatformSyncRequest::BackgroundAppearance { .. },
                    ..
                } | DockViewportPlatformSyncDispatch::Unchanged {
                    request: DockViewportPlatformSyncRequest::Placement { .. }
                        | DockViewportPlatformSyncRequest::PointerInput { .. }
                        | DockViewportPlatformSyncRequest::BackgroundAppearance { .. },
                } | DockViewportPlatformSyncDispatch::Unsupported(
                    crate::DockViewportPlatformSyncUnsupported {
                        request: DockViewportPlatformSyncRequest::Placement { .. }
                            | DockViewportPlatformSyncRequest::PointerInput { .. }
                            | DockViewportPlatformSyncRequest::BackgroundAppearance { .. },
                        ..
                    }
                ) | DockViewportPlatformSyncDispatch::Rejected(
                    crate::DockViewportPlatformSyncRejected {
                        request: DockViewportPlatformSyncRequest::Placement { .. }
                            | DockViewportPlatformSyncRequest::PointerInput { .. }
                            | DockViewportPlatformSyncRequest::BackgroundAppearance { .. },
                        ..
                    }
                )
            )),
            "unchanged adjusted/rejected/unsupported terminal mutations must not retry"
        );
        assert_eq!(
            runtime.runtime_status().recent_platform_observations.len(),
            observation_count,
            "a blocked retry must not create a new generation or observation"
        );

        let changed_bounds = floating_bounds(60.0, 70.0, 500.0, 280.0);
        cx.update(|app| {
            runtime
                .open_viewport_unchecked_policy(
                    secondary_space.clone(),
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(changed_bounds)),
                        accepts_pointer_input: true,
                        window_background: WindowBackgroundAppearance::Transparent,
                        ..Default::default()
                    },
                    app,
                )
                .expect("changed targets should clear the matching terminal retry barrier");
        });
        let changed_dispatch = runtime
            .runtime_status()
            .last_platform_dispatch
            .expect("changed targets should produce a fresh dispatch record");
        assert!(changed_dispatch.dispatches.iter().any(|dispatch| matches!(
            dispatch,
            DockViewportPlatformSyncDispatch::Queued {
                request: DockViewportPlatformSyncRequest::Placement {
                    requested: WindowBounds::Windowed(bounds)
                },
                ..
            } if *bounds == changed_bounds
        )));
        assert!(changed_dispatch.dispatches.iter().any(|dispatch| matches!(
            dispatch,
            DockViewportPlatformSyncDispatch::Unchanged {
                request: DockViewportPlatformSyncRequest::PointerInput { requested: true }
            }
        )));
    }

    #[open_gpui::test]
    fn viewport_runtime_records_external_resize_facts_without_settling_queued_placement(
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

        let opened = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
                    secondary_space.clone(),
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("secondary viewport should open through runtime");
        cx.run_until_parked();
        assert!(runtime.begin_viewport_host_scene(
            secondary_space.clone(),
            opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(floating_bounds(
                0.0, 0.0, 360.0, 220.0,
            ))),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            point(px(180.0), px(110.0)),
        ));

        let reused = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
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
            .expect("live viewport should be reused");
        assert_eq!(reused.window(), opened.window());
        let facts_generation_before_resize = viewport_facts_generation(&runtime, &secondary_space);

        cx.simulate_window_resize(reused.window(), size(px(420.0), px(240.0)));

        assert_eq!(
            reused
                .window()
                .update(cx, |_, window, _| window.platform_facts().bounds.size)
                .expect("reused viewport should remain live"),
            size(px(420.0), px(240.0)),
            "external platform facts should still refresh the GPUI cache"
        );
        assert_eq!(
            runtime.viewport_route_unavailable_reason(&secondary_space),
            None,
            "the invalidated Dock host scene should be republished during the resize event"
        );
        assert!(
            viewport_facts_generation(&runtime, &secondary_space) > facts_generation_before_resize,
            "external facts change should invalidate and republish Dock through its bounds observer"
        );
        let status = runtime.runtime_status();
        let dispatch = status
            .last_platform_dispatch
            .expect("the original queued placement diagnostic should remain available");
        assert!(dispatch.observations.is_empty());
        assert!(
            status.recent_platform_observations.is_empty(),
            "a raw external resize is not a terminal observation for the queued ticket"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_rejects_whole_placement_during_platform_resize(cx: &mut TestAppContext) {
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
                runtime.open_viewport_unchecked_policy(
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
                let _ = window.resize(size(px(520.0), px(300.0)));
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
                runtime.open_viewport_unchecked_policy(
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
        let sync = runtime
            .runtime_status()
            .last_platform_dispatch
            .expect("reuse should record platform dispatch diagnostics");
        assert!(
            sync.dispatches.iter().any(|dispatch| matches!(
                dispatch,
                DockViewportPlatformSyncDispatch::Rejected(rejected)
                    if rejected.reason
                        == DockViewportPlatformSyncRejectedReason::PlatformRequestInProgress
                    && matches!(
                        rejected.request,
                        DockViewportPlatformSyncRequest::Placement { .. }
                    )
            )),
            "an authoritative platform resize rejects the whole placement domain"
        );

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
                runtime.open_viewport_unchecked_policy(
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
        let sync = runtime
            .runtime_status()
            .last_platform_dispatch
            .expect("second reuse should record platform dispatch diagnostics");
        assert!(
            sync.dispatches.iter().any(|dispatch| matches!(
                dispatch,
                DockViewportPlatformSyncDispatch::Unchanged {
                    request: DockViewportPlatformSyncRequest::Placement { .. }
                }
            )),
            "after a fresh observed scene, the current placement is compared through typed facts"
        );
    }

    #[open_gpui::test]
    fn viewport_runtime_reuses_window_and_queues_coherent_placement(cx: &mut TestAppContext) {
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
                runtime.open_viewport_unchecked_policy(
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
                runtime.open_viewport_unchecked_policy(
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
            .last_platform_dispatch
            .expect("reuse should record platform dispatch diagnostics");
        assert!(
            sync.dispatches.iter().any(|dispatch| matches!(
                dispatch,
                DockViewportPlatformSyncDispatch::Queued {
                    request: DockViewportPlatformSyncRequest::Placement { .. },
                    ..
                }
            )),
            "origin, size, state, and restore bounds must share one placement ticket"
        );
    }

    #[test]
    fn unavailable_reused_viewport_window_sync_records_diagnostic() {
        let window = crate::viewport_test_support::handle(42);
        let sync = unavailable_reused_viewport_window_sync(window.window_id());

        assert_eq!(sync.window_id, window.window_id());
        assert_eq!(
            sync.dispatches,
            vec![DockViewportPlatformSyncDispatch::WindowClosed {
                request: DockViewportPlatformSyncRequest::WindowUnavailable,
            }]
        );
        assert!(sync.observations.is_empty());
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
                runtime.open_viewport_unchecked_policy(
                    primary_space,
                    viewport_window_options(360.0, 220.0),
                    app,
                )
            })
            .expect("primary viewport should open");
        let secondary = cx
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
            .expect("secondary viewport should open");
        primary
            .window()
            .update(cx, |_, window, _| window.activate_window())
            .expect("primary viewport should be activatable");
        cx.run_until_parked();
        assert_eq!(cx.update(|app| app.active_window()), Some(primary.window()));

        let reused = cx
            .update(|app| {
                runtime.open_viewport_unchecked_policy(
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
            .last_platform_dispatch
            .expect("reuse should record platform dispatch diagnostics");
        assert!(!sync.dispatches.iter().any(|dispatch| matches!(
            dispatch,
            DockViewportPlatformSyncDispatch::Immediate {
                action: DockViewportPlatformSyncAction::Activate
            }
        )));
        assert!(sync.dispatches.iter().any(|dispatch| matches!(
            dispatch,
            DockViewportPlatformSyncDispatch::Queued {
                request: DockViewportPlatformSyncRequest::Placement { .. },
                ..
            }
        )));
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
    fn payload_drag_preserves_route_facts_and_source_window_input(cx: &mut TestAppContext) {
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
        assert_eq!(runtime.viewport_route_unavailable_reason(&source), None);

        let session = runtime
            .borrow_mut()
            .begin_payload_drag_with_focus(&payload, None);

        assert!(
            opened
                .window()
                .update(cx, |_, window, _| window.accepts_pointer_input())
                .expect("source viewport should remain live"),
            "payload drag must not turn a normal content window into a click-through window"
        );
        assert_eq!(
            runtime.viewport_route_unavailable_reason(&source),
            None,
            "route facts should remain routable until a refreshed window fact observes native no-input"
        );
        assert!(runtime.borrow_mut().finish_payload_drag(&session).changed());
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
                runtime.open_viewport_unchecked_policy(
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
