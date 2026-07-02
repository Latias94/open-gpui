use crate::{
    DockGraph, DockNode, DockSpatialDirection, DockViewportFocusRequest, SplitAxis,
    debug::DockDebugRegion,
    host_test_support::{item, open_host, selector_for, space, split_graph},
    presentation_scene::DockPresentationOverlayAnchorKind,
    transition_geometry::{DockMotionPreference, DockOverlayTransitionKind, DockTransitionEdge},
    zoom_state::{DockZoomScene, DockZoomState},
};
use open_gpui::{AppContext as _, Bounds, TestAppContext, point, px, size};
use open_gpui_ui_core::{MotionDuration, MotionEasing, MotionPreference, MotionSpec};
use slotmap::Key;
use std::time::Duration;

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

#[open_gpui::test]
fn rendered_scene_clears_missing_zoom_target(cx: &mut TestAppContext) {
    let (graph, _root, _left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(220.0)),
    );

    assert!(host.update(cx, |host, cx| host.zoom_pane(right_tabs, cx)));
    assert_eq!(
        host.update(cx, |host, _| host.zoom_target_for_test()),
        Some(right_tabs)
    );

    let controller = host.update(cx, |host, _| host.controller().clone());
    cx.update_entity(&controller, |controller, cx| {
        let mut graph = DockGraph::new();
        let remaining = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        graph.set_root(space(), remaining);
        controller.workspace_mut().set_graph(graph);
        cx.notify();
    });
    window
        .update(cx, |_host, window, _| window.refresh())
        .expect("host window should refresh after graph replacement");
    cx.run_until_parked();
    let _visual = open_gpui::VisualTestContext::from_window(window.into(), cx);

    assert_eq!(host.update(cx, |host, _| host.zoom_target_for_test()), None);
}

#[open_gpui::test]
fn host_zoom_commands_present_target_without_mutating_graph(cx: &mut TestAppContext) {
    let (graph, root, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.4, 0.6);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(500.0), px(240.0)),
    );
    let bounds = host_bounds(500.0, 240.0);

    assert!(host.update(cx, |host, cx| host.zoom_pane(right_tabs, cx)));
    let zoomed = host.update(cx, |host, cx| host.presentation_scene_for_test(bounds, cx));
    assert_eq!(
        host.update(cx, |host, _| host.zoom_target_for_test()),
        Some(right_tabs)
    );
    assert_eq!(zoomed.root, Some(root));
    assert_eq!(zoomed.panes.len(), 1);
    assert_eq!(zoomed.panes[0].node, Some(right_tabs));
    assert_eq!(zoomed.panes[0].bounds, bounds);
    assert!(zoomed.splitters.is_empty());

    let graph_root = host.update(cx, |host, cx| {
        host.with_workspace(cx, |workspace| workspace.graph().root(&space()))
    });
    assert_eq!(graph_root, Some(root));

    assert!(host.update(cx, |host, cx| { host.toggle_zoom_pane(right_tabs, cx) }));
    let restored = host.update(cx, |host, cx| host.presentation_scene_for_test(bounds, cx));
    assert_eq!(
        restored.pane_for_node(left_tabs).map(|pane| pane.bounds),
        Some(Bounds::new(
            point(px(0.0), px(0.0)),
            size(px(200.0), px(240.0))
        ))
    );
    assert_eq!(
        restored.pane_for_node(right_tabs).map(|pane| pane.bounds),
        Some(Bounds::new(
            point(px(200.0), px(0.0)),
            size(px(300.0), px(240.0))
        ))
    );

    assert!(host.update(cx, |host, cx| { host.toggle_zoom_pane(right_tabs, cx) }));
    assert_eq!(
        host.update(cx, |host, _| host.zoom_target_for_test()),
        Some(right_tabs)
    );
    assert!(host.update(cx, |host, cx| host.unzoom(cx)));
}

#[open_gpui::test]
fn public_zoom_command_uses_rendered_scene_for_transition(cx: &mut TestAppContext) {
    let (graph, _root, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (_window, host, visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(220.0)),
    );
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Host).is_some(),
        "opening the host should produce a render frame before commands run"
    );

    host.update(cx, |host, cx| {
        assert!(host.zoom_pane(right_tabs, cx));
        let sample = host
            .sample_transition_for_test(Duration::from_millis(0))
            .expect("public zoom command should use the cached render scene");
        assert_eq!(sample.final_scene.panes.len(), 1);
        assert_eq!(sample.final_scene.panes[0].node, Some(right_tabs));
        assert!(sample.pane_clips.iter().any(|clip| clip.node == left_tabs));
        assert!(sample.overlays.iter().any(|overlay| {
            overlay.kind == DockOverlayTransitionKind::FocusRing
                && overlay.target_node == Some(right_tabs)
        }));
    });
}

