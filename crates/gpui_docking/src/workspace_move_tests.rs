use crate::{
    DockActionApplyError, DockActionOutcome, DockClassId, DockFloatingContainer, DockGraph,
    DockGraphMutationError, DockMoveTarget, DockNode, DockPanelDescriptor, DockPolicyError,
    DockSpaceId, DropZone, SplitAxis, host_test_support::*,
};
use open_gpui::TestAppContext;

#[open_gpui::test]
fn workspace_move_tab_center_moves_item_between_stacks(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let outcome = workspace
        .commit_tab_move(
            &space(),
            left_tabs,
            &item("a"),
            &space(),
            DockMoveTarget::center(right_tabs),
        )
        .expect("tab move transaction should be valid");

    assert_eq!(outcome, DockActionOutcome::Changed);
    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(right_tabs)
        .expect("target tabs should still exist")
    else {
        panic!("target should be tabs");
    };
    assert_eq!(items, &vec![item("b"), item("a")]);
    assert_eq!(selected.as_ref(), items.get(1));
}

#[open_gpui::test]
fn workspace_move_tab_validates_declared_source_tabs(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let err = workspace
        .commit_tab_move(
            &space(),
            right_tabs,
            &item("a"),
            &space(),
            DockMoveTarget::center(right_tabs),
        )
        .expect_err("stale source tabs should not move an item from another stack");

    assert_eq!(
        err,
        DockActionApplyError::ItemNotInTabs {
            tabs: right_tabs,
            item: item("a"),
        }
    );
    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(left_tabs)
        .expect("source tabs should remain unchanged")
    else {
        panic!("source should be tabs");
    };
    assert_eq!(items, &vec![item("a")]);
    assert_eq!(selected.as_ref(), items.get(0));
}

#[open_gpui::test]
fn workspace_move_tab_rejects_source_tabs_outside_source_space(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let secondary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(space(), main_tabs);
    let secondary = DockSpaceId::from("secondary");
    graph.set_root(secondary.clone(), secondary_tabs);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let err = workspace
        .commit_tab_move(
            &space(),
            secondary_tabs,
            &item("b"),
            &space(),
            DockMoveTarget::center(main_tabs),
        )
        .expect_err("source tabs outside the declared source space should fail");

    assert_eq!(
        err,
        DockActionApplyError::Graph(DockGraphMutationError::SourceNodeNotInSpace {
            space: space(),
            node: secondary_tabs,
        })
    );
    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(secondary_tabs)
        .expect("secondary tabs should remain unchanged")
    else {
        panic!("secondary root should be tabs");
    };
    assert_eq!(items, &vec![item("b")]);
    assert_eq!(selected.as_ref(), items.get(0));
}

#[open_gpui::test]
fn workspace_move_tab_respects_target_space_dock_class_policy(cx: &mut TestAppContext) {
    let target = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(space(), source_tabs);
    graph.set_root(target.clone(), target_tabs);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);
    workspace.register_panel_descriptor(
        item("a"),
        DockPanelDescriptor::new("A").with_dock_class("editor"),
    );
    workspace
        .policy_mut()
        .allow_dock_class_in_space(target.clone(), "editor");

    let outcome = workspace
        .commit_tab_move(
            &space(),
            source_tabs,
            &item("a"),
            &target,
            DockMoveTarget::center(target_tabs),
        )
        .expect("matching class should be accepted");

    assert_eq!(outcome, DockActionOutcome::Changed);
    assert_eq!(
        workspace.graph().collect_items_in_space(&target),
        vec![item("b"), item("a")]
    );
}

#[open_gpui::test]
fn workspace_move_tab_rejects_incompatible_target_space_class(cx: &mut TestAppContext) {
    let target = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(space(), source_tabs);
    graph.set_root(target.clone(), target_tabs);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);
    workspace.register_panel_descriptor(
        item("a"),
        DockPanelDescriptor::new("A").with_dock_class("editor"),
    );
    workspace
        .policy_mut()
        .allow_dock_class_in_space(target.clone(), "inspector");

    let err = workspace
        .commit_tab_move(
            &space(),
            source_tabs,
            &item("a"),
            &target,
            DockMoveTarget::center(target_tabs),
        )
        .expect_err("incompatible class should be rejected before mutation");

    assert_eq!(
        err,
        DockActionApplyError::Policy(DockPolicyError::DockClassRejected {
            space: target.clone(),
            item: item("a"),
            dock_class: Some(DockClassId::from("editor")),
        })
    );
    assert_eq!(
        workspace.graph().collect_items_in_space(&space()),
        vec![item("a")]
    );
    assert_eq!(
        workspace.graph().collect_items_in_space(&target),
        vec![item("b")]
    );
}

