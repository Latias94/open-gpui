use crate::{
    DockActionOutcome, DockController, DockEdgeDockSizing, DockGraph, DockItemId, DockNode,
    DockNodeId, DockSpaceId, DockViewportRuntimeHandle, DockWorkspace, DropZone, SplitAxis,
    drag::DockDragPayload,
    drop_target::DockDropResolveSource,
    drop_target::{DockResolvedDropTarget, DockResolvedDropTargetKind},
    geometry::{DockDropBox, DockDropBoxKind},
    host_test_support::item,
    viewport_test_support::handle,
    workspace_drop_target::DockWorkspaceResolvedDropTarget,
    workspace_drop_transaction::{DockWorkspaceDropPayload, DockWorkspacePayloadDropRequest},
};
use open_gpui::{AnyWindowHandle, AppContext as _, TestAppContext};

const MODEL_ITEMS: [&str; 7] = ["a", "b", "c", "d", "e", "f", "g"];

#[derive(Clone, Copy)]
enum ModelDropKind {
    Center,
    InnerEdge(DropZone),
}

#[derive(Clone, Copy)]
struct ModelMove {
    name: &'static str,
    source_space: &'static str,
    item: &'static str,
    target_space: &'static str,
    target_item: &'static str,
    drop_kind: ModelDropKind,
    expected_open_spaces: &'static [&'static str],
}

struct InitialModelNodes {
    source_tabs: DockNodeId,
    left_tabs: DockNodeId,
    right_tabs: DockNodeId,
    left_right_root: DockNodeId,
    top_tabs: DockNodeId,
    bottom_tabs: DockNodeId,
    top_bottom_root: DockNodeId,
}

struct DockModelHarness {
    controller: open_gpui::Entity<DockController>,
    runtime: DockViewportRuntimeHandle,
    opened: [(DockSpaceId, AnyWindowHandle); 3],
    scenario_spaces: [DockSpaceId; 3],
    nodes: InitialModelNodes,
}

#[open_gpui::test]
fn runtime_opened_multi_window_sequential_dock_model_keeps_state_consistent(
    cx: &mut TestAppContext,
) {
    let harness = DockModelHarness::new(cx);
    harness.assert_graph_invariants(cx, "initial model");
    harness.assert_initial_shape(cx);

    for operation in sequential_model_moves() {
        harness.commit_move(cx, operation);
        harness.assert_graph_invariants(cx, operation.name);
        harness.assert_operation_shape(cx, operation);
        harness.assert_runtime_viewports(operation.expected_open_spaces, operation.name);
    }
}

#[open_gpui::test]
fn runtime_opened_multi_window_reoriented_dock_model_keeps_state_consistent(
    cx: &mut TestAppContext,
) {
    let harness = DockModelHarness::new(cx);
    harness.assert_graph_invariants(cx, "initial model");
    harness.assert_initial_shape(cx);

    for operation in reoriented_model_moves() {
        harness.commit_move(cx, operation);
        harness.assert_graph_invariants(cx, operation.name);
        harness.assert_operation_shape(cx, operation);
        harness.assert_runtime_viewports(operation.expected_open_spaces, operation.name);
    }
}

#[open_gpui::test]
fn prepared_source_vacate_cannot_unregister_recreated_registration(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let controller =
        cx.new(|_| DockController::new(DockWorkspace::new(source_space.clone(), DockGraph::new())));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let first = handle(1);
    let second = handle(2);
    assert!(
        runtime
            .borrow_mut()
            .register_opened_viewport(source_space.clone(), first)
            .is_empty()
    );
    let prepared = runtime.prepare_empty_payload_drop_source_vacate(&source_space, &target_space);

    runtime
        .borrow_mut()
        .unregister_adapter_window_for_test(first.window_id());
    assert!(
        runtime
            .borrow_mut()
            .register_opened_viewport(source_space.clone(), second)
            .is_empty()
    );

    let changed = cx
        .update(|app| runtime.finalize_empty_payload_drop_source_vacate(prepared.apply(true), app));

    assert!(!changed);
    assert_eq!(
        runtime.window_id_for_space(&source_space),
        Some(second.window_id())
    );
}

