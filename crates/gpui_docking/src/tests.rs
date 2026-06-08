use crate::*;

fn space() -> DockSpaceId {
    DockSpaceId::new("main")
}

fn item(id: &str) -> DockItemId {
    DockItemId::new(id)
}

fn root_tabs_graph(items: &[&str]) -> (DockGraph, DockNodeId) {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: items.iter().copied().map(item).collect(),
        active: 0,
    });
    graph.set_root(space(), root);
    (graph, root)
}

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

    let DockNode::Tabs { active, .. } = graph.node(root).expect("root tabs node should exist")
    else {
        panic!("expected tabs root");
    };
    assert_eq!(*active, 1);
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
                target_tabs: root,
                zone: DropZone::Center,
                insert_index: None,
            })
            .expect("same-stack center move without insert index should be valid")
    );

    let DockNode::Tabs { items, active } = graph.node(root).expect("root tabs node should exist")
    else {
        panic!("expected tabs root");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(*active, 0);
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
                target_tabs: root,
                zone: DropZone::Center,
                insert_index: None,
            })
            .expect("moving a tabs node onto itself should be a valid no-op")
    );

    let DockNode::Tabs { items, active } = graph.node(root).expect("root tabs node should exist")
    else {
        panic!("expected tabs root");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(*active, 0);
    graph.assert_canonical_space(&space());
}

#[test]
fn checked_move_tabs_reports_empty_source_tabs() {
    let mut graph = DockGraph::new();
    let empty = graph.insert_node(DockNode::Tabs {
        items: Vec::new(),
        active: 0,
    });
    let target = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(space(), empty);
    graph.set_root(DockSpaceId::new("other"), target);

    assert_eq!(
        graph
            .apply_op_checked(&DockOp::MoveTabs {
                source_space: space(),
                source_tabs: empty,
                target_space: DockSpaceId::new("other"),
                target_tabs: target,
                zone: DropZone::Center,
                insert_index: None,
            })
            .expect_err("empty source tabs should be reported"),
        DockOpApplyError::TabsNodeEmpty { tabs: empty }
    );
}

#[test]
fn move_item_center_inserts_and_selects_item() {
    let (mut graph, root) = root_tabs_graph(&["a", "b", "c"]);

    assert!(graph.apply_op(&DockOp::MoveItem {
        source_space: space(),
        item: item("c"),
        target_space: space(),
        target_tabs: root,
        zone: DropZone::Center,
        insert_index: Some(1),
    }));

    let DockNode::Tabs { items, active } = graph.node(root).expect("root tabs node should exist")
    else {
        panic!("expected tabs root");
    };
    assert_eq!(items, &vec![item("a"), item("c"), item("b")]);
    assert_eq!(*active, 1);
    graph.assert_canonical_space(&space());
}

