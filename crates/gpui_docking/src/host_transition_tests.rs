use crate::{
    DropZone, SplitAxis,
    host_test_support::{floating_bounds, open_host, space, split_graph},
    overlay_scene::{DockOverlayLayer, DockOverlayLayerKind, DockOverlayScene},
    presentation_scene::{
        DockPresentationOverlayAnchor, DockPresentationOverlayAnchorKind, DockPresentationPane,
        DockPresentationPaneKind, DockPresentationScene,
    },
    transition_geometry::{
        DockDividerTransitionKind, DockMotionPreference, DockOverlayTransitionKind,
        DockPaneTransitionKind, DockTransitionEdge, DockTransitionPlan,
    },
};
use open_gpui::{Bounds, TestAppContext, point, px, size};
use slotmap::Key;

fn host_bounds(width: f32, height: f32) -> Bounds<open_gpui::Pixels> {
    Bounds::new(point(px(0.0), px(0.0)), size(px(width), px(height)))
}

fn single_pane_scene(
    node: crate::DockNodeId,
    bounds: Bounds<open_gpui::Pixels>,
) -> DockPresentationScene {
    DockPresentationScene {
        space: space(),
        bounds,
        root: Some(node),
        panes: vec![DockPresentationPane {
            node: Some(node),
            kind: DockPresentationPaneKind::Tabs,
            bounds,
            floating: None,
            is_central: false,
        }],
        tab_bars: Vec::new(),
        tab_labels: Vec::new(),
        splitters: Vec::new(),
        floating_containers: Vec::new(),
        focus_regions: Vec::new(),
        overlay_anchors: vec![DockPresentationOverlayAnchor {
            kind: DockPresentationOverlayAnchorKind::Root,
            node: Some(node),
            bounds,
        }],
    }
}

#[open_gpui::test]
fn transition_plan_describes_split_insertion_from_final_scene(cx: &mut TestAppContext) {
    let (graph, root, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    let bounds = host_bounds(400.0, 240.0);
    let previous = single_pane_scene(left_tabs, bounds);
    let next = host.update(cx, |host, cx| host.presentation_scene_for_test(bounds, cx));
    let plan = DockTransitionPlan::between(&previous, &next, DockMotionPreference::Animated);

    assert_eq!(plan.final_scene, next);
    let entering = plan
        .pane_transitions
        .iter()
        .find(|transition| transition.node == right_tabs)
        .expect("new right pane should enter");
    assert_eq!(entering.kind, DockPaneTransitionKind::Entering);
    assert_eq!(
        entering.to,
        next.pane_for_node(right_tabs).map(|pane| pane.bounds)
    );
    assert_eq!(
        entering.slide.as_ref().map(|slide| slide.edge),
        Some(DockTransitionEdge::Right)
    );
    assert_eq!(
        entering.slide.as_ref().map(|slide| slide.final_bounds),
        entering.to
    );
    assert_eq!(
        entering.slide.as_ref().map(|slide| slide.occlusion_bounds),
        entering.to
    );
    assert!(!entering.immediate);

    let resized = plan
        .pane_transitions
        .iter()
        .find(|transition| transition.node == left_tabs)
        .expect("existing left pane should resize");
    assert_eq!(resized.kind, DockPaneTransitionKind::Resizing);

    let divider = plan
        .divider_transitions
        .iter()
        .find(|transition| transition.split == root)
        .expect("new split divider should appear");
    assert_eq!(divider.kind, DockDividerTransitionKind::Appearing);
    assert_eq!(divider.axis, SplitAxis::Horizontal);
}

#[open_gpui::test]
fn transition_plan_marks_reduced_motion_immediate_without_changing_final_scene(
    cx: &mut TestAppContext,
) {
    let (graph, _root, left_tabs, _right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    let bounds = host_bounds(400.0, 240.0);
    let previous = single_pane_scene(left_tabs, bounds);
    let next = host.update(cx, |host, cx| host.presentation_scene_for_test(bounds, cx));
    let plan = DockTransitionPlan::between(&previous, &next, DockMotionPreference::Reduced);

    assert_eq!(plan.final_scene, next);
    assert!(plan.is_immediate());
    assert!(
        plan.pane_transitions
            .iter()
            .all(|transition| transition.immediate)
    );
    assert!(
        plan.divider_transitions
            .iter()
            .all(|transition| transition.immediate)
    );
}

#[test]
fn transition_plan_from_overlay_scene_describes_tab_insertion_and_payload_ghosts() {
    let tabs = crate::DockNodeId::null();
    let scene = single_pane_scene(tabs, host_bounds(320.0, 200.0));
    let overlay = DockOverlayScene {
        layers: vec![
            DockOverlayLayer {
                kind: DockOverlayLayerKind::TabInsertion,
                bounds: floating_bounds(8.0, 0.0, 3.0, 26.0),
                target_node: Some(tabs),
                zone: Some(DropZone::Center),
                preview_layer: None,
                active: true,
                payload_index: None,
                payload_title: None,
                drop_box: None,
                tab_insertion: None,
            },
            DockOverlayLayer {
                kind: DockOverlayLayerKind::PayloadTab,
                bounds: floating_bounds(10.0, 0.0, 90.0, 26.0),
                target_node: Some(tabs),
                zone: Some(DropZone::Center),
                preview_layer: None,
                active: true,
                payload_index: Some(0),
                payload_title: Some("Preview".to_string()),
                drop_box: None,
                tab_insertion: None,
            },
        ],
    };

    let plan =
        DockTransitionPlan::from_overlay_scene(&scene, &overlay, DockMotionPreference::Animated);

    assert!(plan.pane_transitions.is_empty());
    assert_eq!(plan.overlay_transitions.len(), 2);
    assert_eq!(
        plan.overlay_transitions[0].kind,
        DockOverlayTransitionKind::TabInsertion
    );
    assert_eq!(plan.overlay_transitions[0].target_node, Some(tabs));
    assert_eq!(plan.overlay_transitions[0].zone, Some(DropZone::Center));
    assert_eq!(
        plan.overlay_transitions[1].kind,
        DockOverlayTransitionKind::PayloadGhost
    );
    assert_eq!(plan.overlay_transitions[1].payload_index, Some(0));
}

#[test]
fn transition_plan_from_rejected_overlay_is_rejected_noop() {
    let tabs = crate::DockNodeId::null();
    let scene = single_pane_scene(tabs, host_bounds(320.0, 200.0));
    let overlay = DockOverlayScene {
        layers: vec![DockOverlayLayer {
            kind: DockOverlayLayerKind::RejectedState,
            bounds: scene.bounds,
            target_node: Some(tabs),
            zone: None,
            preview_layer: None,
            active: true,
            payload_index: None,
            payload_title: None,
            drop_box: None,
            tab_insertion: None,
        }],
    };

    let plan =
        DockTransitionPlan::from_overlay_scene(&scene, &overlay, DockMotionPreference::Reduced);

    assert!(plan.pane_transitions.is_empty());
    assert_eq!(plan.overlay_transitions.len(), 1);
    assert_eq!(
        plan.overlay_transitions[0].kind,
        DockOverlayTransitionKind::RejectedNoop
    );
    assert!(plan.is_immediate());
}
