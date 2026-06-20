use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockClassId, DockFloatingContainer,
    DockGraph, DockGraphDropTarget, DockGraphMutationError, DockNode, DockPanelDescriptor,
    DockPolicyError, DockSpaceId, DropZone, SplitAxis, host_test_support::*,
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
            DockGraphDropTarget::center(right_tabs),
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
            DockGraphDropTarget::center(right_tabs),
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
fn workspace_move_tabs_center_selects_moved_stack_selected_item(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b"), item("c")],
        selected: Some(item("c")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("target-a"), item("target-b")],
        selected: Some(item("target-b")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![source_tabs, target_tabs],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(space(), root);
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "A", "A"),
            ("b", "B", "B"),
            ("c", "C", "C"),
            ("target-a", "Target A", "Target A"),
            ("target-b", "Target B", "Target B"),
        ],
    );
    workspace
        .commit_select_tab(target_tabs, &item("target-a"))
        .expect("target stack should record local MRU");
    workspace
        .commit_select_tab(source_tabs, &item("a"))
        .expect("source stack should record source-local MRU");
    workspace
        .commit_select_tab(source_tabs, &item("c"))
        .expect("source stack should restore its selected tab before moving");

    workspace
        .commit_tabs_move(
            &space(),
            source_tabs,
            &space(),
            DockGraphDropTarget::center(target_tabs),
        )
        .expect("moving source tabs into target stack should be valid");
    workspace.commit_close_item(&space(), &item("c")).expect(
        "closing moved selected tab should use remaining target-local history or structure",
    );

    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(target_tabs)
        .expect("target tabs should remain")
    else {
        panic!("target should be tabs");
    };
    assert_eq!(
        items,
        &vec![item("target-a"), item("target-b"), item("a"), item("b")]
    );
    assert_eq!(
        selected.as_ref(),
        Some(&item("target-a")),
        "source tabbar MRU must not be imported into the target tabbar"
    );
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
            DockGraphDropTarget::center(main_tabs),
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
            DockGraphDropTarget::center(target_tabs),
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
            DockGraphDropTarget::center(target_tabs),
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
            DockGraphDropTarget::center(target_tabs),
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
            DockGraphDropTarget::edge(
                workspace
                    .graph()
                    .edge_dock_plan(&target, target_root, DropZone::Right)
                    .expect("edge target should be plannable"),
            ),
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
fn workspace_merge_space_preserves_floating_forest(cx: &mut TestAppContext) {
    let target = DockSpaceId::from("target");
    let detached = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("target")],
        selected: Some(item("target")),
    });
    let detached_root = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("b")),
    });
    let floating_left = graph.insert_node(DockNode::Tabs {
        items: vec![item("float-a")],
        selected: Some(item("float-a")),
    });
    let floating_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("float-b")],
        selected: Some(item("float-b")),
    });
    let floating_child = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Vertical,
        children: vec![floating_left, floating_right],
        fractions: vec![0.4, 0.6],
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_child,
    });
    let floating_bounds = floating_bounds(16.0, 24.0, 240.0, 160.0);
    graph.set_root(target.clone(), target_tabs);
    graph.set_root(detached.clone(), detached_root);
    graph
        .floating_containers_mut(detached.clone())
        .push(DockFloatingContainer {
            node: floating,
            bounds: floating_bounds,
        });

    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("target", "Target", "Target"),
            ("a", "A", "A"),
            ("b", "B", "B"),
            ("float-a", "Float A", "Float A"),
            ("float-b", "Float B", "Float B"),
        ],
    );
    workspace.policy_mut().set_allow_platform_viewports(true);

    let outcome = workspace
        .commit_merge_space_into(&detached, &target)
        .expect("merge-back should preserve detached floating trees");

    assert_eq!(outcome, DockActionOutcome::Changed);
    assert_eq!(workspace.graph().root(&detached), None);
    assert!(workspace.graph().floating_containers(&detached).is_empty());
    let target_floatings = workspace.graph().floating_containers(&target);
    assert_eq!(target_floatings.len(), 1);
    assert_eq!(target_floatings[0].bounds, floating_bounds);
    let moved_floating = target_floatings[0].node;
    assert!(
        matches!(workspace.graph().node(moved_floating), Some(DockNode::Floating { child }) if *child == floating_child)
    );
    assert_eq!(
        workspace.graph().collect_items_in_subtree(moved_floating),
        vec![item("float-a"), item("float-b")]
    );
    assert_eq!(
        workspace.graph().collect_items_in_space(&target),
        vec![
            item("target"),
            item("a"),
            item("b"),
            item("float-a"),
            item("float-b")
        ]
    );
}