#[open_gpui::test]
fn host_zoom_command_samples_egress_and_focus_ring_transition(cx: &mut TestAppContext) {
    let (graph, _root, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(220.0)),
    );
    let bounds = host_bounds(400.0, 220.0);
    let previous = host.update(cx, |host, cx| host.presentation_scene_for_test(bounds, cx));
    let spec = MotionSpec::new(
        MotionPreference::Animated,
        MotionDuration::Custom(Duration::from_millis(100)),
        MotionEasing::Linear,
    );

    host.update(cx, |host, cx| {
        assert!(host.zoom_pane_with_scene(right_tabs, previous.clone(), spec, None, cx));
        let start = host
            .sample_transition_for_test(Duration::from_millis(0))
            .expect("zoom command should schedule a transition sample");
        assert_eq!(start.final_scene.panes.len(), 1);
        assert_eq!(start.final_scene.panes[0].node, Some(right_tabs));
        assert_eq!(start.progress, 0.0);
        assert!(start.overlays.iter().any(|overlay| {
            overlay.kind == DockOverlayTransitionKind::FocusRing
                && overlay.target_node == Some(right_tabs)
                && overlay.bounds == bounds
        }));

        let midpoint = host
            .sample_transition_for_test(Duration::from_millis(50))
            .expect("zoom command should keep sampling while animated");
        let leaving = midpoint
            .pane_clips
            .iter()
            .find(|clip| clip.node == left_tabs)
            .expect("zoom egress should clip the pane leaving the zoom target");
        assert_eq!(
            leaving.content_bounds,
            previous
                .pane_for_node(left_tabs)
                .expect("left pane should be in the previous scene")
                .bounds
        );
        assert_eq!(leaving.visible_bounds.size.width, px(100.0));
        assert_eq!(leaving.visible_bounds.origin.x, px(0.0));
    });
}

#[open_gpui::test]
fn host_unzoom_command_samples_restored_scene_without_graph_mutation(cx: &mut TestAppContext) {
    let (graph, root, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.4, 0.6);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(500.0), px(240.0)),
    );
    let bounds = host_bounds(500.0, 240.0);
    let base = host.update(cx, |host, cx| host.presentation_scene_for_test(bounds, cx));
    assert!(host.update(cx, |host, cx| host.zoom_pane(right_tabs, cx)));
    let zoomed = host.update(cx, |host, cx| host.presentation_scene_for_test(bounds, cx));

    host.update(cx, |host, cx| {
        assert!(host.unzoom_with_scene(
            zoomed.clone(),
            base.clone(),
            MotionSpec::layout(DockMotionPreference::Reduced),
            None,
            cx
        ));
        let sample = host
            .sample_transition_for_test(Duration::from_millis(0))
            .expect("reduced unzoom should expose one final sample");
        assert_eq!(sample.final_scene, base);
        assert_eq!(sample.progress, 1.0);
        assert!(sample.complete);
    });

    let graph_root = host.update(cx, |host, cx| {
        host.with_workspace(cx, |workspace| workspace.graph().root(&space()))
    });
    assert_eq!(graph_root, Some(root));
    let restored = host.update(cx, |host, cx| host.presentation_scene_for_test(bounds, cx));
    assert_eq!(
        restored.pane_for_node(left_tabs).map(|pane| pane.bounds),
        base.pane_for_node(left_tabs).map(|pane| pane.bounds)
    );
}

#[open_gpui::test]
fn host_unzoom_command_retargets_from_active_zoom_sample(cx: &mut TestAppContext) {
    let (graph, _root, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(220.0)),
    );
    let bounds = host_bounds(400.0, 220.0);
    let base = host.update(cx, |host, cx| host.presentation_scene_for_test(bounds, cx));
    let spec = MotionSpec::new(
        MotionPreference::Animated,
        MotionDuration::Custom(Duration::from_millis(100)),
        MotionEasing::Linear,
    );

    host.update(cx, |host, cx| {
        assert!(host.zoom_pane_with_scene(right_tabs, base.clone(), spec, None, cx));
        let zoom_midpoint = host
            .sample_transition_for_test(Duration::from_millis(50))
            .expect("zoom should be active at midpoint");
        let sampled_left = zoom_midpoint
            .pane_bounds
            .iter()
            .find(|pane| pane.node == left_tabs)
            .expect("zoom midpoint should include the egressing left pane")
            .bounds;
        let sampled_right = zoom_midpoint
            .pane_bounds
            .iter()
            .find(|pane| pane.node == right_tabs)
            .expect("zoom midpoint should include the zooming right pane")
            .bounds;
        let zoomed = zoom_midpoint.final_scene.clone();

        assert!(host.unzoom_with_scene(zoomed, base.clone(), spec, None, cx));
        let unzoom_start = host
            .sample_transition_for_test(Duration::from_millis(50))
            .expect("unzoom should start from the active zoom sample");
        assert_eq!(
            unzoom_start
                .pane_bounds
                .iter()
                .find(|pane| pane.node == left_tabs)
                .map(|pane| pane.bounds),
            Some(sampled_left),
            "unzoom should retarget the restored pane from the current zoom egress geometry"
        );
        assert_eq!(
            unzoom_start
                .pane_bounds
                .iter()
                .find(|pane| pane.node == right_tabs)
                .map(|pane| pane.bounds),
            Some(sampled_right),
            "unzoom should retarget the zoomed pane from the current zoom geometry"
        );
    });
}

