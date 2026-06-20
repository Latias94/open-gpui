use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockGraph, DockGraphMutationError,
    DockLayoutNode, DockNode, DockNodeId, DockWorkspace, host_test_support::*,
};
use open_gpui::TestAppContext;
use slotmap::Key;

#[open_gpui::test]
fn workspace_applies_actions_and_preserves_registered_panels(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"]);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_view(item("a"), "A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "B", test_view(cx, "B"));

    let outcome = workspace
        .apply_action(&DockAction::SelectTab {
            tabs: root,
            item: item("b"),
        })
        .expect("tab selection should be valid");

    let DockNode::Tabs { selected, .. } = workspace
        .graph()
        .node(root)
        .expect("tabs should still exist")
    else {
        panic!("root should be tabs");
    };
    assert_eq!(outcome, DockActionOutcome::Changed);
    assert_eq!(selected.as_ref(), Some(&item("b")));
    assert!(workspace.panels().contains(&item("a")));
    assert!(workspace.panels().contains(&item("b")));
}

#[open_gpui::test]
fn workspace_selecting_selected_tab_is_noop(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph_with_selected(&["a", "b"], "b");
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);

    let outcome = workspace
        .apply_action(&DockAction::SelectTab {
            tabs: root,
            item: item("b"),
        })
        .expect("tab selection should be valid");

    assert_eq!(outcome, DockActionOutcome::Unchanged);
    assert!(workspace.panels().contains(&item("a")));
    assert!(workspace.panels().contains(&item("b")));
}

#[open_gpui::test]
fn workspace_selecting_selected_tab_records_observed_mru(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph_with_selected(&["a", "b", "c"], "b");
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[("a", "A", "A"), ("b", "B", "B"), ("c", "C", "C")],
    );

    let outcome = workspace
        .apply_action(&DockAction::SelectTab {
            tabs: root,
            item: item("b"),
        })
        .expect("observing selected b should be valid");
    assert_eq!(outcome, DockActionOutcome::Unchanged);

    workspace
        .apply_action(&DockAction::SelectTab {
            tabs: root,
            item: item("c"),
        })
        .expect("selecting c should update history");
    workspace
        .apply_action(&DockAction::CloseItem {
            space: space(),
            item: item("c"),
        })
        .expect("closing selected c should be valid");

    let DockNode::Tabs { selected, .. } = workspace
        .graph()
        .node(root)
        .expect("tabs should remain after closing one item")
    else {
        panic!("root should be tabs");
    };
    assert_eq!(
        selected.as_ref(),
        Some(&item("b")),
        "unchanged selection should still become MRU, matching ImGui's observed selected tab"
    );
}

#[open_gpui::test]
fn workspace_close_selected_tab_restores_recently_selected_sibling(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b", "c"]);
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[("a", "A", "A"), ("b", "B", "B"), ("c", "C", "C")],
    );

    workspace
        .apply_action(&DockAction::SelectTab {
            tabs: root,
            item: item("b"),
        })
        .expect("selecting b should update history");
    workspace
        .apply_action(&DockAction::SelectTab {
            tabs: root,
            item: item("c"),
        })
        .expect("selecting c should update history");

    let outcome = workspace
        .apply_action(&DockAction::CloseItem {
            space: space(),
            item: item("c"),
        })
        .expect("closing selected tab should be valid");

    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(root)
        .expect("tabs should remain after closing one item")
    else {
        panic!("root should be tabs");
    };
    assert_eq!(outcome, DockActionOutcome::Changed);
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(
        selected.as_ref(),
        Some(&item("b")),
        "close should restore the most recently selected remaining tab instead of falling back to tab order"
    );
}

#[open_gpui::test]
fn workspace_close_selected_tab_without_history_uses_first_remaining_tab(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b", "c"]);
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[("a", "A", "A"), ("b", "B", "B"), ("c", "C", "C")],
    );

    let outcome = workspace
        .apply_action(&DockAction::CloseItem {
            space: space(),
            item: item("b"),
        })
        .expect("closing selected tab should be valid");

    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(root)
        .expect("tabs should remain after closing one item")
    else {
        panic!("root should be tabs");
    };
    assert_eq!(outcome, DockActionOutcome::Changed);
    assert_eq!(items, &vec![item("a"), item("c")]);
    assert_eq!(selected.as_ref(), Some(&item("a")));
}

#[open_gpui::test]
fn workspace_rejects_invalid_select_tab_actions(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"]);
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
        DockActionApplyError::Graph(DockGraphMutationError::TabsNodeNotFound {
            tabs: DockNodeId::null()
        })
    );

    let DockNode::Tabs { selected, .. } = workspace
        .graph()
        .node(root)
        .expect("tabs should still exist")
    else {
        panic!("root should be tabs");
    };
    assert_eq!(selected.as_ref(), Some(&item("a")));
    assert!(workspace.panels().contains(&item("a")));
    assert!(workspace.panels().contains(&item("b")));
}

#[open_gpui::test]
fn workspace_action_layout_export_remains_graph_only(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"]);
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
        .expect("tab selection should be valid");
    assert_eq!(outcome, DockActionOutcome::Changed);

    let layout = workspace.graph().export_layout();
    layout.validate().expect("exported layout should validate");
    let json = serde_json::to_string(&layout).expect("layout should serialize");

    assert!(!json.contains("Panel A"));
    assert!(!json.contains("Panel B"));
    assert!(!json.contains("AnyView"));
    assert!(!json.contains("Entity"));
    assert!(!json.contains("WindowHandle"));

    assert!(!json.contains("\"active\""));

    let DockLayoutNode::Tabs {
        items, selected, ..
    } = layout
        .nodes
        .iter()
        .find(|node| matches!(node, DockLayoutNode::Tabs { .. }))
        .expect("layout should contain tabs node")
    else {
        panic!("expected tabs node");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(selected.as_ref(), Some(&item("b")));

    let imported = DockGraph::import_layout(&layout).expect("layout should import");
    let imported_root = imported.root(&space()).expect("space should keep root");
    let DockNode::Tabs { selected, items } = imported
        .node(imported_root)
        .expect("imported root should exist")
    else {
        panic!("imported root should be tabs");
    };
    assert_eq!(selected.as_ref(), items.get(1));
    assert_eq!(items, &vec![item("a"), item("b")]);
}