#[open_gpui::test]
fn workspace_merge_space_rejects_non_unique_target_tabs(cx: &mut TestAppContext) {
    let target = DockSpaceId::from("target");
    let detached = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let target_left = graph.insert_node(DockNode::Tabs {
        items: vec![item("target-left")],
        selected: Some(item("target-left")),
    });
    let target_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("target-right")],
        selected: Some(item("target-right")),
    });
    let target_root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![target_left, target_right],
        fractions: vec![0.5, 0.5],
    });
    let detached_root = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(target.clone(), target_root);
    graph.set_root(detached.clone(), detached_root);

    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("target-left", "Target Left", "Target Left"),
            ("target-right", "Target Right", "Target Right"),
            ("a", "A", "A"),
        ],
    );
    workspace.policy_mut().set_allow_platform_viewports(true);

    let err = workspace
        .commit_merge_space_into(&detached, &target)
        .expect_err("merge-back must not pick a target tabs stack by tree order");

    assert_eq!(
        err,
        DockActionApplyError::Graph(DockGraphMutationError::MergeTargetTabsNotUnique {
            space: target.clone(),
            tabs_len: 2,
        })
    );
    assert_eq!(workspace.graph().root(&detached), Some(detached_root));
    assert_eq!(workspace.graph().root(&target), Some(target_root));
    assert_eq!(
        workspace.graph().collect_items_in_space(&target),
        vec![item("target-left"), item("target-right")]
    );
    assert_eq!(
        workspace.graph().collect_items_in_space(&detached),
        vec![item("a")]
    );
}

#[open_gpui::test]
fn workspace_merge_space_uses_explicit_target_tabs_in_split_target(cx: &mut TestAppContext) {
    let target = DockSpaceId::from("target");
    let detached = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let target_left = graph.insert_node(DockNode::Tabs {
        items: vec![item("target-left")],
        selected: Some(item("target-left")),
    });
    let target_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("target-right")],
        selected: Some(item("target-right")),
    });
    let target_root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![target_left, target_right],
        fractions: vec![0.5, 0.5],
    });
    let detached_root = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(target.clone(), target_root);
    graph.set_root(detached.clone(), detached_root);

    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("target-left", "Target Left", "Target Left"),
            ("target-right", "Target Right", "Target Right"),
            ("a", "A", "A"),
            ("b", "B", "B"),
        ],
    );
    workspace.policy_mut().set_allow_platform_viewports(true);

    let outcome = workspace
        .commit_merge_space_into_tabs(&detached, &target, target_right)
        .expect("explicit merge target should commit into requested tabs");

    assert_eq!(outcome, DockActionOutcome::Changed);
    assert_eq!(workspace.graph().root(&detached), None);
    let DockNode::Tabs {
        items: left_items,
        selected: left_selected,
    } = workspace
        .graph()
        .node(target_left)
        .expect("left target tabs should remain")
    else {
        panic!("left target should be tabs");
    };
    assert_eq!(left_items, &vec![item("target-left")]);
    assert_eq!(left_selected.as_ref(), left_items.first());

    let DockNode::Tabs {
        items: right_items,
        selected: right_selected,
    } = workspace
        .graph()
        .node(target_right)
        .expect("right target tabs should receive the source root")
    else {
        panic!("right target should be tabs");
    };
    assert_eq!(
        right_items,
        &vec![item("target-right"), item("a"), item("b")]
    );
    assert_eq!(right_selected.as_ref(), right_items.get(2));
}