#[test]
fn move_item_to_target_outside_target_space_is_transactional() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);
    let orphan = graph.insert_node(DockNode::Tabs {
        items: vec![item("orphan")],
        active: 0,
    });

    assert!(!graph.apply_op(&DockOp::MoveItem {
        source_space: space(),
        item: item("b"),
        target_space: space(),
        target_tabs: orphan,
        zone: DropZone::Right,
        insert_index: None,
    }));

    let DockNode::Tabs { items, active } = graph.node(root).expect("root tabs node should exist")
    else {
        panic!("expected root tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(*active, 0);

    let DockNode::Tabs { items, active } =
        graph.node(orphan).expect("orphan tabs node should exist")
    else {
        panic!("expected orphan tabs");
    };
    assert_eq!(items, &vec![item("orphan")]);
    assert_eq!(*active, 0);
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
            target_tabs: root,
            zone: DropZone::Center,
            insert_index: None,
        })
        .expect_err("missing source item should fail");

    assert_eq!(
        err,
        DockOpApplyError::ItemNotFound {
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
        active: 0,
    });

    let err = graph
        .apply_op_checked(&DockOp::MoveItem {
            source_space: space(),
            item: item("b"),
            target_space: space(),
            target_tabs: orphan,
            zone: DropZone::Right,
            insert_index: None,
        })
        .expect_err("orphan target should fail");

    assert_eq!(
        err,
        DockOpApplyError::TargetNodeNotInSpace {
            space: space(),
            target: orphan
        }
    );
    let DockNode::Tabs { items, active } = graph.node(root).expect("root tabs should remain")
    else {
        panic!("expected root tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(*active, 0);
    graph.assert_canonical_space(&space());
}

#[test]
fn checked_move_item_reports_center_target_that_is_not_tabs() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);
    assert!(graph.apply_op(&DockOp::MoveItem {
        source_space: space(),
        item: item("b"),
        target_space: space(),
        target_tabs: root,
        zone: DropZone::Right,
        insert_index: None,
    }));
    let split = graph.root(&space()).expect("space should keep root");

    let err = graph
        .apply_op_checked(&DockOp::MoveItem {
            source_space: space(),
            item: item("a"),
            target_space: space(),
            target_tabs: split,
            zone: DropZone::Center,
            insert_index: None,
        })
        .expect_err("center target must be tabs");

    assert_eq!(err, DockOpApplyError::NodeIsNotTabs { node: split });
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

    let DockNode::Tabs { items, active } = graph.node(root).expect("source root should remain")
    else {
        panic!("source root should be tabs");
    };
    assert_eq!(items, &vec![item("a")]);
    assert_eq!(*active, 0);

    let detached_root = graph
        .root(&detached)
        .expect("detached space should get root");
    let DockNode::Tabs { items, active } = graph
        .node(detached_root)
        .expect("detached root should exist")
    else {
        panic!("detached root should be tabs");
    };
    assert_eq!(items, &vec![item("b")]);
    assert_eq!(*active, 0);
    graph.assert_canonical_space(&space());
    graph.assert_canonical_space(&detached);
}

#[test]
fn checked_move_tabs_to_empty_space_preserves_stack_order_and_active_tab() {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        active: 1,
    });
    let sibling_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        active: 0,
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
    let DockNode::Tabs { items, active } = graph
        .node(detached_root)
        .expect("detached root should exist")
    else {
        panic!("detached root should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(*active, 1);
    assert_eq!(graph.collect_items_in_space(&space()), vec![item("c")]);
    graph.assert_canonical_space(&space());
    graph.assert_canonical_space(&detached);
}

#[test]
fn checked_empty_space_moves_reject_non_empty_target_without_mutation() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);
    let detached = DockSpaceId::new("detached");
    let detached_root = graph.insert_node(DockNode::Tabs {
        items: vec![item("existing")],
        active: 0,
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
        DockOpApplyError::TargetSpaceNotEmpty {
            space: detached.clone()
        }
    );

    let DockNode::Tabs { items, active } = graph.node(root).expect("source root should remain")
    else {
        panic!("source root should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(*active, 0);

    let DockNode::Tabs { items, active } = graph
        .node(detached_root)
        .expect("detached root should remain")
    else {
        panic!("detached root should be tabs");
    };
    assert_eq!(items, &vec![item("existing")]);
    assert_eq!(*active, 0);
    graph.assert_canonical_space(&space());
    graph.assert_canonical_space(&detached);
}

#[test]
fn checked_move_tabs_to_empty_space_rejects_source_outside_space() {
    let (mut graph, _root) = root_tabs_graph(&["a"]);
    let other = DockSpaceId::new("other");
    let other_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
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
        DockOpApplyError::SourceNodeNotInSpace {
            space: space(),
            node: other_tabs,
        }
    );
    assert_eq!(graph.collect_items_in_space(&space()), vec![item("a")]);
    assert_eq!(graph.collect_items_in_space(&other), vec![item("b")]);
}
