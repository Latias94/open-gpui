use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockGraph, DockGraphDropTarget,
    DockGraphMutationError, DockNode, DockNodeId, DockPolicyError, DockSplitResize, DropZone,
    SplitAxis, host_test_support::*,
};
use open_gpui::TestAppContext;
use slotmap::Key;

#[open_gpui::test]
fn workspace_resize_split_transaction_updates_fractions(cx: &mut TestAppContext) {
    let (graph, split, _left, _right) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let outcome = workspace
        .apply_action(&DockAction::ResizeSplit {
            split,
            fractions: vec![0.7, 0.3],
        })
        .expect("resize split action should be valid");

    assert_eq!(outcome, DockActionOutcome::Changed);
    let DockNode::Split { fractions, .. } = workspace
        .graph()
        .node(split)
        .expect("split should still exist")
    else {
        panic!("root should be split");
    };
    assert_close(fractions[0], 0.7);
    assert_close(fractions[1], 0.3);
}

#[open_gpui::test]
fn workspace_resize_split_transaction_reports_unchanged_for_same_fractions(
    cx: &mut TestAppContext,
) {
    let (graph, split, _left, _right) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let outcome = workspace
        .apply_action(&DockAction::ResizeSplit {
            split,
            fractions: vec![0.5, 0.5],
        })
        .expect("resize split action should be valid");

    assert_eq!(outcome, DockActionOutcome::Unchanged);
}

#[open_gpui::test]
fn workspace_resize_split_transaction_rejects_invalid_targets(cx: &mut TestAppContext) {
    let (graph, split, left_tabs, _right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let missing = workspace
        .apply_action(&DockAction::ResizeSplit {
            split: DockNodeId::null(),
            fractions: vec![0.5, 0.5],
        })
        .expect_err("missing split should fail");
    assert_eq!(
        missing,
        DockActionApplyError::Graph(DockGraphMutationError::SplitNodeNotFound {
            split: DockNodeId::null()
        })
    );

    let wrong_kind = workspace
        .apply_action(&DockAction::ResizeSplit {
            split: left_tabs,
            fractions: vec![0.5, 0.5],
        })
        .expect_err("tabs node is not a split");
    assert_eq!(
        wrong_kind,
        DockActionApplyError::Graph(DockGraphMutationError::NodeIsNotSplit { node: left_tabs })
    );

    let mismatch = workspace
        .apply_action(&DockAction::ResizeSplit {
            split,
            fractions: vec![1.0],
        })
        .expect_err("fraction length mismatch should fail");
    assert_eq!(
        mismatch,
        DockActionApplyError::Graph(DockGraphMutationError::SplitFractionsLenMismatch {
            split,
            children_len: 2,
            fractions_len: 1
        })
    );
}

#[open_gpui::test]
fn workspace_resize_splits_transaction_updates_corner_axes(cx: &mut TestAppContext) {
    let (graph, root, vertical) = corner_resize_graph();
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[("a", "A", "A"), ("b", "B", "B"), ("c", "C", "C")],
    );

    let outcome = workspace
        .apply_action(&DockAction::ResizeSplits {
            updates: vec![
                DockSplitResize::new(root, [0.65, 0.35]),
                DockSplitResize::new(vertical, [0.25, 0.75]),
            ],
        })
        .expect("corner resize should update both split axes");

    assert_eq!(outcome, DockActionOutcome::Changed);
    assert_split_fractions(workspace.graph().node(root), &[0.65, 0.35]);
    assert_split_fractions(workspace.graph().node(vertical), &[0.25, 0.75]);
}