#[open_gpui::test]
fn workspace_merge_space_rejects_recently_selected_root_tabs_when_target_has_multiple_roots(
    cx: &mut TestAppContext,
) {
    let target = DockSpaceId::from("target");
    let detached = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let target_left = graph.insert_node(DockNode::Tabs {
        items: vec![item("left-a"), item("left-b")],
        selected: Some(item("left-b")),
    });
    let target_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("right-a"), item("right-b")],
        selected: Some(item("right-b")),
    });
    let target_root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![target_left, target_right],
        fractions: vec![0.5, 0.5],
    });
    let detached_root = graph.insert_node(DockNode::Tabs {
        items: vec![item("x")],
        selected: Some(item("x")),
    });
    graph.set_root(target.clone(), target_root);
    graph.set_root(detached.clone(), detached_root);

    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("left-a", "Left A", "Left A"),
            ("left-b", "Left B", "Left B"),
            ("right-a", "Right A", "Right A"),
            ("right-b", "Right B", "Right B"),
            ("x", "X", "X"),
        ],
    );
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace
        .apply_action(&DockAction::SelectTab {
            tabs: target_left,
            item: item("left-a"),
        })
        .expect("left tabs should record a selection stamp");
    workspace
        .apply_action(&DockAction::SelectTab {
            tabs: target_right,
            item: item("right-a"),
        })
        .expect("right tabs should record a selection stamp");

    let err = workspace
        .commit_merge_space_into(&detached, &target)
        .expect_err("merge-back must not infer a target tabs stack from tab MRU");

    assert_eq!(
        err,
        DockActionApplyError::Graph(DockGraphMutationError::MergeTargetTabsNotUnique {
            space: target.clone(),
            tabs_len: 2,
        })
    );
    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(target_left)
        .expect("left target tabs should remain untouched")
    else {
        panic!("left target should be tabs");
    };
    assert_eq!(items, &vec![item("left-a"), item("left-b")]);
    assert_eq!(selected.as_ref(), Some(&item("left-a")));
    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(target_right)
        .expect("right target tabs should remain untouched")
    else {
        panic!("right target should be tabs");
    };
    assert_eq!(items, &vec![item("right-a"), item("right-b")]);
    assert_eq!(selected.as_ref(), Some(&item("right-a")));
    assert_eq!(workspace.graph().root(&detached), Some(detached_root));
}

#[open_gpui::test]
fn workspace_merge_space_preserves_target_tab_mru_for_followup_close(cx: &mut TestAppContext) {
    let target = DockSpaceId::from("target");
    let detached = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b"), item("c")],
        selected: Some(item("c")),
    });
    let detached_root = graph.insert_node(DockNode::Tabs {
        items: vec![item("x")],
        selected: Some(item("x")),
    });
    graph.set_root(target.clone(), target_tabs);
    graph.set_root(detached.clone(), detached_root);

    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "A", "A"),
            ("b", "B", "B"),
            ("c", "C", "C"),
            ("x", "X", "X"),
        ],
    );
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace
        .commit_select_tab(target_tabs, &item("b"))
        .expect("selecting target tab should record tab MRU");

    workspace
        .commit_merge_space_into(&detached, &target)
        .expect("merge-back should append detached tab into target stack");
    workspace
        .commit_close_item(&target, &item("x"))
        .expect("closing merged selected tab should use preserved target MRU");

    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(target_tabs)
        .expect("target tabs should remain")
    else {
        panic!("target should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b"), item("c")]);
    assert_eq!(selected.as_ref(), Some(&item("b")));
}

#[open_gpui::test]
fn workspace_merge_space_does_not_transfer_source_tab_mru_into_target_tabs(
    cx: &mut TestAppContext,
) {
    let target = DockSpaceId::from("target");
    let detached = DockSpaceId::from("detached");
    let mut graph = DockGraph::new();
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("target-a"), item("target-b")],
        selected: Some(item("target-b")),
    });
    let detached_root = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b"), item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(target.clone(), target_tabs);
    graph.set_root(detached.clone(), detached_root);

    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("target-a", "Target A", "Target A"),
            ("target-b", "Target B", "Target B"),
            ("a", "A", "A"),
            ("b", "B", "B"),
            ("c", "C", "C"),
        ],
    );
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace
        .commit_select_tab(target_tabs, &item("target-a"))
        .expect("target tabs should record local MRU");
    workspace
        .commit_select_tab(detached_root, &item("a"))
        .expect("source tabs should record source-local MRU");
    workspace
        .commit_select_tab(detached_root, &item("c"))
        .expect("source tabs should restore its selected tab before merge");

    workspace
        .commit_merge_space_into(&detached, &target)
        .expect("source root tabs should merge into target tabs");
    workspace
        .commit_close_item(&target, &item("c"))
        .expect("closing merged selected tab should use target-local MRU");

    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(target_tabs)
        .expect("target tabs should remain")
    else {
        panic!("target should be tabs");
    };
    assert_eq!(
        items,
        &vec![item("target-a"), item("target-b"), item("a"), item("b")]
    );
    assert_eq!(
        selected.as_ref(),
        Some(&item("target-a")),
        "source tabbar MRU must not be imported into the target tabbar"
    );
}

