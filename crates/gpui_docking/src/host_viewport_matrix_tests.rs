use crate::{
    DockActionOutcome, DockController, DockGraph, DockItemId, DockNode, DockNodeId, DockSpaceId,
    DockViewportDropOutcomeKind, DockViewportDropPayload, DockViewportDropRouteOutcome,
    DockViewportPlatformSignals, DockViewportRouteTarget, DockViewportRuntimeHandle, DockWorkspace,
    SplitAxis,
    debug::DockDebugRegion,
    drop_runtime::DockHostDropSceneFact,
    drop_target::{DockEmptySpaceDropTarget, DockLeafDropTarget, DockRootDropTarget},
    host_test_support::*,
};
use open_gpui::{
    AppContext as _, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext,
    WindowBounds, point, px,
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
    RootEdge,
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

fn matrix_cases() -> [MatrixCase; 6] {
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
            name: "item to root edge",
            payload: MatrixPayload::Item,
            target: MatrixTarget::RootEdge,
        },
        MatrixCase {
            name: "tabs to root edge",
            payload: MatrixPayload::Tabs,
            target: MatrixTarget::RootEdge,
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
            target_opened.window.window_id(),
            target_bounds,
            target_host_bounds,
            point(px(0.0), px(0.0)),
        ),
        "{}: target scene snapshot should be registered",
        case.name
    );
    assert!(
        runtime.begin_viewport_host_scene(
            source_space.clone(),
            source_opened.window.window_id(),
            source_bounds,
            floating_bounds(0.0, 0.0, 360.0, 220.0),
            point(px(0.0), px(0.0)),
        ),
        "{}: source scene snapshot should be registered",
        case.name
    );

    let host_position = push_target_scene_facts(
        &runtime,
        &target_space,
        target_opened.window.window_id(),
        case,
        &nodes,
    );
    let release_screen_position = point(
        target_bounds.get_bounds().origin.x + host_position.x,
        target_bounds.get_bounds().origin.y + host_position.y,
    );
    let source_release_context = source_opened
        .window
        .update(cx, |_, window, app| {
            DockViewportPlatformSignals::from_window(window, app).target_context()
        })
        .unwrap_or_else(|_| panic!("{}: source window should still be live", case.name));

    let result = cx.update(|app| {
        runtime.commit_payload_drop_from_screen_with_context(
            source_space.clone(),
            nodes.source_tabs,
            case.payload.drop_payload(),
            release_screen_position,
            None,
            source_release_context,
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
    assert_eq!(action.action, DockActionOutcome::Changed, "{}", case.name);
    assert!(
        matches!(
            runtime
                .runtime_status()
                .last_route
                .as_ref()
                .map(|record| &record.target),
            Some(DockViewportRouteTarget::KnownViewport { space, host_position: routed_position, .. })
                if space == &target_space && *routed_position == host_position
        ),
        "{}: release should route to target viewport, got {:?}",
        case.name,
        runtime.runtime_status().last_route
    );

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
        .window
        .downcast::<crate::DockHost>()
        .expect("source viewport should render DockHost");
    let target_window = target_opened
        .window
        .downcast::<crate::DockHost>()
        .expect("target viewport should render DockHost");
    let source_host = source_window
        .root(cx)
        .expect("source viewport should expose DockHost root");
    let target_host = target_window
        .root(cx)
        .expect("target viewport should expose DockHost root");
    cx.run_until_parked();

    let mut source_visual = VisualTestContext::from_window(source_opened.window, cx);
    let mut target_visual = VisualTestContext::from_window(target_opened.window, cx);
    let start = source_drag_start(&mut source_visual, &source_host, case, &nodes);
    let threshold = point(start.x + px(24.0), start.y);
    let target_position = target_hover_position(&mut target_visual, &target_host, case, &nodes);

    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    target_visual.simulate_mouse_move(target_position, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    let mut target_visual = VisualTestContext::from_window(target_opened.window, cx);
    let preview = selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview)
        .unwrap_or_else(|| panic!("{}: target hover should render drop preview", case.name));
    let preview_bounds = debug_bounds(&mut target_visual, &preview);
    assert!(
        preview_bounds.size.width > px(0.0) && preview_bounds.size.height > px(0.0),
        "{}: target hover preview should have visible bounds",
        case.name
    );
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
        .window
        .downcast::<crate::DockHost>()
        .expect("source viewport should render DockHost");
    let source_host = source_window
        .root(cx)
        .expect("source viewport should expose DockHost root");
    cx.run_until_parked();

    let mut visual = VisualTestContext::from_window(opened.window, cx);
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
    assert!(
        matches!(
            runtime
                .runtime_status()
                .last_route
                .as_ref()
                .map(|record| &record.target),
            Some(DockViewportRouteTarget::TearOff { .. })
        ),
        "{}: polled outside release should route as tear-off, got {:?}",
        case.name,
        runtime.runtime_status().last_route
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
        MatrixTarget::RootEdge => {
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
            (Some(right_tabs), Some(root))
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
        MatrixTarget::RootEdge => {
            let root = nodes.target_root.expect("root-edge case should have root");
            let right_tabs = nodes
                .target_tabs
                .expect("root-edge case should have right target tabs");
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
                        target_tabs: right_tabs,
                        bounds: floating_bounds(210.0, 0.0, 210.0, 240.0),
                        is_central: false,
                    }),
                ),
                "{}: root-edge leaf fact should publish",
                case.name
            );
            point(px(418.0), px(120.0))
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
        MatrixTarget::RootEdge => {
            let root = nodes.target_root.expect("root-edge case should have root");
            let selector = selector_for(visual, host, DockDebugRegion::Split { node: root })
                .unwrap_or_else(|| {
                    panic!("{}: target split selector should be emitted", case.name)
                });
            let bounds = debug_bounds(visual, &selector);
            point(
                bounds.origin.x + bounds.size.width - px(2.0),
                bounds.center().y,
            )
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
    assert!(
        matches!(
            runtime
                .runtime_status()
                .last_route
                .as_ref()
                .map(|record| &record.target),
            Some(DockViewportRouteTarget::KnownViewport { space, host_position: routed_position, .. })
                if space == target_space && *routed_position == host_position
        ),
        "{}: hover/release should route to target viewport, got {:?}",
        case_name,
        runtime.runtime_status().last_route
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
        MatrixTarget::RootEdge => {
            let root = nodes.target_root.expect("root-edge case should have root");
            let DockNode::Split { children, .. } = controller
                .graph()
                .node(root)
                .expect("root-edge target root should still exist")
            else {
                panic!("{}: root-edge target should remain a split", case.name);
            };
            assert_eq!(children.len(), 3, "{}", case.name);
            let DockNode::Tabs { items, .. } = controller
                .graph()
                .node(
                    *children
                        .last()
                        .expect("root-edge split should have a right child"),
                )
                .expect("rightmost child should exist")
            else {
                panic!("{}: rightmost root-edge child should be tabs", case.name);
            };
            assert_eq!(items, &case.payload.moved_items(), "{}", case.name);
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
