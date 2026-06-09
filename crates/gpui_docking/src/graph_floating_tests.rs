use crate::graph_test_support::{item, main_space as space, root_tabs_graph};
use crate::*;
use slotmap::Key;

#[test]
fn float_item_in_window_creates_floating_container() {
    let (mut graph, root) = root_tabs_graph(&["a", "b"]);

    assert!(graph.apply_op(&DockOp::FloatItemInWindow {
        source_space: space(),
        item: item("b"),
        target_space: space(),
        bounds: dock_bounds(10.0, 20.0, 300.0, 200.0),
    }));

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
        DockOpApplyError::ItemNotFound {
            space: space(),
            item: item("missing"),
        }
    );

    let orphan_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("orphan")],
        active: 0,
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
        DockOpApplyError::SourceNodeNotInSpace {
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
        DockOpApplyError::FloatingContainerNotFound {
            space: space(),
            floating: missing,
        }
    );

    assert!(graph.apply_op(&DockOp::FloatItemInWindow {
        source_space: space(),
        item: item("a"),
        target_space: space(),
        bounds: dock_bounds(10.0, 20.0, 300.0, 200.0),
    }));
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
            .apply_op_checked(&DockOp::MergeFloatingInto {
                space: space(),
                floating,
                target_tabs: floating_tabs,
            })
            .expect_err("floating cannot merge into its own tabs"),
        DockOpApplyError::CannotMergeFloatingIntoOwnSubtree {
            floating,
            target: floating_tabs,
        }
    );

    assert_eq!(
        graph
            .apply_op_checked(&DockOp::MergeFloatingInto {
                space: space(),
                floating,
                target_tabs: orphan_tabs,
            })
            .expect_err("merge target outside space should be reported"),
        DockOpApplyError::TargetNodeNotInSpace {
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
    assert!(graph.apply_op(&DockOp::FloatItemInWindow {
        source_space: space(),
        item: item("b"),
        target_space: space(),
        bounds: dock_bounds(10.0, 20.0, 300.0, 200.0),
    }));

    let floating = graph.floating_containers(&space())[0].node;
    assert!(graph.apply_op(&DockOp::MergeFloatingInto {
        space: space(),
        floating,
        target_tabs: root,
    }));

    assert!(graph.floating_containers(&space()).is_empty());
    assert_eq!(
        graph.collect_items_in_space(&space()),
        vec![item("a"), item("b")]
    );
    graph.assert_canonical_space(&space());
}