#[open_gpui::test]
fn source_vacate_without_registration_cannot_unregister_later_registration(
    cx: &mut TestAppContext,
) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let controller =
        cx.new(|_| DockController::new(DockWorkspace::new(source_space.clone(), DockGraph::new())));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let prepared = runtime.prepare_empty_payload_drop_source_vacate(&source_space, &target_space);
    let later = handle(1);
    assert!(
        runtime
            .borrow_mut()
            .register_opened_viewport(source_space.clone(), later)
            .is_empty()
    );

    let changed = cx
        .update(|app| runtime.finalize_empty_payload_drop_source_vacate(prepared.apply(true), app));

    assert!(!changed);
    assert_eq!(
        runtime.window_id_for_space(&source_space),
        Some(later.window_id())
    );
}

#[open_gpui::test]
fn source_vacate_controller_apply_can_reenter_runtime_and_unregisters_current_once(
    cx: &mut TestAppContext,
) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let controller =
        cx.new(|_| DockController::new(DockWorkspace::new(source_space.clone(), DockGraph::new())));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let source_window = handle(1);
    assert!(
        runtime
            .borrow_mut()
            .register_opened_viewport(source_space.clone(), source_window)
            .is_empty()
    );
    let prepared = runtime.prepare_empty_payload_drop_source_vacate(&source_space, &target_space);

    let source_is_empty = controller.update(cx, |controller, _| {
        let runtime_reentry = runtime.borrow_mut();
        assert_eq!(
            runtime_reentry.adapter().window_for_space(&source_space),
            Some(source_window)
        );
        drop(runtime_reentry);
        controller
            .graph()
            .collect_items_in_space(&source_space)
            .is_empty()
    });
    let first_changed = cx.update(|app| {
        runtime.finalize_empty_payload_drop_source_vacate(prepared.apply(source_is_empty), app)
    });
    let second = runtime.prepare_empty_payload_drop_source_vacate(&source_space, &target_space);
    let second_changed =
        cx.update(|app| runtime.finalize_empty_payload_drop_source_vacate(second.apply(true), app));

    assert!(first_changed);
    assert!(!second_changed);
    assert!(!runtime.is_viewport_open(&source_space));
}

impl DockModelHarness {
    fn new(cx: &mut TestAppContext) -> Self {
        let source_space = DockSpaceId::from("source");
        let left_right_space = DockSpaceId::from("left-right");
        let top_bottom_space = DockSpaceId::from("top-bottom");
        let scenario_spaces = [
            source_space.clone(),
            left_right_space.clone(),
            top_bottom_space.clone(),
        ];

        let (graph, nodes) = model_graph(&source_space, &left_right_space, &top_bottom_space);
        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.policy_mut().set_allow_platform_viewports(true);
        register_model_panels(cx, &mut workspace);
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller.clone());

        let opened = [
            (source_space.clone(), handle(1)),
            (left_right_space.clone(), handle(2)),
            (top_bottom_space.clone(), handle(3)),
        ];
        register_viewports(&runtime, &opened);

        Self {
            controller,
            runtime,
            opened,
            scenario_spaces,
            nodes,
        }
    }

    fn assert_graph_invariants(&self, cx: &TestAppContext, context: &str) {
        assert_graph_invariants(
            cx,
            &self.controller,
            &self.scenario_spaces,
            &MODEL_ITEMS,
            context,
        );
    }

    fn assert_initial_shape(&self, cx: &TestAppContext) {
        assert_initial_model_shape(cx, &self.controller, &self.nodes);
    }

    fn commit_move(&self, cx: &mut TestAppContext, operation: ModelMove) {
        commit_model_move(cx, &self.controller, &self.runtime, operation);
    }

    fn assert_operation_shape(&self, cx: &TestAppContext, operation: ModelMove) {
        assert_operation_shape(cx, &self.controller, operation);
    }

    fn assert_runtime_viewports(&self, expected_spaces: &[&str], context: &str) {
        assert_runtime_viewports(&self.runtime, &self.opened, expected_spaces, context);
    }
}

fn register_model_panels(cx: &mut TestAppContext, workspace: &mut DockWorkspace) {
    for id in MODEL_ITEMS {
        workspace.register_panel_view(
            item(id),
            format!("Panel {}", id.to_ascii_uppercase()),
            crate::host_test_support::test_view(cx, panel_label(id)),
        );
    }
}

