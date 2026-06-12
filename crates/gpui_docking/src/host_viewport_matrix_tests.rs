use crate::{
    DockActionOutcome, DockController, DockGraph, DockItemId, DockNode, DockNodeId, DockSpaceId,
    DockViewportDropOutcomeKind, DockViewportDropPayload, DockViewportDropRouteOutcome,
    DockViewportPlatformSignals, DockViewportRuntimeHandle, DockViewportWindowFacts, DockWorkspace,
    DropZone, SplitAxis,
    debug::DockDebugRegion,
    drop_runtime::DockHostDropSceneFact,
    drop_target::{
        DockDropResolveSource, DockEmptySpaceDropTarget, DockLeafDropTarget,
        DockResolvedDropTargetKind, DockRootDropTarget,
    },
    geometry::{self, DockDropBoxKind, DockDropBoxSet},
    host_test_support::*,
};
use open_gpui::{
    AppContext as _, Bounds, Modifiers, MouseButton, Pixels, Point, TestAppContext,
    VisualTestContext, WindowBounds, point, px,
};
use std::time::Duration;

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
struct PollMatrixCase {
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

    fn moved_items(self) -> Vec<DockItemId> {
        self.source_items()
    }
}

#[open_gpui::test]
fn source_only_known_viewport_release_matrix_commits_payloads_to_rendered_targets(
    cx: &mut TestAppContext,
) {
    for case in matrix_cases() {
        run_source_only_release_case(cx, case);
    }
}

#[open_gpui::test]
fn source_only_known_viewport_root_edge_matrix_commits_without_leaf_hit(cx: &mut TestAppContext) {
    for case in root_only_matrix_cases() {
        run_source_only_root_only_release_case(cx, case);
    }
}

#[open_gpui::test]
fn target_hover_known_viewport_release_matrix_commits_payloads_to_rendered_targets(
    cx: &mut TestAppContext,
) {
    for case in matrix_cases() {
        run_target_hover_release_case(cx, case);
    }
}

#[open_gpui::test]
fn capture_loss_poll_release_matrix_tears_off_payloads_without_mouse_up(cx: &mut TestAppContext) {
    for case in poll_matrix_cases() {
        run_capture_loss_poll_case(cx, case);
    }
}

