use crate::graph_test_support::{item, main_space as space, root_tabs_graph};
use crate::*;

#[test]
fn checked_set_active_tab_reports_only_real_changes() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);

    assert!(
        !graph
            .apply_op_checked(&DockOp::SetActiveTab {
                tabs: root,
                active: 0,
            })
            .expect("selecting the already-active tab should be valid")
    );
    assert!(
        graph
            .apply_op_checked(&DockOp::SetActiveTab {
                tabs: root,
                active: 1,
            })
            .expect("selecting a different tab should be valid")
    );
    assert!(
        !graph
            .apply_op_checked(&DockOp::SetActiveTab {
                tabs: root,
                active: 1,
            })
            .expect("selecting the same new tab should stay valid")
    );

    let DockNode::Tabs { selected, .. } = graph.node(root).expect("root tabs node should exist")
    else {
        panic!("expected tabs root");
    };
    assert_eq!(selected.as_ref(), Some(&item("b")));
}

#[test]
fn checked_move_item_same_stack_center_reports_noop() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);

    assert!(
        !graph
            .apply_op_checked(&DockOp::MoveItem {
                source_space: space(),
                item: item("a"),
                target_space: space(),
                target: DockMoveTarget::center(root),
            })
            .expect("same-stack center move without insert index should be valid")
    );

    let DockNode::Tabs { items, selected } = graph.node(root).expect("root tabs node should exist")
    else {
        panic!("expected tabs root");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(selected.as_ref(), items.get(0));
    graph.assert_canonical_space(&space());
}

#[test]
fn checked_move_tabs_self_center_reports_noop() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);

    assert!(
        !graph
            .apply_op_checked(&DockOp::MoveTabs {
                source_space: space(),
                source_tabs: root,
                target_space: space(),
                target: DockMoveTarget::center(root),
            })
            .expect("moving a tabs node onto itself should be a valid no-op")
    );

    let DockNode::Tabs { items, selected } = graph.node(root).expect("root tabs node should exist")
    else {
        panic!("expected tabs root");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(selected.as_ref(), items.get(0));
    graph.assert_canonical_space(&space());
}

#[test]
fn checked_move_tabs_edge_drop_onto_same_space_root_preserves_items() {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
        selected: Some(item("c")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![source_tabs, target_tabs],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(space(), root);

    assert!(
        graph
            .apply_op_checked(&DockOp::MoveTabs {
                source_space: space(),
                source_tabs,
                target_space: space(),
                target: DockMoveTarget::root_edge(root, DropZone::Right),
            })
            .expect("same-space root-edge tabs move should commit transactionally")
    );

    assert_eq!(
        graph.collect_items_in_space(&space()),
        vec![item("b"), item("a"), item("c")]
    );
    let new_tabs = graph
        .find_item_in_space(&space(), &item("a"))
        .expect("moved item should stay reachable")
        .0;
    assert_eq!(
        graph.find_item_in_space(&space(), &item("c")),
        Some((new_tabs, 1))
    );
    let DockNode::Tabs { items, selected } = graph.node(new_tabs).expect("moved tabs should exist")
    else {
        panic!("moved node should remain a tabs stack");
    };
    assert_eq!(items, &vec![item("a"), item("c")]);
    assert_eq!(selected.as_ref(), items.get(1));
    graph.assert_canonical_space(&space());
}

#[test]
fn checked_move_tabs_reports_empty_source_tabs() {
    let mut graph = DockGraph::new();
    let empty = graph.insert_node(DockNode::Tabs {
        items: Vec::new(),
        selected: None,
    });
    let target = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(space(), empty);
    graph.set_root(DockSpaceId::new("other"), target);

    assert_eq!(
        graph
            .apply_op_checked(&DockOp::MoveTabs {
                source_space: space(),
                source_tabs: empty,
                target_space: DockSpaceId::new("other"),
                target: DockMoveTarget::center(target),
            })
            .expect_err("empty source tabs should be reported"),
        DockGraphMutationError::TabsNodeEmpty { tabs: empty }
    );
}

#[test]
fn checked_open_item_inserts_into_existing_tabs() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);

    assert!(
        graph
            .apply_op_checked(&DockOp::OpenItem {
                space: space(),
                target_tabs: Some(root),
                item: item("reopened"),
                insert_index: Some(1),
            })
            .expect("opening a new item into existing tabs should be valid")
    );

    let DockNode::Tabs { items, selected } = graph.node(root).expect("root tabs node should exist")
    else {
        panic!("expected tabs root");
    };
    assert_eq!(items, &vec![item("a"), item("reopened"), item("b")]);
    assert_eq!(selected.as_ref(), items.get(1));
    graph.validate().expect("opened graph should validate");
}

