use crate::{
    DockCentralRegion, DockGraph, DockNode, SplitAxis,
    drop_runtime::DockHostDropSceneFact,
    drop_scene_fact::presentation_scene_drop_facts,
    host_test_support::{
        assert_close, floating_bounds, floating_overlay_graph, item, open_host, open_workspace,
        space, split_graph, tabs_graph_with_selected, workspace_with_panels,
    },
    presentation_scene::{DockPresentationOverlayAnchorKind, DockPresentationPaneKind},
    transition_geometry::DockMotionPreference,
    visual_affordance_scene::{
        DockVisualAffordanceKind, DockVisualAffordanceScene, DockVisualAffordanceState,
    },
    zoom_state::DockZoomScene,
};
use open_gpui::{Bounds, TestAppContext, point, px, size};

fn host_bounds(width: f32, height: f32) -> Bounds<open_gpui::Pixels> {
    Bounds::new(point(px(0.0), px(0.0)), size(px(width), px(height)))
}

#[open_gpui::test]
fn presentation_scene_resolves_root_split_to_flat_absolute_geometry(cx: &mut TestAppContext) {
    let (graph, root, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.25, 0.75);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    let scene_bounds = host_bounds(400.0, 240.0);
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(scene_bounds, cx)
    });

    assert_eq!(scene.space, space());
    assert_eq!(scene.root, Some(root));
    assert_eq!(scene.bounds, scene_bounds);
    assert_eq!(scene.panes.len(), 2);
    assert_eq!(scene.tab_bars.len(), 2);
    assert_eq!(scene.tab_labels.len(), 2);
    assert_eq!(scene.splitters.len(), 1);

    let left = scene
        .pane_for_node(left_tabs)
        .expect("left tabs pane should be resolved");
    let right = scene
        .pane_for_node(right_tabs)
        .expect("right tabs pane should be resolved");
    assert_eq!(left.kind, DockPresentationPaneKind::Tabs);
    assert_eq!(right.kind, DockPresentationPaneKind::Tabs);
    assert_close(f32::from(left.bounds.size.width), 100.0);
    assert_close(f32::from(right.bounds.origin.x), 100.0);
    assert_close(f32::from(right.bounds.size.width), 300.0);

    let splitter = &scene.splitters[0];
    assert_eq!(splitter.split, root);
    assert_eq!(splitter.index, 0);
    assert_eq!(splitter.axis, SplitAxis::Horizontal);
    assert_eq!(splitter.before, left_tabs);
    assert_eq!(splitter.after, right_tabs);
    assert!(splitter.bounds.contains(&point(px(100.0), px(120.0))));

    assert!(scene.overlay_anchors.iter().any(|anchor| anchor.kind
        == DockPresentationOverlayAnchorKind::Root
        && anchor.node == Some(root)));
    assert!(scene.overlay_anchors.iter().any(|anchor| anchor.kind
        == DockPresentationOverlayAnchorKind::Splitter
        && anchor.node == Some(root)));
}

#[open_gpui::test]
fn presentation_scene_resolves_tab_labels_focus_and_does_not_mutate_selection(
    cx: &mut TestAppContext,
) {
    let (graph, root) = tabs_graph_with_selected(&["a", "b", "c"], "b");
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
        size(px(360.0), px(200.0)),
    );

    let scene_bounds = host_bounds(360.0, 200.0);
    let first = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(scene_bounds, cx)
    });
    let second = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(scene_bounds, cx)
    });

    assert_eq!(first, second);
    assert_eq!(first.tab_labels.len(), 3);
    assert_eq!(
        first
            .tab_labels
            .iter()
            .map(|label| (label.index, label.item.clone(), label.title.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (0, item("a"), "Panel A"),
            (1, item("b"), "Panel B"),
            (2, item("c"), "Panel C"),
        ]
    );
    assert_eq!(first.focus_regions.len(), 1);
    assert_eq!(first.focus_regions[0].tabs, root);
    assert_eq!(first.focus_regions[0].item, item("b"));

    let tab_bar = first
        .tab_bar_for_node(root)
        .expect("root tab bar should be resolved");
    assert_eq!(tab_bar.tabs, root);
    assert_close(f32::from(tab_bar.bounds.size.width), 360.0);
}

#[open_gpui::test]
fn visual_affordance_scene_maps_focus_and_zoom_egress(cx: &mut TestAppContext) {
    let (graph, _root, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.25, 0.75);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(host_bounds(400.0, 240.0), cx)
    });

    let focus_visual = DockVisualAffordanceScene::from_focus_scene(&scene);

    assert!(
        focus_visual.layers.iter().any(|layer| {
            layer.kind == DockVisualAffordanceKind::FocusRing
                && layer.target_node == Some(left_tabs)
                && layer.state == DockVisualAffordanceState::Active
        }),
        "focus regions should become focus-ring affordances"
    );

    let zoom = DockZoomScene::from_scene(&scene, left_tabs, DockMotionPreference::Reduced)
        .expect("left pane should produce a zoom scene");
    let zoom_visual = DockVisualAffordanceScene::from_zoom_scene(&zoom);

    assert!(
        zoom_visual.layers.iter().any(|layer| {
            layer.kind == DockVisualAffordanceKind::ZoomEgress
                && layer.target_node == Some(right_tabs)
                && layer.state == DockVisualAffordanceState::CommittedPreview
        }),
        "reduced-motion zoom should still expose final egress descriptors"
    );
    assert!(
        zoom_visual.layers.iter().any(|layer| {
            layer.kind == DockVisualAffordanceKind::FocusRing
                && layer.target_node == Some(left_tabs)
                && layer.state == DockVisualAffordanceState::Active
        }),
        "zoom focus should share the same focus-ring affordance contract"
    );
}