fn sequential_model_moves() -> [ModelMove; 5] {
    [
        ModelMove {
            name: "source a docks below right child in left-right window",
            source_space: "source",
            item: "a",
            target_space: "left-right",
            target_item: "c",
            drop_kind: ModelDropKind::InnerEdge(DropZone::Bottom),
            expected_open_spaces: &["left-right", "source", "top-bottom"],
        },
        ModelMove {
            name: "source d docks left of bottom child in top-bottom window",
            source_space: "source",
            item: "d",
            target_space: "top-bottom",
            target_item: "f",
            drop_kind: ModelDropKind::InnerEdge(DropZone::Left),
            expected_open_spaces: &["left-right", "source", "top-bottom"],
        },
        ModelMove {
            name: "moved a docks right of top child in another window",
            source_space: "left-right",
            item: "a",
            target_space: "top-bottom",
            target_item: "e",
            drop_kind: ModelDropKind::InnerEdge(DropZone::Right),
            expected_open_spaces: &["left-right", "source", "top-bottom"],
        },
        ModelMove {
            name: "last source item g docks above left child and vacates source window",
            source_space: "source",
            item: "g",
            target_space: "left-right",
            target_item: "b",
            drop_kind: ModelDropKind::InnerEdge(DropZone::Top),
            expected_open_spaces: &["left-right", "top-bottom"],
        },
        ModelMove {
            name: "nested d merges into c tab stack after source window closed",
            source_space: "top-bottom",
            item: "d",
            target_space: "left-right",
            target_item: "c",
            drop_kind: ModelDropKind::Center,
            expected_open_spaces: &["left-right", "top-bottom"],
        },
    ]
}

fn reoriented_model_moves() -> [ModelMove; 3] {
    [
        ModelMove {
            name: "source a docks below right child in left-right window",
            source_space: "source",
            item: "a",
            target_space: "left-right",
            target_item: "c",
            drop_kind: ModelDropKind::InnerEdge(DropZone::Bottom),
            expected_open_spaces: &["left-right", "source", "top-bottom"],
        },
        ModelMove {
            name: "source d docks above moved a in left-right window",
            source_space: "source",
            item: "d",
            target_space: "left-right",
            target_item: "a",
            drop_kind: ModelDropKind::InnerEdge(DropZone::Top),
            expected_open_spaces: &["left-right", "source", "top-bottom"],
        },
        ModelMove {
            name: "last source item g centers onto top-bottom window after reorientation",
            source_space: "source",
            item: "g",
            target_space: "top-bottom",
            target_item: "e",
            drop_kind: ModelDropKind::Center,
            expected_open_spaces: &["left-right", "top-bottom"],
        },
    ]
}

fn register_viewports(
    runtime: &DockViewportRuntimeHandle,
    viewports: &[(DockSpaceId, AnyWindowHandle)],
) {
    for (space, window) in viewports {
        let closed = runtime
            .borrow_mut()
            .register_opened_viewport(space.clone(), window.clone());
        assert!(
            closed.is_empty(),
            "fresh viewport registration should not request cleanup"
        );
    }
}

