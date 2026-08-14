use crate::{
    DockActionApplyError, DockController, DockGraph, DockItemId, DockNode, DockNodeId, DockSpaceId,
    DockViewportDropOutcomeKind, DockViewportDropPayload, DockViewportDropRouteOutcome,
    DockViewportPlatformSignals, DockViewportRouteTarget, DockViewportRuntimeHandle,
    DockViewportWindowFacts, DockWorkspace, DropZone, SplitAxis,
    debug::DockDebugRegion,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    drop_runtime::DockHostDropSceneFact,
    drop_target::{DockEmptySpaceDropTarget, DockLeafDropTarget, DockRootDropTarget},
    geometry::{self, DockDropBoxKind, DockDropBoxSet},
    host_test_support::*,
    host_viewport_runtime_test_support::configure_native_registered_window_hit,
    interaction::DockPayloadDropReleaseOrigin,
};
use open_gpui::{
    AppContext as _, Bounds, DevicePixels, Modifiers, MouseButton, Pixels, PlatformWindowHitStack,
    Point, QuitMode, TestAppContext, VisualTestContext, WindowBounds, point, px, size,
};

#[derive(Clone, Copy)]
enum MatrixPayload {
    Item,
    Tabs,
}

#[derive(Clone, Copy)]
enum MatrixTarget {
    LeafCenter,
    RootEdge { zone: DropZone },
    EmptySpace,
}

#[derive(Clone, Copy)]
struct MatrixCase {
    name: &'static str,
    payload: MatrixPayload,
    target: MatrixTarget,
}

#[derive(Clone, Copy)]
struct CapturedDesktopMatrixCase {
    name: &'static str,
    payload: MatrixPayload,
}

struct MatrixNodes {
    source_tabs: DockNodeId,
    target_tabs: Option<DockNodeId>,
    target_root: Option<DockNodeId>,
}

impl MatrixPayload {
    fn source_items(self) -> Vec<DockItemId> {
        match self {
            Self::Item => vec![item("a")],
            Self::Tabs => vec![item("a"), item("c")],
        }
    }

    fn drop_payload(self) -> DockViewportDropPayload {
        match self {
            Self::Item => DockViewportDropPayload::Item(item("a")),
            Self::Tabs => DockViewportDropPayload::Tabs,
        }
    }

    fn drag_payload(self, source_space: DockSpaceId, source_tabs: DockNodeId) -> DockDragPayload {
        match self {
            Self::Item => {
                DockDragPayload::new_item(source_space, source_tabs, item("a"), "Panel A".into())
            }
            Self::Tabs => DockDragPayload::new_tabs(source_space, source_tabs, "Stack".into()),
        }
    }

    fn moved_items(self) -> Vec<DockItemId> {
        self.source_items()
    }
}

#[open_gpui::test]
fn source_only_known_viewport_release_matrix_fails_closed_without_captured_native_route(
    cx: &mut TestAppContext,
) {
    for case in matrix_cases() {
        run_source_only_release_case(cx, case);
    }
}

#[open_gpui::test]
fn source_only_known_viewport_root_edge_matrix_fails_closed_without_captured_native_route(
    cx: &mut TestAppContext,
) {
    for case in root_only_matrix_cases() {
        run_source_only_root_only_release_case(cx, case);
    }
}

#[open_gpui::test]
fn source_only_known_viewport_release_rejects_overlapping_geometry_without_backend_route_selection(
    cx: &mut TestAppContext,
) {
    for case in [
        MatrixCase {
            name: "overlap item to leaf center",
            payload: MatrixPayload::Item,
            target: MatrixTarget::LeafCenter,
        },
        MatrixCase {
            name: "overlap tabs to leaf center",
            payload: MatrixPayload::Tabs,
            target: MatrixTarget::LeafCenter,
        },
    ] {
        run_overlapping_source_only_release_without_backend_route_selection_case(cx, case);
    }
}

#[open_gpui::test]
fn native_captured_item_release_matrix_commits_payloads_to_rendered_targets(
    cx: &mut TestAppContext,
) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    for case in matrix_cases()
        .into_iter()
        .filter(|case| matches!(case.payload, MatrixPayload::Item))
    {
        run_target_hover_release_case(cx, case);
    }
}

#[open_gpui::test]
fn native_captured_desktop_release_matrix_tears_off_payloads_from_source_mouse_up(
    cx: &mut TestAppContext,
) {
    for case in captured_desktop_matrix_cases() {
        run_native_captured_desktop_release_case(cx, case);
    }
}

