use crate::graph_test_support::{edge_target, item, main_space as space, root_tabs_graph};
use crate::*;

#[test]
fn checked_select_tab_reports_only_real_changes() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);

    assert!(
        !graph
            .apply_op_checked(&DockOp::SelectTab {
                tabs: root,
                item: item("a"),
            })
            .expect("selecting the already-selected tab should be valid")
    );
    assert!(
        graph
            .apply_op_checked(&DockOp::SelectTab {
                tabs: root,
                item: item("b"),
            })
            .expect("selecting a different tab should be valid")
    );
    assert!(
        !graph
            .apply_op_checked(&DockOp::SelectTab {
                tabs: root,
                item: item("b"),
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
fn selected_item_queries_do_not_repair_invalid_selection() {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("missing")),
    });
    graph.set_root(space(), root);

    assert_eq!(graph.selected_item_in_tabs(root), None);

    graph.simplify_space(&space());

    assert_eq!(graph.selected_item_in_tabs(root), None);
    assert_eq!(
        graph.validate(),
        Err(DockGraphValidationError::TabsSelectedItemMissing {
            tabs: root,
            selected: item("missing"),
        })
    );
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
                target: DockGraphDropTarget::center(root),
            })
            .expect("same-stack center move should be valid")
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
fn checked_move_item_same_tab_bar_position_reports_noop() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);

    assert!(
        !graph
            .apply_op_checked(&DockOp::MoveItem {
                source_space: space(),
                item: item("a"),
                target_space: space(),
                target: DockGraphDropTarget::tab_bar(root, 1),
            })
            .expect("dropping a tab back into its current slot should be a valid no-op")
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
                target: DockGraphDropTarget::center(root),
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
                target: edge_target(&graph, &space(), root, DropZone::Right),
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
                target: DockGraphDropTarget::center(target),
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
fn checked_close_item_without_preference_selects_first_remaining_tab() {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b"), item("c")],
        selected: Some(item("b")),
    });
    graph.set_root(space(), root);

    assert!(
        graph
            .apply_op_checked(&DockOp::CloseItem {
                space: space(),
                item: item("b"),
            })
            .expect("closing the selected item should be valid")
    );

    let DockNode::Tabs { items, selected } = graph.node(root).expect("root tabs node should exist")
    else {
        panic!("expected tabs root");
    };
    assert_eq!(items, &vec![item("a"), item("c")]);
    assert_eq!(selected.as_ref(), items.first());
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
fn move_item_tab_bar_inserts_and_selects_item() {
    let (mut graph, root) = root_tabs_graph(&["a", "b", "c"]);

    assert!(
        graph
            .apply_op_checked(&DockOp::MoveItem {
                source_space: space(),
                item: item("c"),
                target_space: space(),
                target: DockGraphDropTarget::tab_bar(root, 1),
            })
            .expect("same-space insert should commit")
    );

    let DockNode::Tabs { items, selected } = graph.node(root).expect("root tabs node should exist")
    else {
        panic!("expected tabs root");
    };
    assert_eq!(items, &vec![item("a"), item("c"), item("b")]);
    assert_eq!(selected.as_ref(), items.get(1));
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
            target: DockGraphDropTarget::center(root),
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

    assert!(
        graph
            .edge_dock_plan(&space(), orphan, DropZone::Right)
            .is_none()
    );
    let err = graph
        .apply_op_checked(&DockOp::MoveItem {
            source_space: space(),
            item: item("b"),
            target_space: space(),
            target: DockGraphDropTarget::center(orphan),
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
    assert!(
        graph
            .apply_op_checked(&DockOp::MoveItem {
                source_space: space(),
                item: item("b"),
                target_space: space(),
                target: edge_target(&graph, &space(), root, DropZone::Right),
            })
            .expect("root-edge move should commit")
    );
    let split = graph.root(&space()).expect("space should keep root");

    let err = graph
        .apply_op_checked(&DockOp::MoveItem {
            source_space: space(),
            item: item("a"),
            target_space: space(),
            target: DockGraphDropTarget::center(split),
        })
        .expect_err("center target must be tabs");

    assert_eq!(err, DockGraphMutationError::NodeIsNotTabs { node: split });
    graph.assert_canonical_space(&space());
}

#[test]
fn checked_move_item_rejects_stale_edge_plan_without_replanning() {
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
    assert!(
        graph
            .edge_dock_plan(&space(), root, DropZone::Right)
            .is_some()
    );

    let stale_plan = DockEdgeDockPlan::InsertIntoSplit {
        split: root,
        zone: DropZone::Right,
        anchor_child: left,
        anchor_index: 1,
        insert_index: 2,
        sizing: crate::DockEdgeDockSizing::fallback(),
        sizing_scope: crate::DockEdgeDockSizingScope::AnchorChild,
    };
    let before = graph.export_layout();

    let err = graph
        .apply_op_checked(&DockOp::MoveItem {
            source_space: space(),
            item: item("a"),
            target_space: space(),
            target: DockGraphDropTarget::edge(stale_plan),
        })
        .expect_err("stale edge plan should not be re-planned at commit time");

    assert_eq!(
        err,
        DockGraphMutationError::MutationInvariantViolation {
            op: "DockGraphDropTarget",
            reason: "edge graph drop plan is no longer current".into(),
        }
    );
    assert_eq!(graph.export_layout(), before);
    graph.assert_canonical_space(&space());
}

#[test]
fn checked_move_item_to_empty_space_creates_target_root() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);
    let detached = DockSpaceId::new("detached");

    assert!(
        graph
            .apply_op_checked(&DockOp::MoveItem {
                source_space: space(),
                item: item("b"),
                target_space: detached.clone(),
                target: DockGraphDropTarget::empty_space(),
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
            .apply_op_checked(&DockOp::MoveItem {
                source_space: space(),
                item: item("b"),
                target_space: central.clone(),
                target: DockGraphDropTarget::empty_space(),
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
fn checked_move_tabs_to_empty_space_preserves_stack_order_and_selected_tab() {
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
            .apply_op_checked(&DockOp::MoveTabs {
                source_space: space(),
                source_tabs,
                target_space: detached.clone(),
                target: DockGraphDropTarget::empty_space(),
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
            .apply_op_checked(&DockOp::MoveTabs {
                source_space: space(),
                source_tabs,
                target_space: central.clone(),
                target: DockGraphDropTarget::empty_space(),
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
    let source_tabs = builder.tabs_with_selected(["a", "b"], "b");
    builder.add_floating(space(), source_tabs, dock_bounds(10.0, 20.0, 300.0, 200.0));
    let mut graph = builder.build();

    assert!(graph.root(&space()).is_none());
    assert_eq!(graph.floating_containers(&space()).len(), 1);

    assert!(
        graph
            .apply_op_checked(&DockOp::MoveTabs {
                source_space: space(),
                source_tabs,
                target_space: space(),
                target: DockGraphDropTarget::empty_space(),
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
        .apply_op_checked(&DockOp::MoveItem {
            source_space: space(),
            item: item("b"),
            target_space: detached.clone(),
            target: DockGraphDropTarget::empty_space(),
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
        .apply_op_checked(&DockOp::MoveItem {
            source_space: space(),
            item: item("b"),
            target_space: detached.clone(),
            target: DockGraphDropTarget::empty_space(),
        })
        .expect_err("floating-only target should still be non-empty");
    assert_eq!(
        err,
        DockGraphMutationError::TargetSpaceNotEmpty {
            space: detached.clone()
        }
    );

    let err = graph
        .apply_op_checked(&DockOp::MoveTabs {
            source_space: space(),
            source_tabs: root,
            target_space: detached.clone(),
            target: DockGraphDropTarget::empty_space(),
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
        .apply_op_checked(&DockOp::MoveItem {
            source_space: space(),
            item: item("missing"),
            target_space: space(),
            target: DockGraphDropTarget::empty_space(),
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
        .apply_op_checked(&DockOp::MoveTabs {
            source_space: space(),
            source_tabs: tabs,
            target_space: space(),
            target: DockGraphDropTarget::empty_space(),
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
        .apply_op_checked(&DockOp::MoveTabs {
            source_space: space(),
            source_tabs: other_tabs,
            target_space: detached,
            target: DockGraphDropTarget::empty_space(),
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