#[test]
fn checked_open_item_creates_root_for_empty_space() {
    let mut graph = DockGraph::new();
    let detached = DockSpaceId::new("detached");

    assert!(
        graph
            .apply_op_checked(&DockOp::OpenItem {
                space: detached.clone(),
                target_tabs: None,
                item: item("reopened"),
                insert_index: None,
            })
            .expect("opening into an empty space should create a root")
    );

    let root = graph
        .root(&detached)
        .expect("detached space should get root");
    let DockNode::Tabs { items, selected } = graph.node(root).expect("root tabs node should exist")
    else {
        panic!("expected tabs root");
    };
    assert_eq!(items, &vec![item("reopened")]);
    assert_eq!(selected.as_ref(), items.get(0));
    graph.validate().expect("opened graph should validate");
}

#[test]
fn checked_open_item_rebinds_empty_central_region() {
    let mut graph = DockGraph::new();
    let central = DockSpaceId::new("central");
    graph.set_central_region(central.clone(), DockCentralRegion::empty());

    assert!(
        graph
            .apply_op_checked(&DockOp::OpenItem {
                space: central.clone(),
                target_tabs: None,
                item: item("reopened"),
                insert_index: None,
            })
            .expect("opening into an empty central space should create a root")
    );

    let root = graph.root(&central).expect("central space should get root");
    assert_eq!(
        graph
            .central_region(&central)
            .expect("central metadata should remain present")
            .node,
        Some(root)
    );
    graph
        .validate()
        .expect("central root recovery should validate");
}

#[test]
fn checked_close_item_rebinds_collapsed_central_region() {
    let mut graph = DockGraph::new();
    let left = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let right = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left, right],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(space(), root);
    graph.set_central_region(space(), DockCentralRegion::with_node(root));

    assert!(
        graph
            .apply_op_checked(&DockOp::CloseItem {
                space: space(),
                item: item("a"),
            })
            .expect("closing a reachable item should be valid")
    );

    let new_root = graph
        .root(&space())
        .expect("space should keep the surviving child");
    assert_eq!(new_root, right);
    assert_eq!(
        graph
            .central_region(&space())
            .expect("central metadata should remain present")
            .node,
        Some(new_root)
    );
    graph
        .validate()
        .expect("collapsed central region should validate");
}

#[test]
fn checked_open_item_rejects_duplicate_items_without_mutation() {
    let (mut graph, root) = root_tabs_graph(&["a"]);

    assert_eq!(
        graph
            .apply_op_checked(&DockOp::OpenItem {
                space: space(),
                target_tabs: Some(root),
                item: item("a"),
                insert_index: Some(1),
            })
            .expect_err("opening an already reachable item should fail"),
        DockGraphMutationError::ItemAlreadyOpen { item: item("a") }
    );

    let DockNode::Tabs { items, selected } = graph.node(root).expect("root tabs node should exist")
    else {
        panic!("expected tabs root");
    };
    assert_eq!(items, &vec![item("a")]);
    assert_eq!(selected.as_ref(), items.get(0));
}

#[test]
fn move_item_center_inserts_and_selects_item() {
    let (mut graph, root) = root_tabs_graph(&["a", "b", "c"]);

    assert!(graph.apply_op(&DockOp::MoveItem {
        source_space: space(),
        item: item("c"),
        target_space: space(),
        target: DockMoveTarget::tab_bar(root, 1),
    }));

    let DockNode::Tabs { items, selected } = graph.node(root).expect("root tabs node should exist")
    else {
        panic!("expected tabs root");
    };
    assert_eq!(items, &vec![item("a"), item("c"), item("b")]);
    assert_eq!(selected.as_ref(), items.get(1));
    graph.assert_canonical_space(&space());
}

