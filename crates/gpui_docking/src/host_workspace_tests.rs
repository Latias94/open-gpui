use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockGraph, DockLayoutNode, DockNode,
    DockNodeId, DockOpApplyError, DockPanel, DockPolicyError, DockSpaceId, DockWorkspace, DropZone,
    SplitAxis, host_test_support::*,
};
use open_gpui::{AppContext as _, TestAppContext};
use slotmap::Key;
use std::{cell::Cell, rc::Rc};

#[open_gpui::test]
fn workspace_applies_actions_and_preserves_registered_panels(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_view(item("a"), "A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "B", test_view(cx, "B"));

    let outcome = workspace
        .apply_action(&DockAction::SelectTab {
            tabs: root,
            item: item("b"),
        })
        .expect("active tab mutation should be valid");

    let DockNode::Tabs { active, .. } = workspace
        .graph()
        .node(root)
        .expect("tabs should still exist")
    else {
        panic!("root should be tabs");
    };
    assert_eq!(outcome, DockActionOutcome::Changed);
    assert_eq!(*active, 1);
    assert!(workspace.panels().contains(&item("a")));
    assert!(workspace.panels().contains(&item("b")));
}

#[open_gpui::test]
fn workspace_selecting_active_tab_is_noop(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 1);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let outcome = workspace
        .apply_action(&DockAction::SelectTab {
            tabs: root,
            item: item("b"),
        })
        .expect("active tab selection should be valid");

    assert_eq!(outcome, DockActionOutcome::Unchanged);
    assert!(workspace.panels().contains(&item("a")));
    assert!(workspace.panels().contains(&item("b")));
}

#[open_gpui::test]
fn workspace_rejects_invalid_select_tab_actions(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let missing_item = workspace
        .apply_action(&DockAction::SelectTab {
            tabs: root,
            item: item("missing"),
        })
        .expect_err("missing tab item should fail");
    assert_eq!(
        missing_item,
        DockActionApplyError::ItemNotInTabs {
            tabs: root,
            item: item("missing")
        }
    );

    let wrong_node = workspace
        .apply_action(&DockAction::SelectTab {
            tabs: DockNodeId::null(),
            item: item("a"),
        })
        .expect_err("missing tabs node should fail");
    assert_eq!(
        wrong_node,
        DockActionApplyError::Graph(DockOpApplyError::TabsNodeNotFound {
            tabs: DockNodeId::null()
        })
    );

    let DockNode::Tabs { active, .. } = workspace
        .graph()
        .node(root)
        .expect("tabs should still exist")
    else {
        panic!("root should be tabs");
    };
    assert_eq!(*active, 0);
    assert!(workspace.panels().contains(&item("a")));
    assert!(workspace.panels().contains(&item("b")));
}

#[open_gpui::test]
fn workspace_action_layout_export_remains_graph_only(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[("a", "Panel A", "Panel A"), ("b", "Panel B", "Panel B")],
    );

    let outcome = workspace
        .apply_action(&DockAction::SelectTab {
            tabs: root,
            item: item("b"),
        })
        .expect("active tab mutation should be valid");
    assert_eq!(outcome, DockActionOutcome::Changed);

    let layout = workspace.graph().export_layout();
    layout.validate().expect("exported layout should validate");
    let json = serde_json::to_string(&layout).expect("layout should serialize");

    assert!(!json.contains("Panel A"));
    assert!(!json.contains("Panel B"));
    assert!(!json.contains("AnyView"));
    assert!(!json.contains("Entity"));
    assert!(!json.contains("WindowHandle"));

    let DockLayoutNode::Tabs { active, items, .. } = layout
        .nodes
        .iter()
        .find(|node| matches!(node, DockLayoutNode::Tabs { .. }))
        .expect("layout should contain tabs node")
    else {
        panic!("expected tabs node");
    };
    assert_eq!(*active, 1);
    assert_eq!(items, &vec![item("a"), item("b")]);

    let imported = DockGraph::import_layout(&layout).expect("layout should import");
    let imported_root = imported.root(&space()).expect("space should keep root");
    let DockNode::Tabs { active, items } = imported
        .node(imported_root)
        .expect("imported root should exist")
    else {
        panic!("imported root should be tabs");
    };
    assert_eq!(*active, 1);
    assert_eq!(items, &vec![item("a"), item("b")]);
}