#[open_gpui::test]
fn workspace_move_tabs_rejects_when_any_item_class_is_incompatible(cx: &mut TestAppContext) {
    let target = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(space(), source_tabs);
    graph.set_root(target.clone(), target_tabs);
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[("a", "A", "A"), ("b", "B", "B"), ("c", "C", "C")],
    );
    workspace.register_panel_descriptor(
        item("a"),
        DockPanelDescriptor::new("A").with_dock_class("editor"),
    );
    workspace.register_panel_descriptor(
        item("c"),
        DockPanelDescriptor::new("C").with_dock_class("inspector"),
    );
    workspace
        .policy_mut()
        .allow_dock_class_in_space(target.clone(), "inspector");

    let err = workspace
        .commit_tabs_move(
            &space(),
            source_tabs,
            &target,
            DockMoveTarget::center(target_tabs),
        )
        .expect_err("one incompatible item should reject the full stack");

    assert_eq!(
        err,
        DockActionApplyError::Policy(DockPolicyError::DockClassRejected {
            space: target.clone(),
            item: item("a"),
            dock_class: Some(DockClassId::from("editor")),
        })
    );
    assert_eq!(
        workspace.graph().collect_items_in_space(&space()),
        vec![item("a"), item("c")]
    );
    assert_eq!(
        workspace.graph().collect_items_in_space(&target),
        vec![item("b")]
    );
}

#[open_gpui::test]
fn workspace_move_floating_rejects_when_subtree_contains_incompatible_class(
    cx: &mut TestAppContext,
) {
    let target = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_root = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let target_root = graph.insert_node(DockNode::Tabs {
        items: vec![item("d")],
        selected: Some(item("d")),
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
        fractions: vec![0.5, 0.5],
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_child,
    });
    graph.set_root(space(), source_root);
    graph.set_root(target.clone(), target_root);
    graph
        .floating_containers_mut(space())
        .push(DockFloatingContainer {
            node: floating,
            bounds: floating_bounds(10.0, 20.0, 240.0, 160.0),
        });
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "A", "A"),
            ("b", "B", "B"),
            ("c", "C", "C"),
            ("d", "D", "D"),
        ],
    );
    workspace.register_panel_descriptor(
        item("a"),
        DockPanelDescriptor::new("A").with_dock_class("editor"),
    );
    workspace.register_panel_descriptor(
        item("c"),
        DockPanelDescriptor::new("C").with_dock_class("inspector"),
    );
    workspace
        .policy_mut()
        .allow_dock_class_in_space(target.clone(), "inspector");

    let err = workspace
        .commit_floating_move(
            &space(),
            floating,
            &target,
            DockMoveTarget::root_edge(target_root, DropZone::Right),
        )
        .expect_err("incompatible floating subtree should be rejected");

    assert_eq!(
        err,
        DockActionApplyError::Policy(DockPolicyError::DockClassRejected {
            space: target.clone(),
            item: item("a"),
            dock_class: Some(DockClassId::from("editor")),
        })
    );
    assert_eq!(workspace.graph().floating_containers(&space()).len(), 1);
    assert_eq!(
        workspace.graph().collect_items_in_space(&target),
        vec![item("d")]
    );
}

#[open_gpui::test]
fn workspace_move_item_to_empty_space_transaction_creates_detached_root(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"]);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);
    workspace.policy_mut().set_allow_platform_viewports(true);
    let detached = DockSpaceId::from("detached");

    let outcome = workspace
        .commit_item_to_empty_dock_space(&space(), &item("b"), &detached)
        .expect("move to empty dock space should be valid");

    assert_eq!(outcome, DockActionOutcome::Changed);
    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(root)
        .expect("source tabs should remain")
    else {
        panic!("source should be tabs");
    };
    assert_eq!(items, &vec![item("a")]);
    assert_eq!(selected.as_ref(), items.get(0));

    let detached_root = workspace
        .graph()
        .root(&detached)
        .expect("detached space should get root");
    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(detached_root)
        .expect("detached root should exist")
    else {
        panic!("detached root should be tabs");
    };
    assert_eq!(items, &vec![item("b")]);
    assert_eq!(selected.as_ref(), items.get(0));
    assert!(workspace.panels().contains(&item("b")));
}

