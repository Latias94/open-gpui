use crate::{
    DockActionOutcome, DockController, DockGraph, DockItemId, DockNode, DockNodeId, DockSpaceId,
    DockViewportDropPayload, DockViewportDropRouteOutcome, DockViewportRouteTarget,
    DockViewportRuntimeHandle, DockViewportTargetContext, DockWorkspace, SplitAxis,
    drop_runtime::DockHostDropSceneFact,
    drop_target::{DockEmptySpaceDropTarget, DockLeafDropTarget, DockRootDropTarget},
    host_test_support::*,
};
use open_gpui::{AppContext as _, TestAppContext, WindowBounds, point, px};

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
    let cases = [
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
    ];

    for case in cases {
        run_source_only_release_case(cx, case);
    }
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
            DockViewportTargetContext::from_window(window, app)
        })
        .unwrap_or_else(|_| panic!("{}: source window should still be live", case.name));

    let result = cx.update(|app| {
        runtime.commit_payload_drop_from_screen_with_context(
            source_space.clone(),
            nodes.source_tabs,
            case.payload.drop_payload(),
            release_screen_position,
            None,
            &source_release_context,
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
