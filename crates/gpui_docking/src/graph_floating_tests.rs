use crate::graph_test_support::{item, main_space as space, root_tabs_graph};
use crate::*;
use slotmap::Key;

#[test]
fn float_item_in_window_creates_floating_container() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);

    assert!(
        graph
            .apply_op_checked(&DockOp::FloatItemInWindow {
                source_space: space(),
                item: item("b"),
                target_space: space(),
                bounds: dock_bounds(10.0, 20.0, 300.0, 200.0),
            })
            .expect("floating item should commit")
    );

    assert_eq!(graph.collect_items_in_space(&space()).len(), 2);
    assert_eq!(graph.floating_containers(&space()).len(), 1);
    let DockNode::Tabs { items, .. } = graph.node(root).expect("root tabs node should exist")
    else {
        panic!("expected root tabs");
    };
    assert_eq!(items, &vec![item("a")]);
    graph.assert_canonical_space(&space());
}

#[test]
fn checked_floating_runtime_ops_report_specific_errors_without_mutation() {
    let (mut graph, _root) = root_tabs_graph(&["a"]);
    let missing = DockNodeId::null();

    assert_eq!(
        graph
            .apply_op_checked(&DockOp::FloatItemInWindow {
                source_space: space(),
                item: item("missing"),
                target_space: space(),
                bounds: dock_bounds(10.0, 20.0, 300.0, 200.0),
            })
            .expect_err("missing floating item should be reported"),
        DockGraphMutationError::ItemNotFound {
            space: space(),
            item: item("missing"),
        }
    );

    let orphan_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("orphan")],
        selected: Some(item("orphan")),
    });
    assert_eq!(
        graph
            .apply_op_checked(&DockOp::FloatTabsInWindow {
                source_space: space(),
                source_tabs: orphan_tabs,
                target_space: space(),
                bounds: dock_bounds(10.0, 20.0, 300.0, 200.0),
            })
            .expect_err("floating tabs outside source space should be reported"),
        DockGraphMutationError::SourceNodeNotInSpace {
            space: space(),
            node: orphan_tabs,
        }
    );

    assert_eq!(
        graph
            .apply_op_checked(&DockOp::RaiseFloating {
                space: space(),
                floating: missing,
            })
            .expect_err("missing floating container should be reported"),
        DockGraphMutationError::FloatingContainerNotFound {
            space: space(),
            floating: missing,
        }
    );

    assert!(
        graph
            .apply_op_checked(&DockOp::FloatItemInWindow {
                source_space: space(),
                item: item("a"),
                target_space: space(),
                bounds: dock_bounds(10.0, 20.0, 300.0, 200.0),
            })
            .expect("floating item should commit")
    );
    let floating = graph.floating_containers(&space())[0].node;
    let DockNode::Floating {
        child: floating_tabs,
    } = graph
        .node(floating)
        .expect("floating node should remain present")
    else {
        panic!("floating container should point to a floating node");
    };
    let floating_tabs = *floating_tabs;

    assert_eq!(
        graph
            .apply_op_checked(&DockOp::MoveFloating {
                source_space: space(),
                floating,
                target_space: space(),
                target: DockMoveTarget::center(floating_tabs),
            })
            .expect_err("floating cannot merge into its own tabs"),
        DockGraphMutationError::CannotMergeFloatingIntoOwnSubtree {
            floating,
            target: floating_tabs,
        }
    );

    assert_eq!(
        graph
            .apply_op_checked(&DockOp::MoveFloating {
                source_space: space(),
                floating,
                target_space: space(),
                target: DockMoveTarget::center(orphan_tabs),
            })
            .expect_err("merge target outside space should be reported"),
        DockGraphMutationError::TargetNodeNotInSpace {
            space: space(),
            target: orphan_tabs,
        }
    );

    assert_eq!(graph.collect_items_in_space(&space()), vec![item("a")]);
    assert_eq!(
        graph.collect_items_in_subtree(orphan_tabs),
        vec![item("orphan")]
    );
}

#[test]
fn merge_floating_into_moves_items_and_removes_floating() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);
    assert!(
        graph
            .apply_op_checked(&DockOp::FloatItemInWindow {
                source_space: space(),
                item: item("b"),
                target_space: space(),
                bounds: dock_bounds(10.0, 20.0, 300.0, 200.0),
            })
            .expect("floating item should commit")
    );

    let floating = graph.floating_containers(&space())[0].node;
    assert!(
        graph
            .apply_op_checked(&DockOp::MoveFloating {
                source_space: space(),
                floating,
                target_space: space(),
                target: DockMoveTarget::center(root),
            })
            .expect("floating merge should commit")
    );

    assert!(graph.floating_containers(&space()).is_empty());
    assert_eq!(
        graph.collect_items_in_space(&space()),
        vec![item("a"), item("b")]
    );
    graph.assert_canonical_space(&space());
}