#[open_gpui::test]
fn host_focus_command_targets_selected_item_for_pane(cx: &mut TestAppContext) {
    let (graph, _root, _left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(220.0)),
    );

    assert!(host.update(cx, |host, cx| host.focus_pane(right_tabs, cx)));
    let focus_reached_target = host.update(cx, |host, _| {
        let pending_matches = host.pending_focus_command().is_some_and(|command| {
            command.request() == &DockViewportFocusRequest::panel(item("b"))
        });
        let recorded_matches = host
            .viewport_runtime()
            .recorded_panel_focus_matches(&space(), &item("b"));
        pending_matches || recorded_matches
    });
    assert!(focus_reached_target);
}

#[open_gpui::test]
fn host_focus_neighbor_command_uses_spatial_navigation(cx: &mut TestAppContext) {
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
    host.update(cx, |host, _| host.set_last_presentation_scene(scene));

    assert!(host.update(cx, |host, cx| {
        host.focus_neighbor_pane(left_tabs, DockSpatialDirection::Right, cx)
    }));
    let focus_reached_neighbor = host.update(cx, |host, _| {
        let pending_matches = host.pending_focus_command().is_some_and(|command| {
            command.request() == &DockViewportFocusRequest::panel(item("b"))
        });
        let recorded_matches = host
            .viewport_runtime()
            .recorded_panel_focus_matches(&space(), &item("b"));
        pending_matches || recorded_matches
    });
    assert!(focus_reached_neighbor);
    assert!(!host.update(cx, |host, cx| {
        host.focus_neighbor_pane(right_tabs, DockSpatialDirection::Right, cx)
    }));
}

#[open_gpui::test]
fn host_focus_command_samples_focus_ring_without_overriding_focus_authority(
    cx: &mut TestAppContext,
) {
    let (graph, _root, _left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(220.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(host_bounds(400.0, 220.0), cx)
    });

    host.update(cx, |host, cx| {
        assert!(host.focus_pane_with_scene(
            right_tabs,
            scene.clone(),
            MotionSpec::layout(DockMotionPreference::Animated),
            None,
            cx
        ));
        assert!(host.pending_focus_command().is_some_and(|command| {
            command.request() == &DockViewportFocusRequest::panel(item("b"))
        }));
        let sample = host
            .sample_transition_for_test(Duration::from_millis(0))
            .expect("focus command should expose focus-ring transition sample");
        assert_eq!(
            sample.final_scene, scene,
            "focus pulse should not replace the semantic presentation scene"
        );
        assert_eq!(sample.overlays.len(), 1);
        assert_eq!(
            sample.overlays[0].kind,
            DockOverlayTransitionKind::FocusRing
        );
        assert_eq!(sample.overlays[0].target_node, Some(right_tabs));
    });
}

#[open_gpui::test]
fn public_focus_command_uses_immediate_overlay_only_feedback(cx: &mut TestAppContext) {
    let (graph, _root, _left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (_window, host, visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(220.0)),
    );
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Host).is_some(),
        "opening the host should produce a render frame before focus commands run"
    );

    let execution = host.update(cx, |host, cx| {
        assert!(host.focus_pane(right_tabs, cx));
        host.clear_transition_execution_for_test()
            .expect("public focus command should expose immediate focus feedback")
    });

    assert!(execution.spec.is_immediate());
    assert_eq!(
        execution.state,
        crate::DockTransitionExecutionState::Immediate
    );
    assert!(
        execution.plan.pane_transitions.is_empty(),
        "focus feedback should not animate layout for high-frequency focus commands"
    );
    assert_eq!(execution.plan.overlay_transitions.len(), 1);
    assert_eq!(
        execution.plan.overlay_transitions[0].kind,
        DockOverlayTransitionKind::FocusRing
    );
    assert!(execution.plan.overlay_transitions[0].immediate);
}
