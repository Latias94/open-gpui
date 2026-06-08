use crate::{
    DockFloatingContainer, DockGraph, DockNode, DockNodeId, SplitAxis, debug::DockDebugRegion,
    host_test_support::*,
};
use open_gpui::{TestAppContext, px, size};
use slotmap::Key;

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
        host.read_with(&visual, |host, _| host
            .graph()
            .expect("owned host should expose graph")
            .spaces()
            .len()),
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
    assert_eq!(
        host.read_with(&visual, |host, _| host
            .panels()
            .expect("owned host should expose panels")
            .len()),
        1
    );
}

#[open_gpui::test]
fn floating_container_renders_panel_inside_overlay_bounds(cx: &mut TestAppContext) {
    let (graph, _root, floating) = floating_overlay_graph();
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(320.0), px(220.0)),
    );

    let frame = selector_for(&visual, &host, DockDebugRegion::Floating { node: floating })
        .expect("floating frame selector should be emitted");
    let handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::FloatingHandle { node: floating },
    )
    .expect("floating handle selector should be emitted");

    let frame_bounds = debug_bounds(&mut visual, &frame);
    assert_close(width(frame_bounds), 220.0);
    assert_close(height(frame_bounds), 140.0);
    assert!(debug_bounds(&mut visual, &handle).size.height > px(0.0));
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "floating panel should render"
    );
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("b") }).is_some(),
        "root panel should still render behind the overlay"
    );
}

#[open_gpui::test]
fn missing_floating_child_renders_missing_node_placeholder(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    graph.set_root(space(), root);
    let missing_child = DockNodeId::null();
    let floating = graph.insert_node(DockNode::Floating {
        child: missing_child,
    });
    graph
        .floating_containers_mut(space())
        .push(DockFloatingContainer {
            node: floating,
            bounds: floating_bounds(10.0, 20.0, 220.0, 140.0),
        });

    let (_window, host, visual) = open_host(
        cx,
        graph,
        &[("b", "Panel B", "B")],
        size(px(320.0), px(220.0)),
    );

    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::MissingNode {
                node: missing_child
            }
        )
        .is_some(),
        "missing floating child should render a test-visible placeholder"
    );
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
