use crate::{
    DockTransitionExecutionState, DockViewportDropRoute, DockViewportRouteSelectionSource,
    DockViewportTargetHit, DropZone, SplitAxis,
    drop_preview::DockDropRoutePreview,
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
    viewport_test_support::handle,
    visual_affordance_scene::DockVisualAffordanceScene,
};
use open_gpui::{Bounds, TestAppContext, point, px, size};
use open_gpui_ui_core::{MotionDuration, MotionEasing, MotionPreference, MotionSpec};
use slotmap::Key;
use std::time::Duration;

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

fn tab_preview_overlay_layer(
    tabs: crate::DockNodeId,
    kind: DockOverlayLayerKind,
    bounds: Bounds<open_gpui::Pixels>,
    zone: Option<DropZone>,
    payload_index: Option<usize>,
) -> DockOverlayLayer {
    DockOverlayLayer {
        kind,
        bounds,
        target_node: Some(tabs),
        zone,
        preview_layer: None,
        active: true,
        payload_index,
        payload_title: payload_index.map(|_| "Preview".to_string()),
        drop_box: None,
        tab_insertion: None,
    }
}

fn transition_plan_from_overlay_scene(
    scene: &DockPresentationScene,
    overlay: &DockOverlayScene,
    preference: DockMotionPreference,
) -> DockTransitionPlan {
    let affordance_scene = DockVisualAffordanceScene::from_overlay_scene(overlay);
    DockTransitionPlan::from_visual_affordance_scene(scene, &affordance_scene, preference)
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

#[open_gpui::test]
fn transition_executor_reduces_or_schedules_without_changing_final_scene(cx: &mut TestAppContext) {
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
    let animated = DockTransitionPlan::between(&previous, &next, DockMotionPreference::Animated);
    let animated_state = host.update(cx, |host, cx| {
        host.execute_transition_plan(
            animated,
            MotionSpec::layout(DockMotionPreference::Animated),
            None,
            cx,
        )
    });
    assert_eq!(animated_state, DockTransitionExecutionState::Scheduled);
    let stored = host
        .update(cx, |host, _| host.clear_transition_execution_for_test())
        .expect("animated transition should be stored");
    assert_eq!(stored.plan.final_scene, next);

    let reduced = DockTransitionPlan::between(&previous, &next, DockMotionPreference::Reduced);
    let reduced_sample = host.update(cx, |host, cx| {
        let reduced_state = host.execute_transition_plan(
            reduced,
            MotionSpec::layout(DockMotionPreference::Reduced),
            None,
            cx,
        );
        assert_eq!(reduced_state, DockTransitionExecutionState::Immediate);
        host.sample_transition_for_test(Duration::from_millis(0))
            .expect("reduced transition should expose a final sample")
    });
    assert_eq!(reduced_sample.final_scene, next);
    assert_eq!(reduced_sample.progress, 1.0);
    assert!(reduced_sample.complete);
}

#[open_gpui::test]
fn transition_executor_samples_timeline_and_reveal_geometry(cx: &mut TestAppContext) {
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
    let spec = MotionSpec::new(
        MotionPreference::Animated,
        MotionDuration::Custom(Duration::from_millis(200)),
        MotionEasing::Linear,
    );

    host.update(cx, |host, cx| {
        assert_eq!(
            host.execute_transition_plan(plan, spec, None, cx),
            DockTransitionExecutionState::Scheduled
        );

        let start = host
            .sample_transition_for_test(Duration::from_millis(0))
            .expect("animated execution should expose a start sample");
        assert_eq!(start.progress, 0.0);
        assert!(!start.complete);
        assert!(start.needs_frame);
        assert_eq!(start.final_scene, next);
        let entering = start
            .pane_clips
            .iter()
            .find(|clip| clip.node == right_tabs)
            .expect("entering pane should expose reveal clip");
        let final_bounds = next
            .pane_for_node(right_tabs)
            .expect("right pane should be in final scene")
            .bounds;
        assert_eq!(
            entering.content_bounds, final_bounds,
            "entering pane content must be final-size from the first frame"
        );
        assert_eq!(
            entering.occlusion_bounds, final_bounds,
            "transition occlusion should be descriptor-driven and cover the final-size pane path"
        );
        assert_eq!(entering.visible_bounds.size.width, px(0.0));
        assert_eq!(
            entering.visible_bounds.size.height,
            final_bounds.size.height
        );

        let midpoint = host
            .sample_transition_for_test(Duration::from_millis(100))
            .expect("animated execution should expose a midpoint sample");
        assert_eq!(midpoint.progress, 0.5);
        assert!(!midpoint.complete);
        assert!(midpoint.needs_frame);
        let midpoint_clip = midpoint
            .pane_clips
            .iter()
            .find(|clip| clip.node == right_tabs)
            .expect("entering pane should still expose reveal clip");
        assert_eq!(midpoint_clip.content_bounds, final_bounds);
        assert_eq!(
            midpoint_clip.visible_bounds.size.width,
            final_bounds.size.width * 0.5
        );
        assert_eq!(
            midpoint_clip.visible_bounds.size.height,
            final_bounds.size.height
        );
        assert!(
            midpoint
                .dividers
                .iter()
                .any(|divider| divider.split == root && divider.progress == 0.5),
            "appearing divider should sample through midpoint progress"
        );

        let end = host
            .sample_transition_for_test(Duration::from_millis(200))
            .expect("completion sample should be returned before clearing");
        assert_eq!(end.progress, 1.0);
        assert!(end.complete);
        assert!(!end.needs_frame);
        assert_eq!(end.final_scene, next);
        assert!(
            host.sample_transition_for_test(Duration::from_millis(201))
                .is_none(),
            "completed transition should clear itself after the completion sample"
        );
    });
}

#[open_gpui::test]
fn transition_executor_replaces_active_execution_and_completes_reduced_motion_immediately(
    cx: &mut TestAppContext,
) {
    let (graph, _root, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    let bounds = host_bounds(400.0, 240.0);
    let previous = single_pane_scene(left_tabs, bounds);
    let next = host.update(cx, |host, cx| host.presentation_scene_for_test(bounds, cx));
    let animated = DockTransitionPlan::between(&previous, &next, DockMotionPreference::Animated);
    let replacement = DockTransitionPlan::between(&previous, &next, DockMotionPreference::Animated);

    host.update(cx, |host, cx| {
        assert_eq!(
            host.execute_transition_plan(
                animated,
                MotionSpec::new(
                    MotionPreference::Animated,
                    MotionDuration::Custom(Duration::from_millis(400)),
                    MotionEasing::Linear,
                ),
                None,
                cx,
            ),
            DockTransitionExecutionState::Scheduled
        );
        assert!(
            host.sample_transition_for_test(Duration::from_millis(0))
                .is_some()
        );
        let midpoint = host
            .sample_transition_for_test(Duration::from_millis(100))
            .expect("active transition should expose midpoint geometry");
        let midpoint_bounds = midpoint
            .pane_bounds
            .iter()
            .find(|pane| pane.node == right_tabs)
            .expect("active transition should expose entering pane visual bounds")
            .clone();

        assert_eq!(
            host.execute_transition_plan(
                replacement,
                MotionSpec::new(
                    MotionPreference::Animated,
                    MotionDuration::Custom(Duration::from_millis(200)),
                    MotionEasing::Linear,
                ),
                None,
                cx,
            ),
            DockTransitionExecutionState::Scheduled
        );
        let sample = host
            .sample_transition_for_test(Duration::from_millis(100))
            .expect("replacement transition should retarget from current geometry");
        assert_eq!(
            sample.progress, 0.0,
            "retargeted transition starts a new timeline from sampled geometry"
        );
        let retargeted_bounds = sample
            .pane_bounds
            .iter()
            .find(|pane| pane.node == right_tabs)
            .expect("replacement transition should expose pane visual bounds");
        assert_eq!(
            retargeted_bounds.bounds, midpoint_bounds.bounds,
            "replacement transition should begin from sampled pane visual geometry"
        );

        let reduced = DockTransitionPlan::between(&previous, &next, DockMotionPreference::Reduced);
        assert_eq!(
            host.execute_transition_plan(reduced, MotionSpec::immediate(), None, cx),
            DockTransitionExecutionState::Immediate
        );
        let reduced_sample = host
            .sample_transition_for_test(Duration::from_millis(999))
            .expect("reduced transition should expose a final sample once");
        assert_eq!(reduced_sample.progress, 1.0);
        assert!(reduced_sample.complete);
        assert!(!reduced_sample.needs_frame);
        assert!(
            host.sample_transition_for_test(Duration::from_millis(1000))
                .is_none(),
            "reduced transition should clear after final sample"
        );
    });
}

#[test]
fn transition_plan_from_overlay_scene_describes_tab_insertion_and_payload_tabs() {
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
            DockOverlayLayer {
                kind: DockOverlayLayerKind::PayloadGhost,
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

    let affordance_scene = DockVisualAffordanceScene::from_overlay_scene(&overlay);
    let plan = DockTransitionPlan::from_visual_affordance_scene(
        &scene,
        &affordance_scene,
        DockMotionPreference::Animated,
    );

    assert!(plan.pane_transitions.is_empty());
    assert_eq!(plan.overlay_transitions.len(), 3);
    assert_eq!(
        plan.overlay_transitions[0].kind,
        DockOverlayTransitionKind::TabInsertion
    );
    assert_eq!(plan.overlay_transitions[0].target_node, Some(tabs));
    assert_eq!(plan.overlay_transitions[0].zone, Some(DropZone::Center));
    assert_eq!(
        plan.overlay_transitions[1].kind,
        DockOverlayTransitionKind::PayloadTab
    );
    assert_eq!(plan.overlay_transitions[1].payload_index, Some(0));
    assert_eq!(
        plan.overlay_transitions[2].kind,
        DockOverlayTransitionKind::PayloadGhost
    );
    assert_eq!(plan.overlay_transitions[2].payload_index, Some(0));
    for (transition, layer) in plan
        .overlay_transitions
        .iter()
        .zip(affordance_scene.layers.iter())
    {
        assert_eq!(
            transition.motion_key, layer.motion_key,
            "overlay motion should use visual affordance identity"
        );
    }
}

#[test]
fn transition_plan_from_overlay_scene_uses_current_bounds_for_matching_layers() {
    let tabs = crate::DockNodeId::null();
    let scene = single_pane_scene(tabs, host_bounds(320.0, 200.0));
    let next = DockOverlayScene {
        layers: vec![DockOverlayLayer {
            kind: DockOverlayLayerKind::GuideBox,
            bounds: floating_bounds(40.0, 20.0, 90.0, 48.0),
            target_node: Some(tabs),
            zone: Some(DropZone::Left),
            preview_layer: None,
            active: true,
            payload_index: None,
            payload_title: None,
            drop_box: None,
            tab_insertion: None,
        }],
    };

    let plan = transition_plan_from_overlay_scene(&scene, &next, DockMotionPreference::Animated);

    assert_eq!(plan.overlay_transitions.len(), 1);
    assert_eq!(plan.overlay_transitions[0].bounds, next.layers[0].bounds);
}

#[test]
fn transition_plan_keeps_preview_layers_at_current_target_bounds() {
    let tabs = crate::DockNodeId::null();
    let scene = single_pane_scene(tabs, host_bounds(320.0, 200.0));
    let next = DockOverlayScene {
        layers: vec![
            DockOverlayLayer {
                kind: DockOverlayLayerKind::TargetBody,
                bounds: floating_bounds(80.0, 26.0, 180.0, 120.0),
                target_node: Some(tabs),
                zone: None,
                preview_layer: None,
                active: true,
                payload_index: None,
                payload_title: None,
                drop_box: None,
                tab_insertion: None,
            },
            DockOverlayLayer {
                kind: DockOverlayLayerKind::TabInsertion,
                bounds: floating_bounds(120.0, 0.0, 3.0, 26.0),
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
                bounds: floating_bounds(124.0, 0.0, 90.0, 26.0),
                target_node: Some(tabs),
                zone: Some(DropZone::Center),
                preview_layer: None,
                active: true,
                payload_index: Some(0),
                payload_title: Some("Preview".to_string()),
                drop_box: None,
                tab_insertion: None,
            },
            DockOverlayLayer {
                kind: DockOverlayLayerKind::PayloadGhost,
                bounds: floating_bounds(124.0, 0.0, 90.0, 26.0),
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

    let plan = transition_plan_from_overlay_scene(&scene, &next, DockMotionPreference::Animated);

    assert_eq!(plan.overlay_transitions.len(), next.layers.len());
    for (transition, layer) in plan.overlay_transitions.iter().zip(&next.layers) {
        assert_eq!(transition.bounds, layer.bounds);
    }
}

#[open_gpui::test]
fn overlay_replacement_keeps_preview_layers_at_current_target_bounds(cx: &mut TestAppContext) {
    let (graph, _root, tabs, _right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );
    let scene = single_pane_scene(tabs, host_bounds(320.0, 200.0));
    let previous_body = floating_bounds(10.0, 26.0, 180.0, 120.0);
    let next_body = floating_bounds(80.0, 26.0, 180.0, 120.0);
    let previous_insertion = floating_bounds(12.0, 0.0, 3.0, 26.0);
    let next_insertion = floating_bounds(120.0, 0.0, 3.0, 26.0);
    let previous_payload = floating_bounds(16.0, 0.0, 80.0, 26.0);
    let next_payload = floating_bounds(124.0, 0.0, 90.0, 26.0);
    let tab_preview_scene = |body, insertion, payload| DockOverlayScene {
        layers: vec![
            tab_preview_overlay_layer(tabs, DockOverlayLayerKind::TargetBody, body, None, None),
            tab_preview_overlay_layer(
                tabs,
                DockOverlayLayerKind::TabInsertion,
                insertion,
                Some(DropZone::Center),
                None,
            ),
            tab_preview_overlay_layer(
                tabs,
                DockOverlayLayerKind::PayloadTab,
                payload,
                Some(DropZone::Center),
                Some(0),
            ),
            tab_preview_overlay_layer(
                tabs,
                DockOverlayLayerKind::PayloadGhost,
                payload,
                Some(DropZone::Center),
                Some(0),
            ),
        ],
    };
    let previous = tab_preview_scene(previous_body, previous_insertion, previous_payload);
    let next = tab_preview_scene(next_body, next_insertion, next_payload);
    let first_plan =
        transition_plan_from_overlay_scene(&scene, &previous, DockMotionPreference::Animated);
    let replacement_plan =
        transition_plan_from_overlay_scene(&scene, &next, DockMotionPreference::Animated);
    let spec = MotionSpec::new(
        MotionPreference::Animated,
        MotionDuration::Custom(Duration::from_millis(400)),
        MotionEasing::Linear,
    );

    host.update(cx, |host, _| {
        assert_eq!(
            host.execute_overlay_transition_plan(first_plan, spec, None),
            DockTransitionExecutionState::Scheduled
        );
        assert!(
            host.sample_overlay_transition_for_test(Duration::from_millis(0))
                .is_some()
        );
        assert!(
            host.sample_overlay_transition_for_test(Duration::from_millis(100))
                .is_some()
        );

        assert_eq!(
            host.execute_overlay_transition_plan(replacement_plan, spec, None),
            DockTransitionExecutionState::Scheduled
        );
        let sample = host
            .sample_overlay_transition_for_test(Duration::from_millis(100))
            .expect("replacement overlay transition should expose a retargeted start sample");
        assert_eq!(sample.progress, 0.0);

        let overlay_bounds = |kind, payload_index| {
            sample
                .overlays
                .iter()
                .find(|overlay| overlay.kind == kind && overlay.payload_index == payload_index)
                .map(|overlay| overlay.bounds)
                .expect("sample should include overlay kind")
        };
        assert_eq!(
            overlay_bounds(DockOverlayTransitionKind::TargetBody, None),
            next_body,
            "target body should stay pinned to the current hover target"
        );
        assert_eq!(
            overlay_bounds(DockOverlayTransitionKind::TabInsertion, None),
            next_insertion,
            "tab insertion should stay pinned to the current pointer target"
        );
        assert_eq!(
            overlay_bounds(DockOverlayTransitionKind::PayloadTab, Some(0)),
            next_payload,
            "payload tab preview should not drift from the current insertion slot"
        );
        assert_eq!(
            overlay_bounds(DockOverlayTransitionKind::PayloadGhost, Some(0)),
            next_payload,
            "payload ghost should stay aligned with the payload tab preview"
        );
    });
}

#[test]
fn transition_plan_from_route_affordance_describes_source_marker() {
    let tabs = crate::DockNodeId::null();
    let scene = single_pane_scene(tabs, host_bounds(320.0, 200.0));
    let route_preview = DockDropRoutePreview::from_route(
        &DockViewportDropRoute::KnownViewport {
            target: DockViewportTargetHit::new(
                crate::DockSpaceId::from("target"),
                handle(171),
                point(px(42.0), px(24.0)),
            ),
            source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
        },
        point(px(24.0), px(48.0)),
    )
    .expect("known cross-window route should produce a source marker");
    let affordance_scene = DockVisualAffordanceScene::from_route_preview(&route_preview);

    let plan = DockTransitionPlan::from_visual_affordance_scene(
        &scene,
        &affordance_scene,
        DockMotionPreference::Animated,
    );

    assert!(plan.pane_transitions.is_empty());
    assert_eq!(plan.overlay_transitions.len(), 1);
    assert_eq!(
        plan.overlay_transitions[0].kind,
        DockOverlayTransitionKind::RouteMarker
    );
    assert_eq!(plan.overlay_transitions[0].bounds, route_preview.bounds);
    assert!(!plan.overlay_transitions[0].immediate);
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

    let plan = transition_plan_from_overlay_scene(&scene, &overlay, DockMotionPreference::Reduced);

    assert!(plan.pane_transitions.is_empty());
    assert_eq!(plan.overlay_transitions.len(), 1);
    assert_eq!(
        plan.overlay_transitions[0].kind,
        DockOverlayTransitionKind::RejectedNoop
    );
    assert!(plan.is_immediate());
}