#[open_gpui::test]
fn workspace_move_tabs_to_empty_space_transaction_preserves_stack(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
        selected: Some(item("c")),
    });
    let sibling_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![source_tabs, sibling_tabs],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(space(), root);
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[("a", "A", "A"), ("b", "B", "B"), ("c", "C", "C")],
    );
    workspace.policy_mut().set_allow_platform_viewports(true);
    let detached = DockSpaceId::from("detached");

    let outcome = workspace
        .commit_tabs_to_empty_dock_space(&space(), source_tabs, &detached)
        .expect("move tabs to empty dock space should be valid");

    assert_eq!(outcome, DockActionOutcome::Changed);
    assert_eq!(
        workspace.graph().collect_items_in_space(&space()),
        vec![item("b")]
    );
    let detached_root = workspace
        .graph()
        .root(&detached)
        .expect("detached space should get root");
    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(detached_root)
        .expect("detached root should exist")
    else {
        panic!("detached root should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("c")]);
    assert_eq!(selected.as_ref(), items.get(1));
}

#[open_gpui::test]
fn workspace_empty_space_transactions_reject_existing_target(cx: &mut TestAppContext) {
    let (mut graph, root) = tabs_graph(&["a", "b"]);
    let detached = DockSpaceId::from("detached");
    let detached_root = graph.insert_node(DockNode::Tabs {
        items: vec![item("existing")],
        selected: Some(item("existing")),
    });
    graph.set_root(detached.clone(), detached_root);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);
    workspace.policy_mut().set_allow_platform_viewports(true);

    let err = workspace
        .commit_item_to_empty_dock_space(&space(), &item("b"), &detached)
        .expect_err("non-empty target should be rejected");

    assert_eq!(
        err,
        DockActionApplyError::Graph(DockGraphMutationError::TargetSpaceNotEmpty {
            space: detached.clone()
        })
    );
    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(root)
        .expect("source tabs should remain")
    else {
        panic!("source should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(selected.as_ref(), items.get(0));
    assert_eq!(
        workspace.graph().collect_items_in_space(&detached),
        vec![item("existing")]
    );
}

#[open_gpui::test]
fn workspace_empty_space_transactions_reject_floating_only_target(cx: &mut TestAppContext) {
    let (mut graph, root) = tabs_graph(&["a", "b"]);
    let detached = DockSpaceId::from("detached");
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
            bounds: floating_bounds(10.0, 20.0, 240.0, 160.0),
        });
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);
    workspace.policy_mut().set_allow_platform_viewports(true);

    let item_err = workspace
        .commit_item_to_empty_dock_space(&space(), &item("b"), &detached)
        .expect_err("floating-only target should reject item moves");
    assert_eq!(
        item_err,
        DockActionApplyError::Graph(DockGraphMutationError::TargetSpaceNotEmpty {
            space: detached.clone()
        })
    );

    let tabs_err = workspace
        .commit_tabs_to_empty_dock_space(&space(), root, &detached)
        .expect_err("floating-only target should reject tabs moves");
    assert_eq!(
        tabs_err,
        DockActionApplyError::Graph(DockGraphMutationError::TargetSpaceNotEmpty {
            space: detached.clone()
        })
    );

    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(root)
        .expect("source tabs should remain")
    else {
        panic!("source should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(selected.as_ref(), items.get(0));
    assert!(workspace.graph().root(&detached).is_none());
    assert_eq!(workspace.graph().floating_containers(&detached).len(), 1);
    assert_eq!(
        workspace.graph().collect_items_in_space(&detached),
        vec![item("existing")]
    );
}

#[open_gpui::test]
fn workspace_empty_space_transactions_require_platform_viewport_policy(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"]);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);
    let detached = DockSpaceId::from("detached");

    let err = workspace
        .commit_item_to_empty_dock_space(&space(), &item("b"), &detached)
        .expect_err("platform viewport policy should block detached-space mutation");

    assert_eq!(
        err,
        DockActionApplyError::Policy(DockPolicyError::PlatformViewportsDisabled)
    );
    assert!(workspace.graph().root(&detached).is_none());
    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(root)
        .expect("source tabs should remain")
    else {
        panic!("source should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(selected.as_ref(), items.get(0));
}
#[open_gpui::test]
fn workspace_same_stack_center_drop_is_noop(cx: &mut TestAppContext) {
    let (graph, tabs) = tabs_graph(&["a", "b"]);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let outcome = workspace
        .commit_tab_move(
            &space(),
            tabs,
            &item("a"),
            &space(),
            DockMoveTarget::center(tabs),
        )
        .expect("same-stack center drop should be valid");

    assert_eq!(outcome, DockActionOutcome::Unchanged);
    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(tabs)
        .expect("tabs should still exist")
    else {
        panic!("target should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(selected.as_ref(), items.get(0));
}

#[open_gpui::test]
fn workspace_same_stack_center_drop_reorders_with_insert_index(cx: &mut TestAppContext) {
    let (graph, tabs) = tabs_graph(&["a", "b", "c"]);
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[("a", "A", "A"), ("b", "B", "B"), ("c", "C", "C")],
    );

    let outcome = workspace
        .commit_tab_move(
            &space(),
            tabs,
            &item("a"),
            &space(),
            DockMoveTarget::tab_bar(tabs, 3),
        )
        .expect("same-stack center drop with an index should reorder");

    assert_eq!(outcome, DockActionOutcome::Changed);
    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(tabs)
        .expect("tabs should still exist")
    else {
        panic!("target should be tabs");
    };
    assert_eq!(items, &vec![item("b"), item("c"), item("a")]);
    assert_eq!(selected.as_ref(), items.get(2));
}