fn commit_model_move(
    cx: &mut TestAppContext,
    controller: &open_gpui::Entity<DockController>,
    runtime: &DockViewportRuntimeHandle,
    operation: ModelMove,
) {
    let source_space = DockSpaceId::from(operation.source_space);
    let target_space = DockSpaceId::from(operation.target_space);
    let moving_item = item(operation.item);
    let target_item = item(operation.target_item);
    let (source_tabs, target_tabs, target_root) = cx.read_entity(controller, |controller, _| {
        let source_tabs = tabs_containing_item(
            controller.graph(),
            &source_space,
            &moving_item,
            operation.name,
        );
        let target_tabs = tabs_containing_item(
            controller.graph(),
            &target_space,
            &target_item,
            operation.name,
        );
        let target_root = controller
            .graph()
            .root_for_node_in_space(&target_space, target_tabs)
            .unwrap_or_else(|| panic!("{}: target tabs should have a root", operation.name));
        (source_tabs, target_tabs, target_root)
    });

    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        moving_item.clone(),
        format!("Panel {}", operation.item.to_ascii_uppercase()),
    );
    let session = runtime.borrow_mut().begin_payload_drag(&payload);
    let prepared_source_vacate =
        runtime.prepare_empty_payload_drop_source_vacate(&source_space, &target_space);

    let target = cx.read_entity(controller, |controller, _| match operation.drop_kind {
        ModelDropKind::Center => DockWorkspaceResolvedDropTarget::new(
            target_space.clone(),
            resolved_target(DockResolvedDropTargetKind::LeafCenter {
                root: target_root,
                target_tabs,
            }),
        ),
        ModelDropKind::InnerEdge(zone) => {
            let mut target = resolved_target(DockResolvedDropTargetKind::InnerEdge {
                root: target_root,
                target_tabs,
                zone,
            });
            let sizing = target.edge_sizing.expect("edge target should have sizing");
            target.edge_plan = Some(
                controller
                    .graph()
                    .edge_dock_plan_with_sizing(&target_space, target_tabs, zone, sizing)
                    .unwrap_or_else(|| panic!("{}: edge plan should resolve", operation.name)),
            );
            DockWorkspaceResolvedDropTarget::new(target_space.clone(), target)
        }
    });

    let source_is_empty = controller.update(cx, |controller, _| {
        let outcome = controller
            .workspace_mut()
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                source_space: &source_space,
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs,
                    item: &moving_item,
                },
                target,
                frozen_focus_item: None,
            })
            .unwrap_or_else(|error| panic!("{}: commit should succeed: {error}", operation.name));
        assert_eq!(
            outcome.action(),
            DockActionOutcome::Changed,
            "{}",
            operation.name
        );
        controller
            .graph()
            .collect_items_in_space(&source_space)
            .is_empty()
    });

    cx.update(|app| {
        let _ = runtime.finalize_empty_payload_drop_source_vacate(
            prepared_source_vacate.apply(source_is_empty),
            app,
        );
    });

    let _ = runtime.borrow_mut().finish_payload_drag(&session);
}

fn assert_runtime_viewports(
    runtime: &DockViewportRuntimeHandle,
    viewports: &[(DockSpaceId, open_gpui::AnyWindowHandle)],
    expected_spaces: &[&str],
    context: &str,
) {
    for (space, window) in viewports {
        let should_be_open = expected_spaces
            .iter()
            .any(|expected| *expected == space.as_str());
        assert_eq!(
            runtime.is_viewport_open(space),
            should_be_open,
            "{context}: viewport open state should match expectations for {space}"
        );
        if should_be_open {
            assert_eq!(
                runtime.window_id_for_space(space),
                Some(window.window_id()),
                "{context}: viewport should stay bound to its registered window"
            );
        }
    }
}

fn resolved_target(kind: DockResolvedDropTargetKind) -> DockResolvedDropTarget {
    let bounds = crate::host_test_support::floating_bounds(40.0, 32.0, 300.0, 180.0);
    let drop_box = match kind {
        DockResolvedDropTargetKind::LeafCenter { .. } => Some(DockDropBox {
            kind: DockDropBoxKind::Center,
            hit_bounds: bounds,
            draw_bounds: bounds,
            preview_bounds: bounds,
        }),
        DockResolvedDropTargetKind::InnerEdge { zone, .. } => Some(DockDropBox {
            kind: DockDropBoxKind::InnerEdge(zone),
            hit_bounds: bounds,
            draw_bounds: bounds,
            preview_bounds: bounds,
        }),
        DockResolvedDropTargetKind::RootEdge { .. }
        | DockResolvedDropTargetKind::TabBar { .. }
        | DockResolvedDropTargetKind::FloatingTitleBar { .. }
        | DockResolvedDropTargetKind::EmptyDockSpace { .. } => None,
    };
    let edge_sizing = match kind {
        DockResolvedDropTargetKind::InnerEdge { .. }
        | DockResolvedDropTargetKind::RootEdge { .. } => Some(DockEdgeDockSizing::fallback()),
        DockResolvedDropTargetKind::TabBar { .. }
        | DockResolvedDropTargetKind::LeafCenter { .. }
        | DockResolvedDropTargetKind::FloatingTitleBar { .. }
        | DockResolvedDropTargetKind::EmptyDockSpace { .. } => None,
    };
    DockResolvedDropTarget {
        kind,
        source: DockDropResolveSource::LeafBody,
        target_bounds: Some(bounds),
        inner_target_bounds: Some(bounds),
        availability: crate::drop_target::DockResolvedDropTargetAvailability::all(),
        drop_box,
        hit_bounds: Some(bounds),
        preview_bounds: Some(bounds),
        tab_insertion_bounds: None,
        edge_sizing,
        edge_plan: None,
        is_central_region: false,
    }
}

