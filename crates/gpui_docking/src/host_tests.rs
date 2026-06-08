use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockGraph, DockHost, DockItemId,
    DockLayoutNode, DockNode, DockNodeId, DockOpApplyError, DockSpaceId, DockWorkspace, SplitAxis,
    debug::DockDebugRegion,
};
use open_gpui::{
    AppContext as _, Bounds, Context, Entity, InteractiveElement, IntoElement, Modifiers,
    ParentElement, Pixels, Render, Styled, TestAppContext, VisualTestContext, Window, WindowHandle,
    div, px, rgb, size,
};
use slotmap::Key;

const SPACE: &str = "main";

struct TestPanel {
    label: &'static str,
}

impl Render for TestPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let selector = format!("test-panel:{}", self.label);
        div()
            .debug_selector(move || selector)
            .size_full()
            .bg(rgb(0xffffff))
            .child(self.label)
    }
}

fn space() -> DockSpaceId {
    DockSpaceId::from(SPACE)
}

fn item(id: &str) -> DockItemId {
    DockItemId::from(id)
}

fn tabs_graph(items: &[&str], active: usize) -> (DockGraph, DockNodeId) {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: items.iter().copied().map(DockItemId::from).collect(),
        active,
    });
    graph.set_root(space(), root);
    (graph, root)
}

fn split_graph(
    axis: SplitAxis,
    first_fraction: f32,
    second_fraction: f32,
) -> (DockGraph, DockNodeId, DockNodeId, DockNodeId) {
    let mut graph = DockGraph::new();
    let first = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    let second = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    let root = graph.insert_node(DockNode::Split {
        axis,
        children: vec![first, second],
        fractions: vec![first_fraction, second_fraction],
    });
    graph.set_root(space(), root);
    (graph, root, first, second)
}

fn test_view(cx: &mut TestAppContext, label: &'static str) -> Entity<TestPanel> {
    cx.new(|_| TestPanel { label })
}

fn workspace_with_panels(
    cx: &mut TestAppContext,
    graph: DockGraph,
    panels: &[(&str, &str, &'static str)],
) -> DockWorkspace {
    let mut workspace = DockWorkspace::new(space(), graph);
    for (id, title, label) in panels {
        workspace.register_panel_view(item(id), *title, test_view(cx, label));
    }
    workspace
}

fn open_host(
    cx: &mut TestAppContext,
    graph: DockGraph,
    panels: &[(&str, &str, &'static str)],
    window_size: open_gpui::Size<Pixels>,
) -> (WindowHandle<DockHost>, Entity<DockHost>, VisualTestContext) {
    let workspace = workspace_with_panels(cx, graph, panels);
    let window = cx.open_window(window_size, move |_, _| DockHost::from_workspace(workspace));
    let host = window.root(cx).expect("window should expose DockHost root");
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);
    (window, host, visual)
}

fn selector_for(
    cx: &VisualTestContext,
    host: &Entity<DockHost>,
    region: DockDebugRegion,
) -> Option<String> {
    cx.read_entity(host, |host, _| {
        host.debug_selector(&region).map(ToString::to_string)
    })
}

fn debug_bounds(cx: &mut VisualTestContext, selector: &str) -> Bounds<Pixels> {
    let selector: &'static str = Box::leak(selector.to_owned().into_boxed_str());
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("debug selector {selector} should have bounds"))
}

fn width(bounds: Bounds<Pixels>) -> f32 {
    f32::from(bounds.size.width)
}

fn height(bounds: Bounds<Pixels>) -> f32 {
    f32::from(bounds.size.height)
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 2.0,
        "expected {actual} to be within 2px of {expected}"
    );
}

#[open_gpui::test]
fn registry_replaces_registered_panel(cx: &mut TestAppContext) {
    let mut registry = crate::DockPanelRegistry::new();
    let first = test_view(cx, "first");
    let second = test_view(cx, "second");

    assert!(registry.register_view(item("a"), "First", first).is_none());
    let previous = registry
        .register_view(item("a"), "Second", second)
        .expect("second registration should return previous panel");

    assert_eq!(previous.title(), "First");
    assert_eq!(
        registry.get(&item("a")).map(crate::DockPanel::title),
        Some("Second")
    );
    assert_eq!(registry.len(), 1);
}

#[open_gpui::test]
fn host_applies_actions_through_workspace(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let workspace = workspace_with_panels(cx, graph, &[("a", "A", "A"), ("b", "B", "B")]);
    let mut host = DockHost::from_workspace(workspace);

    let outcome = host
        .apply_action(&DockAction::SelectTab {
            tabs: root,
            item: item("b"),
        })
        .expect("active tab mutation should be valid");

    let DockNode::Tabs { active, .. } = host.graph().node(root).expect("tabs should exist") else {
        panic!("root should be tabs");
    };
    assert_eq!(outcome, DockActionOutcome::Changed);
    assert_eq!(*active, 1);
    assert!(host.panels().contains(&item("a")));
    assert!(host.panels().contains(&item("b")));
}

#[open_gpui::test]
fn compatibility_constructor_delegates_to_workspace(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph(&["a"], 0);
    let mut host = DockHost::new(space(), graph);
    host.register_panel_view(item("a"), "A", test_view(cx, "A"));

    assert_eq!(host.workspace().space(), &space());
    assert!(host.workspace().panels().contains(&item("a")));
    assert!(host.graph().root(&space()).is_some());
}

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
fn single_tabs_render_active_panel_and_all_tab_labels(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 1);
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    let tab_a = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: root,
            item: item("a"),
        },
    )
    .expect("tab A selector should be emitted");
    let tab_b = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: root,
            item: item("b"),
        },
    )
    .expect("tab B selector should be emitted");
    let panel_b = selector_for(&visual, &host, DockDebugRegion::Panel { item: item("b") })
        .expect("active panel selector should be emitted");

    assert!(debug_bounds(&mut visual, &tab_a).size.width > px(0.0));
    assert!(debug_bounds(&mut visual, &tab_b).size.width > px(0.0));
    assert!(debug_bounds(&mut visual, &panel_b).size.height > px(0.0));
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_none(),
        "inactive panel should not be mounted"
    );
}