fn matrix_cases() -> [MatrixCase; 12] {
    [
        MatrixCase {
            name: "tabs to leaf center",
            payload: MatrixPayload::Tabs,
            target: MatrixTarget::LeafCenter,
        },
        MatrixCase {
            name: "item to leaf center",
            payload: MatrixPayload::Item,
            target: MatrixTarget::LeafCenter,
        },
        MatrixCase {
            name: "item to left root edge",
            payload: MatrixPayload::Item,
            target: MatrixTarget::RootEdge {
                zone: DropZone::Left,
            },
        },
        MatrixCase {
            name: "tabs to left root edge",
            payload: MatrixPayload::Tabs,
            target: MatrixTarget::RootEdge {
                zone: DropZone::Left,
            },
        },
        MatrixCase {
            name: "item to right root edge",
            payload: MatrixPayload::Item,
            target: MatrixTarget::RootEdge {
                zone: DropZone::Right,
            },
        },
        MatrixCase {
            name: "tabs to right root edge",
            payload: MatrixPayload::Tabs,
            target: MatrixTarget::RootEdge {
                zone: DropZone::Right,
            },
        },
        MatrixCase {
            name: "item to top root edge",
            payload: MatrixPayload::Item,
            target: MatrixTarget::RootEdge {
                zone: DropZone::Top,
            },
        },
        MatrixCase {
            name: "tabs to top root edge",
            payload: MatrixPayload::Tabs,
            target: MatrixTarget::RootEdge {
                zone: DropZone::Top,
            },
        },
        MatrixCase {
            name: "item to bottom root edge",
            payload: MatrixPayload::Item,
            target: MatrixTarget::RootEdge {
                zone: DropZone::Bottom,
            },
        },
        MatrixCase {
            name: "tabs to bottom root edge",
            payload: MatrixPayload::Tabs,
            target: MatrixTarget::RootEdge {
                zone: DropZone::Bottom,
            },
        },
        MatrixCase {
            name: "item to empty space",
            payload: MatrixPayload::Item,
            target: MatrixTarget::EmptySpace,
        },
        MatrixCase {
            name: "tabs to empty space",
            payload: MatrixPayload::Tabs,
            target: MatrixTarget::EmptySpace,
        },
    ]
}

fn captured_desktop_matrix_cases() -> [CapturedDesktopMatrixCase; 2] {
    [
        CapturedDesktopMatrixCase {
            name: "item outside release",
            payload: MatrixPayload::Item,
        },
        CapturedDesktopMatrixCase {
            name: "tabs outside release",
            payload: MatrixPayload::Tabs,
        },
    ]
}

fn root_only_matrix_cases() -> [MatrixCase; 8] {
    [
        MatrixCase {
            name: "item to root-only left edge",
            payload: MatrixPayload::Item,
            target: MatrixTarget::RootEdge {
                zone: DropZone::Left,
            },
        },
        MatrixCase {
            name: "tabs to root-only left edge",
            payload: MatrixPayload::Tabs,
            target: MatrixTarget::RootEdge {
                zone: DropZone::Left,
            },
        },
        MatrixCase {
            name: "item to root-only right edge",
            payload: MatrixPayload::Item,
            target: MatrixTarget::RootEdge {
                zone: DropZone::Right,
            },
        },
        MatrixCase {
            name: "tabs to root-only right edge",
            payload: MatrixPayload::Tabs,
            target: MatrixTarget::RootEdge {
                zone: DropZone::Right,
            },
        },
        MatrixCase {
            name: "item to root-only top edge",
            payload: MatrixPayload::Item,
            target: MatrixTarget::RootEdge {
                zone: DropZone::Top,
            },
        },
        MatrixCase {
            name: "tabs to root-only top edge",
            payload: MatrixPayload::Tabs,
            target: MatrixTarget::RootEdge {
                zone: DropZone::Top,
            },
        },
        MatrixCase {
            name: "item to root-only bottom edge",
            payload: MatrixPayload::Item,
            target: MatrixTarget::RootEdge {
                zone: DropZone::Bottom,
            },
        },
        MatrixCase {
            name: "tabs to root-only bottom edge",
            payload: MatrixPayload::Tabs,
            target: MatrixTarget::RootEdge {
                zone: DropZone::Bottom,
            },
        },
    ]
}