fn model_graph(
    source_space: &DockSpaceId,
    left_right_space: &DockSpaceId,
    top_bottom_space: &DockSpaceId,
) -> (DockGraph, InitialModelNodes) {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("d"), item("g")],
        selected: Some(item("a")),
    });
    graph.set_root(source_space.clone(), source_tabs);

    let left_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let right_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let left_right_root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left_tabs, right_tabs],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(left_right_space.clone(), left_right_root);

    let top_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("e")],
        selected: Some(item("e")),
    });
    let bottom_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("f")],
        selected: Some(item("f")),
    });
    let top_bottom_root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Vertical,
        children: vec![top_tabs, bottom_tabs],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(top_bottom_space.clone(), top_bottom_root);

    (
        graph,
        InitialModelNodes {
            source_tabs,
            left_tabs,
            right_tabs,
            left_right_root,
            top_tabs,
            bottom_tabs,
            top_bottom_root,
        },
    )
}

fn assert_initial_model_shape(
    cx: &TestAppContext,
    controller: &open_gpui::Entity<DockController>,
    nodes: &InitialModelNodes,
) {
    cx.read_entity(controller, |controller, _| {
        assert_tabs_items(
            controller.graph(),
            nodes.source_tabs,
            &[item("a"), item("d"), item("g")],
            "initial source tabs",
        );
        assert_tabs_items(
            controller.graph(),
            nodes.left_tabs,
            &[item("b")],
            "initial left tabs",
        );
        assert_tabs_items(
            controller.graph(),
            nodes.right_tabs,
            &[item("c")],
            "initial right tabs",
        );
        assert_tabs_items(
            controller.graph(),
            nodes.top_tabs,
            &[item("e")],
            "initial top tabs",
        );
        assert_tabs_items(
            controller.graph(),
            nodes.bottom_tabs,
            &[item("f")],
            "initial bottom tabs",
        );
        assert_split_axis(
            controller.graph(),
            nodes.left_right_root,
            SplitAxis::Horizontal,
            "initial left-right root",
        );
        assert_split_axis(
            controller.graph(),
            nodes.top_bottom_root,
            SplitAxis::Vertical,
            "initial top-bottom root",
        );
    });
}

fn assert_graph_invariants(
    cx: &TestAppContext,
    controller: &open_gpui::Entity<DockController>,
    scenario_spaces: &[DockSpaceId],
    expected_items: &[&str],
    context: &str,
) {
    cx.read_entity(controller, |controller, _| {
        controller
            .graph()
            .validate()
            .unwrap_or_else(|error| panic!("{context}: graph should validate: {error}"));
        for space in scenario_spaces {
            controller.graph().assert_canonical_space(space);
        }

        let mut collected = Vec::new();
        for space in scenario_spaces {
            collected.extend(controller.graph().collect_items_in_space(space));
        }
        for expected in expected_items {
            let expected = item(expected);
            let count = collected
                .iter()
                .filter(|candidate| **candidate == expected)
                .count();
            assert_eq!(
                count, 1,
                "{context}: expected item {expected} to appear exactly once"
            );
        }
        assert_eq!(
            collected.len(),
            expected_items.len(),
            "{context}: graph should not contain extra model items"
        );
    });
}

fn assert_operation_shape(
    cx: &TestAppContext,
    controller: &open_gpui::Entity<DockController>,
    operation: ModelMove,
) {
    let target_space = DockSpaceId::from(operation.target_space);
    let moved = item(operation.item);
    let target = item(operation.target_item);
    cx.read_entity(controller, |controller, _| match operation.drop_kind {
        ModelDropKind::Center => assert_items_share_tabs(
            controller.graph(),
            &target_space,
            &moved,
            &target,
            operation.name,
        ),
        ModelDropKind::InnerEdge(zone) => assert_edge_relationship(
            controller.graph(),
            &target_space,
            &moved,
            &target,
            zone,
            operation.name,
        ),
    });
}