#[open_gpui::test]
fn workspace_merge_space_root_into_empty_target_preserves_source_tab_mru(cx: &mut TestAppContext) {
    let source = DockSpaceId::from("source");
    let target = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b"), item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(source.clone(), source_tabs);

    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[("a", "A", "A"), ("b", "B", "B"), ("c", "C", "C")],
    );
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace
        .commit_select_tab(source_tabs, &item("a"))
        .expect("source tabs should record MRU before merge");

    workspace
        .commit_merge_space_into(&source, &target)
        .expect("merge-back into an empty target should preserve source root tabs");
    workspace
        .commit_close_item(&target, &item("c"))
        .expect("closing selected tab should use preserved source MRU");

    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(source_tabs)
        .expect("moved root tabs should keep its node id")
    else {
        panic!("target root should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(selected.as_ref(), Some(&item("a")));
}

#[open_gpui::test]
fn workspace_merge_space_floating_forest_preserves_floating_tab_mru(cx: &mut TestAppContext) {
    let source = DockSpaceId::from("source");
    let target = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("target")],
        selected: Some(item("target")),
    });
    let floating_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b"), item("c")],
        selected: Some(item("c")),
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_tabs,
    });
    graph.set_root(target.clone(), target_tabs);
    graph
        .floating_containers_mut(source.clone())
        .push(DockFloatingContainer {
            node: floating,
            bounds: floating_bounds(16.0, 24.0, 240.0, 160.0),
        });

    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("target", "Target", "Target"),
            ("a", "A", "A"),
            ("b", "B", "B"),
            ("c", "C", "C"),
        ],
    );
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace
        .commit_select_tab(floating_tabs, &item("a"))
        .expect("floating tabs should record MRU before merge");

    workspace
        .commit_merge_space_into(&source, &target)
        .expect("floating-only source space should merge its floating forest");
    workspace
        .commit_close_item(&target, &item("c"))
        .expect("closing floating selected tab should use preserved floating MRU");

    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(floating_tabs)
        .expect("moved floating tabs should keep its node id")
    else {
        panic!("floating child should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(selected.as_ref(), Some(&item("a")));
}

#[open_gpui::test]
fn workspace_move_floating_center_selects_moved_stack_selected_item(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("target-a"), item("target-b")],
        selected: Some(item("target-b")),
    });
    let floating_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b"), item("c")],
        selected: Some(item("c")),
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_tabs,
    });
    graph.set_root(space(), target_tabs);
    graph
        .floating_containers_mut(space())
        .push(DockFloatingContainer {
            node: floating,
            bounds: floating_bounds(16.0, 24.0, 240.0, 160.0),
        });

    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("target-a", "Target A", "Target A"),
            ("target-b", "Target B", "Target B"),
            ("a", "A", "A"),
            ("b", "B", "B"),
            ("c", "C", "C"),
        ],
    );
    workspace
        .commit_select_tab(target_tabs, &item("target-a"))
        .expect("target stack should record local MRU");
    workspace
        .commit_select_tab(floating_tabs, &item("a"))
        .expect("floating tabs should record source-local MRU");
    workspace
        .commit_select_tab(floating_tabs, &item("c"))
        .expect("floating tabs should restore its selected tab before moving");

    workspace
        .commit_floating_move(
            &space(),
            floating,
            &space(),
            DockGraphDropTarget::center(target_tabs),
        )
        .expect("floating tabs should move into target tabs");
    workspace.commit_close_item(&space(), &item("c")).expect(
        "closing moved selected tab should use remaining target-local history or structure",
    );

    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(target_tabs)
        .expect("target tabs should remain")
    else {
        panic!("target should be tabs");
    };
    assert_eq!(
        items,
        &vec![item("target-a"), item("target-b"), item("a"), item("b")]
    );
    assert_eq!(
        selected.as_ref(),
        Some(&item("target-a")),
        "floating tabbar MRU must not be imported into the target tabbar"
    );
}

#[open_gpui::test]
fn workspace_merge_space_preserves_root_split_tree_on_empty_target(cx: &mut TestAppContext) {
    let detached = DockSpaceId::from("detached");
    let target = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let detached_left = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let detached_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let detached_root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![detached_left, detached_right],
        fractions: vec![0.25, 0.75],
    });
    graph.set_root(detached.clone(), detached_root);

    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);
    workspace.policy_mut().set_allow_platform_viewports(true);

    let outcome = workspace
        .commit_merge_space_into(&detached, &target)
        .expect("merge-back should preserve the detached split root tree");

    assert_eq!(outcome, DockActionOutcome::Changed);
    assert_eq!(workspace.graph().root(&detached), None);

    let target_root = workspace
        .graph()
        .root(&target)
        .expect("target space should receive the detached root");
    assert_eq!(target_root, detached_root);
    let DockNode::Split {
        axis,
        children,
        fractions,
    } = workspace
        .graph()
        .node(target_root)
        .expect("target root should still be a split")
    else {
        panic!("target root should be split");
    };
    assert_eq!(*axis, SplitAxis::Horizontal);
    assert_eq!(children, &vec![detached_left, detached_right]);
    assert_eq!(fractions, &vec![0.25, 0.75]);

    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(detached_left)
        .expect("left child should remain tabs")
    else {
        panic!("left child should be tabs");
    };
    assert_eq!(items, &vec![item("a")]);
    assert_eq!(selected.as_ref(), items.get(0));

    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(detached_right)
        .expect("right child should remain tabs")
    else {
        panic!("right child should be tabs");
    };
    assert_eq!(items, &vec![item("b")]);
    assert_eq!(selected.as_ref(), items.get(0));

    assert_eq!(
        workspace.graph().collect_items_in_space(&target),
        vec![item("a"), item("b")]
    );
}

