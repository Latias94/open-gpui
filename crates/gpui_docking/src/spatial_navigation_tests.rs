use crate::{
    DockItemId, DockNodeId, DockSpaceId,
    presentation_scene::{
        DockPresentationFocusRegion, DockPresentationPane, DockPresentationPaneKind,
        DockPresentationScene,
    },
    spatial_navigation::{DockSpatialDirection, resolve_neighbor},
};
use open_gpui::{Bounds, point, px, size};

fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<open_gpui::Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

fn tabs(id: u64) -> DockNodeId {
    DockNodeId::from(slotmap::KeyData::from_ffi(id))
}

fn item(id: &str) -> DockItemId {
    DockItemId::from(id)
}

fn scene(panes: &[(DockNodeId, Bounds<open_gpui::Pixels>, &str)]) -> DockPresentationScene {
    DockPresentationScene {
        space: DockSpaceId::from("main"),
        bounds: bounds(0.0, 0.0, 500.0, 500.0),
        root: None,
        panes: panes
            .iter()
            .map(|(node, pane_bounds, _)| DockPresentationPane {
                node: Some(*node),
                kind: DockPresentationPaneKind::Tabs,
                bounds: *pane_bounds,
                floating: None,
                is_central: false,
            })
            .collect(),
        tab_bars: Vec::new(),
        tab_labels: Vec::new(),
        splitters: Vec::new(),
        floating_containers: Vec::new(),
        focus_regions: panes
            .iter()
            .map(|(node, pane_bounds, item_id)| DockPresentationFocusRegion {
                tabs: *node,
                item: item(item_id),
                bounds: *pane_bounds,
            })
            .collect(),
        overlay_anchors: Vec::new(),
    }
}

#[test]
fn spatial_navigation_prefers_perpendicular_overlap_before_distance() {
    let current = tabs(1);
    let high_overlap = tabs(2);
    let near_low_overlap = tabs(3);
    let scene = scene(&[
        (current, bounds(100.0, 100.0, 100.0, 100.0), "current"),
        (high_overlap, bounds(220.0, 110.0, 100.0, 80.0), "high"),
        (near_low_overlap, bounds(205.0, 10.0, 100.0, 40.0), "near"),
    ]);

    let target = resolve_neighbor(&scene, current, DockSpatialDirection::Right)
        .expect("right neighbor should resolve");

    assert_eq!(target.tabs, high_overlap);
    assert_eq!(target.item, Some(item("high")));
}

#[test]
fn spatial_navigation_uses_distance_as_tie_breaker() {
    let current = tabs(1);
    let near = tabs(2);
    let far = tabs(3);
    let scene = scene(&[
        (current, bounds(100.0, 100.0, 100.0, 100.0), "current"),
        (far, bounds(270.0, 100.0, 100.0, 100.0), "far"),
        (near, bounds(220.0, 100.0, 100.0, 100.0), "near"),
    ]);

    let target = resolve_neighbor(&scene, current, DockSpatialDirection::Right)
        .expect("right neighbor should resolve");

    assert_eq!(target.tabs, near);
    assert_eq!(target.item, Some(item("near")));
}

#[test]
fn spatial_navigation_returns_none_at_directional_edge() {
    let current = tabs(1);
    let right = tabs(2);
    let scene = scene(&[
        (current, bounds(100.0, 100.0, 100.0, 100.0), "current"),
        (right, bounds(220.0, 100.0, 100.0, 100.0), "right"),
    ]);

    assert_eq!(
        resolve_neighbor(&scene, current, DockSpatialDirection::Left),
        None
    );
}

#[test]
fn spatial_navigation_resolves_vertical_neighbors() {
    let current = tabs(1);
    let up = tabs(2);
    let down = tabs(3);
    let scene = scene(&[
        (up, bounds(100.0, 10.0, 100.0, 70.0), "up"),
        (current, bounds(100.0, 100.0, 100.0, 100.0), "current"),
        (down, bounds(100.0, 220.0, 100.0, 100.0), "down"),
    ]);

    assert_eq!(
        resolve_neighbor(&scene, current, DockSpatialDirection::Up)
            .expect("up neighbor should resolve")
            .tabs,
        up
    );
    assert_eq!(
        resolve_neighbor(&scene, current, DockSpatialDirection::Down)
            .expect("down neighbor should resolve")
            .tabs,
        down
    );
}
