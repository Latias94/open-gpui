use crate::{
    DockGraph, DockNode, SplitAxis,
    divider_hit_map::{DockDividerAffordanceState, DockDividerHitMap, DockDividerHitTarget},
    host_test_support::{item, open_host, space, split_graph},
};
use open_gpui::{Bounds, TestAppContext, point, px, size};

fn host_bounds(width: f32, height: f32) -> Bounds<open_gpui::Pixels> {
    Bounds::new(point(px(0.0), px(0.0)), size(px(width), px(height)))
}

#[open_gpui::test]
fn divider_hit_map_resolves_single_axis_splitter(cx: &mut TestAppContext) {
    let (graph, root, _left_tabs, _right_tabs) = split_graph(SplitAxis::Horizontal, 0.25, 0.75);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(host_bounds(400.0, 240.0), cx)
    });

    let hit_map = DockDividerHitMap::from_scene(&scene);
    assert_eq!(hit_map.targets().len(), 1);
    let target = hit_map
        .hit(point(px(100.0), px(120.0)))
        .expect("splitter center should hit");

    match target {
        DockDividerHitTarget::Single(handle) => {
            assert_eq!(handle.key.split, root);
            assert_eq!(handle.key.index, 0);
            assert_eq!(handle.key.axis, SplitAxis::Horizontal);
        }
        DockDividerHitTarget::Corner(_) => panic!("single split should not resolve corner"),
    }
}

#[open_gpui::test]
fn divider_hit_map_prefers_corner_when_splitter_hits_intersect(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let left = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let top_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let bottom_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let vertical = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Vertical,
        children: vec![top_right, bottom_right],
        fractions: vec![0.5, 0.5],
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left, vertical],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(space(), root);

    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
        size(px(400.0), px(240.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(host_bounds(400.0, 240.0), cx)
    });

    let hit_map = DockDividerHitMap::from_scene(&scene);
    assert_eq!(scene.splitters.len(), 2);
    assert_eq!(hit_map.targets().len(), 3);
    let target = hit_map
        .hit(point(px(200.0), px(120.0)))
        .expect("splitter junction should hit");

    match target {
        DockDividerHitTarget::Corner(corner) => {
            assert_eq!(corner.horizontal.key.split, root);
            assert_eq!(corner.horizontal.key.axis, SplitAxis::Horizontal);
            assert_eq!(corner.vertical.key.split, vertical);
            assert_eq!(corner.vertical.key.axis, SplitAxis::Vertical);
            assert!(corner.bounds.contains(&point(px(200.0), px(120.0))));
        }
        DockDividerHitTarget::Single(_) => panic!("junction should prefer corner target"),
    }
}

#[open_gpui::test]
fn divider_corner_affordance_reports_visible_interaction_states(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let left = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let top_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let bottom_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let vertical = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Vertical,
        children: vec![top_right, bottom_right],
        fractions: vec![0.5, 0.5],
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left, vertical],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(space(), root);

    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
        size(px(400.0), px(240.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(host_bounds(400.0, 240.0), cx)
    });
    let hit_map = DockDividerHitMap::from_scene(&scene);

    let idle = hit_map.corner_affordances(Some(point(px(20.0), px(20.0))), false, true);
    assert_eq!(idle[0].state, DockDividerAffordanceState::Idle);

    let hover = hit_map.corner_affordances(Some(point(px(200.0), px(120.0))), false, true);
    assert_eq!(hover[0].state, DockDividerAffordanceState::Hover);

    let active = hit_map.corner_affordances(Some(point(px(200.0), px(120.0))), true, true);
    assert_eq!(active[0].state, DockDividerAffordanceState::Active);

    let disabled = hit_map.corner_affordances(Some(point(px(200.0), px(120.0))), true, false);
    assert_eq!(disabled[0].state, DockDividerAffordanceState::Disabled);
}