#[open_gpui::test]
fn workspace_move_tab_center_moves_item_between_stacks(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let outcome = workspace
        .apply_action(&DockAction::MoveTab {
            source_space: space(),
            source_tabs: left_tabs,
            item: item("a"),
            target_space: space(),
            target_tabs: right_tabs,
            zone: DropZone::Center,
            insert_index: None,
        })
        .expect("move tab action should be valid");

    assert_eq!(outcome, DockActionOutcome::Changed);
    let DockNode::Tabs { items, active } = workspace
        .graph()
        .node(right_tabs)
        .expect("target tabs should still exist")
    else {
        panic!("target should be tabs");
    };
    assert_eq!(items, &vec![item("b"), item("a")]);
    assert_eq!(*active, 1);
}

#[open_gpui::test]
fn workspace_move_tab_validates_declared_source_tabs(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let err = workspace
        .apply_action(&DockAction::MoveTab {
            source_space: space(),
            source_tabs: right_tabs,
            item: item("a"),
            target_space: space(),
            target_tabs: right_tabs,
            zone: DropZone::Center,
            insert_index: None,
        })
        .expect_err("stale source tabs should not move an item from another stack");

    assert_eq!(
        err,
        DockActionApplyError::ItemNotInTabs {
            tabs: right_tabs,
            item: item("a"),
        }
    );
    let DockNode::Tabs { items, active } = workspace
        .graph()
        .node(left_tabs)
        .expect("source tabs should remain unchanged")
    else {
        panic!("source should be tabs");
    };
    assert_eq!(items, &vec![item("a")]);
    assert_eq!(*active, 0);
}

#[open_gpui::test]
fn workspace_move_tab_rejects_source_tabs_outside_source_space(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let main_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    let secondary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(space(), main_tabs);
    let secondary = DockSpaceId::from("secondary");
    graph.set_root(secondary.clone(), secondary_tabs);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let err = workspace
        .apply_action(&DockAction::MoveTab {
            source_space: space(),
            source_tabs: secondary_tabs,
            item: item("b"),
            target_space: space(),
            target_tabs: main_tabs,
            zone: DropZone::Center,
            insert_index: None,
        })
        .expect_err("source tabs outside the declared source space should fail");

    assert_eq!(
        err,
        DockActionApplyError::Graph(DockOpApplyError::SourceNodeNotInSpace {
            space: space(),
            node: secondary_tabs,
        })
    );
    let DockNode::Tabs { items, active } = workspace
        .graph()
        .node(secondary_tabs)
        .expect("secondary tabs should remain unchanged")
    else {
        panic!("secondary root should be tabs");
    };
    assert_eq!(items, &vec![item("b")]);
    assert_eq!(*active, 0);
}

#[open_gpui::test]
fn workspace_move_item_to_empty_space_action_creates_detached_root(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);
    workspace.policy_mut().set_allow_platform_viewports(true);
    let detached = DockSpaceId::from("detached");

    let outcome = workspace
        .apply_action(&DockAction::MoveItemToEmptyDockSpace {
            source_space: space(),
            item: item("b"),
            target_space: detached.clone(),
        })
        .expect("move to empty dock space should be valid");

    assert_eq!(outcome, DockActionOutcome::Changed);
    let DockNode::Tabs { items, active } = workspace
        .graph()
        .node(root)
        .expect("source tabs should remain")
    else {
        panic!("source should be tabs");
    };
    assert_eq!(items, &vec![item("a")]);
    assert_eq!(*active, 0);

    let detached_root = workspace
        .graph()
        .root(&detached)
        .expect("detached space should get root");
    let DockNode::Tabs { items, active } = workspace
        .graph()
        .node(detached_root)
        .expect("detached root should exist")
    else {
        panic!("detached root should be tabs");
    };
    assert_eq!(items, &vec![item("b")]);
    assert_eq!(*active, 0);
    assert!(workspace.panels().contains(&item("b")));
}