#[test]
fn move_item_to_target_outside_target_space_is_transactional() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);
    let orphan = graph.insert_node(DockNode::Tabs {
        items: vec![item("orphan")],
        selected: Some(item("orphan")),
    });

    assert!(!graph.apply_op(&DockOp::MoveItem {
        source_space: space(),
        item: item("b"),
        target_space: space(),
        target: DockMoveTarget::root_edge(orphan, DropZone::Right),
    }));

    let DockNode::Tabs { items, selected } = graph.node(root).expect("root tabs node should exist")
    else {
        panic!("expected root tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(selected.as_ref(), items.get(0));

    let DockNode::Tabs { items, selected } =
        graph.node(orphan).expect("orphan tabs node should exist")
    else {
        panic!("expected orphan tabs");
    };
    assert_eq!(items, &vec![item("orphan")]);
    assert_eq!(selected.as_ref(), items.get(0));
    graph.assert_canonical_space(&space());
}

#[test]
fn checked_move_item_reports_missing_source_item() {
    let (mut graph, root) = root_tabs_graph(&["a"]);
    let err = graph
        .apply_op_checked(&DockOp::MoveItem {
            source_space: space(),
            item: item("missing"),
            target_space: space(),
            target: DockMoveTarget::center(root),
        })
        .expect_err("missing source item should fail");

    assert_eq!(
        err,
        DockGraphMutationError::ItemNotFound {
            space: space(),
            item: item("missing")
        }
    );
    graph.assert_canonical_space(&space());
}

#[test]
fn checked_move_item_reports_target_outside_space_without_mutation() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);
    let orphan = graph.insert_node(DockNode::Tabs {
        items: vec![item("orphan")],
        selected: Some(item("orphan")),
    });

    let err = graph
        .apply_op_checked(&DockOp::MoveItem {
            source_space: space(),
            item: item("b"),
            target_space: space(),
            target: DockMoveTarget::root_edge(orphan, DropZone::Right),
        })
        .expect_err("orphan target should fail");

    assert_eq!(
        err,
        DockGraphMutationError::TargetNodeNotInSpace {
            space: space(),
            target: orphan
        }
    );
    let DockNode::Tabs { items, selected } = graph.node(root).expect("root tabs should remain")
    else {
        panic!("expected root tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(selected.as_ref(), items.get(0));
    graph.assert_canonical_space(&space());
}

#[test]
fn checked_move_item_reports_center_target_that_is_not_tabs() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);
    assert!(graph.apply_op(&DockOp::MoveItem {
        source_space: space(),
        item: item("b"),
        target_space: space(),
        target: DockMoveTarget::root_edge(root, DropZone::Right),
    }));
    let split = graph.root(&space()).expect("space should keep root");

    let err = graph
        .apply_op_checked(&DockOp::MoveItem {
            source_space: space(),
            item: item("a"),
            target_space: space(),
            target: DockMoveTarget::center(split),
        })
        .expect_err("center target must be tabs");

    assert_eq!(err, DockGraphMutationError::NodeIsNotTabs { node: split });
    graph.assert_canonical_space(&space());
}

#[test]
fn checked_move_item_to_empty_space_creates_target_root() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);
    let detached = DockSpaceId::new("detached");

    assert!(
        graph
            .apply_op_checked(&DockOp::MoveItemToEmptyDockSpace {
                source_space: space(),
                item: item("b"),
                target_space: detached.clone(),
            })
            .expect("move to empty space should be valid")
    );

    let DockNode::Tabs { items, selected } = graph.node(root).expect("source root should remain")
    else {
        panic!("source root should be tabs");
    };
    assert_eq!(items, &vec![item("a")]);
    assert_eq!(selected.as_ref(), items.get(0));

    let detached_root = graph
        .root(&detached)
        .expect("detached space should get root");
    let DockNode::Tabs { items, selected } = graph
        .node(detached_root)
        .expect("detached root should exist")
    else {
        panic!("detached root should be tabs");
    };
    assert_eq!(items, &vec![item("b")]);
    assert_eq!(selected.as_ref(), items.get(0));
    graph.assert_canonical_space(&space());
    graph.assert_canonical_space(&detached);
}