#[open_gpui::test]
fn workspace_merge_space_rejects_visible_split_root_into_non_empty_target(cx: &mut TestAppContext) {
    let detached = DockSpaceId::from("detached");
    let target = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("target")],
        selected: Some(item("target")),
    });
    let detached_left = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let detached_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let detached_root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![detached_left, detached_right],
        fractions: vec![0.25, 0.75],
    });
    graph.set_root(target.clone(), target_tabs);
    graph.set_root(detached.clone(), detached_root);

    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("target", "Target", "Target"),
            ("a", "A", "A"),
            ("b", "B", "B"),
        ],
    );
    workspace.policy_mut().set_allow_platform_viewports(true);

    let err = workspace
        .commit_merge_space_into(&detached, &target)
        .expect_err("visible split payload should not be flattened into a non-empty center");

    assert_eq!(
        err,
        DockActionApplyError::Graph(
            DockGraphMutationError::VisibleSplitPayloadCannotDockOverNonEmptyTarget {
                payload: detached_root,
                target: target_tabs,
            },
        )
    );
    assert_eq!(workspace.graph().root(&detached), Some(detached_root));
    assert_eq!(workspace.graph().root(&target), Some(target_tabs));
    assert_eq!(
        workspace.graph().collect_items_in_space(&target),
        vec![item("target")]
    );
    assert_eq!(
        workspace.graph().collect_items_in_space(&detached),
        vec![item("a"), item("b")]
    );
}

#[open_gpui::test]
fn workspace_merge_space_rejects_wrapped_visible_split_root_into_non_empty_target(
    cx: &mut TestAppContext,
) {
    let detached = DockSpaceId::from("detached");
    let target = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("target")],
        selected: Some(item("target")),
    });
    let detached_left = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let detached_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let detached_split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![detached_left, detached_right],
        fractions: vec![0.25, 0.75],
    });
    let detached_root = graph.insert_node(DockNode::Floating {
        child: detached_split,
    });
    graph.set_root(target.clone(), target_tabs);
    graph.set_root(detached.clone(), detached_root);

    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("target", "Target", "Target"),
            ("a", "A", "A"),
            ("b", "B", "B"),
        ],
    );
    workspace.policy_mut().set_allow_platform_viewports(true);

    let err = workspace
        .commit_merge_space_into(&detached, &target)
        .expect_err(
            "wrapped visible split payload should not be flattened into a non-empty center",
        );

    assert_eq!(
        err,
        DockActionApplyError::Graph(
            DockGraphMutationError::VisibleSplitPayloadCannotDockOverNonEmptyTarget {
                payload: detached_root,
                target: target_tabs,
            },
        )
    );
    assert_eq!(workspace.graph().root(&detached), Some(detached_root));
    assert_eq!(workspace.graph().root(&target), Some(target_tabs));
}

#[open_gpui::test]
fn workspace_merge_space_rejects_incompatible_target_space_class(cx: &mut TestAppContext) {
    let detached = DockSpaceId::from("detached");
    let target = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let detached_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(detached.clone(), detached_tabs);
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
        .commit_merge_space_into(&detached, &target)
        .expect_err("merge-back must respect target-space dock class policy");

    assert_eq!(
        err,
        DockActionApplyError::Policy(DockPolicyError::DockClassRejected {
            space: target.clone(),
            item: item("a"),
            dock_class: Some(DockClassId::from("editor")),
        })
    );
    assert_eq!(workspace.graph().root(&detached), Some(detached_tabs));
    assert_eq!(workspace.graph().root(&target), Some(target_tabs));
    assert_eq!(
        workspace.graph().collect_items_in_space(&detached),
        vec![item("a")]
    );
    assert_eq!(
        workspace.graph().collect_items_in_space(&target),
        vec![item("b")]
    );
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
            DockGraphDropTarget::center(tabs),
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
fn workspace_same_stack_tab_bar_drop_reorders(cx: &mut TestAppContext) {
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
            DockGraphDropTarget::tab_bar(tabs, 3),
        )
        .expect("same-stack tab-bar drop should reorder");

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