fn matrix_cases() -> [MatrixCase; 12] {
    [
        MatrixCase {
            name: "item to leaf center",
            payload: MatrixPayload::Item,
            target: MatrixTarget::LeafCenter,
        },
        MatrixCase {
            name: "tabs to leaf center",
            payload: MatrixPayload::Tabs,
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

fn poll_matrix_cases() -> [PollMatrixCase; 2] {
    [
        PollMatrixCase {
            name: "item outside release",
            payload: MatrixPayload::Item,
        },
        PollMatrixCase {
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

    let target_opened = cx
        .update(|app| {
            runtime.open_viewport(
                target_space.clone(),
                viewport_window_options(420.0, 240.0),
                app,
            )
        })
        .unwrap_or_else(|error| panic!("{}: target viewport should open: {error}", case.name));
    let source_opened = cx
        .update(|app| {
            runtime.open_viewport(
                source_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .unwrap_or_else(|error| panic!("{}: source viewport should open: {error}", case.name));

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
        .update(cx, |_, _, app| DockViewportPlatformSignals::from_app(app))
        .unwrap_or_else(|_| panic!("{}: source window should still be live", case.name));

    let result = cx.update(|app| {
        runtime.commit_payload_drop_from_screen_with_platform_signals(
            source_space.clone(),
            nodes.source_tabs,
            case.payload.drop_payload(),
            release_screen_position,
            None,
            source_release_signals,
            app,
        )
    });

    let DockViewportDropRouteOutcome::Action(action) = result.unwrap_or_else(|error| {
        panic!("{}: source-only release should commit: {error}", case.name)
    }) else {
        panic!(
            "{}: source-only release should produce an action",
            case.name
        );
    };
    assert_eq!(action.action(), DockActionOutcome::Changed, "{}", case.name);
    let status = runtime.runtime_status();
    let target = &status
        .last_route
        .as_ref()
        .unwrap_or_else(|| panic!("{}: release should record a route", case.name))
        .target;
    assert_eq!(target.space(), Some(&target_space), "{}", case.name);
    assert_eq!(target.host_position(), Some(host_position), "{}", case.name);

    assert_case_graph(cx, &controller, &target_space, case, &nodes);
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
            runtime.open_viewport(
                target_space.clone(),
                viewport_window_options(420.0, 240.0),
                app,
            )
        })
        .unwrap_or_else(|error| panic!("{}: target viewport should open: {error}", case.name));
    let source_opened = cx
        .update(|app| {
            runtime.open_viewport(
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
        .update(cx, |_, _, app| DockViewportPlatformSignals::from_app(app))
        .unwrap_or_else(|_| panic!("{}: source window should still be live", case.name));

    let result = cx.update(|app| {
        runtime.commit_payload_drop_from_screen_with_platform_signals(
            source_space.clone(),
            nodes.source_tabs,
            case.payload.drop_payload(),
            release_screen_position,
            None,
            source_release_signals,
            app,
        )
    });

    let DockViewportDropRouteOutcome::Action(action) = result.unwrap_or_else(|error| {
        panic!(
            "{}: root-only source release should commit: {error}",
            case.name
        )
    }) else {
        panic!(
            "{}: root-only source release should produce an action",
            case.name
        );
    };
    assert_eq!(action.action(), DockActionOutcome::Changed, "{}", case.name);
    let status = runtime.runtime_status();
    let target = &status
        .last_route
        .as_ref()
        .unwrap_or_else(|| panic!("{}: release should record a route", case.name))
        .target;
    assert_eq!(target.space(), Some(&target_space), "{}", case.name);
    assert_eq!(target.host_position(), Some(host_position), "{}", case.name);

    assert_case_graph(cx, &controller, &target_space, case, &nodes);
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

    let target_opened = cx
        .update(|app| {
            runtime.open_viewport(
                target_space.clone(),
                viewport_window_options(420.0, 240.0),
                app,
            )
        })
        .unwrap_or_else(|error| panic!("{}: target viewport should open: {error}", case.name));
    let source_opened = cx
        .update(|app| {
            runtime.open_viewport(
                source_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .unwrap_or_else(|error| panic!("{}: source viewport should open: {error}", case.name));

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

    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    target_visual.simulate_mouse_move(target_position, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    let mut target_visual = VisualTestContext::from_window(target_opened.window(), cx);
    let preview = selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview)
        .unwrap_or_else(|| panic!("{}: target hover should render drop preview", case.name));
    let preview_bounds = debug_bounds(&mut target_visual, &preview);
    assert!(
        preview_bounds.size.width > px(0.0) && preview_bounds.size.height > px(0.0),
        "{}: target hover preview should have visible bounds",
        case.name
    );
    assert_target_hover_resolution(cx, &target_host, case, &nodes);
    assert_known_viewport_route(&runtime, &target_space, target_position, case.name);

    target_visual.simulate_mouse_up(target_position, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    assert_case_graph(cx, &controller, &target_space, case, &nodes);
}

fn run_capture_loss_poll_case(cx: &mut TestAppContext, case: PollMatrixCase) {
    let source_space = DockSpaceId::from(format!("poll source:{}", case.name));
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: case.payload.source_items(),
        active: 0,
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
            runtime.open_viewport(
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

    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(true));
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(outside_window, MouseButton::Left, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(20));
    cx.run_until_parked();
    assert!(
        cx.read(|app| app.has_active_drag()),
        "{}: active drag should continue while platform reports the left button pressed",
        case.name
    );
    assert_eq!(
        runtime.registered_viewport_spaces(),
        vec![source_space.clone()],
        "{}: pressed-button poll must not open a tear-off viewport early",
        case.name
    );

    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(false));
    cx.executor().advance_clock(Duration::from_millis(20));
    cx.run_until_parked();
    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, None);
    assert!(
        !cx.read(|app| app.has_active_drag()),
        "{}: fallback poll should stop the active drag after committing release",
        case.name
    );
    let status = runtime.runtime_status();
    let target = &status
        .last_route
        .as_ref()
        .unwrap_or_else(|| {
            panic!(
                "{}: polled outside release should record a route",
                case.name
            )
        })
        .target;
    assert!(
        target.release_position().is_some(),
        "{}: polled outside release should route as tear-off, got {:?}",
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
        "{}: polled outside release should complete a tear-off, got {:?}",
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
                    "{}: polled outside release should create detached space with prefix {detached_prefix}",
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
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: case.payload.source_items(),
        active: 0,
    });
    graph.set_root(source_space.clone(), source_tabs);

    let (target_tabs, target_root) = match case.target {
        MatrixTarget::LeafCenter => {
            let target_tabs = graph.insert_node(DockNode::Tabs {
                items: vec![item("b")],
                active: 0,
            });
            graph.set_root(target_space.clone(), target_tabs);
            (Some(target_tabs), Some(target_tabs))
        }
        MatrixTarget::RootEdge { zone } => {
            let left_tabs = graph.insert_node(DockNode::Tabs {
                items: vec![item("b")],
                active: 0,
            });
            let right_tabs = graph.insert_node(DockNode::Tabs {
                items: vec![item("d")],
                active: 0,
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
    assert_eq!(target.space(), Some(target_space), "{}", case_name);
    assert_eq!(target.host_position(), Some(host_position), "{}", case_name);
}

fn assert_target_hover_resolution(
    cx: &TestAppContext,
    host: &open_gpui::Entity<crate::DockHost>,
    case: MatrixCase,
    nodes: &MatrixNodes,
) {
    let MatrixTarget::RootEdge { zone } = case.target else {
        return;
    };
    let target = cx
        .read_entity(host, |host, _| {
            host.interaction().resolved_drop_target().cloned()
        })
        .unwrap_or_else(|| panic!("{}: target hover should resolve locally", case.name));
    let root = nodes.target_root.expect("root-edge case should have root");
    let leaf_tabs = nodes
        .target_tabs
        .expect("root-edge case should have target tabs");

    assert_eq!(
        target.source,
        DockDropResolveSource::RootEdge,
        "{}: expected RootEdge target, got {:?}",
        case.name,
        target
    );
    assert!(
        matches!(
            target.kind,
            DockResolvedDropTargetKind::RootEdge {
                root: matched_root,
                leaf_tabs: Some(matched_leaf_tabs),
                zone: matched_zone,
            } if matched_root == root && matched_leaf_tabs == leaf_tabs && matched_zone == zone
        ),
        "{}: unexpected root-edge target {:?}",
        case.name,
        target
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