#[test]
fn checked_move_item_to_empty_space_rebinds_empty_central_region() {
    let (mut graph, _) = root_tabs_graph(&["a", "b"]);
    let central = DockSpaceId::new("central");
    graph.set_central_region(central.clone(), DockCentralRegion::empty());

    assert!(
        graph
            .apply_op_checked(&DockOp::MoveItemToEmptyDockSpace {
                source_space: space(),
                item: item("b"),
                target_space: central.clone(),
            })
            .expect("moving into an empty central space should create a root")
    );

    let root = graph.root(&central).expect("central space should get root");
    assert_eq!(
        graph
            .central_region(&central)
            .expect("central metadata should remain present")
            .node,
        Some(root)
    );
    graph
        .validate()
        .expect("central move recovery should validate");
}

#[test]
fn checked_move_tabs_to_empty_space_preserves_stack_order_and_active_tab() {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("b")),
    });
    let sibling_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![source_tabs, sibling_tabs],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(space(), root);
    let detached = DockSpaceId::new("detached");

    assert!(
        graph
            .apply_op_checked(&DockOp::MoveTabsToEmptyDockSpace {
                source_space: space(),
                source_tabs,
                target_space: detached.clone(),
            })
            .expect("moving tabs to empty space should be valid")
    );

    let detached_root = graph
        .root(&detached)
        .expect("detached space should get root");
    let DockNode::Tabs { items, selected } = graph
        .node(detached_root)
        .expect("detached root should exist")
    else {
        panic!("detached root should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(selected.as_ref(), items.get(1));
    assert_eq!(graph.collect_items_in_space(&space()), vec![item("c")]);
    graph.assert_canonical_space(&space());
    graph.assert_canonical_space(&detached);
}

#[test]
fn checked_move_tabs_to_empty_space_rebinds_empty_central_region() {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(space(), source_tabs);
    let central = DockSpaceId::new("central");
    graph.set_central_region(central.clone(), DockCentralRegion::empty());

    assert!(
        graph
            .apply_op_checked(&DockOp::MoveTabsToEmptyDockSpace {
                source_space: space(),
                source_tabs,
                target_space: central.clone(),
            })
            .expect("moving tabs into an empty central space should create a root")
    );

    let root = graph.root(&central).expect("central space should get root");
    assert_eq!(
        graph
            .central_region(&central)
            .expect("central metadata should remain present")
            .node,
        Some(root)
    );
    graph
        .validate()
        .expect("central tabs recovery should validate");
}

#[test]
fn checked_move_floating_tabs_to_empty_same_space_removes_floating_and_creates_root() {
    let mut builder = DockLayoutBuilder::new();
    let source_tabs = builder.tabs(["a", "b"], 1);
    builder.add_floating(space(), source_tabs, dock_bounds(10.0, 20.0, 300.0, 200.0));
    let mut graph = builder.build();

    assert!(graph.root(&space()).is_none());
    assert_eq!(graph.floating_containers(&space()).len(), 1);

    assert!(
        graph
            .apply_op_checked(&DockOp::MoveTabsToEmptyDockSpace {
                source_space: space(),
                source_tabs,
                target_space: space(),
            })
            .expect("floating tabs should move to the empty root in the same space")
    );

    assert!(graph.floating_containers(&space()).is_empty());
    let root = graph.root(&space()).expect("space should get a root");
    let DockNode::Tabs { items, selected } = graph.node(root).expect("root tabs should exist")
    else {
        panic!("root should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(selected.as_ref(), items.get(1));
    graph.assert_canonical_space(&space());
}

#[test]
fn checked_empty_space_moves_reject_non_empty_target_without_mutation() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);
    let detached = DockSpaceId::new("detached");
    let detached_root = graph.insert_node(DockNode::Tabs {
        items: vec![item("existing")],
        selected: Some(item("existing")),
    });
    graph.set_root(detached.clone(), detached_root);

    let err = graph
        .apply_op_checked(&DockOp::MoveItemToEmptyDockSpace {
            source_space: space(),
            item: item("b"),
            target_space: detached.clone(),
        })
        .expect_err("non-empty target should be rejected");
    assert_eq!(
        err,
        DockGraphMutationError::TargetSpaceNotEmpty {
            space: detached.clone()
        }
    );

    let DockNode::Tabs { items, selected } = graph.node(root).expect("source root should remain")
    else {
        panic!("source root should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(selected.as_ref(), items.get(0));

    let DockNode::Tabs { items, selected } = graph
        .node(detached_root)
        .expect("detached root should remain")
    else {
        panic!("detached root should be tabs");
    };
    assert_eq!(items, &vec![item("existing")]);
    assert_eq!(selected.as_ref(), items.get(0));
    graph.assert_canonical_space(&space());
    graph.assert_canonical_space(&detached);
}

#[test]
fn checked_empty_space_moves_reject_floating_only_target_without_mutation() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);
    let detached = DockSpaceId::new("detached");
    let detached_floating_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("existing")],
        selected: Some(item("existing")),
    });
    let detached_floating = graph.insert_node(DockNode::Floating {
        child: detached_floating_tabs,
    });
    graph
        .floating_containers_mut(detached.clone())
        .push(DockFloatingContainer {
            node: detached_floating,
            bounds: dock_bounds(10.0, 20.0, 240.0, 160.0),
        });

    let err = graph
        .apply_op_checked(&DockOp::MoveItemToEmptyDockSpace {
            source_space: space(),
            item: item("b"),
            target_space: detached.clone(),
        })
        .expect_err("floating-only target should still be non-empty");
    assert_eq!(
        err,
        DockGraphMutationError::TargetSpaceNotEmpty {
            space: detached.clone()
        }
    );

    let err = graph
        .apply_op_checked(&DockOp::MoveTabsToEmptyDockSpace {
            source_space: space(),
            source_tabs: root,
            target_space: detached.clone(),
        })
        .expect_err("floating-only target should reject tab-group moves too");
    assert_eq!(
        err,
        DockGraphMutationError::TargetSpaceNotEmpty {
            space: detached.clone()
        }
    );

    let DockNode::Tabs { items, selected } = graph.node(root).expect("source root should remain")
    else {
        panic!("source root should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(selected.as_ref(), items.get(0));
    assert!(graph.root(&detached).is_none());
    assert_eq!(graph.floating_containers(&detached).len(), 1);
    assert_eq!(
        graph.collect_items_in_space(&detached),
        vec![item("existing")]
    );
    graph.assert_canonical_space(&space());
    graph.assert_canonical_space(&detached);
}

#[test]
fn checked_empty_same_space_moves_report_missing_source() {
    let mut graph = DockGraph::new();

    let item_err = graph
        .apply_op_checked(&DockOp::MoveItemToEmptyDockSpace {
            source_space: space(),
            item: item("missing"),
            target_space: space(),
        })
        .expect_err("empty same-space item move should still validate the source item");
    assert_eq!(
        item_err,
        DockGraphMutationError::ItemNotFound {
            space: space(),
            item: item("missing")
        }
    );

    let tabs = graph.insert_node(DockNode::Tabs {
        items: Vec::new(),
        selected: None,
    });
    let tabs_err = graph
        .apply_op_checked(&DockOp::MoveTabsToEmptyDockSpace {
            source_space: space(),
            source_tabs: tabs,
            target_space: space(),
        })
        .expect_err("empty same-space tabs move should still validate the source tabs");
    assert_eq!(tabs_err, DockGraphMutationError::TabsNodeEmpty { tabs });
}

#[test]
fn checked_move_tabs_to_empty_space_rejects_source_outside_space() {
    let (mut graph, _root) = root_tabs_graph(&["a"]);
    let other = DockSpaceId::new("other");
    let other_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(other.clone(), other_tabs);
    let detached = DockSpaceId::new("detached");

    let err = graph
        .apply_op_checked(&DockOp::MoveTabsToEmptyDockSpace {
            source_space: space(),
            source_tabs: other_tabs,
            target_space: detached,
        })
        .expect_err("source tabs outside source space should fail");

    assert_eq!(
        err,
        DockGraphMutationError::SourceNodeNotInSpace {
            space: space(),
            node: other_tabs,
        }
    );
    assert_eq!(graph.collect_items_in_space(&space()), vec![item("a")]);
    assert_eq!(graph.collect_items_in_space(&other), vec![item("b")]);
}