#[open_gpui::test]
fn workspace_resize_splits_transaction_rejects_without_partial_mutation(cx: &mut TestAppContext) {
    let (graph, root, vertical) = corner_resize_graph();
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[("a", "A", "A"), ("b", "B", "B"), ("c", "C", "C")],
    );

    let err = workspace
        .apply_action(&DockAction::ResizeSplits {
            updates: vec![
                DockSplitResize::new(root, [0.65, 0.35]),
                DockSplitResize::new(vertical, [1.0]),
            ],
        })
        .expect_err("invalid second axis should reject the whole corner resize");

    assert_eq!(
        err,
        DockActionApplyError::Graph(DockGraphMutationError::SplitFractionsLenMismatch {
            split: vertical,
            children_len: 2,
            fractions_len: 1,
        })
    );
    assert_split_fractions(workspace.graph().node(root), &[0.5, 0.5]);
    assert_split_fractions(workspace.graph().node(vertical), &[0.5, 0.5]);
}

#[open_gpui::test]
fn workspace_policy_blocks_edge_drop_without_mutating_graph(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);
    workspace.policy_mut().set_allow_edge_split(false);

    let err = workspace
        .commit_tab_move(
            &space(),
            left_tabs,
            &item("a"),
            &space(),
            DockGraphDropTarget::edge(
                workspace
                    .graph()
                    .edge_dock_plan(&space(), right_tabs, DropZone::Right)
                    .expect("edge target should be plannable"),
            ),
        )
        .expect_err("edge drop should be rejected by policy");

    assert_eq!(
        err,
        DockActionApplyError::Policy(DockPolicyError::EdgeSplitDisabled)
    );
    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(left_tabs)
        .expect("source tabs should remain")
    else {
        panic!("source should be tabs");
    };
    assert_eq!(items, &vec![item("a")]);
    assert_eq!(selected.as_ref(), items.get(0));
}

#[open_gpui::test]
fn workspace_policy_blocks_splitter_resize_without_mutating_graph(cx: &mut TestAppContext) {
    let (graph, split, _left, _right) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);
    workspace.policy_mut().set_allow_splitter_resize(false);

    let err = workspace
        .apply_action(&DockAction::ResizeSplit {
            split,
            fractions: vec![0.7, 0.3],
        })
        .expect_err("splitter resize should be rejected by policy");

    assert_eq!(
        err,
        DockActionApplyError::Policy(DockPolicyError::SplitterResizeDisabled)
    );
    let DockNode::Split { fractions, .. } =
        workspace.graph().node(split).expect("split should remain")
    else {
        panic!("root should be split");
    };
    assert_close(fractions[0], 0.5);
    assert_close(fractions[1], 0.5);
}

#[open_gpui::test]
fn workspace_policy_blocks_corner_resize_without_mutating_graph(cx: &mut TestAppContext) {
    let (graph, root, vertical) = corner_resize_graph();
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[("a", "A", "A"), ("b", "B", "B"), ("c", "C", "C")],
    );
    workspace.policy_mut().set_allow_splitter_resize(false);

    let err = workspace
        .apply_action(&DockAction::ResizeSplits {
            updates: vec![
                DockSplitResize::new(root, [0.65, 0.35]),
                DockSplitResize::new(vertical, [0.25, 0.75]),
            ],
        })
        .expect_err("corner resize should be rejected by policy");

    assert_eq!(
        err,
        DockActionApplyError::Policy(DockPolicyError::SplitterResizeDisabled)
    );
    assert_split_fractions(workspace.graph().node(root), &[0.5, 0.5]);
    assert_split_fractions(workspace.graph().node(vertical), &[0.5, 0.5]);
}

fn corner_resize_graph() -> (DockGraph, DockNodeId, DockNodeId) {
    let mut graph = DockGraph::new();
    let left = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let top_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let bottom_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let vertical = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Vertical,
        children: vec![top_right, bottom_right],
        fractions: vec![0.5, 0.5],
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left, vertical],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(space(), root);
    (graph, root, vertical)
}

fn assert_split_fractions(node: Option<&DockNode>, expected: &[f32]) {
    let Some(DockNode::Split { fractions, .. }) = node else {
        panic!("node should be a split");
    };
    assert_eq!(fractions.len(), expected.len());
    for (actual, expected) in fractions.iter().zip(expected.iter()) {
        assert_close(*actual, *expected);
    }
}
