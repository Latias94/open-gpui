use crate::{
    DockCentralRegion, DockFloatingContainer, DockGraph, DockNode, DockNodeId, DockWorkspace,
    SplitAxis, debug::DockDebugRegion, host_test_support::*,
};
use open_gpui::{
    AppContext as _, Focusable, Modifiers, MouseButton, TestAppContext, VisualTestContext, px, size,
};
use slotmap::Key;

#[open_gpui::test]
fn single_tabs_render_selected_panel_and_all_tab_labels(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph_with_selected(&["a", "b"], "b");
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
fn drop_guides_render_while_tab_drag_is_active(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"]);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: root,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        open_gpui::point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    for zone in [
        crate::DropZone::Center,
        crate::DropZone::Left,
        crate::DropZone::Right,
        crate::DropZone::Top,
        crate::DropZone::Bottom,
    ] {
        let guide = selector_for(
            &visual,
            &host,
            DockDebugRegion::DropGuide {
                node: Some(root),
                zone,
            },
        )
        .unwrap_or_else(|| panic!("{zone:?} drop guide selector should be emitted"));
        assert!(
            debug_bounds(&mut visual, &guide).size.width > px(0.0),
            "{zone:?} guide should have visible bounds"
        );
    }
}

#[open_gpui::test]
fn drop_guides_are_scoped_to_each_target_tabs_node(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(500.0), px(240.0)),
    );

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: left_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let right_stack = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("right tabs selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        open_gpui::point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    let right_bounds = debug_bounds(&mut visual, &right_stack);
    let right_center_guide = selector_for(
        &visual,
        &host,
        DockDebugRegion::DropGuide {
            node: Some(right_tabs),
            zone: crate::DropZone::Center,
        },
    )
    .expect("right stack center guide selector should be emitted");
    assert!(
        right_bounds.contains(&debug_bounds(&mut visual, &right_center_guide).center()),
        "right stack guide should be positioned inside the right tab stack"
    );
}

#[open_gpui::test]
fn pending_panel_focus_targets_active_focusable_panel(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph(&["a"]);
    let panel = test_view(cx, "A");
    let expected_focus = cx.read_entity(&panel, |panel, cx| panel.focus_handle(cx));
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel);
    let (_window, host, mut visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

    host.update(cx, |host, cx| {
        assert!(host.request_panel_focus(item("a")));
        cx.notify();
    });
    visual.run_until_parked();

    visual.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(expected_focus));
    });
}

#[open_gpui::test]
fn missing_selected_panel_renders_placeholder(cx: &mut TestAppContext) {
    let (graph, _root) = tabs_graph_with_selected(&["a", "missing"], "missing");
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
}

#[open_gpui::test]
fn empty_central_passthrough_renders_full_host_drop_target(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    graph.set_central_region(
        space(),
        DockCentralRegion::empty().with_passthrough_when_empty(true),
    );
    let (_window, host, mut visual) = open_host(cx, graph, &[], size(px(320.0), px(200.0)));

    let empty = selector_for(&visual, &host, DockDebugRegion::EmptySpace)
        .expect("empty central passthrough selector should be emitted");
    let bounds = debug_bounds(&mut visual, &empty);

    assert_close(width(bounds), 320.0);
    assert_close(height(bounds), 200.0);
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
        selected: Some(item("b")),
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

#[open_gpui::test]
fn central_split_child_uses_remaining_render_space(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let left = graph.insert_node(DockNode::Tabs {
        items: vec![item("left")],
        selected: Some(item("left")),
    });
    let main = graph.insert_node(DockNode::Tabs {
        items: vec![item("main")],
        selected: Some(item("main")),
    });
    let right = graph.insert_node(DockNode::Tabs {
        items: vec![item("right")],
        selected: Some(item("right")),
    });
    let split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left, main, right],
        fractions: vec![0.2, 0.0, 0.3],
    });
    graph.set_root(space(), split);
    graph.set_central_region(space(), DockCentralRegion::with_node(main));

    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[
            ("left", "Left", "Left"),
            ("main", "Main", "Main"),
            ("right", "Right", "Right"),
        ],
        size(px(1000.0), px(200.0)),
    );

    let left_selector = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 0 },
    )
    .expect("left split selector should be emitted");
    let main_selector = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 1 },
    )
    .expect("main split selector should be emitted");
    let right_selector = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitChild { split, index: 2 },
    )
    .expect("right split selector should be emitted");

    assert_close(width(debug_bounds(&mut visual, &left_selector)), 200.0);
    assert_close(width(debug_bounds(&mut visual, &main_selector)), 500.0);
    assert_close(width(debug_bounds(&mut visual, &right_selector)), 300.0);
}
