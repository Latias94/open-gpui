use crate::{
    DockCentralRegion, DockGraph, DockNode, SplitAxis, debug::DockDebugRegion,
    host_test_support::*, presentation_scene::DockPresentationPaneKind,
};
use open_gpui::{TestAppContext, VisualTestContext, px, size};

#[open_gpui::test]
fn render_split_child_bounds_match_presentation_scene_panes(cx: &mut TestAppContext) {
    let (graph, split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.25, 0.75);
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(200.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 400.0, 200.0), cx)
    });

    assert_scene_pane_matches_render_region(
        &mut visual,
        &host,
        &scene,
        left_tabs,
        DockDebugRegion::SplitChild { split, index: 0 },
        "left split child",
    );
    assert_scene_pane_matches_render_region(
        &mut visual,
        &host,
        &scene,
        right_tabs,
        DockDebugRegion::SplitChild { split, index: 1 },
        "right split child",
    );
}

#[open_gpui::test]
fn render_nested_split_bounds_match_presentation_scene_panes(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let left_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("left")],
        selected: Some(item("left")),
    });
    let upper_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("upper")],
        selected: Some(item("upper")),
    });
    let lower_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("lower")],
        selected: Some(item("lower")),
    });
    let right_split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Vertical,
        children: vec![upper_tabs, lower_tabs],
        fractions: vec![0.4, 0.6],
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left_tabs, right_split],
        fractions: vec![0.3, 0.7],
    });
    graph.set_root(space(), root);
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[
            ("left", "Left", "Left"),
            ("upper", "Upper", "Upper"),
            ("lower", "Lower", "Lower"),
        ],
        size(px(600.0), px(300.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 600.0, 300.0), cx)
    });

    assert_scene_pane_matches_render_region(
        &mut visual,
        &host,
        &scene,
        left_tabs,
        DockDebugRegion::SplitChild {
            split: root,
            index: 0,
        },
        "root left child",
    );
    assert_scene_pane_matches_render_region(
        &mut visual,
        &host,
        &scene,
        upper_tabs,
        DockDebugRegion::SplitChild {
            split: right_split,
            index: 0,
        },
        "nested upper child",
    );
    assert_scene_pane_matches_render_region(
        &mut visual,
        &host,
        &scene,
        lower_tabs,
        DockDebugRegion::SplitChild {
            split: right_split,
            index: 1,
        },
        "nested lower child",
    );
}

#[open_gpui::test]
fn render_splitter_handle_bounds_match_presentation_scene_splitters(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let left_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("left")],
        selected: Some(item("left")),
    });
    let upper_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("upper")],
        selected: Some(item("upper")),
    });
    let lower_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("lower")],
        selected: Some(item("lower")),
    });
    let right_split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Vertical,
        children: vec![upper_tabs, lower_tabs],
        fractions: vec![0.4, 0.6],
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left_tabs, right_split],
        fractions: vec![0.3, 0.7],
    });
    graph.set_root(space(), root);
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[
            ("left", "Left", "Left"),
            ("upper", "Upper", "Upper"),
            ("lower", "Lower", "Lower"),
        ],
        size(px(600.0), px(300.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 600.0, 300.0), cx)
    });

    let root_splitter = scene
        .splitters
        .iter()
        .find(|splitter| splitter.split == root && splitter.index == 0)
        .expect("root splitter should be in presentation scene");
    assert_render_region_matches_bounds(
        &mut visual,
        &host,
        DockDebugRegion::SplitterHandle {
            split: root,
            index: 0,
        },
        root_splitter.bounds,
        "root splitter handle",
    );

    let nested_splitter = scene
        .splitters
        .iter()
        .find(|splitter| splitter.split == right_split && splitter.index == 0)
        .expect("nested splitter should be in presentation scene");
    assert_render_region_matches_bounds(
        &mut visual,
        &host,
        DockDebugRegion::SplitterHandle {
            split: right_split,
            index: 0,
        },
        nested_splitter.bounds,
        "nested splitter handle",
    );
}

#[open_gpui::test]
fn render_three_child_split_bounds_match_presentation_scene_layout(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let left_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("left")],
        selected: Some(item("left")),
    });
    let middle_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("middle")],
        selected: Some(item("middle")),
    });
    let right_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("right")],
        selected: Some(item("right")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left_tabs, middle_tabs, right_tabs],
        fractions: vec![0.2, 0.3, 0.5],
    });
    graph.set_root(space(), root);
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[
            ("left", "Left", "Left"),
            ("middle", "Middle", "Middle"),
            ("right", "Right", "Right"),
        ],
        size(px(1000.0), px(240.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 1000.0, 240.0), cx)
    });

    for (index, tabs) in [left_tabs, middle_tabs, right_tabs].into_iter().enumerate() {
        assert_scene_pane_matches_render_region(
            &mut visual,
            &host,
            &scene,
            tabs,
            DockDebugRegion::SplitChild { split: root, index },
            &format!("three-child split child {index}"),
        );
    }

    for index in 0..2 {
        let splitter = scene
            .splitters
            .iter()
            .find(|splitter| splitter.split == root && splitter.index == index)
            .unwrap_or_else(|| panic!("splitter {index} should be in presentation scene"));
        assert_render_region_matches_bounds(
            &mut visual,
            &host,
            DockDebugRegion::SplitterHandle { split: root, index },
            splitter.bounds,
            &format!("three-child splitter handle {index}"),
        );
    }
}