fn run_source_only_release_case(cx: &mut TestAppContext, case: MatrixCase) {
    let source_space = DockSpaceId::from(format!("source:{}", case.name));
    let target_space = DockSpaceId::from(format!("target:{}", case.name));
    let (graph, nodes) = matrix_graph(&source_space, &target_space, case);
    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    workspace.register_panel_view(item("d"), "Panel D", test_view(cx, "D"));
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
        .unwrap_or_else(|error| panic!("{}: source viewport should open: {error}", case.name));
    let target_opened = cx
        .update(|app| {
            runtime.open_viewport_unchecked_policy(
                target_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .unwrap_or_else(|error| panic!("{}: target viewport should open: {error}", case.name));

    let target_bounds = WindowBounds::Windowed(floating_bounds(120.0, 80.0, 420.0, 240.0));
    let source_bounds = WindowBounds::Windowed(floating_bounds(640.0, 80.0, 360.0, 220.0));
    let target_host_bounds = floating_bounds(0.0, 0.0, 420.0, 240.0);
    assert!(
        runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(target_bounds),
            target_host_bounds,
            point(px(0.0), px(0.0)),
        ),
        "{}: target scene snapshot should be registered",
        case.name
    );
    assert!(
        runtime.begin_viewport_host_scene(
            source_space.clone(),
            source_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(source_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            point(px(0.0), px(0.0)),
        ),
        "{}: source scene snapshot should be registered",
        case.name
    );

    let host_position = push_target_scene_facts(
        &runtime,
        &target_space,
        target_opened.window().window_id(),
        case,
        &nodes,
    );
    let release_screen_position = point(
        target_bounds.get_bounds().origin.x + host_position.x,
        target_bounds.get_bounds().origin.y + host_position.y,
    );
    let source_release_signals = source_opened
        .window()
        .update(cx, |_, _, app| {
            DockViewportPlatformSignals::from_app_without_hovered_window_signal(app)
        })
        .unwrap_or_else(|_| panic!("{}: source window should still be live", case.name));
    let drag_payload = case
        .payload
        .drag_payload(source_space.clone(), nodes.source_tabs);
    let drag_session = runtime.begin_payload_drag(&drag_payload);
    let tear_off_geometry = DockDragTearOffGeometry::from_source_bounds(
        source_bounds.get_bounds(),
        source_bounds.get_bounds().center(),
    );

    let result = cx.update(|app| {
        let request = crate::DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source_space.clone(),
            nodes.source_tabs,
            case.payload.drop_payload(),
            release_screen_position,
            None,
            source_release_signals,
            DockPayloadDropReleaseOrigin::SourceOnly,
        )
        .with_tear_off_geometry(Some(tear_off_geometry))
        .with_drag_session(Some(drag_session.clone()));
        runtime.commit_payload_drop_from_screen(&request, app)
    });

    assert_source_only_release_without_captured_native_route_rejected(
        cx,
        &controller,
        &runtime,
        &source_space,
        &target_space,
        result,
        case,
    );
}

fn run_source_only_root_only_release_case(cx: &mut TestAppContext, case: MatrixCase) {
    let source_space = DockSpaceId::from(format!("root-only source:{}", case.name));
    let target_space = DockSpaceId::from(format!("root-only target:{}", case.name));
    let (graph, nodes) = matrix_graph(&source_space, &target_space, case);
    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    workspace.register_panel_view(item("d"), "Panel D", test_view(cx, "D"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let target_opened = cx
        .update(|app| {
            runtime.open_viewport_unchecked_policy(
                target_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .unwrap_or_else(|error| panic!("{}: target viewport should open: {error}", case.name));
    let source_opened = cx
        .update(|app| {
            runtime.open_viewport_unchecked_policy(
                source_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .unwrap_or_else(|error| panic!("{}: source viewport should open: {error}", case.name));

    let target_bounds = WindowBounds::Windowed(floating_bounds(120.0, 80.0, 420.0, 240.0));
    let source_bounds = WindowBounds::Windowed(floating_bounds(640.0, 80.0, 360.0, 220.0));
    assert!(
        runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(target_bounds),
            floating_bounds(0.0, 0.0, 420.0, 240.0),
            point(px(0.0), px(0.0)),
        ),
        "{}: target scene snapshot should be registered",
        case.name
    );
    assert!(
        runtime.begin_viewport_host_scene(
            source_space.clone(),
            source_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(source_bounds),
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            point(px(0.0), px(0.0)),
        ),
        "{}: source scene snapshot should be registered",
        case.name
    );
    let host_position = push_root_only_scene_fact(
        &runtime,
        &target_space,
        target_opened.window().window_id(),
        case,
        &nodes,
    );
    let release_screen_position = point(
        target_bounds.get_bounds().origin.x + host_position.x,
        target_bounds.get_bounds().origin.y + host_position.y,
    );
    let source_release_signals = source_opened
        .window()
        .update(cx, |_, _, app| {
            DockViewportPlatformSignals::from_app_without_hovered_window_signal(app)
        })
        .unwrap_or_else(|_| panic!("{}: source window should still be live", case.name));
    let drag_payload = case
        .payload
        .drag_payload(source_space.clone(), nodes.source_tabs);
    let drag_session = runtime.begin_payload_drag(&drag_payload);
    let tear_off_geometry = DockDragTearOffGeometry::from_source_bounds(
        source_bounds.get_bounds(),
        source_bounds.get_bounds().center(),
    );

    let result = cx.update(|app| {
        let request = crate::DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source_space.clone(),
            nodes.source_tabs,
            case.payload.drop_payload(),
            release_screen_position,
            None,
            source_release_signals,
            DockPayloadDropReleaseOrigin::SourceOnly,
        )
        .with_tear_off_geometry(Some(tear_off_geometry))
        .with_drag_session(Some(drag_session.clone()));
        runtime.commit_payload_drop_from_screen(&request, app)
    });

    assert_source_only_release_without_captured_native_route_rejected(
        cx,
        &controller,
        &runtime,
        &source_space,
        &target_space,
        result,
        case,
    );
}

fn run_overlapping_source_only_release_without_backend_route_selection_case(
    cx: &mut TestAppContext,
    case: MatrixCase,
) {
    let source_space = DockSpaceId::from(format!("overlap source:{}", case.name));
    let target_space = DockSpaceId::from(format!("overlap target:{}", case.name));
    let (graph, nodes) = matrix_graph(&source_space, &target_space, case);
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
        .unwrap_or_else(|error| panic!("{}: target viewport should open: {error}", case.name));
    let source_opened = cx
        .update(|app| {
            runtime.open_viewport_unchecked_policy(
                source_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .unwrap_or_else(|error| panic!("{}: source viewport should open: {error}", case.name));

    let overlapping_bounds = WindowBounds::Windowed(floating_bounds(120.0, 80.0, 420.0, 240.0));
    let target_host_bounds = floating_bounds(0.0, 0.0, 420.0, 240.0);
    assert!(
        runtime.begin_viewport_host_scene(
            target_space.clone(),
            target_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(overlapping_bounds),
            target_host_bounds,
            point(px(0.0), px(0.0)),
        ),
        "{}: target scene snapshot should be registered",
        case.name
    );
    assert!(
        runtime.begin_viewport_host_scene(
            source_space.clone(),
            source_opened.window().window_id(),
            DockViewportWindowFacts::from_window_bounds(overlapping_bounds),
            floating_bounds(0.0, 0.0, 420.0, 240.0),
            point(px(0.0), px(0.0)),
        ),
        "{}: source scene snapshot should be registered",
        case.name
    );

    let host_position = push_target_scene_facts(
        &runtime,
        &target_space,
        target_opened.window().window_id(),
        case,
        &nodes,
    );
    let release_screen_position = point(
        overlapping_bounds.get_bounds().origin.x + host_position.x,
        overlapping_bounds.get_bounds().origin.y + host_position.y,
    );
    let source_release_signals = source_opened
        .window()
        .update(cx, |_, _, app| {
            DockViewportPlatformSignals::from_app_without_hovered_window_signal(app)
        })
        .unwrap_or_else(|_| panic!("{}: source window should still be live", case.name));

    let result = cx.update(|app| {
        runtime.commit_payload_drop_from_screen_with_platform_signals(
            source_space.clone(),
            nodes.source_tabs,
            case.payload.drop_payload(),
            release_screen_position,
            None,
            source_release_signals,
            DockPayloadDropReleaseOrigin::SourceOnly,
            app,
        )
    });

    assert_eq!(
        result,
        Err(DockActionApplyError::DropTargetUnavailable),
        "{}: overlapping source-only release should fail without current backend route selection",
        case.name
    );
    let status = runtime.runtime_status();
    assert!(
        matches!(
            &status
                .last_route
                .as_ref()
                .unwrap_or_else(|| panic!("{}: release should record a route", case.name))
                .target,
            DockViewportRouteTarget::Unavailable
        ),
        "{}: overlapping source-only release must not retarget from geometry alone, got {:?}",
        case.name,
        status.last_route
    );
    assert_case_graph_unmoved(cx, &controller, &source_space, &target_space, case);
}

fn assert_source_only_release_without_captured_native_route_rejected(
    cx: &TestAppContext,
    controller: &open_gpui::Entity<DockController>,
    runtime: &DockViewportRuntimeHandle,
    source_space: &DockSpaceId,
    target_space: &DockSpaceId,
    result: Result<DockViewportDropRouteOutcome, DockActionApplyError>,
    case: MatrixCase,
) {
    assert_eq!(
        result,
        Err(DockActionApplyError::DropTargetUnavailable),
        "{}: source-only release must fail closed without a captured native route",
        case.name
    );
    let status = runtime.runtime_status();
    assert!(
        matches!(
            status
                .last_route
                .as_ref()
                .unwrap_or_else(|| panic!("{}: release should record a route", case.name))
                .target,
            DockViewportRouteTarget::Unavailable
        ),
        "{}: source-only release without a captured native route should be recorded as unavailable, got {:?}",
        case.name,
        status.last_route
    );
    assert_eq!(
        status
            .last_activation
            .as_ref()
            .map(|activation| activation.space.clone()),
        None,
        "{}: rejected source-only fallback must not activate the target viewport",
        case.name
    );
    let registered_spaces = runtime.registered_viewport_spaces();
    assert_eq!(
        registered_spaces.len(),
        2,
        "{}: untrusted tear-off geometry must not create a detached viewport",
        case.name
    );
    assert!(registered_spaces.contains(source_space));
    assert!(registered_spaces.contains(target_space));
    assert_case_graph_unmoved(cx, controller, source_space, target_space, case);
}

fn assert_case_graph_unmoved(
    cx: &TestAppContext,
    controller: &open_gpui::Entity<DockController>,
    source_space: &DockSpaceId,
    target_space: &DockSpaceId,
    case: MatrixCase,
) {
    cx.read_entity(controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(source_space),
            case.payload.source_items(),
            "{}: source payload should remain in the source space",
            case.name
        );
        let expected_target_items = match case.target {
            MatrixTarget::LeafCenter => vec![item("b")],
            MatrixTarget::RootEdge { .. } => vec![item("b"), item("d")],
            MatrixTarget::EmptySpace => Vec::new(),
        };
        assert_eq!(
            controller.graph().collect_items_in_space(target_space),
            expected_target_items,
            "{}: target space should not receive an untrusted source-only drop",
            case.name
        );
    });
}

fn run_target_hover_release_case(cx: &mut TestAppContext, case: MatrixCase) {
    let source_space = DockSpaceId::from(format!("hover source:{}", case.name));
    let target_space = DockSpaceId::from(format!("hover target:{}", case.name));
    let (graph, nodes) = matrix_graph(&source_space, &target_space, case);
    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    workspace.register_panel_view(item("d"), "Panel D", test_view(cx, "D"));
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
        .unwrap_or_else(|error| panic!("{}: source viewport should open: {error}", case.name));
    let target_opened = cx
        .update(|app| {
            runtime.open_viewport_unchecked_policy(
                target_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .unwrap_or_else(|error| panic!("{}: target viewport should open: {error}", case.name));

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
    let start = source_drag_start(&mut source_visual, &source_host, case, &nodes);
    let threshold = point(start.x + px(24.0), start.y);
    let target_position = target_hover_position(&mut target_visual, &target_host, case, &nodes);
    let target_from_source = point(px(400.0) + target_position.x, target_position.y);
    configure_native_registered_window_hit(
        cx,
        source_opened.window(),
        target_opened.window(),
        target_from_source,
    );
    assert!(
        cx.update(|app| app.viewport_capabilities().window_hit_stack),
        "{}: the test platform must advertise exact native window hit observations",
        case.name
    );

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    assert!(
        cx.update(|app| {
            crate::native_captured_drag::has_active_native_captured_drag_route_for_test(app)
        }),
        "{}: stack and tab drags must install a source native-capture route",
        case.name
    );
    source_visual.simulate_mouse_move(target_from_source, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    let mut target_visual = VisualTestContext::from_window(target_opened.window(), cx);
    let preview = selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview)
        .unwrap_or_else(|| {
            panic!(
                "{}: target hover should render drop preview; runtime status: {:?}",
                case.name,
                runtime.runtime_status()
            )
        });
    let preview_bounds = debug_bounds(&mut target_visual, &preview);
    assert!(
        preview_bounds.size.width > px(0.0) && preview_bounds.size.height > px(0.0),
        "{}: target hover preview should have visible bounds",
        case.name
    );
    assert_target_hover_routed_preview(
        &runtime,
        &target_space,
        target_opened.window().window_id(),
        case,
    );
    assert_known_viewport_route(&runtime, &target_space, target_position, case.name);

    source_visual.simulate_mouse_up(target_from_source, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    assert_case_graph(cx, &controller, &target_space, case, &nodes);
    let _ = source_opened
        .window()
        .update(cx, |_, window, app| window.remove_window(app));
    let _ = target_opened
        .window()
        .update(cx, |_, window, app| window.remove_window(app));
    cx.run_until_parked();
}

fn run_native_captured_desktop_release_case(
    cx: &mut TestAppContext,
    case: CapturedDesktopMatrixCase,
) {
    let source_space = DockSpaceId::from(format!("captured source:{}", case.name));
    let mut graph = DockGraph::new();
    let source_items = case.payload.source_items();
    let selected = source_items.last().cloned();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: source_items,
        selected,
    });
    graph.set_root(source_space.clone(), source_tabs);
    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let opened = cx
        .update(|app| {
            runtime.open_viewport_unchecked_policy(
                source_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .unwrap_or_else(|error| panic!("{}: source viewport should open: {error}", case.name));
    let source_window = opened
        .window()
        .downcast::<crate::DockHost>()
        .expect("source viewport should render DockHost");
    let source_host = source_window
        .root(cx)
        .expect("source viewport should expose DockHost root");
    cx.run_until_parked();

    let mut visual = VisualTestContext::from_window(opened.window(), cx);
    let source_case = MatrixCase {
        name: case.name,
        payload: case.payload,
        target: MatrixTarget::EmptySpace,
    };
    let nodes = MatrixNodes {
        source_tabs,
        target_tabs: None,
        target_root: None,
    };
    let start = source_drag_start(&mut visual, &source_host, source_case, &nodes);
    let threshold = point(start.x + px(24.0), start.y);
    let outside_window = point(px(900.0), px(900.0));
    let source_bounds = Bounds::new(
        point(DevicePixels(0), DevicePixels(0)),
        size(DevicePixels(720), DevicePixels(440)),
    );
    cx.set_platform_window_physical_client_geometry(opened.window(), Some(source_bounds), 2.0);
    let sampled_point = point(DevicePixels(1800), DevicePixels(1800));
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available_open_desktop(sampled_point, Vec::new())
            .expect("desktop hit observation should be valid"),
    );

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(outside_window, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(
        cx.read(|app| app.has_active_drag()),
        "{}: active drag should continue until the source capture reports MouseUp",
        case.name
    );
    assert_eq!(
        runtime.registered_viewport_spaces(),
        vec![source_space.clone()],
        "{}: captured movement must not open a tear-off viewport before release",
        case.name
    );

    visual.simulate_mouse_up(outside_window, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(
        !cx.read(|app| app.has_active_drag()),
        "{}: source-owned MouseUp should stop the active drag after committing release",
        case.name
    );
    let status = runtime.runtime_status();
    let target = &status
        .last_route
        .as_ref()
        .unwrap_or_else(|| {
            panic!(
                "{}: captured desktop release should record a route",
                case.name
            )
        })
        .target;
    assert!(
        target.release_position().is_some(),
        "{}: captured desktop release should route as tear-off, got {:?}",
        case.name,
        status.last_route
    );
    assert!(
        matches!(
            runtime
                .runtime_status()
                .last_drop_outcome
                .as_ref()
                .map(|record| record.kind),
            Some(DockViewportDropOutcomeKind::TearOffCompleted)
        ),
        "{}: captured desktop release should complete a tear-off, got {:?}",
        case.name,
        runtime.runtime_status().last_drop_outcome
    );

    let detached_space = cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            Vec::<DockItemId>::new(),
            "{}: source space should be emptied by the tear-off",
            case.name
        );
        let detached_prefix = format!(
            "{}:tear-off:{}:",
            source_space,
            case.payload.drop_payload().label()
        );
        let detached_space = controller
            .graph()
            .spaces()
            .into_iter()
            .find(|space| space.as_str().starts_with(&detached_prefix))
            .unwrap_or_else(|| {
                panic!(
                    "{}: captured desktop release should create detached space with prefix {detached_prefix}",
                    case.name
                )
            });
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            case.payload.moved_items(),
            "{}: detached space should receive the full dragged payload",
            case.name
        );
        detached_space
    });
    assert!(
        runtime
            .borrow()
            .adapter()
            .window_for_space(&detached_space)
            .is_some(),
        "{}: detached space should be registered with a runtime window",
        case.name
    );
}

fn matrix_graph(
    source_space: &DockSpaceId,
    target_space: &DockSpaceId,
    case: MatrixCase,
) -> (DockGraph, MatrixNodes) {
    let mut graph = DockGraph::new();
    let source_items = case.payload.source_items();
    let selected = source_items.first().cloned();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: source_items,
        selected,
    });
    graph.set_root(source_space.clone(), source_tabs);

    let (target_tabs, target_root) = match case.target {
        MatrixTarget::LeafCenter => {
            let target_tabs = graph.insert_node(DockNode::Tabs {
                items: vec![item("b")],
                selected: Some(item("b")),
            });
            graph.set_root(target_space.clone(), target_tabs);
            (Some(target_tabs), Some(target_tabs))
        }
        MatrixTarget::RootEdge { zone } => {
            let left_tabs = graph.insert_node(DockNode::Tabs {
                items: vec![item("b")],
                selected: Some(item("b")),
            });
            let right_tabs = graph.insert_node(DockNode::Tabs {
                items: vec![item("d")],
                selected: Some(item("d")),
            });
            let root = graph.insert_node(DockNode::Split {
                axis: SplitAxis::Horizontal,
                children: vec![left_tabs, right_tabs],
                fractions: vec![0.5, 0.5],
            });
            graph.set_root(target_space.clone(), root);
            let target_tabs = match zone {
                DropZone::Left => left_tabs,
                DropZone::Right | DropZone::Top | DropZone::Bottom => right_tabs,
                DropZone::Center => unreachable!(),
            };
            (Some(target_tabs), Some(root))
        }
        MatrixTarget::EmptySpace => (None, None),
    };

    (
        graph,
        MatrixNodes {
            source_tabs,
            target_tabs,
            target_root,
        },
    )
}

fn push_target_scene_facts(
    runtime: &DockViewportRuntimeHandle,
    target_space: &DockSpaceId,
    window_id: open_gpui::WindowId,
    case: MatrixCase,
    nodes: &MatrixNodes,
) -> open_gpui::Point<open_gpui::Pixels> {
    match case.target {
        MatrixTarget::LeafCenter => {
            let target_tabs = nodes
                .target_tabs
                .expect("leaf case should have target tabs");
            assert!(
                runtime.push_viewport_host_scene_fact(
                    target_space,
                    window_id,
                    DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                        root: target_tabs,
                        target_tabs,
                        bounds: floating_bounds(0.0, 0.0, 420.0, 240.0),
                        is_central: false,
                    }),
                ),
                "{}: leaf fact should publish",
                case.name
            );
            point(px(210.0), px(120.0))
        }
        MatrixTarget::RootEdge { zone } => {
            let root = nodes.target_root.expect("root-edge case should have root");
            let target_tabs = nodes
                .target_tabs
                .expect("root-edge case should have target tabs");
            assert!(
                runtime.push_viewport_host_scene_fact(
                    target_space,
                    window_id,
                    DockHostDropSceneFact::Root(DockRootDropTarget {
                        root,
                        bounds: floating_bounds(0.0, 0.0, 420.0, 240.0),
                    }),
                ),
                "{}: root fact should publish",
                case.name
            );
            assert!(
                runtime.push_viewport_host_scene_fact(
                    target_space,
                    window_id,
                    DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                        root,
                        target_tabs,
                        bounds: root_edge_leaf_bounds(zone),
                        is_central: false,
                    }),
                ),
                "{}: root-edge leaf fact should publish",
                case.name
            );
            root_edge_host_position(zone)
        }
        MatrixTarget::EmptySpace => {
            assert!(
                runtime.push_viewport_host_scene_fact(
                    target_space,
                    window_id,
                    DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget {
                        space: target_space.clone(),
                        bounds: floating_bounds(0.0, 0.0, 420.0, 240.0),
                        is_central: false,
                    }),
                ),
                "{}: empty-space fact should publish",
                case.name
            );
            point(px(210.0), px(120.0))
        }
    }
}

fn push_root_only_scene_fact(
    runtime: &DockViewportRuntimeHandle,
    target_space: &DockSpaceId,
    window_id: open_gpui::WindowId,
    case: MatrixCase,
    nodes: &MatrixNodes,
) -> Point<Pixels> {
    let MatrixTarget::RootEdge { zone } = case.target else {
        panic!("{}: root-only scene requires a root-edge case", case.name);
    };
    let root = nodes.target_root.expect("root-only case should have root");
    assert!(
        runtime.push_viewport_host_scene_fact(
            target_space,
            window_id,
            DockHostDropSceneFact::Root(DockRootDropTarget {
                root,
                bounds: floating_bounds(0.0, 0.0, 420.0, 240.0),
            }),
        ),
        "{}: root-only fact should publish",
        case.name
    );
    root_edge_host_position(zone)
}

fn source_drag_start(
    visual: &mut VisualTestContext,
    host: &open_gpui::Entity<crate::DockHost>,
    case: MatrixCase,
    nodes: &MatrixNodes,
) -> Point<Pixels> {
    match case.payload {
        MatrixPayload::Item => {
            let source_tab = selector_for(
                visual,
                host,
                DockDebugRegion::Tab {
                    tabs: nodes.source_tabs,
                    item: item("a"),
                },
            )
            .unwrap_or_else(|| panic!("{}: source tab selector should be emitted", case.name));
            debug_bounds(visual, &source_tab).center()
        }
        MatrixPayload::Tabs => {
            let source_tabs = selector_for(
                visual,
                host,
                DockDebugRegion::Tabs {
                    node: nodes.source_tabs,
                },
            )
            .unwrap_or_else(|| panic!("{}: source tabs selector should be emitted", case.name));
            let source_bounds = debug_bounds(visual, &source_tabs);
            point(
                source_bounds.origin.x + source_bounds.size.width - px(8.0),
                source_bounds.origin.y + px(12.0),
            )
        }
    }
}

fn target_hover_position(
    visual: &mut VisualTestContext,
    host: &open_gpui::Entity<crate::DockHost>,
    case: MatrixCase,
    nodes: &MatrixNodes,
) -> Point<Pixels> {
    match case.target {
        MatrixTarget::LeafCenter => {
            let target_tabs = nodes
                .target_tabs
                .expect("leaf case should have target tabs");
            let selector = selector_for(visual, host, DockDebugRegion::Tabs { node: target_tabs })
                .unwrap_or_else(|| panic!("{}: target tabs selector should be emitted", case.name));
            debug_bounds(visual, &selector).center()
        }
        MatrixTarget::RootEdge { zone } => {
            let selector = selector_for(visual, host, DockDebugRegion::Host)
                .unwrap_or_else(|| panic!("{}: target host selector should be emitted", case.name));
            let bounds = debug_bounds(visual, &selector);
            root_edge_position_in_bounds(zone, bounds)
        }
        MatrixTarget::EmptySpace => {
            let selector =
                selector_for(visual, host, DockDebugRegion::EmptySpace).unwrap_or_else(|| {
                    panic!("{}: empty target selector should be emitted", case.name)
                });
            debug_bounds(visual, &selector).center()
        }
    }
}

fn assert_known_viewport_route(
    runtime: &DockViewportRuntimeHandle,
    target_space: &DockSpaceId,
    host_position: Point<Pixels>,
    case_name: &str,
) {
    let status = runtime.runtime_status();
    let target = &status
        .last_route
        .as_ref()
        .unwrap_or_else(|| panic!("{}: hover/release should record a route", case_name))
        .target;
    assert_eq!(
        target.space(),
        Some(target_space),
        "{}: target={:?}",
        case_name,
        target
    );
    let routed_host_position = target
        .host_position()
        .unwrap_or_else(|| panic!("{}: target={:?}", case_name, target));
    assert_point_close(routed_host_position, host_position);
}

fn assert_target_hover_routed_preview(
    runtime: &DockViewportRuntimeHandle,
    target_space: &DockSpaceId,
    target_window_id: open_gpui::WindowId,
    case: MatrixCase,
) {
    let MatrixTarget::RootEdge { .. } = case.target else {
        return;
    };
    assert!(
        runtime
            .routed_drop_preview_for(target_space, target_window_id)
            .is_some(),
        "{}: target hover should publish a routed preview for the target viewport",
        case.name
    );
}

fn assert_case_graph(
    cx: &TestAppContext,
    controller: &open_gpui::Entity<DockController>,
    target_space: &DockSpaceId,
    case: MatrixCase,
    nodes: &MatrixNodes,
) {
    cx.read_entity(controller, |controller, _| match case.target {
        MatrixTarget::LeafCenter => {
            let target_tabs = nodes
                .target_tabs
                .expect("leaf case should have target tabs");
            let DockNode::Tabs { items, .. } = controller
                .graph()
                .node(target_tabs)
                .expect("leaf target should still exist")
            else {
                panic!("{}: leaf target should remain tabs", case.name);
            };
            let mut expected = vec![item("b")];
            expected.extend(case.payload.moved_items());
            assert_eq!(items, &expected, "{}", case.name);
        }
        MatrixTarget::RootEdge { zone } => {
            let root = nodes.target_root.expect("root-edge case should have root");
            assert_root_edge_graph(
                controller.graph(),
                target_space,
                root,
                zone,
                &case.payload.moved_items(),
                case.name,
            );
        }
        MatrixTarget::EmptySpace => {
            let target_root = controller
                .graph()
                .root(target_space)
                .expect("empty target should receive a root");
            let DockNode::Tabs { items, .. } = controller
                .graph()
                .node(target_root)
                .expect("empty target root should exist")
            else {
                panic!("{}: empty target root should be tabs", case.name);
            };
            assert_eq!(items, &case.payload.moved_items(), "{}", case.name);
        }
    });
}

fn root_edge_leaf_bounds(zone: DropZone) -> Bounds<Pixels> {
    let position = root_edge_host_position(zone);
    Bounds::new(
        point(position.x - px(60.0), position.y - px(60.0)),
        open_gpui::size(px(120.0), px(120.0)),
    )
}

fn root_edge_host_position(zone: DropZone) -> Point<Pixels> {
    outer_drop_box_center(zone, floating_bounds(0.0, 0.0, 420.0, 240.0))
}

fn root_edge_position_in_bounds(zone: DropZone, bounds: Bounds<Pixels>) -> Point<Pixels> {
    outer_drop_box_center(zone, bounds)
}

fn outer_drop_box_center(zone: DropZone, bounds: Bounds<Pixels>) -> Point<Pixels> {
    geometry::drop_boxes(bounds, DockDropBoxSet::Outer)
        .into_iter()
        .find(|drop_box| drop_box.kind == DockDropBoxKind::OuterEdge(zone))
        .map(|drop_box| drop_box.hit_bounds.center())
        .unwrap_or_else(|| panic!("{zone:?} outer box should exist"))
}

fn assert_root_edge_graph(
    graph: &DockGraph,
    target_space: &DockSpaceId,
    old_root: DockNodeId,
    zone: DropZone,
    expected_items: &[DockItemId],
    case_name: &str,
) {
    match zone {
        DropZone::Left | DropZone::Right => {
            assert_eq!(graph.root(target_space), Some(old_root), "{}", case_name);
            let DockNode::Split { axis, children, .. } = graph
                .node(old_root)
                .expect("root-edge target root should exist")
            else {
                panic!("{}: root-edge target should remain a split", case_name);
            };
            assert_eq!(*axis, SplitAxis::Horizontal, "{}", case_name);
            assert_eq!(children.len(), 3, "{}", case_name);
            let moved_index = if zone == DropZone::Left { 0 } else { 2 };
            assert_tabs_child_items(graph, children[moved_index], expected_items, case_name);
        }
        DropZone::Top | DropZone::Bottom => {
            let new_root = graph
                .root(target_space)
                .expect("root-edge target space should keep a root");
            assert_ne!(new_root, old_root, "{}", case_name);
            let DockNode::Split { axis, children, .. } =
                graph.node(new_root).expect("new root split should exist")
            else {
                panic!("{}: root-edge target space should be wrapped", case_name);
            };
            assert_eq!(*axis, SplitAxis::Vertical, "{}", case_name);
            assert_eq!(children.len(), 2, "{}", case_name);
            let (moved_index, old_root_index) = if zone == DropZone::Top {
                (0, 1)
            } else {
                (1, 0)
            };
            assert_tabs_child_items(graph, children[moved_index], expected_items, case_name);
            assert_eq!(children[old_root_index], old_root, "{}", case_name);
        }
        DropZone::Center => unreachable!(),
    }
}

fn assert_tabs_child_items(
    graph: &DockGraph,
    tabs: DockNodeId,
    expected_items: &[DockItemId],
    case_name: &str,
) {
    let DockNode::Tabs { items, .. } = graph.node(tabs).expect("moved child should exist") else {
        panic!("{}: moved root-edge child should be tabs", case_name);
    };
    assert_eq!(items, expected_items, "{}", case_name);
}
