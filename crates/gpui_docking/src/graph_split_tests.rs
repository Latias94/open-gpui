use crate::graph_test_support::{edge_target, item, main_space as space, root_tabs_graph};
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
                target: edge_target(&graph, &space(), root, DropZone::Right),
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
                    DockSplitResize {
                        split,
                        fractions: vec![0.25, 0.75],
                    },
                    DockSplitResize {
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
                updates: vec![DockSplitResize {
                    split,
                    fractions: vec![0.5, 0.5],
                }],
            })
            .expect("matching batch fractions should be a valid no-op")
    );
    assert!(
        graph
            .apply_op_checked(&DockOp::SetSplitFractionsMany {
                updates: vec![DockSplitResize {
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
                target: edge_target(&graph, &space(), root, DropZone::Right),
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
                target: edge_target(&graph, &space(), target_tabs, DropZone::Right),
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
                target: edge_target(&graph, &space(), root, DropZone::Right),
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
                target: edge_target(&graph, &space(), left_tabs, DropZone::Top),
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

#[test]
fn inner_edge_dock_does_not_cross_opposing_axis_ancestor() {
    let (mut graph, _root) = root_tabs_graph(&["a"]);
    let left_tabs = graph
        .find_item_in_space(&space(), &item("a"))
        .expect("left item should remain findable")
        .0;
    let top_right_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let bottom_right_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let right_vertical_split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Vertical,
        children: vec![top_right_tabs, bottom_right_tabs],
        fractions: vec![0.5, 0.5],
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left_tabs, right_vertical_split],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(space(), root);

    for (target, zone) in [
        (top_right_tabs, DropZone::Left),
        (top_right_tabs, DropZone::Right),
        (bottom_right_tabs, DropZone::Left),
        (bottom_right_tabs, DropZone::Right),
    ] {
        assert_eq!(
            graph.edge_dock_plan(&space(), target, zone),
            Some(DockEdgeDockPlan::WrapTarget {
                target,
                axis: SplitAxis::Horizontal,
                zone,
                sizing: DockEdgeDockSizing::fallback(),
            }),
            "{zone:?} docking inside a right-side leaf must wrap that leaf, not cross the vertical split and insert beside the whole right region"
        );
    }
}

#[test]
fn inner_edge_dock_does_not_cross_opposing_axis_ancestor_mirrored() {
    let (mut graph, _root) = root_tabs_graph(&["a"]);
    let top_tabs = graph
        .find_item_in_space(&space(), &item("a"))
        .expect("top item should remain findable")
        .0;
    let bottom_left_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let bottom_right_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let bottom_horizontal_split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![bottom_left_tabs, bottom_right_tabs],
        fractions: vec![0.5, 0.5],
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Vertical,
        children: vec![top_tabs, bottom_horizontal_split],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(space(), root);

    for (target, zone) in [
        (bottom_left_tabs, DropZone::Top),
        (bottom_left_tabs, DropZone::Bottom),
        (bottom_right_tabs, DropZone::Top),
        (bottom_right_tabs, DropZone::Bottom),
    ] {
        assert_eq!(
            graph.edge_dock_plan(&space(), target, zone),
            Some(DockEdgeDockPlan::WrapTarget {
                target,
                axis: SplitAxis::Vertical,
                zone,
                sizing: DockEdgeDockSizing::fallback(),
            }),
            "{zone:?} docking inside a bottom leaf must wrap that leaf, not cross the horizontal split and insert beside the whole bottom region"
        );
    }
}