#[open_gpui::test]
fn render_floating_bounds_match_presentation_scene_container(cx: &mut TestAppContext) {
    let (graph, _root, floating) = floating_overlay_graph();
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(320.0), px(220.0)),
    );
    let (floating_tabs, scene) = host.update(cx, |host, cx| {
        let session = host.render_session(cx);
        (
            session
                .floating_child(floating)
                .expect("floating container should resolve to child tabs"),
            host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 320.0, 220.0), cx),
        )
    });
    let container = scene
        .floating_containers
        .iter()
        .find(|container| container.node == floating)
        .expect("floating container should be in presentation scene");

    assert_render_region_matches_bounds(
        &mut visual,
        &host,
        DockDebugRegion::Floating { node: floating },
        container.bounds,
        "floating frame",
    );
    assert_render_region_matches_bounds(
        &mut visual,
        &host,
        DockDebugRegion::FloatingHandle { node: floating },
        container.title_bar_bounds,
        "floating handle",
    );
    assert_scene_pane_matches_render_region(
        &mut visual,
        &host,
        &scene,
        floating_tabs,
        DockDebugRegion::Tabs {
            node: floating_tabs,
        },
        "floating tabs content",
    );
}

#[open_gpui::test]
fn render_tiny_floating_handle_clamps_to_presentation_title_bar(cx: &mut TestAppContext) {
    let (mut graph, _root, floating) = floating_overlay_graph();
    graph
        .floating_containers_mut(space())
        .iter_mut()
        .find(|container| container.node == floating)
        .expect("floating container should exist")
        .bounds = floating_bounds(10.0, 20.0, 220.0, 12.0);
    let (_window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(320.0), px(220.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 320.0, 220.0), cx)
    });
    let container = scene
        .floating_containers
        .iter()
        .find(|container| container.node == floating)
        .expect("floating container should be in presentation scene");

    assert_close(f32::from(container.title_bar_bounds.size.height), 12.0);
    assert_render_region_matches_bounds(
        &mut visual,
        &host,
        DockDebugRegion::FloatingHandle { node: floating },
        container.title_bar_bounds,
        "tiny floating handle",
    );
}

#[open_gpui::test]
fn render_plain_empty_central_bounds_match_presentation_scene(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    graph.set_central_region(space(), DockCentralRegion::empty());
    let (_window, host, mut visual) = open_host(cx, graph, &[], size(px(320.0), px(200.0)));
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 320.0, 200.0), cx)
    });
    let empty_pane = scene
        .panes
        .iter()
        .find(|pane| pane.kind == DockPresentationPaneKind::EmptyCentral)
        .expect("empty central pane should be in presentation scene");

    assert_render_region_matches_bounds(
        &mut visual,
        &host,
        DockDebugRegion::EmptySpace,
        empty_pane.bounds,
        "plain empty central",
    );
}

#[open_gpui::test]
fn render_empty_central_passthrough_bounds_match_presentation_scene(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    graph.set_central_region(
        space(),
        DockCentralRegion::empty().with_passthrough_when_empty(true),
    );
    let (_window, host, mut visual) = open_host(cx, graph, &[], size(px(320.0), px(200.0)));
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 320.0, 200.0), cx)
    });
    let empty_pane = scene
        .panes
        .iter()
        .find(|pane| pane.kind == DockPresentationPaneKind::EmptyCentral)
        .expect("empty central pane should be in presentation scene");

    assert_render_region_matches_bounds(
        &mut visual,
        &host,
        DockDebugRegion::EmptySpace,
        empty_pane.bounds,
        "empty central passthrough",
    );
}

#[open_gpui::test]
fn render_zoomed_pane_bounds_match_presentation_scene(cx: &mut TestAppContext) {
    let (graph, _root, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.4, 0.6);
    let (window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(500.0), px(240.0)),
    );
    assert!(host.update(cx, |host, cx| host.zoom_pane(right_tabs, cx)));
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 500.0, 240.0), cx)
    });

    assert_scene_pane_matches_render_region(
        &mut visual,
        &host,
        &scene,
        right_tabs,
        DockDebugRegion::Tabs { node: right_tabs },
        "zoomed right tabs",
    );
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Tabs { node: left_tabs }).is_none(),
        "zoomed render should not mount the egressing sibling as normal pane content"
    );
}
