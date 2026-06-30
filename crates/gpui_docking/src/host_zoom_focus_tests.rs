use crate::{
    SplitAxis,
    host_test_support::{open_host, space, split_graph},
    presentation_scene::DockPresentationOverlayAnchorKind,
    transition_geometry::{DockMotionPreference, DockTransitionEdge},
    zoom_state::{DockZoomScene, DockZoomState},
};
use open_gpui::{Bounds, TestAppContext, point, px, size};
use slotmap::Key;

fn host_bounds(width: f32, height: f32) -> Bounds<open_gpui::Pixels> {
    Bounds::new(point(px(0.0), px(0.0)), size(px(width), px(height)))
}

#[open_gpui::test]
fn zoom_scene_presents_target_at_full_bounds_without_mutating_base_scene(cx: &mut TestAppContext) {
    let (graph, _root, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.4, 0.6);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(500.0), px(240.0)),
    );
    let bounds = host_bounds(500.0, 240.0);
    let scene = host.update(cx, |host, cx| host.presentation_scene_for_test(bounds, cx));
    let original = scene.clone();

    let mut zoom = DockZoomState::default();
    zoom.zoom(space(), right_tabs);
    let zoomed = zoom
        .resolve(&scene, DockMotionPreference::Animated)
        .expect("target pane should zoom");

    assert_eq!(scene, original);
    assert_eq!(zoom.target(&space()), Some(right_tabs));
    assert_eq!(zoomed.target, right_tabs);
    assert_eq!(zoomed.scene.panes.len(), 1);
    assert_eq!(zoomed.scene.panes[0].node, Some(right_tabs));
    assert_eq!(zoomed.scene.panes[0].bounds, bounds);
    assert_eq!(zoomed.scene.tab_bars[0].bounds.origin, bounds.origin);
    assert_eq!(
        zoomed.scene.tab_bars[0].bounds.size.width,
        bounds.size.width
    );
    assert_eq!(zoomed.scene.tab_labels[0].bounds.origin, bounds.origin);
    assert_eq!(
        zoomed.scene.tab_labels[0].bounds.size.width,
        bounds.size.width
    );
    assert_eq!(
        zoomed.focus.as_ref().map(|focus| focus.bounds),
        Some(bounds)
    );
    assert!(
        zoomed.scene.overlay_anchors.iter().any(|anchor| {
            anchor.kind == DockPresentationOverlayAnchorKind::Pane
                && anchor.node == Some(right_tabs)
                && anchor.bounds == bounds
        }),
        "zoomed pane overlay anchor should use the zoomed scene bounds"
    );
    assert!(zoomed.scene.splitters.is_empty());
    assert_eq!(zoomed.egress.len(), 1);
    assert_eq!(zoomed.egress[0].node, left_tabs);
    assert_eq!(zoomed.egress[0].edge, DockTransitionEdge::Left);
    assert!(!zoomed.immediate);

    assert_eq!(
        zoom.unzoom(&space())
            .map(|presentation| presentation.target),
        Some(right_tabs)
    );
    assert_eq!(zoom.target(&space()), None);
}

#[open_gpui::test]
fn zoom_focus_descriptor_tracks_target_focus_and_reduced_motion(cx: &mut TestAppContext) {
    let (graph, _root, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(220.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(host_bounds(400.0, 220.0), cx)
    });

    let zoomed = DockZoomScene::from_scene(&scene, left_tabs, DockMotionPreference::Reduced)
        .expect("left pane should zoom");

    assert!(zoomed.immediate);
    assert_eq!(
        zoomed.focus.as_ref().map(|focus| focus.tabs),
        Some(left_tabs)
    );
    assert_eq!(zoomed.egress.len(), 1);
    assert_eq!(zoomed.egress[0].node, right_tabs);
    assert_eq!(
        zoomed.egress[0].edge,
        DockTransitionEdge::Right,
        "a pane touching the right host edge should prefer that edge over nearest distance"
    );
}

#[open_gpui::test]
fn zoom_state_clears_missing_target(cx: &mut TestAppContext) {
    let (graph, _root, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(220.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(host_bounds(400.0, 220.0), cx)
    });

    let mut zoom = DockZoomState::default();
    zoom.zoom(space(), right_tabs);
    assert!(!zoom.clear_missing_target(&space(), &scene));
    zoom.zoom(space(), crate::DockNodeId::null());
    assert!(zoom.clear_missing_target(&space(), &scene));
    assert_eq!(zoom.target(&space()), None);

    assert!(
        DockZoomScene::from_scene(
            &scene,
            crate::DockNodeId::null(),
            DockMotionPreference::Animated
        )
        .is_none()
    );
    assert!(DockZoomScene::from_scene(&scene, left_tabs, DockMotionPreference::Animated).is_some());
}
