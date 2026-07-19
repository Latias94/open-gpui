use crate::{
    DockFloatingContainer, DockGraph, DockNode, SplitAxis,
    divider_hit_map::{
        DockDividerAffordanceState, DockDividerHitMap, DockDividerHitTarget, DockDividerSurface,
    },
    host_test_support::{floating_bounds, item, open_host, space, split_graph},
    visual_affordance_scene::{
        DockVisualAffordanceKind, DockVisualAffordanceScene, DockVisualAffordanceState,
    },
};
use open_gpui::{Bounds, TestAppContext, point, px, size};
use open_gpui_ui_core::AccessibleAction;

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
fn divider_hit_map_targets_match_scene_splitter_bounds(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let left = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let middle = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let right = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left, middle, right],
        fractions: vec![0.2, 0.3, 0.5],
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
        size(px(1000.0), px(240.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(host_bounds(1000.0, 240.0), cx)
    });
    let hit_map = DockDividerHitMap::from_scene(&scene);

    assert_eq!(scene.splitters.len(), 2);
    for splitter in &scene.splitters {
        let target = hit_map
            .hit(splitter.bounds.center())
            .unwrap_or_else(|| panic!("splitter {} should have hit target", splitter.index));
        let DockDividerHitTarget::Single(handle) = target else {
            panic!("same-axis splitters should not resolve as corners");
        };
        assert_eq!(handle.key.split, root);
        assert_eq!(handle.key.index, splitter.index);
        assert_eq!(handle.bounds, splitter.bounds);
        assert_eq!(handle.extent, splitter.extent);
    }
}

#[open_gpui::test]
fn divider_hit_map_targets_match_scene_after_fraction_update(cx: &mut TestAppContext) {
    let (graph, root, _left, _right) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );
    let initial_scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(host_bounds(400.0, 240.0), cx)
    });
    host.update(cx, |host, _| {
        host.set_last_presentation_scene(initial_scene)
    });

    let resized = host.update(cx, |host, cx| {
        host.resize_splitter_from_accessibility(
            root,
            SplitAxis::Horizontal,
            0,
            AccessibleAction::Increment,
            cx,
        )
    });
    assert!(resized, "resize should update split fractions");

    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(host_bounds(400.0, 240.0), cx)
    });
    let splitter = scene
        .splitters
        .first()
        .expect("updated scene should keep a splitter");
    let hit_map = DockDividerHitMap::from_scene(&scene);
    let target = hit_map
        .hit(splitter.bounds.center())
        .expect("updated splitter should have hit target");
    let DockDividerHitTarget::Single(handle) = target else {
        panic!("single updated splitter should not resolve as corner");
    };
    assert_eq!(handle.key.split, root);
    assert_eq!(handle.bounds, splitter.bounds);
    assert_eq!(handle.extent, splitter.extent);
}

#[open_gpui::test]
fn divider_event_scene_uses_zoom_resolved_splitters(cx: &mut TestAppContext) {
    let (graph, _root, _left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.25, 0.75);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    assert!(host.update(cx, |host, cx| host.zoom_pane(right_tabs, cx)));
    cx.run_until_parked();

    let scene = host.update(cx, |host, cx| {
        host.divider_event_scene_for_test(host_bounds(400.0, 240.0), cx)
    });
    assert!(
        scene.splitters.is_empty(),
        "zoomed divider event scene must not expose hidden base splitters"
    );
    assert!(
        DockDividerHitMap::from_scene(&scene).targets().is_empty(),
        "zoomed divider hit map must not resolve hidden base splitters"
    );
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
fn divider_hit_map_resolves_only_the_topmost_visual_surface(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let root_left = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let root_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let root_split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![root_left, root_right],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(space(), root_split);

    let floating_top = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let floating_bottom = graph.insert_node(DockNode::Tabs {
        items: vec![item("d")],
        selected: Some(item("d")),
    });
    let floating_split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Vertical,
        children: vec![floating_top, floating_bottom],
        fractions: vec![0.5, 0.5],
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_split,
    });
    graph
        .floating_containers_mut(space())
        .push(DockFloatingContainer {
            node: floating,
            bounds: floating_bounds(150.0, 40.0, 100.0, 160.0),
        });

    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
            ("d", "Panel D", "D"),
        ],
        size(px(400.0), px(240.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(host_bounds(400.0, 240.0), cx)
    });
    let root_handle = scene
        .splitters
        .iter()
        .find(|splitter| splitter.split == root_split)
        .expect("root splitter should be present");
    let floating_handle = scene
        .splitters
        .iter()
        .find(|splitter| splitter.split == floating_split)
        .expect("floating splitter should be present");
    let floating_scene = scene
        .floating_containers
        .first()
        .expect("floating surface should be present");
    let hit_map = DockDividerHitMap::from_scene(&scene);

    let covered_root_point = point(
        root_handle.bounds.center().x,
        floating_scene.title_bar_bounds.center().y,
    );
    assert!(root_handle.bounds.contains(&covered_root_point));
    assert!(floating_scene.bounds.contains(&covered_root_point));
    assert!(
        hit_map.hit(covered_root_point).is_none(),
        "floating chrome must occlude a root splitter even when it has no divider there"
    );

    let overlap = point(
        root_handle.bounds.center().x,
        floating_handle.bounds.center().y,
    );
    assert!(root_handle.bounds.contains(&overlap));
    assert!(floating_handle.bounds.contains(&overlap));
    let DockDividerHitTarget::Single(handle) = hit_map
        .hit(overlap)
        .expect("the floating splitter should own the overlap")
    else {
        panic!("root and floating splitters must not synthesize a cross-surface corner");
    };
    assert_eq!(handle.key.split, floating_split);
    assert_eq!(handle.surface, DockDividerSurface::Floating(floating));
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

#[open_gpui::test]
fn visual_affordance_scene_maps_divider_handles_and_corner_state(cx: &mut TestAppContext) {
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

    let visual = DockVisualAffordanceScene::from_divider_hit_map(
        &scene,
        &hit_map,
        Some(point(px(200.0), px(120.0))),
        true,
        true,
    );

    assert!(
        visual.layers.iter().any(|layer| {
            layer.kind == DockVisualAffordanceKind::DividerCorner
                && layer.state == DockVisualAffordanceState::Active
                && layer.target_node == Some(root)
        }),
        "corner drag should expose an active two-axis divider affordance"
    );
    assert!(
        visual
            .layers
            .iter()
            .filter(|layer| layer.kind == DockVisualAffordanceKind::DividerHandle)
            .count()
            >= 2,
        "corner affordance should include the contributing divider handles"
    );
    assert!(
        visual
            .layers
            .iter()
            .all(|layer| layer.id == layer.motion_key),
        "divider affordances should have stable motion identities"
    );
}