#[open_gpui::test]
fn presentation_scene_resolves_floating_container_separately(cx: &mut TestAppContext) {
    let (graph, root, floating) = floating_overlay_graph();
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(500.0), px(300.0)),
    );

    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(host_bounds(500.0, 300.0), cx)
    });

    assert_eq!(scene.root, Some(root));
    assert_eq!(scene.floating_containers.len(), 1);
    assert_eq!(scene.floating_containers[0].node, floating);
    assert_eq!(
        scene.floating_containers[0].bounds,
        floating_bounds(10.0, 20.0, 220.0, 140.0)
    );
    assert_eq!(scene.panes.len(), 2);
    assert!(
        scene
            .panes
            .iter()
            .any(|pane| pane.node == Some(root) && pane.floating.is_none())
    );
    assert!(
        scene
            .panes
            .iter()
            .any(|pane| pane.floating == Some(floating))
    );
    assert!(scene.overlay_anchors.iter().any(|anchor| anchor.kind
        == DockPresentationOverlayAnchorKind::FloatingTitleBar
        && anchor.node == Some(floating)));
}

#[open_gpui::test]
fn presentation_scene_exports_viewport_drop_facts(cx: &mut TestAppContext) {
    let (graph, root, floating) = floating_overlay_graph();
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(500.0), px(300.0)),
    );

    let bounds = host_bounds(500.0, 300.0);
    let (scene, facts) = host.update(cx, |host, cx| {
        let session = host.render_session(cx);
        let scene = host.presentation_scene_for_test(bounds, cx);
        let facts = presentation_scene_drop_facts(&scene, &session);
        (scene, facts)
    });

    assert!(
        facts
            .iter()
            .any(|fact| matches!(fact, DockHostDropSceneFact::Root(target)
                if target.root == root && target.bounds == bounds)),
        "root fact should come from the scene bounds"
    );
    for pane in scene
        .panes
        .iter()
        .filter_map(|pane| pane.node.map(|node| (node, pane)))
    {
        assert!(
            facts
                .iter()
                .any(|fact| matches!(fact, DockHostDropSceneFact::Leaf(target)
                    if target.target_tabs == pane.0 && target.bounds == pane.1.bounds)),
            "leaf fact for pane {:?} should use scene pane bounds",
            pane.0
        );
    }
    for tab_bar in &scene.tab_bars {
        assert!(
            facts
                .iter()
                .any(|fact| matches!(fact, DockHostDropSceneFact::TabBar(target)
                    if target.target_tabs == tab_bar.tabs && target.bounds == tab_bar.bounds)),
            "tab bar fact for {:?} should use scene tab bar bounds",
            tab_bar.tabs
        );
    }
    assert!(
        facts.iter().any(
            |fact| matches!(fact, DockHostDropSceneFact::FloatingTitleBar(target)
                if target.floating == floating
                    && scene
                        .floating_containers
                        .iter()
                        .any(|container| container.node == floating
                            && target.title_bounds == container.title_bar_bounds
                            && target.preview_bounds == container.bounds))
        ),
        "floating title fact should use scene floating bounds"
    );
}

#[open_gpui::test]
fn rendered_host_scene_frame_seeds_deterministic_facts_from_presentation_scene(
    cx: &mut TestAppContext,
) {
    let (graph, root, floating) = floating_overlay_graph();
    let (window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(500.0), px(300.0)),
    );

    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(host_bounds(500.0, 300.0), cx)
    });
    let drop_scene = host
        .update(cx, |host, _| {
            host.viewport_runtime()
                .borrow()
                .rendered_host_drop_scene_for_window(host.space(), window.window_id())
        })
        .expect("rendered host drop scene should be registered");

    let root_target = drop_scene
        .root
        .expect("rendered scene should include scene-owned root fact");
    assert_eq!(root_target.root, root);
    assert_eq!(root_target.bounds, scene.bounds);

    for pane in scene
        .panes
        .iter()
        .filter(|pane| pane.kind == DockPresentationPaneKind::Tabs)
    {
        let target_tabs = pane.node.expect("tabs pane should have node id");
        let leaf = drop_scene
            .leaves
            .iter()
            .find(|leaf| leaf.target_tabs == target_tabs)
            .unwrap_or_else(|| panic!("leaf fact for {target_tabs:?} should be scene-owned"));
        assert_eq!(leaf.bounds, pane.bounds);
        assert_eq!(leaf.is_central, pane.is_central);
    }

    for tab_bar in &scene.tab_bars {
        let tab_bar_fact = drop_scene
            .tab_bars
            .iter()
            .find(|target| target.target_tabs == tab_bar.tabs)
            .unwrap_or_else(|| panic!("tab-bar fact for {:?} should be scene-owned", tab_bar.tabs));
        assert_eq!(tab_bar_fact.bounds, tab_bar.bounds);
        assert_eq!(tab_bar_fact.is_central, tab_bar.is_central);
    }

    let floating_container = scene
        .floating_containers
        .iter()
        .find(|container| container.node == floating)
        .expect("floating container should be in presentation scene");
    let floating_fact = drop_scene
        .floating_title_bars
        .iter()
        .find(|target| target.floating == floating)
        .expect("floating title fact should be scene-owned");
    assert_eq!(
        floating_fact.title_bounds,
        floating_container.title_bar_bounds
    );
    assert_eq!(floating_fact.preview_bounds, floating_container.bounds);
}

