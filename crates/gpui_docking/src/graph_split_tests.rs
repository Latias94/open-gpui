use crate::graph_test_support::{item, main_space as space, root_tabs_graph};
use crate::*;
use slotmap::Key;

#[test]
fn checked_set_split_fraction_two_reports_only_real_changes() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);
    assert!(
        graph
            .apply_op_checked(&DockOp::MoveItem {
                source_space: space(),
                item: item("b"),
                target_space: space(),
                target: DockMoveTarget::root_edge(root, DropZone::Right),
            })
            .expect("initial edge dock should commit")
    );
    let split = graph.root(&space()).expect("space should keep root");

    assert!(
        !graph
            .apply_op_checked(&DockOp::SetSplitFractionTwo {
                split,
                first_fraction: 0.5,
            })
            .expect("setting the same split fraction should be valid")
    );
    assert!(
        graph
            .apply_op_checked(&DockOp::SetSplitFractionTwo {
                split,
                first_fraction: 0.25,
            })
            .expect("changing the split fraction should be valid")
    );
    assert!(
        !graph
            .apply_op_checked(&DockOp::SetSplitFractionTwo {
                split,
                first_fraction: 0.25,
            })
            .expect("setting the same changed split fraction should stay valid")
    );

    let DockNode::Split { fractions, .. } = graph.node(split).expect("split should remain") else {
        panic!("root should be split");
    };
    assert_eq!(fractions, &vec![0.25, 0.75]);
    graph.assert_canonical_space(&space());
}

#[test]
fn checked_resize_reports_split_errors_without_mutation() {
    let (mut graph, split_a) = root_tabs_graph(&["a"]);
    let tabs_b = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![split_a, tabs_b],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(space(), split);

    assert_eq!(
        graph
            .apply_op_checked(&DockOp::SetSplitFractions {
                split: DockNodeId::null(),
                fractions: vec![0.5, 0.5],
            })
            .expect_err("missing split should fail"),
        DockGraphMutationError::SplitNodeNotFound {
            split: DockNodeId::null()
        }
    );
    assert_eq!(
        graph
            .apply_op_checked(&DockOp::SetSplitFractions {
                split: split_a,
                fractions: vec![0.5, 0.5],
            })
            .expect_err("tabs node is not a split"),
        DockGraphMutationError::NodeIsNotSplit { node: split_a }
    );
    assert_eq!(
        graph
            .apply_op_checked(&DockOp::SetSplitFractions {
                split,
                fractions: vec![1.0],
            })
            .expect_err("fraction length mismatch should fail"),
        DockGraphMutationError::SplitFractionsLenMismatch {
            split,
            children_len: 2,
            fractions_len: 1,
        }
    );
    assert_eq!(
        graph
            .apply_op_checked(&DockOp::SetSplitFractions {
                split,
                fractions: vec![0.5, f32::NAN],
            })
            .expect_err("invalid fraction should fail"),
        DockGraphMutationError::SplitFractionInvalid { split, index: 1 }
    );
    assert_eq!(
        graph
            .apply_op_checked(&DockOp::SetSplitFractionsMany {
                updates: vec![
                    SplitFractionsUpdate {
                        split,
                        fractions: vec![0.25, 0.75],
                    },
                    SplitFractionsUpdate {
                        split,
                        fractions: vec![0.75, 0.25],
                    },
                ],
            })
            .expect_err("duplicate split updates should fail before mutation"),
        DockGraphMutationError::DuplicateSplitFractionUpdate { split }
    );

    assert!(
        !graph
            .apply_op_checked(&DockOp::SetSplitFractionsMany {
                updates: vec![SplitFractionsUpdate {
                    split,
                    fractions: vec![0.5, 0.5],
                }],
            })
            .expect("matching batch fractions should be a valid no-op")
    );
    assert!(
        graph
            .apply_op_checked(&DockOp::SetSplitFractionsMany {
                updates: vec![SplitFractionsUpdate {
                    split,
                    fractions: vec![0.25, 0.75],
                }],
            })
            .expect("changed batch fractions should apply")
    );

    let DockNode::Split { fractions, .. } = graph.node(split).expect("split should remain") else {
        panic!("root should be split");
    };
    assert_eq!(fractions, &vec![0.25, 0.75]);
}

#[test]
fn repeated_same_axis_edge_docks_flatten_into_nary_split() {
    let (mut graph, root) = root_tabs_graph(&["a", "b", "c"]);

    assert!(
        graph
            .apply_op_checked(&DockOp::MoveItem {
                source_space: space(),
                item: item("b"),
                target_space: space(),
                target: DockMoveTarget::root_edge(root, DropZone::Right),
            })
            .expect("first edge dock should commit")
    );
    let target_tabs = graph
        .find_item_in_space(&space(), &item("b"))
        .expect("moved item should remain findable")
        .0;
    assert!(
        graph
            .apply_op_checked(&DockOp::MoveItem {
                source_space: space(),
                item: item("c"),
                target_space: space(),
                target: DockMoveTarget::inner_edge(
                    graph.root(&space()).expect("space should keep root"),
                    target_tabs,
                    DropZone::Right,
                ),
            })
            .expect("second edge dock should commit")
    );

    let root = graph.root(&space()).expect("space should keep a root");
    let DockNode::Split {
        axis,
        children,
        fractions,
    } = graph.node(root).expect("root split node should exist")
    else {
        panic!("expected split root");
    };
    assert_eq!(*axis, SplitAxis::Horizontal);
    assert_eq!(children.len(), 3);
    assert_eq!(children.len(), fractions.len());
    graph.assert_canonical_space(&space());
}

#[test]
fn cross_axis_edge_dock_wraps_target_without_flattening_parent_axis() {
    let (mut graph, root) = root_tabs_graph(&["a", "b", "c"]);

    assert!(
        graph
            .apply_op_checked(&DockOp::MoveItem {
                source_space: space(),
                item: item("b"),
                target_space: space(),
                target: DockMoveTarget::root_edge(root, DropZone::Right),
            })
            .expect("first edge dock should commit")
    );
    let left_tabs = graph
        .find_item_in_space(&space(), &item("a"))
        .expect("left item should remain findable")
        .0;

    assert!(
        graph
            .apply_op_checked(&DockOp::MoveItem {
                source_space: space(),
                item: item("c"),
                target_space: space(),
                target: DockMoveTarget::inner_edge(
                    graph.root(&space()).expect("space should keep root"),
                    left_tabs,
                    DropZone::Top,
                ),
            })
            .expect("cross-axis edge dock should commit")
    );

    let root = graph.root(&space()).expect("space should keep a root");
    let DockNode::Split {
        axis,
        children,
        fractions,
    } = graph.node(root).expect("root split node should exist")
    else {
        panic!("expected horizontal root split");
    };
    assert_eq!(*axis, SplitAxis::Horizontal);
    assert_eq!(children.len(), 2);
    assert_eq!(children.len(), fractions.len());

    let DockNode::Split {
        axis,
        children,
        fractions,
    } = graph
        .node(children[0])
        .expect("left child should become cross-axis split")
    else {
        panic!("expected vertical child split");
    };
    assert_eq!(*axis, SplitAxis::Vertical);
    assert_eq!(children.len(), 2);
    assert_eq!(children.len(), fractions.len());
    graph.assert_canonical_space(&space());
}