#[test]
fn merge_floating_tabs_preserves_selected_item() {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: vec![item("root")],
        selected: Some(item("root")),
    });
    graph.set_root(space(), root);
    let floating_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("b")),
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_tabs,
    });
    graph
        .floating_containers_mut(space())
        .push(DockFloatingContainer {
            node: floating,
            bounds: dock_bounds(10.0, 20.0, 300.0, 200.0),
        });

    assert!(
        graph
            .apply_op_checked(&DockOp::MoveFloating {
                source_space: space(),
                floating,
                target_space: space(),
                target: DockMoveTarget::center(root),
            })
            .expect("floating merge should commit")
    );

    let DockNode::Tabs { items, selected } = graph.node(root).expect("root tabs node should exist")
    else {
        panic!("expected root tabs");
    };
    assert_eq!(items, &vec![item("root"), item("a"), item("b")]);
    assert_eq!(selected.as_ref(), items.get(2));
    assert!(graph.floating_containers(&space()).is_empty());
    graph.assert_canonical_space(&space());
}

#[test]
fn move_floating_edge_preserves_child_subtree() {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let floating_left = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let floating_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let floating_child = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Vertical,
        children: vec![floating_left, floating_right],
        fractions: vec![0.4, 0.6],
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_child,
    });
    graph.set_root(space(), root);
    graph
        .floating_containers_mut(space())
        .push(DockFloatingContainer {
            node: floating,
            bounds: dock_bounds(10.0, 20.0, 300.0, 200.0),
        });

    assert!(
        graph
            .apply_op_checked(&DockOp::MoveFloating {
                source_space: space(),
                floating,
                target_space: space(),
                target: DockMoveTarget::root_edge(root, DropZone::Right),
            })
            .expect("floating edge drop should be valid")
    );

    assert!(graph.floating_containers(&space()).is_empty());
    let new_root = graph
        .root(&space())
        .expect("space should still have a root");
    let DockNode::Split { axis, children, .. } =
        graph.node(new_root).expect("new root should exist")
    else {
        panic!("root should become a split");
    };
    assert_eq!(*axis, SplitAxis::Horizontal);
    assert_eq!(children, &vec![root, floating_child]);
    let DockNode::Split { axis, children, .. } = graph
        .node(floating_child)
        .expect("floating child subtree should be docked intact")
    else {
        panic!("floating child should remain a split subtree");
    };
    assert_eq!(*axis, SplitAxis::Vertical);
    assert_eq!(children, &vec![floating_left, floating_right]);
    assert_eq!(
        graph.collect_items_in_subtree(floating_child),
        vec![item("a"), item("c")]
    );
    graph.assert_canonical_space(&space());
}

#[test]
fn move_floating_to_empty_space_promotes_child_as_root() {
    let mut graph = DockGraph::new();
    let floating_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
        selected: Some(item("c")),
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_tabs,
    });
    graph
        .floating_containers_mut(space())
        .push(DockFloatingContainer {
            node: floating,
            bounds: dock_bounds(10.0, 20.0, 300.0, 200.0),
        });
    let detached = DockSpaceId::new("detached");

    assert!(
        graph
            .apply_op_checked(&DockOp::MoveFloating {
                source_space: space(),
                floating,
                target_space: detached.clone(),
                target: DockMoveTarget::empty_space(),
            })
            .expect("floating tear-off move should be valid")
    );

    assert!(graph.floating_containers(&space()).is_empty());
    assert_eq!(graph.root(&detached), Some(floating_tabs));
    let DockNode::Tabs { items, selected } = graph
        .node(floating_tabs)
        .expect("promoted floating tabs should exist")
    else {
        panic!("detached root should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("c")]);
    assert_eq!(selected.as_ref(), items.get(1));
    graph.assert_canonical_space(&detached);
}

#[test]
fn move_floating_to_empty_space_rebinds_empty_central_region() {
    let mut graph = DockGraph::new();
    let floating_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
        selected: Some(item("c")),
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_tabs,
    });
    graph
        .floating_containers_mut(space())
        .push(DockFloatingContainer {
            node: floating,
            bounds: dock_bounds(10.0, 20.0, 300.0, 200.0),
        });
    let central = DockSpaceId::new("central");
    graph.set_central_region(central.clone(), DockCentralRegion::empty());

    assert!(
        graph
            .apply_op_checked(&DockOp::MoveFloating {
                source_space: space(),
                floating,
                target_space: central.clone(),
                target: DockMoveTarget::empty_space(),
            })
            .expect("moving floating content into an empty central space should create a root")
    );

    assert_eq!(
        graph
            .central_region(&central)
            .expect("central metadata should remain present")
            .node,
        Some(floating_tabs)
    );
    graph
        .validate()
        .expect("central floating recovery should validate");
}
