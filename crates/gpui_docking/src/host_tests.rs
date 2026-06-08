use crate::{
    DockDebugRegion, DockGraph, DockHost, DockItemId, DockNode, DockNodeId, DockOp, DockSpaceId,
    DockWorkspace, SplitAxis,
};
use open_gpui::{
    AppContext as _, Bounds, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Pixels, Render, Styled, TestAppContext, VisualTestContext, Window, WindowHandle, div, px, rgb,
    size,
};

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

fn open_host(
    cx: &mut TestAppContext,
    graph: DockGraph,
    panels: &[(&str, &str, &'static str)],
    window_size: open_gpui::Size<Pixels>,
) -> (WindowHandle<DockHost>, Entity<DockHost>, VisualTestContext) {
    let panels: Vec<_> = panels
        .iter()
        .map(|(id, title, label)| {
            (
                DockItemId::from(*id),
                (*title).to_string(),
                test_view(cx, label),
            )
        })
        .collect();

    let window = cx.open_window(window_size, move |_, _| {
        let mut host = DockHost::new(space(), graph);
        for (id, title, view) in panels {
            host.register_panel_view(id, title, view);
        }
        host
    });
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
fn host_graph_mutation_preserves_registry(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let mut host = DockHost::new(space(), graph);
    host.register_panel_view(item("a"), "A", test_view(cx, "A"));
    host.register_panel_view(item("b"), "B", test_view(cx, "B"));

    host.graph_mut()
        .apply_op_checked(&DockOp::SetActiveTab {
            tabs: root,
            active: 1,
        })
        .expect("active tab mutation should be valid");

    let DockNode::Tabs { active, .. } = host.graph().node(root).expect("tabs should exist") else {
        panic!("root should be tabs");
    };
    assert_eq!(*active, 1);
    assert!(host.panels().contains(&item("a")));
    assert!(host.panels().contains(&item("b")));
}

#[open_gpui::test]
fn workspace_applies_ops_and_preserves_registered_panels(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_view(item("a"), "A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "B", test_view(cx, "B"));

    workspace
        .apply_op_checked(&DockOp::SetActiveTab {
            tabs: root,
            active: 1,
        })
        .expect("active tab mutation should be valid");

    let DockNode::Tabs { active, .. } = workspace
        .graph()
        .node(root)
        .expect("tabs should still exist")
    else {
        panic!("root should be tabs");
    };
    assert_eq!(*active, 1);
    assert!(workspace.panels().contains(&item("a")));
    assert!(workspace.panels().contains(&item("b")));
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
fn notified_graph_mutation_updates_active_panel(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let (window, host, visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "panel A should be active before mutation"
    );

    window
        .update(cx, |host, _window, cx| {
            host.graph_mut()
                .apply_op_checked(&DockOp::SetActiveTab {
                    tabs: root,
                    active: 1,
                })
                .expect("active tab mutation should be valid");
            cx.notify();
        })
        .expect("host update should succeed");
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