#[open_gpui::test]
fn rendered_empty_central_frame_seeds_scene_owned_empty_space_fact(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    graph.set_central_region(space(), DockCentralRegion::empty());
    let (window, host, _visual) = open_host(cx, graph, &[], size(px(320.0), px(180.0)));

    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(host_bounds(320.0, 180.0), cx)
    });
    let drop_scene = host
        .update(cx, |host, _| {
            host.viewport_runtime()
                .borrow()
                .rendered_host_drop_scene_for_window(host.space(), window.window_id())
        })
        .expect("rendered host drop scene should be registered");

    assert!(drop_scene.root.is_none());
    assert!(drop_scene.leaves.is_empty());
    assert!(drop_scene.tab_bars.is_empty());
    assert!(drop_scene.floating_title_bars.is_empty());
    let empty = drop_scene
        .empty_spaces
        .first()
        .expect("empty central fact should be scene-owned");
    assert_eq!(empty.bounds, scene.bounds);
    assert!(empty.is_central);
}

#[open_gpui::test]
fn rendered_empty_root_frame_seeds_non_central_empty_space_fact(cx: &mut TestAppContext) {
    let graph = DockGraph::new();
    let (window, host, _visual) = open_host(cx, graph, &[], size(px(320.0), px(180.0)));

    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(host_bounds(320.0, 180.0), cx)
    });
    let drop_scene = host
        .update(cx, |host, _| {
            host.viewport_runtime()
                .borrow()
                .rendered_host_drop_scene_for_window(host.space(), window.window_id())
        })
        .expect("rendered host drop scene should be registered");

    assert!(scene.panes.is_empty());
    assert!(drop_scene.root.is_none());
    let empty = drop_scene
        .empty_spaces
        .first()
        .expect("empty root fact should be scene-owned");
    assert_eq!(empty.bounds, scene.bounds);
    assert!(!empty.is_central);
}

#[open_gpui::test]
fn presentation_scene_resolves_empty_central_region(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    graph.set_central_region(space(), DockCentralRegion::empty());
    let workspace = workspace_with_panels(cx, graph, &[]);
    let (_window, host, _visual) = open_workspace(cx, workspace, size(px(320.0), px(180.0)));

    let scene_bounds = host_bounds(320.0, 180.0);
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(scene_bounds, cx)
    });

    assert_eq!(scene.root, None);
    assert_eq!(scene.panes.len(), 1);
    assert_eq!(scene.panes[0].node, None);
    assert_eq!(scene.panes[0].kind, DockPresentationPaneKind::EmptyCentral);
    assert_eq!(scene.panes[0].bounds, scene_bounds);
    assert!(scene.panes[0].is_central);
    assert!(scene.overlay_anchors.iter().any(|anchor| anchor.kind
        == DockPresentationOverlayAnchorKind::EmptyCentral
        && anchor.bounds == scene_bounds));
}

#[open_gpui::test]
fn presentation_scene_uses_central_child_fraction_for_resolved_grid(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let left = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let central = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let right = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left, central, right],
        fractions: vec![0.2, 0.0, 0.3],
    });
    graph.set_root(space(), root);
    graph.set_central_region(space(), DockCentralRegion::with_node(central));

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

    let left_pane = scene.pane_for_node(left).expect("left pane");
    let central_pane = scene.pane_for_node(central).expect("central pane");
    let right_pane = scene.pane_for_node(right).expect("right pane");
    assert_close(f32::from(left_pane.bounds.size.width), 200.0);
    assert_close(f32::from(central_pane.bounds.size.width), 500.0);
    assert_close(f32::from(right_pane.bounds.size.width), 300.0);
    assert!(central_pane.is_central);

    assert_eq!(scene.splitters.len(), 2);
    assert_close(f32::from(scene.splitters[0].bounds.origin.x), 197.0);
    assert_close(f32::from(scene.splitters[0].bounds.size.width), 6.0);
    assert_close(f32::from(scene.splitters[1].bounds.origin.x), 697.0);
    assert_close(f32::from(scene.splitters[1].bounds.size.width), 6.0);
}