#[open_gpui::test]
fn workspace_move_tabs_to_empty_space_action_preserves_stack(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
        active: 1,
    });
    let sibling_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
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
        .apply_action(&DockAction::MoveTabsToEmptyDockSpace {
            source_space: space(),
            source_tabs,
            target_space: detached.clone(),
        })
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
    let DockNode::Tabs { items, active } = workspace
        .graph()
        .node(detached_root)
        .expect("detached root should exist")
    else {
        panic!("detached root should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("c")]);
    assert_eq!(*active, 1);
}

#[open_gpui::test]
fn workspace_empty_space_actions_reject_existing_target(cx: &mut TestAppContext) {
    let (mut graph, root) = tabs_graph(&["a", "b"], 0);
    let detached = DockSpaceId::from("detached");
    let detached_root = graph.insert_node(DockNode::Tabs {
        items: vec![item("existing")],
        active: 0,
    });
    graph.set_root(detached.clone(), detached_root);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);
    workspace.policy_mut().set_allow_platform_viewports(true);

    let err = workspace
        .apply_action(&DockAction::MoveItemToEmptyDockSpace {
            source_space: space(),
            item: item("b"),
            target_space: detached.clone(),
        })
        .expect_err("non-empty target should be rejected");

    assert_eq!(
        err,
        DockActionApplyError::Graph(DockOpApplyError::TargetSpaceNotEmpty {
            space: detached.clone()
        })
    );
    let DockNode::Tabs { items, active } = workspace
        .graph()
        .node(root)
        .expect("source tabs should remain")
    else {
        panic!("source should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(*active, 0);
    assert_eq!(
        workspace.graph().collect_items_in_space(&detached),
        vec![item("existing")]
    );
}

#[open_gpui::test]
fn workspace_empty_space_actions_require_platform_viewport_policy(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);
    let detached = DockSpaceId::from("detached");

    let err = workspace
        .apply_action(&DockAction::MoveItemToEmptyDockSpace {
            source_space: space(),
            item: item("b"),
            target_space: detached.clone(),
        })
        .expect_err("platform viewport policy should block detached-space mutation");

    assert_eq!(
        err,
        DockActionApplyError::Policy(DockPolicyError::PlatformViewportsDisabled)
    );
    assert!(workspace.graph().root(&detached).is_none());
    let DockNode::Tabs { items, active } = workspace
        .graph()
        .node(root)
        .expect("source tabs should remain")
    else {
        panic!("source should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(*active, 0);
}

#[open_gpui::test]
fn workspace_close_item_action_respects_panel_policy(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel(
        item("a"),
        DockPanel::new("A", test_view(cx, "A")).closable(false),
    );
    workspace.register_panel_view(item("b"), "B", test_view(cx, "B"));

    let err = workspace
        .apply_action(&DockAction::CloseItem {
            space: space(),
            item: item("a"),
        })
        .expect_err("non-closable panel should block close");
    assert_eq!(
        err,
        DockActionApplyError::PanelNotClosable { item: item("a") }
    );

    let outcome = workspace
        .apply_action(&DockAction::CloseItem {
            space: space(),
            item: item("b"),
        })
        .expect("closable panel should close");
    assert_eq!(outcome, DockActionOutcome::Changed);
    let DockNode::Tabs { items, active } = workspace
        .graph()
        .node(root)
        .expect("source tabs should remain")
    else {
        panic!("source should be tabs");
    };
    assert_eq!(items, &vec![item("a")]);
    assert_eq!(*active, 0);
    assert!(workspace.panels().contains(&item("b")));
}

#[open_gpui::test]
fn workspace_close_item_action_uses_metadata_without_instantiating_lazy_panel(
    _cx: &mut TestAppContext,
) {
    let (graph, root) = tabs_graph(&["lazy"], 0);
    let mut workspace = DockWorkspace::new(space(), graph);
    let instantiations = Rc::new(Cell::new(0));
    let observed_instantiations = instantiations.clone();
    workspace.register_panel_factory(item("lazy"), "Lazy", move |cx| {
        instantiations.set(instantiations.get() + 1);
        cx.new(|_| TestPanel { label: "Lazy" }).into()
    });

    let outcome = workspace
        .apply_action(&DockAction::CloseItem {
            space: space(),
            item: item("lazy"),
        })
        .expect("closable lazy panel should close from metadata");

    assert_eq!(outcome, DockActionOutcome::Changed);
    assert_eq!(observed_instantiations.get(), 0);
    assert!(
        !workspace
            .panels()
            .get(&item("lazy"))
            .expect("panel registration should remain available")
            .has_view()
    );
    let DockNode::Tabs { items, .. } = workspace
        .graph()
        .node(root)
        .expect("source tabs should remain")
    else {
        panic!("source should be tabs");
    };
    assert!(items.is_empty());
}

#[open_gpui::test]
fn workspace_close_item_action_requires_registered_panel(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a"], 0);
    let mut workspace = workspace_with_panels(cx, graph, &[]);

    let err = workspace
        .apply_action(&DockAction::CloseItem {
            space: space(),
            item: item("a"),
        })
        .expect_err("missing panel metadata should block close policy");

    assert_eq!(
        err,
        DockActionApplyError::PanelNotRegistered { item: item("a") }
    );
    let DockNode::Tabs { items, active } = workspace
        .graph()
        .node(root)
        .expect("source tabs should remain")
    else {
        panic!("source should be tabs");
    };
    assert_eq!(items, &vec![item("a")]);
    assert_eq!(*active, 0);
}

#[open_gpui::test]
fn workspace_same_stack_center_drop_is_noop(cx: &mut TestAppContext) {
    let (graph, tabs) = tabs_graph(&["a", "b"], 0);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let outcome = workspace
        .apply_action(&DockAction::MoveTab {
            source_space: space(),
            source_tabs: tabs,
            item: item("a"),
            target_space: space(),
            target_tabs: tabs,
            zone: DropZone::Center,
            insert_index: None,
        })
        .expect("same-stack center drop should be valid");

    assert_eq!(outcome, DockActionOutcome::Unchanged);
    let DockNode::Tabs { items, active } = workspace
        .graph()
        .node(tabs)
        .expect("tabs should still exist")
    else {
        panic!("target should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(*active, 0);
}

#[open_gpui::test]
fn workspace_same_stack_center_drop_reorders_with_insert_index(cx: &mut TestAppContext) {
    let (graph, tabs) = tabs_graph(&["a", "b", "c"], 0);
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[("a", "A", "A"), ("b", "B", "B"), ("c", "C", "C")],
    );

    let outcome = workspace
        .apply_action(&DockAction::MoveTab {
            source_space: space(),
            source_tabs: tabs,
            item: item("a"),
            target_space: space(),
            target_tabs: tabs,
            zone: DropZone::Center,
            insert_index: Some(3),
        })
        .expect("same-stack center drop with an index should reorder");

    assert_eq!(outcome, DockActionOutcome::Changed);
    let DockNode::Tabs { items, active } = workspace
        .graph()
        .node(tabs)
        .expect("tabs should still exist")
    else {
        panic!("target should be tabs");
    };
    assert_eq!(items, &vec![item("b"), item("c"), item("a")]);
    assert_eq!(*active, 2);
}

#[open_gpui::test]
fn workspace_resize_split_action_updates_fractions(cx: &mut TestAppContext) {
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
fn workspace_resize_split_action_reports_unchanged_for_same_fractions(cx: &mut TestAppContext) {
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
fn workspace_resize_split_action_rejects_invalid_targets(cx: &mut TestAppContext) {
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
        DockActionApplyError::Graph(DockOpApplyError::SplitNodeNotFound {
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
        DockActionApplyError::Graph(DockOpApplyError::NodeIsNotSplit { node: left_tabs })
    );

    let mismatch = workspace
        .apply_action(&DockAction::ResizeSplit {
            split,
            fractions: vec![1.0],
        })
        .expect_err("fraction length mismatch should fail");
    assert_eq!(
        mismatch,
        DockActionApplyError::Graph(DockOpApplyError::SplitFractionsLenMismatch {
            split,
            children_len: 2,
            fractions_len: 1
        })
    );
}

#[open_gpui::test]
fn workspace_policy_blocks_edge_drop_without_mutating_graph(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);
    workspace.policy_mut().set_allow_edge_split(false);

    let err = workspace
        .apply_action(&DockAction::MoveTab {
            source_space: space(),
            source_tabs: left_tabs,
            item: item("a"),
            target_space: space(),
            target_tabs: right_tabs,
            zone: DropZone::Right,
            insert_index: None,
        })
        .expect_err("edge drop should be rejected by policy");

    assert_eq!(
        err,
        DockActionApplyError::Policy(DockPolicyError::EdgeSplitDisabled)
    );
    let DockNode::Tabs { items, active } = workspace
        .graph()
        .node(left_tabs)
        .expect("source tabs should remain")
    else {
        panic!("source should be tabs");
    };
    assert_eq!(items, &vec![item("a")]);
    assert_eq!(*active, 0);
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
