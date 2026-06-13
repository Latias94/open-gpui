use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockGraphMutationError, DockNode,
    DockNodeId, DockPolicyError, DropZone, SplitAxis,
    host_test_support::*,
    workspace_move_transaction::{DockWorkspaceMoveTabRequest, DockWorkspaceMoveTarget},
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
fn workspace_policy_blocks_edge_drop_without_mutating_graph(cx: &mut TestAppContext) {
    let (graph, split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);
    workspace.policy_mut().set_allow_edge_split(false);

    let err = workspace
        .commit_tab_move(DockWorkspaceMoveTabRequest {
            source_space: &space(),
            source_tabs: left_tabs,
            item: &item("a"),
            target_space: &space(),
            target: DockWorkspaceMoveTarget::inner_edge(split, right_tabs, DropZone::Right),
        })
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