#[open_gpui::test]
fn missing_active_panel_renders_placeholder(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph(&["a", "missing"], 1);
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A")],
        size(px(400.0), px(240.0)),
    );

    let missing = selector_for(
        &visual,
        &host,
        DockDebugRegion::MissingPanel {
            item: item("missing"),
        },
    )
    .expect("missing panel selector should be emitted");

    assert!(debug_bounds(&mut visual, &missing).size.width > px(0.0));
    assert_eq!(
        host.read_with(&visual, |host, _| host.graph().spaces().len()),
        1
    );
}

#[open_gpui::test]
fn empty_root_renders_placeholder(cx: &mut TestAppContext) {
    let graph = DockGraph::new();
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A")],
        size(px(320.0), px(200.0)),
    );

    let empty = selector_for(&visual, &host, DockDebugRegion::EmptySpace)
        .expect("empty selector should be emitted");

    assert!(debug_bounds(&mut visual, &empty).size.width > px(0.0));
    assert_eq!(host.read_with(&visual, |host, _| host.panels().len()), 1);
}

#[open_gpui::test]
fn floating_node_renders_deferred_placeholder(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    let floating = graph.insert_node(DockNode::Floating { child: tabs });
    graph.set_root(space(), floating);

    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A")],
        size(px(320.0), px(200.0)),
    );

    let deferred = selector_for(
        &visual,
        &host,
        DockDebugRegion::DeferredFloating { node: floating },
    )
    .expect("deferred floating selector should be emitted");

    assert!(debug_bounds(&mut visual, &deferred).size.width > px(0.0));
}

#[open_gpui::test]
fn horizontal_split_uses_normalized_flex_shares(cx: &mut TestAppContext) {
    let (graph, split, _left, _right) = split_graph(SplitAxis::Horizontal, 0.25, 0.75);
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(200.0)),
    );

    let left = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 0 },
    )
    .expect("left split selector should be emitted");
    let right = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 1 },
    )
    .expect("right split selector should be emitted");

    assert_close(width(debug_bounds(&mut visual, &left)), 100.0);
    assert_close(width(debug_bounds(&mut visual, &right)), 300.0);
}

#[open_gpui::test]
fn vertical_split_uses_normalized_flex_shares(cx: &mut TestAppContext) {
    let (graph, split, _top, _bottom) = split_graph(SplitAxis::Vertical, 0.25, 0.75);
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(200.0)),
    );

    let top = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 0 },
    )
    .expect("top split selector should be emitted");
    let bottom = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 1 },
    )
    .expect("bottom split selector should be emitted");

    assert_close(height(debug_bounds(&mut visual, &top)), 50.0);
    assert_close(height(debug_bounds(&mut visual, &bottom)), 150.0);
}

#[open_gpui::test]
fn unnormalized_split_fractions_are_repaired_for_rendering(cx: &mut TestAppContext) {
    let (graph, split, _left, _right) = split_graph(SplitAxis::Horizontal, 2.0, 1.0);
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(600.0), px(200.0)),
    );

    let left = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 0 },
    )
    .expect("left split selector should be emitted");
    let right = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 1 },
    )
    .expect("right split selector should be emitted");

    assert_close(width(debug_bounds(&mut visual, &left)), 400.0);
    assert_close(width(debug_bounds(&mut visual, &right)), 200.0);
}

#[open_gpui::test]
fn clicking_inactive_tab_updates_active_panel(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "panel A should be active before mutation"
    );

    let tab_b = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: root,
            item: item("b"),
        },
    )
    .expect("tab B selector should be emitted");
    let tab_b_bounds = debug_bounds(&mut visual, &tab_b);
    visual.simulate_click(tab_b_bounds.center(), Modifiers::none());
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("b") }).is_some(),
        "panel B should be active after mutation"
    );
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_none(),
        "panel A should no longer be mounted after mutation"
    );
}