fn assert_items_share_tabs(
    graph: &DockGraph,
    space: &DockSpaceId,
    first: &DockItemId,
    second: &DockItemId,
    context: &str,
) {
    let (first_tabs, _) = graph
        .find_item_in_space(space, first)
        .unwrap_or_else(|| panic!("{context}: first item should be in target space"));
    let (second_tabs, _) = graph
        .find_item_in_space(space, second)
        .unwrap_or_else(|| panic!("{context}: second item should be in target space"));
    assert_eq!(
        first_tabs, second_tabs,
        "{context}: center drop should merge items into the same tabs node"
    );
}

fn assert_edge_relationship(
    graph: &DockGraph,
    space: &DockSpaceId,
    moved: &DockItemId,
    target: &DockItemId,
    zone: DropZone,
    context: &str,
) {
    let root = graph
        .root(space)
        .unwrap_or_else(|| panic!("{context}: target space should have a root"));
    assert!(
        split_contains_edge_relationship(graph, root, moved, target, zone),
        "{context}: expected {moved} to be docked {zone:?} of {target}"
    );
}

fn split_contains_edge_relationship(
    graph: &DockGraph,
    node: DockNodeId,
    moved: &DockItemId,
    target: &DockItemId,
    zone: DropZone,
) -> bool {
    let Some(current) = graph.node(node) else {
        return false;
    };
    match current {
        DockNode::Tabs { .. } => false,
        DockNode::Floating { child } => {
            split_contains_edge_relationship(graph, *child, moved, target, zone)
        }
        DockNode::Split { axis, children, .. } => {
            if *axis == axis_for_zone(zone)
                && let (Some(moved_index), Some(target_index)) = (
                    child_index_containing_item(graph, children, moved),
                    child_index_containing_item(graph, children, target),
                )
                && moved_index != target_index
                && moved_is_on_zone_side(moved_index, target_index, zone)
            {
                return true;
            }
            children
                .iter()
                .copied()
                .any(|child| split_contains_edge_relationship(graph, child, moved, target, zone))
        }
    }
}

fn child_index_containing_item(
    graph: &DockGraph,
    children: &[DockNodeId],
    needle: &DockItemId,
) -> Option<usize> {
    children.iter().position(|child| {
        graph
            .collect_items_in_subtree(*child)
            .iter()
            .any(|candidate| candidate == needle)
    })
}

fn axis_for_zone(zone: DropZone) -> SplitAxis {
    match zone {
        DropZone::Left | DropZone::Right => SplitAxis::Horizontal,
        DropZone::Top | DropZone::Bottom => SplitAxis::Vertical,
        DropZone::Center => unreachable!("center does not create an edge split"),
    }
}

fn moved_is_on_zone_side(moved_index: usize, target_index: usize, zone: DropZone) -> bool {
    match zone {
        DropZone::Left | DropZone::Top => moved_index < target_index,
        DropZone::Right | DropZone::Bottom => moved_index > target_index,
        DropZone::Center => unreachable!("center does not create an edge split"),
    }
}

fn assert_tabs_items(graph: &DockGraph, tabs: DockNodeId, expected: &[DockItemId], context: &str) {
    let DockNode::Tabs { items, selected } = graph
        .node(tabs)
        .unwrap_or_else(|| panic!("{context}: tabs should exist"))
    else {
        panic!("{context}: node should be tabs");
    };
    assert_eq!(items.as_slice(), expected, "{context}");
    assert_eq!(selected.as_ref(), expected.first(), "{context}");
}

fn assert_split_axis(graph: &DockGraph, split: DockNodeId, expected: SplitAxis, context: &str) {
    let DockNode::Split { axis, .. } = graph
        .node(split)
        .unwrap_or_else(|| panic!("{context}: split should exist"))
    else {
        panic!("{context}: node should be split");
    };
    assert_eq!(*axis, expected, "{context}");
}

fn tabs_containing_item(
    graph: &DockGraph,
    space: &DockSpaceId,
    item: &DockItemId,
    context: &str,
) -> DockNodeId {
    graph
        .find_item_in_space(space, item)
        .map(|(tabs, _)| tabs)
        .unwrap_or_else(|| panic!("{context}: item {item} should be in {space}"))
}

fn panel_label(id: &str) -> &'static str {
    match id {
        "a" => "A",
        "b" => "B",
        "c" => "C",
        "d" => "D",
        "e" => "E",
        "f" => "F",
        "g" => "G",
        _ => unreachable!(),
    }
}
