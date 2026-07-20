use crate::{
    DockCentralRegion, DockController, DockFloatingContainer, DockGraph, DockHost, DockItemId,
    DockNode, DockNodeId, DockPanel, DockPanelDescriptor, DockSpaceId, DockViewportRuntimeHandle,
    DockWorkspace, DropZone, SplitAxis,
    debug::DockDebugRegion,
    divider_hit_map::{DockDividerHitMap, DockDividerHitTarget},
    drag::DockDragPayload,
    drop_scene_fact,
    drop_target::{DockDropResolveSource, DockResolvedDropTargetKind},
    host_test_support::*,
    interaction::DockPayloadDropRelease,
    transition_geometry::DockVisualAffordanceTransitionKind,
};
use open_gpui::{
    AnyView, AppContext as _, Context, Entity, Focusable, InteractiveElement, IntoElement,
    Modifiers, MouseButton, ParentElement, Render, Styled, SubtreeTransform, SubtreeTransformExt,
    SubtreeTransformOrigin, TestAppContext, VisualTestContext, Window, div, point, px, size,
};
use slotmap::Key;
use std::time::Duration;

struct OccludedDockHostFixture {
    host: Entity<DockHost>,
}

impl Render for OccludedDockHostFixture {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .size_full()
            .child(AnyView::from(self.host.clone()))
            .child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .size_full()
                    .occlude(),
            )
    }
}

struct IndependentDockHostsFixture {
    first: Entity<DockHost>,
    second: Entity<DockHost>,
}

struct ConditionalDockHostFixture {
    host: Entity<DockHost>,
    show_host: bool,
    transform: Option<SubtreeTransform>,
}

impl Render for ConditionalDockHostFixture {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut content = div().size_full();
        if self.show_host {
            content = content.child(AnyView::from(self.host.clone()));
        }
        match self.transform {
            Some(transform) => content.with_subtree_transform(transform).into_any_element(),
            None => content.into_any_element(),
        }
    }
}

impl Render for IndependentDockHostsFixture {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .child(
                div()
                    .w(px(320.0))
                    .h_full()
                    .child(AnyView::from(self.first.clone())),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .child(AnyView::from(self.second.clone())),
            )
    }
}

fn floating_host_for_space(
    cx: &mut TestAppContext,
    space: &str,
    root_item: &str,
    floating_item: &str,
    root_label: &'static str,
    floating_label: &'static str,
) -> (Entity<DockHost>, DockNodeId) {
    let space = DockSpaceId::from(space);
    let root_item = DockItemId::from(root_item);
    let floating_item = DockItemId::from(floating_item);
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: vec![root_item.clone()],
        selected: Some(root_item.clone()),
    });
    graph.set_root(space.clone(), root);
    let floating_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![floating_item.clone()],
        selected: Some(floating_item.clone()),
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_tabs,
    });
    graph
        .floating_containers_mut(space.clone())
        .push(DockFloatingContainer {
            node: floating,
            bounds: floating_bounds(20.0, 24.0, 240.0, 160.0),
        });

    let mut workspace = DockWorkspace::new(space.clone(), graph);
    workspace.register_panel_view(root_item, "Root", test_view(cx, root_label));
    workspace.register_panel_view(floating_item, "Floating", test_view(cx, floating_label));
    workspace.policy_mut().set_allow_floating(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let host = cx.new(|cx| DockHost::from_controller(controller, space, runtime, cx));
    (host, floating)
}

fn retained_host_for_workspace(
    cx: &mut TestAppContext,
    workspace: DockWorkspace,
) -> Entity<DockHost> {
    let space = workspace.space().clone();
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    cx.new(|cx| DockHost::from_controller(controller, space, runtime, cx))
}

#[open_gpui::test]
fn stale_floating_drag_begin_does_not_leave_transient_drag(cx: &mut TestAppContext) {
    let (graph, _root, _floating) = floating_overlay_graph();
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.policy_mut().set_allow_floating(true);
    let (_window, host, _visual) = open_workspace(cx, workspace, size(px(320.0), px(220.0)));

    let began = cx.update_entity(&host, |host, cx| {
        host.begin_floating_drag_from_render(
            space(),
            DockNodeId::null(),
            point(px(10.0), px(20.0)),
            floating_bounds(10.0, 20.0, 220.0, 140.0),
            cx,
        )
    });

    assert!(!began);
    assert!(cx.read_entity(&host, |host, _| host.floating_drag().is_none()));
}

#[open_gpui::test]
fn rejected_single_tabs_floating_drag_does_not_leave_payload_state(cx: &mut TestAppContext) {
    let (graph, _root, floating) = floating_overlay_graph();
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let (_window, host, mut visual) = open_workspace(cx, workspace, size(px(320.0), px(220.0)));
    let handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::FloatingHandle { node: floating },
    )
    .expect("single-tabs floating handle should be emitted");
    let start = debug_bounds(&mut visual, &handle).center();

    visual.update(|window, _| window.activate_window());
    cx.run_until_parked();
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    assert!(visual.update(|window, cx| {
        window.captured_pointer().is_none() && cx.active_drag_value::<DockDragPayload>().is_none()
    }));
    host.read_with(&visual, |host, _| {
        assert!(host.floating_drag().is_none());
        let payload = DockDragPayload::new_floating(space(), floating, "Panel A".to_string());
        assert!(host.active_payload_drag_session(&payload).is_none());
    });
}

#[open_gpui::test]
fn rejected_single_tabs_geometry_does_not_leave_payload_or_capture(cx: &mut TestAppContext) {
    let (graph, _root, floating) = floating_overlay_graph();
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.policy_mut().set_allow_floating(true);
    let host = retained_host_for_workspace(cx, workspace);
    let window_host = host.clone();
    let transform = SubtreeTransform::try_new(
        size(0.01, 0.01),
        point(px(0.0), px(0.0)),
        SubtreeTransformOrigin::TOP_LEFT,
    )
    .expect("small test transform should remain representable");
    let window = cx.open_window(size(px(320.0), px(240.0)), move |_, _| {
        ConditionalDockHostFixture {
            host: window_host,
            show_host: true,
            transform: Some(transform),
        }
    });
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::FloatingHandle { node: floating },
    )
    .expect("transformed floating handle should be emitted");
    let start = debug_bounds(&mut visual, &handle).center();

    visual.update(|window, _| window.activate_window());
    cx.run_until_parked();
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        point(px(f32::MAX), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    assert!(visual.update(|window, cx| {
        window.captured_pointer().is_none() && cx.active_drag_value::<DockDragPayload>().is_none()
    }));
    host.read_with(&visual, |host, _| {
        assert!(host.floating_drag().is_none());
        let payload = DockDragPayload::new_floating(space(), floating, "Panel A".to_string());
        assert!(host.active_payload_drag_session(&payload).is_none());
    });
}

#[open_gpui::test]
fn top_floating_content_occludes_a_lower_floating_title_bar(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(space(), root);
    let lower_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let lower = graph.insert_node(DockNode::Floating { child: lower_tabs });
    let upper_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let upper = graph.insert_node(DockNode::Floating { child: upper_tabs });
    graph.floating_containers_mut(space()).extend([
        DockFloatingContainer {
            node: lower,
            bounds: floating_bounds(20.0, 100.0, 220.0, 140.0),
        },
        DockFloatingContainer {
            node: upper,
            bounds: floating_bounds(40.0, 20.0, 220.0, 240.0),
        },
    ]);
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
    );
    workspace.policy_mut().set_allow_floating(true);
    let (_window, host, mut visual) = open_workspace(cx, workspace, size(px(320.0), px(280.0)));
    let lower_handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::FloatingHandle { node: lower },
    )
    .expect("lower floating handle should be emitted");
    let upper_surface = selector_for(&visual, &host, DockDebugRegion::Floating { node: upper })
        .expect("upper floating surface should be emitted");
    let start = debug_bounds(&mut visual, &lower_handle).center();
    assert!(debug_bounds(&mut visual, &upper_surface).contains(&start));

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    let active_payload = visual.update(|_, cx| cx.active_drag_value::<DockDragPayload>().cloned());
    assert!(
        active_payload.is_none(),
        "upper floating content must block the lower handle, but started {active_payload:?}"
    );
    host.read_with(&visual, |host, _| {
        assert!(host.floating_drag().is_none());
    });
    visual.simulate_mouse_up(start, MouseButton::Left, Modifiers::none());
}

#[open_gpui::test]
fn floating_drag_update_preserves_the_captured_grab_offset(cx: &mut TestAppContext) {
    let (graph, _root, floating) = floating_overlay_graph();
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.policy_mut().set_allow_floating(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let (_window, host, _visual) =
        open_controller_workspace(cx, controller.clone(), size(px(320.0), px(220.0)));
    let initial_bounds = floating_bounds(10.0, 20.0, 220.0, 140.0);
    let start_position = point(px(35.0), px(32.0));
    let current_position = point(px(105.0), px(92.0));
    let expected_bounds = floating_bounds(80.0, 80.0, 220.0, 140.0);

    let updated_bounds = cx.update_entity(&host, |host, cx| {
        host.begin_floating_drag_from_render(space(), floating, start_position, initial_bounds, cx);
        host.update_floating_drag_from_render(current_position, cx)
            .expect("active floating drag should produce canonical bounds")
    });

    assert_eq!(updated_bounds, expected_bounds);
    cx.read_entity(&controller, |controller, _| {
        let bounds = controller
            .graph()
            .floating_containers(&space())
            .iter()
            .find(|container| container.node == floating)
            .map(|container| container.bounds)
            .expect("floating container should remain in the graph");
        assert_eq!(bounds, expected_bounds);
    });
}

#[open_gpui::test]
fn occluding_overlay_blocks_raw_composite_floating_drag_acquisition(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(space(), root);
    let floating_left = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let floating_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let floating_split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![floating_left, floating_right],
        fractions: vec![0.5, 0.5],
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_split,
    });
    graph
        .floating_containers_mut(space())
        .push(DockFloatingContainer {
            node: floating,
            bounds: floating_bounds(20.0, 24.0, 260.0, 160.0),
        });
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
    );
    workspace.policy_mut().set_allow_floating(true);
    let dock_space = workspace.space().clone();
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let host =
        cx.new(|cx| DockHost::from_controller(controller.clone(), dock_space, runtime.clone(), cx));
    let window_host = host.clone();
    let window = cx.open_window(size(px(360.0), px(240.0)), move |_, _| {
        OccludedDockHostFixture { host: window_host }
    });
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::FloatingHandle { node: floating },
    )
    .expect("composite floating handle selector should be emitted");
    let start = debug_bounds(&mut visual, &handle).center();

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    host.read_with(&visual, |host, _| {
        assert!(
            host.floating_drag().is_none(),
            "a composite floating handle behind a blocking hitbox must not acquire the mouse down"
        );
    });
    visual.simulate_mouse_up(start, MouseButton::Left, Modifiers::none());
}

#[open_gpui::test]
fn window_deactivation_cancels_a_captured_composite_floating_drag(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(space(), root);
    let floating_left = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let floating_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let floating_split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![floating_left, floating_right],
        fractions: vec![0.5, 0.5],
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_split,
    });
    graph
        .floating_containers_mut(space())
        .push(DockFloatingContainer {
            node: floating,
            bounds: floating_bounds(20.0, 24.0, 260.0, 160.0),
        });
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
    );
    workspace.policy_mut().set_allow_floating(true);
    let (window, host, mut visual) = open_workspace(cx, workspace, size(px(360.0), px(240.0)));
    let handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::FloatingHandle { node: floating },
    )
    .expect("composite floating handle should be emitted");
    let start = debug_bounds(&mut visual, &handle).center();

    visual.update(|window, _| window.activate_window());
    cx.run_until_parked();
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    assert!(visual.update(|window, _| window.captured_pointer().is_some()));
    host.read_with(&visual, |host, _| {
        assert!(host.floating_drag().is_some());
    });

    visual.deactivate_window();
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    assert!(visual.update(|window, _| window.captured_pointer().is_none()));
    host.read_with(&visual, |host, _| {
        assert!(
            host.floating_drag().is_none(),
            "PointerCancel must clear Dock's internal floating drag"
        );
    });
}

#[open_gpui::test]
fn window_deactivation_cancels_a_single_tabs_floating_payload_drag(cx: &mut TestAppContext) {
    let (graph, _root, floating) = floating_overlay_graph();
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.policy_mut().set_allow_floating(true);
    let (window, host, mut visual) = open_workspace(cx, workspace, size(px(360.0), px(240.0)));
    let handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::FloatingHandle { node: floating },
    )
    .expect("single-tabs floating handle should be emitted");
    let start = debug_bounds(&mut visual, &handle).center();
    let threshold = point(start.x + px(24.0), start.y);

    visual.update(|window, _| window.activate_window());
    cx.run_until_parked();
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        point(threshold.x + px(2.0), threshold.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    let payload = visual.update(|window, cx| {
        assert!(window.captured_pointer().is_some());
        assert!(
            window.accepts_pointer_input(),
            "payload drag must keep the source content window interactive for local routing"
        );
        cx.active_drag_value::<DockDragPayload>()
            .cloned()
            .expect("single-tabs floating drag should establish a payload")
    });
    cx.update_entity(&host, |host, _| {
        if !host.interaction().outside_release_poll_running() {
            let session = host
                .active_payload_drag_session(&payload)
                .expect("payload runtime session should be active");
            assert!(
                host.interaction_mut()
                    .begin_outside_release_poll_with_session(&payload, Some(session))
                    .is_some()
            );
        }
    });
    host.read_with(&visual, |host, _| {
        assert!(host.floating_drag().is_some());
        assert!(host.interaction().outside_release_poll_running());
        assert!(host.active_payload_drag_session(&payload).is_some());
    });

    visual.deactivate_window();
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    assert!(visual.update(|window, cx| {
        window.captured_pointer().is_none()
            && window.accepts_pointer_input()
            && cx.active_drag_value::<DockDragPayload>().is_none()
    }));
    host.read_with(&visual, |host, _| {
        assert!(
            host.floating_drag().is_none(),
            "PointerCancel must clear the floating state paired with the GPUI drag"
        );
        assert!(host.interaction().drop_preview().is_none());
        assert!(!host.interaction().outside_release_poll_running());
        assert!(host.active_payload_drag_session(&payload).is_none());
    });
}

#[open_gpui::test]
fn host_subtree_removal_cancels_a_captured_single_tabs_payload_drag(cx: &mut TestAppContext) {
    let (graph, _root, floating) = floating_overlay_graph();
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.policy_mut().set_allow_floating(true);
    let host = retained_host_for_workspace(cx, workspace);
    let window_host = host.clone();
    let window = cx.open_window(size(px(360.0), px(240.0)), move |_, _| {
        ConditionalDockHostFixture {
            host: window_host,
            show_host: true,
            transform: None,
        }
    });
    let fixture = window
        .root(cx)
        .expect("window should expose the conditional host fixture");
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::FloatingHandle { node: floating },
    )
    .expect("single-tabs floating handle should be emitted");
    let start = debug_bounds(&mut visual, &handle).center();

    visual.update(|window, _| window.activate_window());
    cx.run_until_parked();
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let payload = visual.update(|window, cx| {
        assert!(window.captured_pointer().is_some());
        cx.active_drag_value::<DockDragPayload>()
            .cloned()
            .expect("single-tabs drag should establish a payload")
    });
    cx.update_entity(&host, |host, _| {
        let session = host
            .active_payload_drag_session(&payload)
            .expect("payload runtime session should be active");
        assert!(
            host.interaction_mut()
                .begin_outside_release_poll_with_session(&payload, Some(session))
                .is_some()
        );
    });

    cx.update_entity(&fixture, |fixture, cx| {
        fixture.show_host = false;
        cx.notify();
    });
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    assert!(visual.update(|window, cx| {
        window.captured_pointer().is_none() && cx.active_drag_value::<DockDragPayload>().is_none()
    }));
    host.read_with(&visual, |host, _| {
        assert!(host.active_payload_drag_session(&payload).is_none());
        assert!(host.floating_drag().is_none());
        assert!(!host.interaction().outside_release_poll_running());
    });
}

#[open_gpui::test]
fn host_subtree_removal_cancels_a_captured_tab_item_payload_drag(cx: &mut TestAppContext) {
    let (graph, tabs) = tabs_graph(&["a", "b"]);
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let host = retained_host_for_workspace(cx, workspace);
    let window_host = host.clone();
    let window = cx.open_window(size(px(360.0), px(240.0)), move |_, _| {
        ConditionalDockHostFixture {
            host: window_host,
            show_host: true,
            transform: None,
        }
    });
    let fixture = window
        .root(cx)
        .expect("window should expose the conditional host fixture");
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let source = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs,
            item: item("a"),
        },
    )
    .expect("source tab should be emitted");
    let start = debug_bounds(&mut visual, &source).center();

    visual.update(|window, _| window.activate_window());
    cx.run_until_parked();
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let payload = visual.update(|window, cx| {
        assert!(window.captured_pointer().is_some());
        cx.active_drag_value::<DockDragPayload>()
            .cloned()
            .expect("tab drag should establish a payload")
    });
    host.read_with(&visual, |host, _| {
        assert!(host.active_payload_drag_session(&payload).is_some());
    });

    cx.update_entity(&fixture, |fixture, cx| {
        fixture.show_host = false;
        cx.notify();
    });
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    assert!(visual.update(|window, cx| {
        window.captured_pointer().is_none() && cx.active_drag_value::<DockDragPayload>().is_none()
    }));
    host.read_with(&visual, |host, _| {
        assert!(host.active_payload_drag_session(&payload).is_none());
    });
}

#[open_gpui::test]
fn pointer_cancel_reaches_the_payload_owner_with_multiple_dock_hosts(cx: &mut TestAppContext) {
    let (non_owner, _non_owner_floating) = floating_host_for_space(
        cx,
        "non-owner",
        "non-owner-root",
        "non-owner-floating",
        "Non-owner root",
        "Non-owner floating",
    );
    let (owner, owner_floating) = floating_host_for_space(
        cx,
        "owner",
        "owner-root",
        "owner-floating",
        "Owner root",
        "Owner floating",
    );
    let window_non_owner = non_owner.clone();
    let window_owner = owner.clone();
    let window = cx.open_window(size(px(640.0), px(240.0)), move |_, _| {
        IndependentDockHostsFixture {
            first: window_non_owner,
            second: window_owner,
        }
    });
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let handle = selector_for(
        &visual,
        &owner,
        DockDebugRegion::FloatingHandle {
            node: owner_floating,
        },
    )
    .expect("owner floating handle should be emitted");
    let start = debug_bounds(&mut visual, &handle).center();
    let threshold = point(start.x + px(24.0), start.y);

    visual.update(|window, _| window.activate_window());
    cx.run_until_parked();
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        point(threshold.x + px(2.0), threshold.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let payload = visual.update(|_, cx| {
        cx.active_drag_value::<DockDragPayload>()
            .cloned()
            .expect("owner should establish a Dock payload drag")
    });
    cx.update_entity(&owner, |host, _| {
        if !host.interaction().outside_release_poll_running() {
            let session = host
                .active_payload_drag_session(&payload)
                .expect("owner payload runtime session should be active");
            assert!(
                host.interaction_mut()
                    .begin_outside_release_poll_with_session(&payload, Some(session))
                    .is_some()
            );
        }
    });
    owner.read_with(&visual, |host, _| {
        assert!(host.active_payload_drag_session(&payload).is_some());
        assert!(host.interaction().outside_release_poll_running());
    });

    visual.deactivate_window();
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    assert!(visual.update(|_, cx| cx.active_drag_value::<DockDragPayload>().is_none()));
    owner.read_with(&visual, |host, _| {
        assert!(host.active_payload_drag_session(&payload).is_none());
        assert!(host.floating_drag().is_none());
        assert!(!host.interaction().outside_release_poll_running());
    });
}

#[open_gpui::test]
fn horizontal_splitter_drag_updates_width_fractions(cx: &mut TestAppContext) {
    let (graph, split, _left, _right) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(400.0), px(240.0)));

    let handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitterHandle { split, index: 0 },
    )
    .expect("splitter handle selector should be emitted");
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

    assert_close(width(debug_bounds(&mut visual, &left)), 200.0);
    assert_close(width(debug_bounds(&mut visual, &right)), 200.0);

    let start = debug_bounds(&mut visual, &handle).center();
    let end = point(start.x + px(80.0), start.y);
    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    assert_close(width(debug_bounds(&mut visual, &left)), 280.0);
    assert_close(width(debug_bounds(&mut visual, &right)), 120.0);
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Split { fractions, .. } =
            controller.graph().node(split).expect("split should exist")
        else {
            panic!("root should be split");
        };
        assert_close(fractions[0], 0.7);
        assert_close(fractions[1], 0.3);
    });
    host.read_with(&visual, |host, _| {
        assert!(host.splitter_drag().is_none());
    });
}

#[open_gpui::test]
fn floating_splitter_wins_over_an_overlapped_root_splitter(cx: &mut TestAppContext) {
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

    let floating_left = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let floating_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("d")],
        selected: Some(item("d")),
    });
    let floating_split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![floating_left, floating_right],
        fractions: vec![0.5, 0.5],
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_split,
    });
    graph
        .floating_containers_mut(space())
        .push(DockFloatingContainer {
            node: floating,
            bounds: floating_bounds(50.0, 24.0, 300.0, 180.0),
        });

    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
            ("d", "Panel D", "D"),
        ],
    );
    workspace.policy_mut().set_allow_floating(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(400.0), px(240.0)));
    let floating_handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitterHandle {
            split: floating_split,
            index: 0,
        },
    )
    .expect("floating splitter handle should be emitted");
    let start = debug_bounds(&mut visual, &floating_handle).center();
    let end = point(start.x + px(30.0), start.y);

    visual.update(|window, _| window.activate_window());
    cx.run_until_parked();
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    cx.read_entity(&controller, |controller, _| {
        let DockNode::Split {
            fractions: root_fractions,
            ..
        } = controller
            .graph()
            .node(root_split)
            .expect("root split should remain")
        else {
            panic!("root should remain split");
        };
        let DockNode::Split {
            fractions: floating_fractions,
            ..
        } = controller
            .graph()
            .node(floating_split)
            .expect("floating split should remain")
        else {
            panic!("floating child should remain split");
        };
        assert_eq!(root_fractions, &vec![0.5, 0.5]);
        assert!(
            floating_fractions[0] > 0.5 && floating_fractions[1] < 0.5,
            "only the topmost floating splitter should resize"
        );
    });
    host.read_with(&visual, |host, _| {
        assert!(host.splitter_drag().is_none());
    });
}

#[open_gpui::test]
fn window_deactivation_cancels_a_captured_splitter_drag(cx: &mut TestAppContext) {
    let (graph, split, _left, _right) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );
    let handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitterHandle { split, index: 0 },
    )
    .expect("splitter handle should be emitted");
    let start = debug_bounds(&mut visual, &handle).center();

    visual.update(|window, _| window.activate_window());
    cx.run_until_parked();
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    assert!(visual.update(|window, _| window.captured_pointer().is_some()));
    host.read_with(&visual, |host, _| {
        assert!(host.splitter_drag().is_some());
    });

    visual.deactivate_window();
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    assert!(visual.update(|window, _| window.captured_pointer().is_none()));
    host.read_with(&visual, |host, _| {
        assert!(
            host.splitter_drag().is_none(),
            "PointerCancel must clear Dock's internal splitter drag"
        );
    });
}

#[open_gpui::test]
fn occluding_overlay_blocks_raw_splitter_drag_acquisition(cx: &mut TestAppContext) {
    let (graph, split, _left, _right) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let dock_space = workspace.space().clone();
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let host =
        cx.new(|cx| DockHost::from_controller(controller.clone(), dock_space, runtime.clone(), cx));
    let window_host = host.clone();
    let window = cx.open_window(size(px(400.0), px(240.0)), move |_, _| {
        OccludedDockHostFixture { host: window_host }
    });
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitterHandle { split, index: 0 },
    )
    .expect("splitter handle selector should be emitted");
    let start = debug_bounds(&mut visual, &handle).center();

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    host.read_with(&visual, |host, _| {
        assert!(
            host.splitter_drag().is_none(),
            "a splitter behind a blocking hitbox must not acquire the mouse down"
        );
    });
    visual.simulate_mouse_up(start, MouseButton::Left, Modifiers::none());
}

#[open_gpui::test]
fn vertical_splitter_drag_updates_height_fractions(cx: &mut TestAppContext) {
    let (graph, split, _top, _bottom) = split_graph(SplitAxis::Vertical, 0.5, 0.5);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(320.0), px(400.0)),
    );

    let handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitterHandle { split, index: 0 },
    )
    .expect("splitter handle selector should be emitted");
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

    let start = debug_bounds(&mut visual, &handle).center();
    let end = point(start.x, start.y + px(80.0));
    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    assert_close(height(debug_bounds(&mut visual, &top)), 280.0);
    assert_close(height(debug_bounds(&mut visual, &bottom)), 120.0);
}

#[open_gpui::test]
fn splitter_drag_clamps_to_minimum_pane_size(cx: &mut TestAppContext) {
    let (graph, split, _left, _right) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    let handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::SplitterHandle { split, index: 0 },
    )
    .expect("splitter handle selector should be emitted");
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

    let start = debug_bounds(&mut visual, &handle).center();
    let end = point(start.x - px(300.0), start.y);
    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    assert_close(width(debug_bounds(&mut visual, &left)), 96.0);
    assert_close(width(debug_bounds(&mut visual, &right)), 304.0);
}

#[open_gpui::test]
fn corner_splitter_drag_updates_both_axes_through_rendered_events(cx: &mut TestAppContext) {
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
    let workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
    );
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, _host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(400.0), px(240.0)));
    let scene = window
        .root(cx)
        .expect("window should expose host")
        .update(cx, |host, cx| {
            host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 400.0, 240.0), cx)
        });
    let hit_map = DockDividerHitMap::from_scene(&scene);
    let start = match hit_map
        .hit(point(px(200.0), px(120.0)))
        .expect("junction should resolve")
    {
        DockDividerHitTarget::Corner(corner) => corner.bounds.center(),
        DockDividerHitTarget::Single(_) => panic!("junction should prefer corner"),
    };
    let end = point(start.x + px(80.0), start.y + px(48.0));

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    cx.read_entity(&controller, |controller, _| {
        let DockNode::Split {
            fractions: root_fractions,
            ..
        } = controller
            .graph()
            .node(root)
            .expect("root split should exist")
        else {
            panic!("root should be split");
        };
        let DockNode::Split {
            fractions: vertical_fractions,
            ..
        } = controller
            .graph()
            .node(vertical)
            .expect("vertical split should exist")
        else {
            panic!("right side should be vertical split");
        };
        assert_close(root_fractions[0], 0.7);
        assert_close(root_fractions[1], 0.3);
        assert_close(vertical_fractions[0], 0.7);
        assert_close(vertical_fractions[1], 0.3);
    });
}

#[open_gpui::test]
fn dragging_tab_to_other_stack_center_moves_panel(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(500.0), px(240.0)));

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: left_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("target tabs selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    let end = debug_bounds(&mut visual, &target_tabs).center();

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut drag_visual = VisualTestContext::from_window(window.into(), cx);
    let preview = selector_for(&drag_visual, &host, DockDebugRegion::DropPreview)
        .expect("center hover should render a drop preview");
    let preview_body = selector_for(&drag_visual, &host, DockDebugRegion::DropPreviewBody)
        .expect("center hover should render a preview body below the payload tab preview");
    let preview_tab = selector_for(
        &drag_visual,
        &host,
        DockDebugRegion::DropPayloadTabPreview { index: 0 },
    )
    .expect("center hover should render a payload tab preview");
    let preview_bounds = debug_bounds(&mut drag_visual, &preview);
    let preview_body_bounds = debug_bounds(&mut drag_visual, &preview_body);
    let preview_tab_bounds = debug_bounds(&mut drag_visual, &preview_tab);
    let visual_affordance_sample = cx
        .update_entity(&host, |host, _| {
            host.sample_visual_affordance_transition_for_test(Duration::from_millis(0))
        })
        .expect("center hover should schedule an visual affordance transition sample");
    let affordance_kinds = visual_affordance_sample
        .visual_affordances
        .iter()
        .map(|overlay| overlay.kind)
        .collect::<Vec<_>>();
    assert!(
        affordance_kinds.contains(&DockVisualAffordanceTransitionKind::TargetBody)
            && affordance_kinds.contains(&DockVisualAffordanceTransitionKind::TabInsertion)
            && affordance_kinds.contains(&DockVisualAffordanceTransitionKind::PayloadTab)
            && affordance_kinds.contains(&DockVisualAffordanceTransitionKind::PayloadGhost),
        "center hover should route body, insertion slot, payload tabs, and payload ghosts through the visual affordance transition runtime: {affordance_kinds:?}"
    );
    assert!(
        preview_bounds.contains(&preview_tab_bounds.center()),
        "payload tab preview should stay inside the center drop preview"
    );
    assert_close(
        f32::from(preview_body_bounds.origin.y),
        f32::from(preview_tab_bounds.origin.y + preview_tab_bounds.size.height),
    );
    assert!(
        preview_body_bounds.origin.y
            >= preview_tab_bounds.origin.y + preview_tab_bounds.size.height,
        "center preview body should start below the payload tab preview"
    );
    assert_close(
        f32::from(preview_body_bounds.origin.x),
        f32::from(preview_bounds.origin.x),
    );
    assert_close(
        f32::from(preview_body_bounds.size.width),
        f32::from(preview_bounds.size.width),
    );

    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "panel A should be visible after center drop"
    );
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(right_tabs)
            .expect("target tabs should exist")
        else {
            panic!("target should be tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a")]);
        assert_eq!(selected.as_ref(), items.get(1));
    });
}

#[open_gpui::test]
fn tab_bar_append_preview_shifts_payload_tab_right_of_existing_tab(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(500.0), px(240.0)));

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: left_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: right_tabs,
            item: item("b"),
        },
    )
    .expect("target tab selector should be emitted");

    let start = debug_bounds(&mut visual, &source_tab).center();
    let target_bounds = debug_bounds(&mut visual, &target_tab);
    let append_hover = point(target_bounds.right() + px(16.0), target_bounds.center().y);

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_move(append_hover, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    let mut drag_visual = VisualTestContext::from_window(window.into(), cx);
    let preview_tab = selector_for(
        &drag_visual,
        &host,
        DockDebugRegion::DropPayloadTabPreview { index: 0 },
    )
    .expect("tab bar append hover should render a payload tab preview");
    let preview_tab_bounds = debug_bounds(&mut drag_visual, &preview_tab);
    let runtime_target_tab_bounds = host
        .update(cx, |host, _| {
            host.viewport_runtime().rendered_tab_label_bounds_for_tabs(
                host.space(),
                Some(window.window_id()),
                right_tabs,
                0,
            )
        })
        .expect("runtime should expose target tab label bounds");

    assert!(
        preview_tab_bounds.origin.x >= runtime_target_tab_bounds.center().x,
        "append preview should move toward the append slot after the existing tab: preview={preview_tab_bounds:?} runtime_target={runtime_target_tab_bounds:?} debug_target={target_bounds:?}"
    );
}

#[open_gpui::test]
fn tab_bar_preview_positions_payload_tab_at_leading_and_middle_slots(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b"), item("c")],
        selected: Some(item("b")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![source_tabs, target_tabs],
        fractions: vec![0.4, 0.6],
    });
    graph.set_root(space(), root);
    let workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
    );
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(640.0), px(240.0)));

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_first = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: target_tabs,
            item: item("b"),
        },
    )
    .expect("first target tab selector should be emitted");
    let target_second = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: target_tabs,
            item: item("c"),
        },
    )
    .expect("second target tab selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );

    cx.run_until_parked();
    let mut hover_visual = VisualTestContext::from_window(window.into(), cx);
    let first_bounds = debug_bounds(&mut hover_visual, &target_first);
    let leading_hover = point(first_bounds.origin.x + px(2.0), first_bounds.center().y);
    hover_visual.simulate_mouse_move(leading_hover, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let leading_index = host
        .update(cx, |host, _| {
            match host.interaction().resolved_drop_target() {
                Some(target) => match target.kind {
                    DockResolvedDropTargetKind::TabBar { insert_index, .. } => Some(insert_index),
                    _ => None,
                },
                None => None,
            }
        })
        .expect("leading tab hover should resolve a tab insertion target");
    assert_eq!(leading_index, 0);
    let mut leading_visual = VisualTestContext::from_window(window.into(), cx);
    let leading_preview_tab = selector_for(
        &leading_visual,
        &host,
        DockDebugRegion::DropPayloadTabPreview { index: 0 },
    )
    .expect("leading tab hover should render a payload tab preview");
    let leading_insertion = selector_for(
        &leading_visual,
        &host,
        DockDebugRegion::DropTabInsertionPreview,
    )
    .expect("leading tab hover should render an insertion preview");
    let leading_preview_tab_bounds = debug_bounds(&mut leading_visual, &leading_preview_tab);
    let leading_insertion_bounds = debug_bounds(&mut leading_visual, &leading_insertion);
    assert!(
        leading_preview_tab_bounds.origin.x >= first_bounds.origin.x
            && leading_preview_tab_bounds.origin.x < first_bounds.center().x,
        "leading payload tab should start inside the first insertion slot, not append after the stack: preview={leading_preview_tab_bounds:?} first={first_bounds:?}"
    );
    assert!(
        leading_insertion_bounds.center().x <= first_bounds.origin.x + px(4.0),
        "leading slot should align with first tab start: insertion={leading_insertion_bounds:?} first={first_bounds:?}"
    );

    let second_bounds = debug_bounds(&mut leading_visual, &target_second);
    let middle_hover = point(second_bounds.origin.x + px(2.0), second_bounds.center().y);
    leading_visual.simulate_mouse_move(middle_hover, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut middle_visual = VisualTestContext::from_window(window.into(), cx);
    let middle_preview_tab = selector_for(
        &middle_visual,
        &host,
        DockDebugRegion::DropPayloadTabPreview { index: 0 },
    )
    .expect("middle tab hover should render a payload tab preview");
    let middle_insertion = selector_for(
        &middle_visual,
        &host,
        DockDebugRegion::DropTabInsertionPreview,
    )
    .expect("middle tab hover should render an insertion preview");
    let middle_preview_tab_bounds = debug_bounds(&mut middle_visual, &middle_preview_tab);
    let middle_insertion_bounds = debug_bounds(&mut middle_visual, &middle_insertion);
    assert!(
        (f32::from(middle_preview_tab_bounds.origin.x) - f32::from(second_bounds.origin.x)).abs()
            <= 4.0,
        "middle payload tab should start at the second tab slot, not append after the stack: preview={middle_preview_tab_bounds:?} second={second_bounds:?}"
    );
    assert_close(
        f32::from(middle_insertion_bounds.center().x),
        f32::from(second_bounds.origin.x),
    );
}

#[open_gpui::test]
fn local_release_on_first_target_hit_does_not_commit(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(500.0), px(240.0)));

    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("target tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut visual, &target_tabs);
    let release_position = target_bounds.center();
    let payload = DockDragPayload::new_item(space(), left_tabs, item("a"), "Panel A".to_string());

    window
        .update(cx, |host, window, cx| {
            host.begin_host_drop_scene_from_render(
                &payload,
                target_bounds,
                release_position,
                window,
                cx,
            );
            host.update_local_drop_scene_fact_from_render(
                &payload,
                drop_scene_fact::leaf(right_tabs, right_tabs, target_bounds, false),
                release_position,
                window,
                cx,
            );
            host.drop_payload_release_from_render(
                DockPayloadDropRelease::hovered_host(payload.clone(), space(), release_position),
                window,
                cx,
            )
        })
        .expect("host should handle release");
    cx.run_until_parked();

    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(right_tabs)
            .expect("target tabs should exist")
        else {
            panic!("target should be tabs");
        };
        assert_eq!(items, &vec![item("b")]);
        assert_eq!(selected.as_ref(), items.first());
    });
}

#[open_gpui::test]
fn local_release_after_preview_miss_does_not_commit(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(500.0), px(240.0)));

    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("target tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut visual, &target_tabs);
    let preview_position = target_bounds.center();
    let release_position = point(px(900.0), px(900.0));
    let payload = DockDragPayload::new_item(space(), left_tabs, item("a"), "Panel A".to_string());

    window
        .update(cx, |host, window, cx| {
            host.begin_host_drop_scene_from_render(
                &payload,
                target_bounds,
                preview_position,
                window,
                cx,
            );
            host.update_local_drop_scene_fact_from_render(
                &payload,
                drop_scene_fact::leaf(right_tabs, right_tabs, target_bounds, false),
                preview_position,
                window,
                cx,
            );
            host.drop_payload_release_from_render(
                DockPayloadDropRelease::hovered_host(payload.clone(), space(), release_position),
                window,
                cx,
            )
        })
        .expect("host should handle release");
    cx.run_until_parked();

    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(right_tabs)
            .expect("target tabs should exist")
        else {
            panic!("target should be tabs");
        };
        assert_eq!(items, &vec![item("b")]);
        assert_eq!(selected.as_ref(), items.first());
    });
}

#[open_gpui::test]
fn source_only_release_does_not_commit_cached_local_delivery_without_hover_signal(
    cx: &mut TestAppContext,
) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(500.0), px(240.0)));

    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("target tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut visual, &target_tabs);
    let release_position = target_bounds.center();
    let payload = DockDragPayload::new_item(space(), left_tabs, item("a"), "Panel A".to_string());

    cx.set_platform_hovered_window(None);
    window
        .update(cx, |host, window, cx| {
            host.begin_host_drop_scene_from_render(
                &payload,
                target_bounds,
                release_position,
                window,
                cx,
            );
            host.update_local_drop_scene_fact_from_render(
                &payload,
                drop_scene_fact::leaf(right_tabs, right_tabs, target_bounds, false),
                release_position,
                window,
                cx,
            );
            host.drop_payload_release_from_render(
                DockPayloadDropRelease::source_only(payload.clone(), space(), release_position),
                window,
                cx,
            )
        })
        .expect("host should handle release");
    cx.run_until_parked();

    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(right_tabs)
            .expect("target tabs should exist")
        else {
            panic!("target should be tabs");
        };
        assert_eq!(items, &vec![item("b")]);
        assert_eq!(selected.as_ref(), items.first());
    });
}

#[open_gpui::test]
fn dragging_tab_bar_empty_area_moves_whole_stack(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
        selected: Some(item("c")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![source_tabs, target_tabs],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(space(), root);
    let workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
    );
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(560.0), px(240.0)));

    let source_stack = selector_for(&visual, &host, DockDebugRegion::Tabs { node: source_tabs })
        .expect("source tabs selector should be emitted");
    let target_stack = selector_for(&visual, &host, DockDebugRegion::Tabs { node: target_tabs })
        .expect("target tabs selector should be emitted");
    let source_bounds = debug_bounds(&mut visual, &source_stack);
    let start = point(
        source_bounds.origin.x + source_bounds.size.width - px(8.0),
        source_bounds.origin.y + px(12.0),
    );
    let end = debug_bounds(&mut visual, &target_stack).center();

    simulate_left_drag(&mut visual, start, end);
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("c") }).is_some(),
        "previously active stack item should remain active after stack drop"
    );
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should still exist")
        else {
            panic!("target should be tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a"), item("c")]);
        assert_eq!(selected.as_ref(), items.get(2));
    });
}

#[open_gpui::test]
fn dragging_tab_bar_empty_area_renders_multi_tab_center_preview(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
        selected: Some(item("c")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![source_tabs, target_tabs],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(space(), root);
    let workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
    );
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(560.0), px(240.0)));

    let source_stack = selector_for(&visual, &host, DockDebugRegion::Tabs { node: source_tabs })
        .expect("source tabs selector should be emitted");
    let target_stack = selector_for(&visual, &host, DockDebugRegion::Tabs { node: target_tabs })
        .expect("target tabs selector should be emitted");
    let source_bounds = debug_bounds(&mut visual, &source_stack);
    let start = point(
        source_bounds.origin.x + source_bounds.size.width - px(8.0),
        source_bounds.origin.y + px(12.0),
    );
    let end = debug_bounds(&mut visual, &target_stack).center();

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    let mut drag_visual = VisualTestContext::from_window(window.into(), cx);
    let preview_body = selector_for(&drag_visual, &host, DockDebugRegion::DropPreviewBody)
        .expect("center stack hover should render a preview body");
    let first_tab = selector_for(
        &drag_visual,
        &host,
        DockDebugRegion::DropPayloadTabPreview { index: 0 },
    )
    .expect("center stack hover should render the first payload tab preview");
    let second_tab = selector_for(
        &drag_visual,
        &host,
        DockDebugRegion::DropPayloadTabPreview { index: 1 },
    )
    .expect("center stack hover should render the second payload tab preview");
    let preview_body_bounds = debug_bounds(&mut drag_visual, &preview_body);
    let first_tab_bounds = debug_bounds(&mut drag_visual, &first_tab);
    let second_tab_bounds = debug_bounds(&mut drag_visual, &second_tab);

    assert!(
        second_tab_bounds.origin.x >= first_tab_bounds.right(),
        "payload tab previews should render in payload order: first={first_tab_bounds:?} second={second_tab_bounds:?}"
    );
    assert_close(
        f32::from(preview_body_bounds.origin.y),
        f32::from(first_tab_bounds.origin.y + first_tab_bounds.size.height),
    );

    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
}

#[open_gpui::test]
fn multi_tab_center_preview_clamps_payload_tabs_in_narrow_target(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("c")],
        selected: Some(item("c")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![source_tabs, target_tabs],
        fractions: vec![0.72, 0.28],
    });
    graph.set_root(space(), root);
    let workspace = workspace_with_panels(
        cx,
        graph,
        &[("a", "A", "A"), ("b", "B", "B"), ("c", "C", "C")],
    );
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(520.0), px(220.0)));

    let source_stack = selector_for(&visual, &host, DockDebugRegion::Tabs { node: source_tabs })
        .expect("source tabs selector should be emitted");
    let target_stack = selector_for(&visual, &host, DockDebugRegion::Tabs { node: target_tabs })
        .expect("target tabs selector should be emitted");
    let source_bounds = debug_bounds(&mut visual, &source_stack);
    let start = point(
        source_bounds.origin.x + source_bounds.size.width - px(8.0),
        source_bounds.origin.y + px(12.0),
    );
    let end = debug_bounds(&mut visual, &target_stack).center();

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    let mut drag_visual = VisualTestContext::from_window(window.into(), cx);
    let preview = selector_for(&drag_visual, &host, DockDebugRegion::DropPreview)
        .expect("center stack hover should render a narrow preview");
    let first_tab = selector_for(
        &drag_visual,
        &host,
        DockDebugRegion::DropPayloadTabPreview { index: 0 },
    )
    .expect("narrow center preview should render the first payload tab preview");
    let second_tab = selector_for(
        &drag_visual,
        &host,
        DockDebugRegion::DropPayloadTabPreview { index: 1 },
    )
    .expect("narrow center preview should render the second payload tab preview");
    let preview_bounds = debug_bounds(&mut drag_visual, &preview);
    let first_tab_bounds = debug_bounds(&mut drag_visual, &first_tab);
    let second_tab_bounds = debug_bounds(&mut drag_visual, &second_tab);

    assert!(
        first_tab_bounds.size.width <= preview_bounds.size.width,
        "first payload tab should be clipped to preview width: preview={preview_bounds:?} first={first_tab_bounds:?}"
    );
    assert!(
        second_tab_bounds.size.width <= preview_bounds.size.width,
        "second payload tab should be clipped to preview width: preview={preview_bounds:?} second={second_tab_bounds:?}"
    );
    assert!(
        first_tab_bounds.origin.x >= preview_bounds.origin.x
            && first_tab_bounds.origin.x < preview_bounds.right(),
        "first payload tab should start inside preview bounds: preview={preview_bounds:?} first={first_tab_bounds:?}"
    );
    assert!(
        second_tab_bounds.origin.x >= preview_bounds.origin.x
            && second_tab_bounds.origin.x < preview_bounds.right(),
        "second payload tab should retain a visible start inside preview bounds: preview={preview_bounds:?} second={second_tab_bounds:?}"
    );

    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
}

#[open_gpui::test]
fn dragging_tab_within_same_stack_reorders_tabs(cx: &mut TestAppContext) {
    let (graph, tabs) = tabs_graph(&["a", "b", "c"]);
    let workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
    );
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(560.0), px(240.0)));

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs,
            item: item("c"),
        },
    )
    .expect("target tab selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    let target_bounds = debug_bounds(&mut visual, &target_tab);
    let end = point(
        target_bounds.origin.x + target_bounds.size.width - px(2.0),
        target_bounds.center().y,
    );

    simulate_left_drag(&mut visual, start, end);
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "panel A should be active after reorder"
    );
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(tabs)
            .expect("tabs should still exist")
        else {
            panic!("target should be tabs");
        };
        assert_eq!(items, &vec![item("b"), item("c"), item("a")]);
        assert_eq!(selected.as_ref(), items.get(2));
    });
}

#[open_gpui::test]
fn same_stack_tab_preview_is_stable_when_pointer_is_stationary(cx: &mut TestAppContext) {
    let (graph, tabs) = tabs_graph(&["a", "b"]);
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(420.0), px(220.0)));

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    let hold = point(start.x + px(28.0), start.y);

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(hold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(hold, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    let mut drag_visual = VisualTestContext::from_window(window.into(), cx);
    let insertion = selector_for(
        &drag_visual,
        &host,
        DockDebugRegion::DropTabInsertionPreview,
    )
    .expect("same-stack stationary hover should render an insertion preview");
    let preview_tab = selector_for(
        &drag_visual,
        &host,
        DockDebugRegion::DropPayloadTabPreview { index: 0 },
    )
    .expect("same-stack stationary hover should render a payload tab preview");
    let first_insertion_bounds = debug_bounds(&mut drag_visual, &insertion);
    let first_preview_tab_bounds = debug_bounds(&mut drag_visual, &preview_tab);
    cx.update_entity(&host, |host, _| {
        host.sample_visual_affordance_transition_for_test(Duration::from_millis(0))
    })
    .expect("same-stack stationary hover should schedule a visual affordance transition");
    let mut previous_progress = cx
        .update_entity(&host, |host, _| {
            host.sample_visual_affordance_transition_for_test(Duration::from_millis(40))
        })
        .expect("same-stack visual affordance transition should still be active")
        .progress;

    for step in 0..4 {
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        visual.simulate_mouse_move(hold, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();

        let mut drag_visual = VisualTestContext::from_window(window.into(), cx);
        assert_eq!(
            debug_bounds(&mut drag_visual, &insertion),
            first_insertion_bounds,
            "stationary same-stack drag must not move the tab insertion preview"
        );
        assert_eq!(
            debug_bounds(&mut drag_visual, &preview_tab),
            first_preview_tab_bounds,
            "stationary same-stack drag must not move the payload tab preview"
        );
        let progress = cx
            .update_entity(&host, |host, _| {
                host.sample_visual_affordance_transition_for_test(Duration::from_millis(
                    50 + step * 10,
                ))
            })
            .expect("stationary same-stack hover should keep the transition alive")
            .progress;
        assert!(
            progress >= previous_progress,
            "stationary same-stack hover must not restart the visual affordance transition: previous={previous_progress} current={progress}"
        );
        previous_progress = progress;
    }
}

#[open_gpui::test]
fn same_stack_tab_preview_holds_slot_across_center_jitter(cx: &mut TestAppContext) {
    let (graph, tabs) = tabs_graph(&["a", "b"]);
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(420.0), px(220.0)));

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let source_bounds = debug_bounds(&mut visual, &source_tab);
    let start = source_bounds.center();
    let drag_threshold = point(start.x + px(24.0), start.y);
    let left_of_center = point(start.x - px(12.0), start.y + px(4.0));
    let right_of_center = point(start.x + px(4.0), start.y + px(4.0));

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(drag_threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(left_of_center, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let initial_insert_index = cx
        .update_entity(&host, |host, _| {
            match host.interaction().resolved_drop_target() {
                Some(target) => match target.kind {
                    DockResolvedDropTargetKind::TabBar { insert_index, .. } => Some(insert_index),
                    _ => None,
                },
                None => None,
            }
        })
        .expect("same-stack left-of-center hover should resolve a tab insertion target");
    assert_eq!(initial_insert_index, 0);

    let mut drag_visual = VisualTestContext::from_window(window.into(), cx);
    let insertion = selector_for(
        &drag_visual,
        &host,
        DockDebugRegion::DropTabInsertionPreview,
    )
    .expect("same-stack hover should render an insertion preview");
    let initial_insertion_bounds = debug_bounds(&mut drag_visual, &insertion);

    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_mouse_move(right_of_center, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let held_insert_index = cx
        .update_entity(&host, |host, _| {
            match host.interaction().resolved_drop_target() {
                Some(target) => match target.kind {
                    DockResolvedDropTargetKind::TabBar { insert_index, .. } => Some(insert_index),
                    _ => None,
                },
                None => None,
            }
        })
        .expect("same-stack right-of-center jitter should keep a tab insertion target");
    assert_eq!(
        held_insert_index, initial_insert_index,
        "adjacent slot jitter around a tab center should keep the existing insert index"
    );

    let mut drag_visual = VisualTestContext::from_window(window.into(), cx);
    assert_eq!(
        debug_bounds(&mut drag_visual, &insertion),
        initial_insertion_bounds,
        "adjacent slot jitter around a tab center should not move the insertion preview"
    );
}

#[open_gpui::test]
fn viewport_same_stack_tab_preview_is_stable_when_pointer_is_stationary(cx: &mut TestAppContext) {
    let secondary_space = DockSpaceId::from("secondary");
    let mut graph = DockGraph::new();
    let tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    graph.set_root(secondary_space.clone(), tabs);

    let mut workspace = DockWorkspace::new(secondary_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller);
    let opened = cx
        .update(|app| {
            runtime.open_viewport_unchecked_policy(
                secondary_space.clone(),
                viewport_window_options(420.0, 220.0),
                app,
            )
        })
        .expect("secondary viewport should open through runtime handle");
    let window = opened
        .window()
        .downcast::<DockHost>()
        .expect("secondary viewport should render DockHost");
    let host = window
        .root(cx)
        .expect("secondary viewport should expose DockHost root");
    cx.run_until_parked();

    let mut visual = VisualTestContext::from_window(opened.window(), cx);
    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    let hold = point(start.x + px(28.0), start.y);

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(hold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(hold, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    let mut drag_visual = VisualTestContext::from_window(opened.window(), cx);
    let insertion = selector_for(
        &drag_visual,
        &host,
        DockDebugRegion::DropTabInsertionPreview,
    )
    .expect("same-stack stationary hover should render an insertion preview");
    let preview_tab = selector_for(
        &drag_visual,
        &host,
        DockDebugRegion::DropPayloadTabPreview { index: 0 },
    )
    .expect("same-stack stationary hover should render a payload tab preview");
    let first_insertion_bounds = debug_bounds(&mut drag_visual, &insertion);
    let first_preview_tab_bounds = debug_bounds(&mut drag_visual, &preview_tab);
    cx.update_entity(&host, |host, _| {
        host.sample_visual_affordance_transition_for_test(Duration::from_millis(0))
    })
    .expect("viewport same-stack stationary hover should schedule a visual affordance transition");
    let mut previous_progress = cx
        .update_entity(&host, |host, _| {
            host.sample_visual_affordance_transition_for_test(Duration::from_millis(40))
        })
        .expect("viewport same-stack visual affordance transition should still be active")
        .progress;

    for step in 0..4 {
        let mut visual = VisualTestContext::from_window(opened.window(), cx);
        visual.simulate_mouse_move(hold, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();

        let mut drag_visual = VisualTestContext::from_window(opened.window(), cx);
        assert_eq!(
            debug_bounds(&mut drag_visual, &insertion),
            first_insertion_bounds,
            "stationary viewport drag must not move the tab insertion preview"
        );
        assert_eq!(
            debug_bounds(&mut drag_visual, &preview_tab),
            first_preview_tab_bounds,
            "stationary viewport drag must not move the payload tab preview"
        );
        let progress = cx
            .update_entity(&host, |host, _| {
                host.sample_visual_affordance_transition_for_test(Duration::from_millis(
                    50 + step * 10,
                ))
            })
            .expect("stationary viewport same-stack hover should keep the transition alive")
            .progress;
        assert!(
            progress >= previous_progress,
            "stationary viewport hover must not restart the visual affordance transition: previous={previous_progress} current={progress}"
        );
        previous_progress = progress;
    }
}

#[open_gpui::test]
fn dragging_tab_to_target_tab_bar_empty_area_appends(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b"), item("c")],
        selected: Some(item("b")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![source_tabs, target_tabs],
        fractions: vec![0.4, 0.6],
    });
    graph.set_root(space(), root);
    let workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
    );
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(640.0), px(240.0)));

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_stack = selector_for(&visual, &host, DockDebugRegion::Tabs { node: target_tabs })
        .expect("target tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut visual, &target_stack);
    let start = debug_bounds(&mut visual, &source_tab).center();
    let end = point(
        target_bounds.origin.x + target_bounds.size.width - px(10.0),
        target_bounds.origin.y + px(12.0),
    );

    simulate_left_drag(&mut visual, start, end);
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "panel A should be active after tab bar append"
    );
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(target_tabs)
            .expect("target tabs should exist")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("b"), item("c"), item("a")]);
        assert_eq!(selected.as_ref(), items.get(2));
    });
}

#[open_gpui::test]
fn dragging_tab_to_right_edge_creates_horizontal_split(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(500.0), px(240.0)));

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: left_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("target tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut visual, &target_tabs);
    let start = debug_bounds(&mut visual, &source_tab).center();
    let end = inner_edge_drop_position(target_bounds, DropZone::Right);

    simulate_left_drag(&mut visual, start, end);
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "panel A should be visible after edge drop"
    );
    cx.read_entity(&controller, |controller, _| {
        let root = controller
            .graph()
            .root(&space())
            .expect("space should keep root");
        let DockNode::Split { axis, children, .. } =
            controller.graph().node(root).expect("root should exist")
        else {
            panic!("root should be split after edge drop");
        };
        assert_eq!(*axis, SplitAxis::Horizontal);
        assert_eq!(children.len(), 2);
    });
}

#[open_gpui::test]
fn cross_window_tab_drag_to_bottom_edge_creates_vertical_split(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_tabs);

    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.policy_mut().set_allow_platform_viewports(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let (source_window, source_host, mut source_visual) = open_controller_space_with_runtime(
        cx,
        controller.clone(),
        runtime.clone(),
        source_space.clone(),
        size(px(360.0), px(220.0)),
    );
    let (target_window, target_host, mut target_visual) = open_controller_space_with_runtime(
        cx,
        controller.clone(),
        runtime.clone(),
        target_space.clone(),
        size(px(360.0), px(220.0)),
    );

    let source_tab = selector_for(
        &source_visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_tabs_selector = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::Tabs { node: target_tabs },
    )
    .expect("target tabs selector should be emitted");
    let start = debug_bounds(&mut source_visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let target_bounds = debug_bounds(&mut target_visual, &target_tabs_selector);
    let end = inner_edge_drop_position(target_bounds, DropZone::Bottom);

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    cx.set_platform_hovered_window(Some(target_window.into()));
    target_visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut target_visual = VisualTestContext::from_window(target_window.into(), cx);

    let preview = selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview)
        .expect("target host should render a vertical split preview during cross-window drag");
    let preview_bounds = debug_bounds(&mut target_visual, &preview);
    let preview_body = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::DropPreviewBody,
    )
    .expect("edge split preview should render a split body");
    let preview_body_bounds = debug_bounds(&mut target_visual, &preview_body);
    assert!(preview_bounds.size.width > px(0.0));
    assert!(preview_bounds.contains(&preview_body_bounds.center()));
    assert!(
        preview_body_bounds.size.height < target_bounds.size.height,
        "bottom-edge preview should occupy only a horizontal band"
    );

    target_visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.set_platform_hovered_window(None);
    let source_visual = VisualTestContext::from_window(source_window.into(), cx);
    let target_visual = VisualTestContext::from_window(target_window.into(), cx);

    assert!(
        selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::Panel { item: item("a") }
        )
        .is_some(),
        "panel A should render in the target window after a bottom-edge drop"
    );
    assert!(
        selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::Panel { item: item("b") }
        )
        .is_some(),
        "panel B should remain visible in the target window after the split"
    );
    assert!(
        selector_for(
            &source_visual,
            &source_host,
            DockDebugRegion::Panel { item: item("a") }
        )
        .is_none(),
        "panel A should leave the source window after the cross-window split"
    );

    cx.read_entity(&controller, |controller, _| {
        assert_eq!(controller.graph().root(&source_space), None);
        let target_root = controller
            .graph()
            .root(&target_space)
            .expect("target space should keep a root after the split");
        let DockNode::Split { axis, children, .. } = controller
            .graph()
            .node(target_root)
            .expect("target root should exist")
        else {
            panic!("target root should become a split after the cross-window drop");
        };
        assert_eq!(*axis, SplitAxis::Vertical);
        assert_eq!(children.len(), 2);
        assert_eq!(
            controller.graph().collect_items_in_subtree(children[0]),
            vec![item("b")],
            "bottom-edge drop should keep the target tab in the top child"
        );
        assert_eq!(
            controller.graph().collect_items_in_subtree(children[1]),
            vec![item("a")],
            "bottom-edge drop should place the moved tab in the bottom child"
        );
    });
}

#[open_gpui::test]
fn cross_window_tab_drag_into_existing_split_reorients_target_child(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let left_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let right_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let target_root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left_tabs, right_tabs],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_root);

    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
    );
    workspace.policy_mut().set_allow_platform_viewports(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let (source_window, source_host, mut source_visual) = open_controller_space_with_runtime(
        cx,
        controller.clone(),
        runtime.clone(),
        source_space.clone(),
        size(px(360.0), px(220.0)),
    );
    let (target_window, target_host, mut target_visual) = open_controller_space_with_runtime(
        cx,
        controller.clone(),
        runtime.clone(),
        target_space.clone(),
        size(px(480.0), px(260.0)),
    );

    let source_tab = selector_for(
        &source_visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let right_child = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::SplitChild {
            split: target_root,
            index: 1,
        },
    )
    .expect("target right child selector should be emitted");
    let start = debug_bounds(&mut source_visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let right_child_bounds = debug_bounds(&mut target_visual, &right_child);
    let end = inner_edge_drop_position(right_child_bounds, DropZone::Bottom);

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    cx.set_platform_hovered_window(Some(target_window.into()));
    target_visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut target_visual = VisualTestContext::from_window(target_window.into(), cx);

    let preview = selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview)
        .expect("nested target should render a vertical preview during the drag");
    let preview_bounds = debug_bounds(&mut target_visual, &preview);
    let preview_body = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::DropPreviewBody,
    )
    .expect("nested edge split preview should render a split body");
    let preview_body_bounds = debug_bounds(&mut target_visual, &preview_body);
    assert!(preview_bounds.size.width > px(0.0));
    assert!(preview_bounds.contains(&preview_body_bounds.center()));
    assert!(
        preview_body_bounds.size.height < right_child_bounds.size.height,
        "nested bottom-edge preview should occupy only a horizontal band"
    );

    target_visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.set_platform_hovered_window(None);
    let source_visual = VisualTestContext::from_window(source_window.into(), cx);
    let target_visual = VisualTestContext::from_window(target_window.into(), cx);

    assert!(
        selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::Panel { item: item("a") }
        )
        .is_some(),
        "panel A should render in the target window after reorienting the child split"
    );
    assert!(
        selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::Panel { item: item("b") }
        )
        .is_some(),
        "panel B should remain in the left child"
    );
    assert!(
        selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::Panel { item: item("c") }
        )
        .is_some(),
        "panel C should remain visible in the target window after the reorientation"
    );
    assert!(
        selector_for(
            &source_visual,
            &source_host,
            DockDebugRegion::Panel { item: item("a") }
        )
        .is_none(),
        "panel A should leave the source window after the cross-window drop"
    );

    cx.read_entity(&controller, |controller, _| {
        assert_eq!(controller.graph().root(&source_space), None);
        let target_root = controller
            .graph()
            .root(&target_space)
            .expect("target space should keep a root after the nested split");
        let DockNode::Split { axis, children, .. } = controller
            .graph()
            .node(target_root)
            .expect("target root should still be a split")
        else {
            panic!("target root should remain a split");
        };
        assert_eq!(*axis, SplitAxis::Horizontal);
        assert_eq!(children.len(), 2);
        assert_eq!(
            controller.graph().collect_items_in_subtree(children[0]),
            vec![item("b")],
            "left child should remain unchanged"
        );
        let DockNode::Split {
            axis: child_axis,
            children: child_children,
            ..
        } = controller
            .graph()
            .node(children[1])
            .expect("right child should become a split")
        else {
            panic!("right child should be reoriented into a split");
        };
        assert_eq!(*child_axis, SplitAxis::Vertical);
        assert_eq!(child_children.len(), 2);
        assert_eq!(
            controller
                .graph()
                .collect_items_in_subtree(child_children[0]),
            vec![item("c")],
            "top child should keep the original right tab"
        );
        assert_eq!(
            controller
                .graph()
                .collect_items_in_subtree(child_children[1]),
            vec![item("a")],
            "bottom child should contain the moved tab"
        );
    });
}

fn cross_window_tab_drag_to_edge_creates_split(
    cx: &mut TestAppContext,
    zone: DropZone,
    expected_axis: SplitAxis,
) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_tabs);

    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.policy_mut().set_allow_platform_viewports(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let (source_window, source_host, mut source_visual) = open_controller_space_with_runtime(
        cx,
        controller.clone(),
        runtime.clone(),
        source_space.clone(),
        size(px(360.0), px(220.0)),
    );
    let (target_window, target_host, mut target_visual) = open_controller_space_with_runtime(
        cx,
        controller.clone(),
        runtime.clone(),
        target_space.clone(),
        size(px(360.0), px(220.0)),
    );

    let source_tab = selector_for(
        &source_visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_tabs_selector = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::Tabs { node: target_tabs },
    )
    .expect("target tabs selector should be emitted");
    let start = debug_bounds(&mut source_visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let target_bounds = debug_bounds(&mut target_visual, &target_tabs_selector);
    let end = inner_edge_drop_position(target_bounds, zone);

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    cx.set_platform_hovered_window(Some(target_window.into()));
    target_visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut target_visual = VisualTestContext::from_window(target_window.into(), cx);

    let preview = selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview)
        .expect("target host should render a split preview during the cross-window drag");
    let preview_bounds = debug_bounds(&mut target_visual, &preview);
    let preview_body = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::DropPreviewBody,
    )
    .expect("edge split preview should render a split body");
    let preview_body_bounds = debug_bounds(&mut target_visual, &preview_body);
    assert!(preview_bounds.size.width > px(0.0));
    assert!(preview_bounds.size.height > px(0.0));
    assert!(preview_bounds.contains(&preview_body_bounds.center()));
    match zone {
        DropZone::Left | DropZone::Right => assert!(
            preview_body_bounds.size.width < target_bounds.size.width,
            "side-edge preview should occupy only a vertical band"
        ),
        DropZone::Top | DropZone::Bottom => assert!(
            preview_body_bounds.size.height < target_bounds.size.height,
            "top/bottom-edge preview should occupy only a horizontal band"
        ),
        DropZone::Center => unreachable!("center is not an edge drop"),
    }

    target_visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.set_platform_hovered_window(None);
    let source_visual = VisualTestContext::from_window(source_window.into(), cx);
    let target_visual = VisualTestContext::from_window(target_window.into(), cx);

    assert!(
        selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::Panel { item: item("a") }
        )
        .is_some(),
        "panel A should render in the target window after the cross-window drop"
    );
    assert!(
        selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::Panel { item: item("b") }
        )
        .is_some(),
        "panel B should remain visible in the target window after the split"
    );
    assert!(
        selector_for(
            &source_visual,
            &source_host,
            DockDebugRegion::Panel { item: item("a") }
        )
        .is_none(),
        "panel A should leave the source window after the cross-window split"
    );

    let (first_expected, second_expected) = match zone {
        DropZone::Left | DropZone::Top => (item("a"), item("b")),
        DropZone::Right | DropZone::Bottom => (item("b"), item("a")),
        DropZone::Center => unreachable!("center is not an edge drop"),
    };

    cx.read_entity(&controller, |controller, _| {
        assert_eq!(controller.graph().root(&source_space), None);
        let target_root = controller
            .graph()
            .root(&target_space)
            .expect("target space should keep a root after the split");
        let DockNode::Split { axis, children, .. } = controller
            .graph()
            .node(target_root)
            .expect("target root should exist")
        else {
            panic!("target root should become a split after the cross-window drop");
        };
        assert_eq!(*axis, expected_axis);
        assert_eq!(children.len(), 2);
        assert_eq!(
            controller.graph().collect_items_in_subtree(children[0]),
            vec![first_expected],
            "edge drop should keep the expected item in the first child"
        );
        assert_eq!(
            controller.graph().collect_items_in_subtree(children[1]),
            vec![second_expected],
            "edge drop should place the moved item in the second child"
        );
    });
}

#[open_gpui::test]
fn cross_window_tab_drag_to_top_edge_creates_vertical_split(cx: &mut TestAppContext) {
    cross_window_tab_drag_to_edge_creates_split(cx, DropZone::Top, SplitAxis::Vertical);
}

#[open_gpui::test]
fn cross_window_tab_drag_to_left_edge_creates_horizontal_split(cx: &mut TestAppContext) {
    cross_window_tab_drag_to_edge_creates_split(cx, DropZone::Left, SplitAxis::Horizontal);
}

#[open_gpui::test]
fn cross_window_tab_drag_to_right_edge_creates_horizontal_split(cx: &mut TestAppContext) {
    cross_window_tab_drag_to_edge_creates_split(cx, DropZone::Right, SplitAxis::Horizontal);
}

#[open_gpui::test]
fn dragging_tab_to_edge_renders_drop_preview(cx: &mut TestAppContext) {
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
    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("target tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut visual, &target_tabs);
    let start = debug_bounds(&mut visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let end = inner_edge_drop_position(target_bounds, DropZone::Right);
    let window_id = window.window_id();

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::DropPreview).is_some(),
        "edge drop preview should be visible during the drag"
    );
    let preview = selector_for(&visual, &host, DockDebugRegion::DropPreview)
        .expect("drop preview selector should be emitted");
    let preview_bounds = debug_bounds(&mut visual, &preview);
    let preview_body = selector_for(&visual, &host, DockDebugRegion::DropPreviewBody)
        .expect("edge split preview should render a split body");
    let preview_body_bounds = debug_bounds(&mut visual, &preview_body);
    assert!(preview_bounds.size.width > px(0.0));
    assert!(preview_bounds.size.height > px(0.0));
    assert!(preview_bounds.contains(&preview_body_bounds.center()));
    assert!(
        preview_body_bounds.size.width < target_bounds.size.width,
        "edge preview should occupy only an edge band"
    );
    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::DropPayloadTabPreview { index: 0 }
        )
        .is_none(),
        "edge split previews should not render a payload tab label"
    );
    let status = cx.read_entity(&host, |host, _| host.viewport_runtime().runtime_status());
    let affordance = status
        .visual_affordances
        .iter()
        .find(|record| record.space == space() && record.window_id == window_id)
        .expect("rendered drop preview should publish a runtime affordance diagnostic");
    assert!(
        affordance.summary.active_count > 0,
        "runtime affordance diagnostic should describe the active preview"
    );

    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::DropPreview).is_none(),
        "edge drop preview should clear after release"
    );
    let status = cx.read_entity(&host, |host, _| host.viewport_runtime().runtime_status());
    assert!(
        status
            .visual_affordances
            .iter()
            .all(|record| record.window_id != window_id),
        "cleared drop preview should clear the runtime affordance diagnostic"
    );
}

#[open_gpui::test]
fn dragging_tab_to_root_edge_resolves_from_render_leaf_fact_root(cx: &mut TestAppContext) {
    let (graph, root, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
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
    let target_tabs_selector =
        selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
            .expect("target tabs selector should be emitted");
    let root_selector = selector_for(&visual, &host, DockDebugRegion::Split { node: root })
        .expect("root split selector should be emitted");
    let root_bounds = debug_bounds(&mut visual, &root_selector);
    let target_bounds = debug_bounds(&mut visual, &target_tabs_selector);
    let source_bounds = debug_bounds(&mut visual, &source_tab);
    let start = debug_bounds(&mut visual, &source_tab).center();
    let end = outer_edge_drop_position(root_bounds, DropZone::Right);

    let payload = DockDragPayload::new_item(space(), left_tabs, item("a"), "Panel A".to_string());
    window
        .update(cx, |host, window, cx| {
            host.begin_tab_item_drag_from_render(left_tabs, item("a"), &payload, window, cx);
            host.update_payload_drag_tear_off_geometry_from_render(
                &payload,
                crate::drag::DockDragTearOffGeometry::from_source_bounds(source_bounds, start)
                    .with_preferred_size(source_bounds.size),
            );
            host.begin_host_drop_scene_from_render(&payload, root_bounds, end, window, cx);
            host.update_local_root_drop_scene_from_render(
                &payload,
                root,
                root_bounds,
                end,
                window,
                cx,
            );
            host.update_local_drop_scene_fact_from_render(
                &payload,
                drop_scene_fact::leaf(root, right_tabs, target_bounds, false),
                end,
                window,
                cx,
            );
            true
        })
        .expect("host should update root-edge scene");
    cx.run_until_parked();
    let _visual = VisualTestContext::from_window(window.into(), cx);

    let target = cx
        .read_entity(&host, |host, _| {
            host.interaction().resolved_drop_target().cloned()
        })
        .expect("root edge should resolve before release");
    assert_eq!(target.source, DockDropResolveSource::RootEdge);
    assert!(matches!(
        target.kind,
        DockResolvedDropTargetKind::RootEdge {
            root: matched_root,
            leaf_tabs: Some(leaf_tabs),
            zone: DropZone::Right,
        } if matched_root == root && leaf_tabs == right_tabs
    ));
}

#[open_gpui::test]
fn floating_leaf_render_fact_does_not_resolve_against_primary_root(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let primary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![source_tabs, primary_tabs],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(space(), root);
    let floating_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_tabs,
    });
    graph
        .floating_containers_mut(space())
        .push(crate::DockFloatingContainer {
            node: floating,
            bounds: floating_bounds(40.0, 48.0, 220.0, 140.0),
        });
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
    );
    workspace.policy_mut().set_allow_floating(true);
    let (window, host, mut visual) = open_workspace(cx, workspace, size(px(420.0), px(260.0)));

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("c"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_tabs = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tabs {
            node: floating_tabs,
        },
    )
    .expect("floating tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut visual, &target_tabs);
    let start = debug_bounds(&mut visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let end = inner_edge_drop_position(target_bounds, DropZone::Right);

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let _visual = VisualTestContext::from_window(window.into(), cx);

    let target = cx
        .read_entity(&host, |host, _| {
            host.interaction().resolved_drop_target().cloned()
        })
        .expect("floating leaf should resolve before release");
    assert_eq!(target.source, DockDropResolveSource::InnerEdge);
    assert!(matches!(
        target.kind,
        DockResolvedDropTargetKind::InnerEdge {
            root: matched_root,
            target_tabs,
            zone: DropZone::Right,
        } if matched_root == floating && target_tabs == floating_tabs
    ));
}

#[open_gpui::test]
fn dragging_tab_to_empty_host_space_moves_item(cx: &mut TestAppContext) {
    let source_space = space();
    let empty_space = crate::DockSpaceId::from("empty");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(source_space.clone(), source_tabs);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "Panel A", "A")]);
    workspace.policy_mut().set_allow_platform_viewports(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let (_source_window, source_host, mut source_visual) = open_controller_space_with_runtime(
        cx,
        controller.clone(),
        runtime.clone(),
        source_space.clone(),
        size(px(360.0), px(220.0)),
    );
    let (target_window, target_host, mut target_visual) = open_controller_space_with_runtime(
        cx,
        controller.clone(),
        runtime.clone(),
        empty_space.clone(),
        size(px(360.0), px(220.0)),
    );

    let source_tab = selector_for(
        &source_visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_empty = selector_for(&target_visual, &target_host, DockDebugRegion::EmptySpace)
        .expect("empty target selector should be emitted");
    let start = debug_bounds(&mut source_visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let end = debug_bounds(&mut target_visual, &target_empty).center();

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    cx.set_platform_hovered_window(Some(target_window.into()));
    target_visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    target_visual = VisualTestContext::from_window(target_window.into(), cx);
    let preview = selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview)
        .expect("empty target should render a host-level drop preview");
    assert!(debug_bounds(&mut target_visual, &preview).size.width > px(0.0));

    target_visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.set_platform_hovered_window(None);
    let target_visual = VisualTestContext::from_window(target_window.into(), cx);

    assert!(
        selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::Panel { item: item("a") }
        )
        .is_some(),
        "panel A should render in the previously empty host after drop"
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(controller.graph().root(&source_space), None);
        let target_root = controller
            .graph()
            .root(&empty_space)
            .expect("empty space should receive a root");
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(target_root)
            .expect("target root should exist")
        else {
            panic!("target root should be tabs");
        };
        assert_eq!(items, &vec![item("a")]);
        assert_eq!(selected.as_ref(), items.get(0));
    });
}

#[open_gpui::test]
fn runtime_rendered_mouse_up_outside_viewports_tears_off_tab(cx: &mut TestAppContext) {
    let source_space = crate::DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    graph.set_root(source_space.clone(), source_tabs);

    let panel_a = test_view(cx, "A");
    let panel_a_focus = cx.read_entity(&panel_a, |panel, cx| panel.focus_handle(cx));
    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel_a);
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let opened = cx
        .update(|app| {
            runtime.open_viewport_unchecked_policy(
                source_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("source viewport should open through runtime");
    let source_window = opened
        .window()
        .downcast::<crate::DockHost>()
        .expect("runtime viewport should render DockHost");
    let source_host = source_window
        .root(cx)
        .expect("runtime viewport should expose DockHost root");
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(opened.window(), cx);

    let source_tab = selector_for(
        &visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let outside_window = point(px(900.0), px(900.0));

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(false));
    visual.simulate_mouse_up(outside_window, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    let detached_space = cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("b")]
        );
        let detached_space = controller
            .graph()
            .spaces()
            .into_iter()
            .find(|space| space.as_str().starts_with("source:tear-off:a:"))
            .expect("outside release should create a detached viewport space");
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            vec![item("a")]
        );
        detached_space
    });
    let detached_window = runtime
        .borrow()
        .adapter()
        .window_for_space(&detached_space)
        .expect("detached space should have a runtime window");
    let active_window = opened
        .window()
        .update(cx, |_, _, app| app.active_window())
        .expect("source viewport should still be live");
    assert_eq!(
        active_window.map(|window| window.window_id()),
        Some(detached_window.window_id()),
        "rendered tear-off should activate the new detached viewport"
    );
    detached_window
        .update(cx, |_, window, cx| {
            assert_eq!(
                window.focused(cx),
                Some(panel_a_focus),
                "rendered tear-off should focus the torn-off panel"
            );
        })
        .expect("detached viewport should remain live");
    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, None);
}

#[open_gpui::test]
fn runtime_nested_tab_tear_off_uses_leaf_size_not_tab_label(cx: &mut TestAppContext) {
    let source_space = crate::DockSpaceId::from("source:nested-tear-off");
    let mut graph = DockGraph::new();
    let left_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("left")],
        selected: Some(item("left")),
    });
    let top_right_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("top")],
        selected: Some(item("top")),
    });
    let bottom_right_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("bottom"), item("other")],
        selected: Some(item("bottom")),
    });
    let right_split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Vertical,
        children: vec![top_right_tabs, bottom_right_tabs],
        fractions: vec![0.5, 0.5],
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left_tabs, right_split],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(source_space.clone(), root);

    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("left", "Panel Left", "Left"),
            ("top", "Panel Top", "Top"),
            ("bottom", "Panel Bottom", "Bottom"),
            ("other", "Panel Other", "Other"),
        ],
    );
    workspace.policy_mut().set_allow_platform_viewports(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let opened = cx
        .update(|app| {
            runtime.open_viewport_unchecked_policy(
                source_space.clone(),
                viewport_window_options(640.0, 420.0),
                app,
            )
        })
        .expect("source viewport should open through runtime");
    let source_window = opened
        .window()
        .downcast::<crate::DockHost>()
        .expect("runtime viewport should render DockHost");
    let source_host = source_window
        .root(cx)
        .expect("runtime viewport should expose DockHost root");
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(opened.window(), cx);

    let bottom_leaf = selector_for(
        &visual,
        &source_host,
        DockDebugRegion::Tabs {
            node: bottom_right_tabs,
        },
    )
    .expect("bottom-right leaf selector should be emitted");
    let bottom_tab = selector_for(
        &visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: bottom_right_tabs,
            item: item("bottom"),
        },
    )
    .expect("bottom-right tab selector should be emitted");
    let leaf_bounds = debug_bounds(&mut visual, &bottom_leaf);
    let tab_bounds = debug_bounds(&mut visual, &bottom_tab);
    let rendered_leaf_bounds = runtime
        .borrow()
        .rendered_leaf_bounds_for_tabs(
            &source_space,
            Some(opened.window().window_id()),
            bottom_right_tabs,
        )
        .expect("source leaf bounds should be available before tab drag starts");
    assert!(
        rendered_leaf_bounds.size.width > tab_bounds.size.width * 2.0,
        "test must distinguish the leaf from the tab label"
    );
    assert!(
        rendered_leaf_bounds.size.height > tab_bounds.size.height * 3.0,
        "test must distinguish the leaf from the tab label"
    );
    assert_eq!(
        rendered_leaf_bounds, leaf_bounds,
        "rendered leaf bounds should describe the source leaf interior"
    );

    let start = tab_bounds.center();
    let threshold = point(start.x + px(24.0), start.y);
    let outside_window = point(px(900.0), px(900.0));

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(false));
    visual.simulate_mouse_up(outside_window, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    let detached_space = cx.read_entity(&controller, |controller, _| {
        let detached_space = controller
            .graph()
            .spaces()
            .into_iter()
            .find(|space| {
                space
                    .as_str()
                    .starts_with("source:nested-tear-off:tear-off:bottom:")
            })
            .expect("outside release should create a detached viewport space");
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            vec![item("bottom")]
        );
        detached_space
    });
    let detached_bounds = runtime
        .borrow()
        .adapter()
        .window_for_space(&detached_space)
        .expect("detached space should have a runtime window")
        .update(cx, |_, window, _| window.window_bounds().get_bounds())
        .expect("detached viewport should remain live");

    assert!(
        detached_bounds.size.width >= rendered_leaf_bounds.size.width,
        "tear-off width should come from the source leaf, not the tab label"
    );
    assert!(
        detached_bounds.size.height >= rendered_leaf_bounds.size.height,
        "tear-off height should come from the source leaf, not the tab label"
    );
    let expected_origin = outside_window - (start - rendered_leaf_bounds.origin);
    assert_close(
        f32::from(detached_bounds.origin.x),
        f32::from(expected_origin.x),
    );
    assert_close(
        f32::from(detached_bounds.origin.y),
        f32::from(expected_origin.y),
    );
    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, None);
}

#[open_gpui::test]
fn runtime_torn_off_tab_can_dock_back_to_source_viewport(cx: &mut TestAppContext) {
    let source_space = crate::DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    graph.set_root(source_space.clone(), source_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let opened = cx
        .update(|app| {
            runtime.open_viewport_unchecked_policy(
                source_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("source viewport should open through runtime");
    let source_window = opened
        .window()
        .downcast::<crate::DockHost>()
        .expect("runtime viewport should render DockHost");
    let source_host = source_window
        .root(cx)
        .expect("runtime viewport should expose DockHost root");
    cx.run_until_parked();
    let mut source_visual = VisualTestContext::from_window(opened.window(), cx);

    let source_tab = selector_for(
        &source_visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let start = debug_bounds(&mut source_visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let outside_window = point(px(900.0), px(900.0));

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(false));
    source_visual.simulate_mouse_up(outside_window, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, None);

    let (detached_space, detached_tabs) = cx.read_entity(&controller, |controller, _| {
        let detached_space = controller
            .graph()
            .spaces()
            .into_iter()
            .find(|space| space.as_str().starts_with("source:tear-off:a:"))
            .expect("outside release should create a detached viewport space");
        let (detached_tabs, _) = controller
            .graph()
            .find_item_in_space(&detached_space, &item("a"))
            .expect("detached viewport should contain torn-off item");
        (detached_space, detached_tabs)
    });
    let detached_window = runtime
        .borrow()
        .adapter()
        .window_for_space(&detached_space)
        .expect("detached space should have a runtime window");
    let detached_window = detached_window
        .downcast::<crate::DockHost>()
        .expect("detached viewport should render DockHost");
    let detached_host = detached_window
        .root(cx)
        .expect("detached viewport should expose DockHost root");
    cx.run_until_parked();
    let mut detached_visual = VisualTestContext::from_window(detached_window.into(), cx);

    let detached_tab = selector_for(
        &detached_visual,
        &detached_host,
        DockDebugRegion::Tab {
            tabs: detached_tabs,
            item: item("a"),
        },
    )
    .expect("detached tab selector should be emitted");
    let target_tabs = selector_for(
        &source_visual,
        &source_host,
        DockDebugRegion::Tabs { node: source_tabs },
    )
    .expect("source target tabs selector should remain emitted");
    let start = debug_bounds(&mut detached_visual, &detached_tab).center();
    let end = debug_bounds(&mut source_visual, &target_tabs).center();

    activate_window_for_pointer_input(&mut detached_visual);
    detached_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    detached_visual.simulate_mouse_move(
        point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.set_platform_hovered_window(Some(opened.window()));
    source_visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.set_platform_hovered_window(None);

    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("b"), item("a")]
        );
        assert!(
            controller
                .graph()
                .collect_items_in_space(&detached_space)
                .is_empty(),
            "detached viewport should be empty after docking its tab back"
        );
    });
}

#[open_gpui::test]
fn runtime_secondary_single_tab_outside_release_creates_detached_viewport(cx: &mut TestAppContext) {
    let primary_space = crate::DockSpaceId::from("primary");
    let secondary_space = crate::DockSpaceId::from("secondary");
    let mut graph = DockGraph::new();
    let primary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let secondary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(primary_space.clone(), primary_tabs);
    graph.set_root(secondary_space.clone(), secondary_tabs);

    let mut workspace = DockWorkspace::new(primary_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    cx.update(|app| {
        runtime
            .open_viewport(
                primary_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
            .expect("primary viewport should open through runtime");
    });
    let opened = cx
        .update(|app| {
            runtime.open_viewport_unchecked_policy(
                secondary_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("secondary viewport should open through runtime");
    let secondary_any_window = opened.window();
    let secondary_window = secondary_any_window
        .downcast::<crate::DockHost>()
        .expect("secondary viewport should render DockHost");
    let secondary_host = secondary_window
        .root(cx)
        .expect("secondary viewport should expose DockHost root");
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(secondary_any_window, cx);

    let source_tab = selector_for(
        &visual,
        &secondary_host,
        DockDebugRegion::Tab {
            tabs: secondary_tabs,
            item: item("b"),
        },
    )
    .expect("secondary tab selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let outside_window = point(px(900.0), px(900.0));

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(false));
    visual.simulate_mouse_up(outside_window, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    drop(visual);

    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&secondary_space),
            vec![]
        );
        let detached_space = controller
            .graph()
            .spaces()
            .into_iter()
            .find(|space| space.as_str().starts_with("secondary:tear-off:b:"))
            .expect("outside release should create a detached viewport space");
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            vec![item("b")]
        );
        assert_eq!(
            runtime.registered_viewport_spaces(),
            vec![primary_space.clone(), detached_space.clone()],
            "outside release should create a detached viewport and vacate the empty source viewport"
        );
        assert_eq!(
            runtime
                .borrow()
                .adapter()
                .window_for_space(&secondary_space),
            None,
            "empty source viewport should be unregistered after its only tab tears off"
        );
        assert!(
            runtime
                .borrow()
                .adapter()
                .window_for_space(&detached_space)
                .is_some()
        );
    });
    cx.update(|app| app.refresh_windows());
    assert!(
        secondary_any_window.update(cx, |_, _, _| ()).is_err(),
        "empty source viewport should close after its only tab tears off"
    );
    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, None);
}

#[open_gpui::test]
fn runtime_rendered_mouse_up_with_unknown_button_state_does_not_tear_off(cx: &mut TestAppContext) {
    let source_space = crate::DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    graph.set_root(source_space.clone(), source_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let opened = cx
        .update(|app| {
            runtime.open_viewport_unchecked_policy(
                source_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("source viewport should open through runtime");
    let source_window = opened
        .window()
        .downcast::<crate::DockHost>()
        .expect("runtime viewport should render DockHost");
    let source_host = source_window
        .root(cx)
        .expect("runtime viewport should expose DockHost root");
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(opened.window(), cx);

    let source_tab = selector_for(
        &visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let outside_window = point(px(900.0), px(900.0));

    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, None);
    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_up(outside_window, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    assert!(
        !cx.read(|app| app.has_active_drag()),
        "ambiguous outside release should stop the active drag session"
    );
    assert_eq!(
        runtime.registered_viewport_spaces(),
        vec![source_space.clone()],
        "unknown button state must not authorize a detached viewport"
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("a"), item("b")]
        );
    });
}

#[open_gpui::test]
fn runtime_poll_released_left_button_tears_off_without_mouse_up_event(cx: &mut TestAppContext) {
    let source_space = crate::DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    graph.set_root(source_space.clone(), source_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let opened = cx
        .update(|app| {
            runtime.open_viewport_unchecked_policy(
                source_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("source viewport should open through runtime");
    let source_window = opened
        .window()
        .downcast::<crate::DockHost>()
        .expect("runtime viewport should render DockHost");
    let source_host = source_window
        .root(cx)
        .expect("runtime viewport should expose DockHost root");
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(opened.window(), cx);

    let source_tab = selector_for(
        &visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let outside_window = point(px(900.0), px(900.0));

    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(true));
    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(outside_window, MouseButton::Left, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(20));
    cx.run_until_parked();
    assert!(
        cx.read(|app| app.has_active_drag()),
        "active drag should remain while the platform reports the left button as pressed"
    );
    assert_eq!(
        runtime.registered_viewport_spaces().len(),
        1,
        "pressed-button polling must not tear off early"
    );

    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(false));
    cx.executor().advance_clock(Duration::from_millis(20));
    cx.run_until_parked();
    assert!(
        !cx.read(|app| app.has_active_drag()),
        "fallback poll should stop the active drag after committing the release"
    );

    let detached_space = cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("b")]
        );
        let detached_space = controller
            .graph()
            .spaces()
            .into_iter()
            .find(|space| space.as_str().starts_with("source:tear-off:a:"))
            .expect("polled outside release should create a detached viewport space");
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            vec![item("a")]
        );
        detached_space
    });
    assert!(
        runtime
            .borrow()
            .adapter()
            .window_for_space(&detached_space)
            .is_some(),
        "detached space should be registered with a runtime window"
    );
    cx.set_platform_mouse_button_is_pressed(MouseButton::Left, None);
}

#[open_gpui::test]
fn runtime_rendered_mouse_up_outside_viewports_rejects_when_platform_viewports_disabled(
    cx: &mut TestAppContext,
) {
    let source_space = crate::DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    graph.set_root(source_space.clone(), source_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());

    let opened = cx
        .update(|app| {
            runtime.open_viewport_unchecked_policy(
                source_space.clone(),
                viewport_window_options(360.0, 220.0),
                app,
            )
        })
        .expect("source viewport should open through runtime");
    let source_window = opened
        .window()
        .downcast::<crate::DockHost>()
        .expect("runtime viewport should render DockHost");
    let source_host = source_window
        .root(cx)
        .expect("runtime viewport should expose DockHost root");
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(opened.window(), cx);

    let source_tab = selector_for(
        &visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let outside_window = point(px(900.0), px(900.0));

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_up(outside_window, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(opened.window(), cx);

    assert_eq!(
        runtime.registered_viewport_spaces().len(),
        1,
        "disabled platform viewports should not open a detached viewport"
    );
    assert!(
        selector_for(&visual, &source_host, DockDebugRegion::DropPreview).is_none(),
        "rejected outside release should clear the drop preview"
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("a"), item("b")]
        );
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(source_tabs)
            .expect("source tabs should remain")
        else {
            panic!("source should remain tabs");
        };
        assert_eq!(items, &vec![item("a"), item("b")]);
        assert_eq!(selected.as_ref(), items.get(0));
    });
}

#[open_gpui::test]
fn non_runtime_mouse_up_outside_host_does_not_commit_stale_drop(cx: &mut TestAppContext) {
    let (graph, tabs) = tabs_graph(&["a", "b"]);
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(360.0), px(220.0)));

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let outside_window = point(px(900.0), px(900.0));

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_up(outside_window, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "non-runtime outside release should leave the source panel active"
    );
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(tabs)
            .expect("source tabs should remain")
        else {
            panic!("source should remain tabs");
        };
        assert_eq!(items, &vec![item("a"), item("b")]);
        assert_eq!(selected.as_ref(), items.get(0));
    });
}

#[open_gpui::test]
fn dragging_tab_to_floating_title_bar_merges_into_floating_stack(cx: &mut TestAppContext) {
    let (graph, root, floating) = floating_overlay_graph();
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(360.0), px(240.0)));

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: root,
            item: item("b"),
        },
    )
    .expect("source tab selector should be emitted");
    let floating_handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::FloatingHandle { node: floating },
    )
    .expect("floating handle selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    let end = debug_bounds(&mut visual, &floating_handle).center();

    simulate_left_drag(&mut visual, start, end);
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("b") }).is_some(),
        "panel B should be active in the floating stack after title-bar drop"
    );
    cx.read_entity(&controller, |controller, _| {
        let floating_tabs = controller
            .graph()
            .floating_containers(&space())
            .iter()
            .find(|container| container.node == floating)
            .and_then(|container| match controller.graph().node(container.node) {
                Some(DockNode::Floating { child }) => Some(*child),
                _ => None,
            })
            .expect("floating child should remain");
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(floating_tabs)
            .expect("floating tabs should exist")
        else {
            panic!("floating child should be tabs");
        };
        assert_eq!(items, &vec![item("a"), item("b")]);
        assert_eq!(selected.as_ref(), items.get(1));
        assert_eq!(controller.graph().root(&space()), None);
    });
}

#[open_gpui::test]
fn dragging_floating_title_bar_to_tabs_merges_floating_stack(cx: &mut TestAppContext) {
    let (graph, root, floating) = floating_overlay_graph();
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.policy_mut().set_allow_floating(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(360.0), px(240.0)));

    let floating_handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::FloatingHandle { node: floating },
    )
    .expect("floating handle selector should be emitted");
    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: root })
        .expect("root tabs selector should be emitted");
    let start = debug_bounds(&mut visual, &floating_handle).center();
    let end = debug_bounds(&mut visual, &target_tabs).center();

    let threshold = point(start.x + px(24.0), start.y);
    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "panel A should be active in the root stack after floating title-bar drop"
    );
    cx.read_entity(&controller, |controller, _| {
        assert!(
            controller.graph().floating_containers(&space()).is_empty(),
            "floating container should be removed after its stack merges into root"
        );
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(root)
            .expect("root tabs should still exist")
        else {
            panic!("root should remain tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a")]);
        assert_eq!(selected.as_ref(), items.get(1));
    });
}

#[open_gpui::test]
fn dragging_split_floating_title_bar_to_center_rejects_visible_split_payload(
    cx: &mut TestAppContext,
) {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    graph.set_root(space(), root);
    let floating_left = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let floating_right = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let floating_split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![floating_left, floating_right],
        fractions: vec![0.5, 0.5],
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_split,
    });
    graph
        .floating_containers_mut(space())
        .push(crate::DockFloatingContainer {
            node: floating,
            bounds: floating_bounds(10.0, 20.0, 260.0, 150.0),
        });
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
    );
    workspace.policy_mut().set_allow_floating(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(420.0), px(260.0)));

    let floating_handle = selector_for(
        &visual,
        &host,
        DockDebugRegion::FloatingHandle { node: floating },
    )
    .expect("floating handle selector should be emitted");
    let target_panel = selector_for(&visual, &host, DockDebugRegion::Panel { item: item("b") })
        .expect("root panel selector should be emitted");
    let start = debug_bounds(&mut visual, &floating_handle).center();
    let end = debug_bounds(&mut visual, &target_panel).center();

    let threshold = point(start.x + px(24.0), start.y);
    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::DropPreview).is_none(),
        "split floating title bar should not publish a delivery-capable preview"
    );
    cx.read_entity(&host, |host, _| {
        assert!(
            host.interaction().drop_preview().is_none(),
            "split floating title bar should not track a drop preview for a non-single-tabs chrome target"
        );
    });

    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.read_entity(&controller, |controller, _| {
        assert!(
            !controller.graph().floating_containers(&space()).is_empty(),
            "visible split floating payload should remain floating after rejected center merge"
        );
        let DockNode::Tabs { items, selected } = controller
            .graph()
            .node(root)
            .expect("root tabs should still exist")
        else {
            panic!("root should remain tabs");
        };
        assert_eq!(items, &vec![item("b")]);
        assert_eq!(selected.as_ref(), items.first());
    });
}

#[open_gpui::test]
fn policy_rejected_edge_hover_renders_rejected_drop_preview_without_commit(
    cx: &mut TestAppContext,
) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.policy_mut().set_allow_edge_split(false);
    let (window, host, mut visual) = open_workspace(cx, workspace, size(px(500.0), px(240.0)));

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: left_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("target tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut visual, &target_tabs);
    let start = debug_bounds(&mut visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let end = inner_edge_drop_position(target_bounds, DropZone::Right);

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    let preview = selector_for(&visual, &host, DockDebugRegion::DropPreview)
        .expect("policy-rejected edge hover should render a rejected preview");
    assert!(debug_bounds(&mut visual, &preview).size.width > px(0.0));

    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "rejected release should leave the source panel in place"
    );
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("b") }).is_some(),
        "rejected release should leave the target panel in place"
    );
}

#[open_gpui::test]
fn class_rejected_edge_hover_renders_rejected_drop_preview_without_commit(cx: &mut TestAppContext) {
    let (graph, _split, left_tabs, right_tabs) = split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace.register_panel_descriptor(
        item("a"),
        DockPanelDescriptor::new("Panel A").with_dock_class("editor"),
    );
    workspace
        .policy_mut()
        .allow_dock_class_in_space(space(), "inspector");
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(500.0), px(240.0)));

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: left_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_tabs = selector_for(&visual, &host, DockDebugRegion::Tabs { node: right_tabs })
        .expect("target tabs selector should be emitted");
    let target_bounds = debug_bounds(&mut visual, &target_tabs);
    let start = debug_bounds(&mut visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let end = inner_edge_drop_position(target_bounds, DropZone::Right);

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    let preview = selector_for(&visual, &host, DockDebugRegion::DropPreview)
        .expect("class-rejected hover should render a rejected preview");
    assert!(debug_bounds(&mut visual, &preview).size.width > px(0.0));
    cx.read_entity(&host, |host, _| {
        let preview = host
            .interaction()
            .drop_preview()
            .expect("drop preview should be tracked");
        assert!(!preview.scene.decision.is_allowed());
    });

    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&space()),
            vec![item("a"), item("b")]
        );
    });
}

#[open_gpui::test]
fn policy_rejected_central_body_hover_renders_preview_without_commit(cx: &mut TestAppContext) {
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let central_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![source_tabs, central_tabs],
        fractions: vec![0.35, 0.65],
    });
    graph.set_root(space(), root);
    graph.set_central_region(space(), DockCentralRegion::with_node(central_tabs));
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    workspace
        .policy_mut()
        .set_allow_central_region_dock_over(false);
    let controller = cx.new(|_| DockController::new(workspace));
    let (window, host, mut visual) =
        open_controller_workspace(cx, controller.clone(), size(px(500.0), px(240.0)));

    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let target_panel = selector_for(&visual, &host, DockDebugRegion::Panel { item: item("b") })
        .expect("central target panel selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let end = debug_bounds(&mut visual, &target_panel).center();

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    let preview = selector_for(&visual, &host, DockDebugRegion::DropPreview)
        .expect("central policy rejection should render a drop preview");
    assert!(debug_bounds(&mut visual, &preview).size.width > px(0.0));

    visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "rejected central release should leave the source panel in place"
    );
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("b") }).is_some(),
        "rejected central release should leave the central panel in place"
    );
    cx.read_entity(&controller, |controller, _| {
        let DockNode::Tabs {
            items: source_items,
            selected: source_selected,
        } = controller
            .graph()
            .node(source_tabs)
            .expect("source tabs should remain")
        else {
            panic!("source node should remain tabs");
        };
        assert_eq!(source_items, &vec![item("a")]);
        assert_eq!(source_selected.as_ref(), source_items.get(0));

        let DockNode::Tabs {
            items: central_items,
            selected: central_selected,
        } = controller
            .graph()
            .node(central_tabs)
            .expect("central tabs should remain")
        else {
            panic!("central node should remain tabs");
        };
        assert_eq!(central_items, &vec![item("b")]);
        assert_eq!(central_selected.as_ref(), central_items.get(0));
    });
}

#[open_gpui::test]
fn clicking_inactive_tab_updates_selected_panel(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"]);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "panel A should be selected before mutation"
    );

    let tab_b = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: root,
            item: item("b"),
        },
    )
    .expect("tab B selector should be emitted");
    let tab_b_bounds = debug_bounds(&mut visual, &tab_b);
    visual.simulate_click(tab_b_bounds.center(), Modifiers::none());
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("b") }).is_some(),
        "panel B should be selected after mutation"
    );
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_none(),
        "panel A should no longer be mounted after mutation"
    );
}

#[open_gpui::test]
fn clicking_tab_close_removes_closable_panel_from_graph(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"]);
    let (window, host, mut visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(240.0)),
    );

    let close_b = selector_for(
        &visual,
        &host,
        DockDebugRegion::TabClose {
            tabs: root,
            item: item("b"),
        },
    )
    .expect("closable tab should render a close control");
    let close_b_bounds = debug_bounds(&mut visual, &close_b);
    visual.simulate_click(close_b_bounds.center(), Modifiers::none());
    cx.run_until_parked();
    let visual = VisualTestContext::from_window(window.into(), cx);

    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::Tab {
                tabs: root,
                item: item("b"),
            },
        )
        .is_none(),
        "closed tab should be removed from rendered graph state"
    );
    assert!(
        selector_for(&visual, &host, DockDebugRegion::Panel { item: item("a") }).is_some(),
        "closing an inactive tab should keep the previous selected panel mounted"
    );
    let (items, selected, metadata_still_registered) = cx.update_entity(&host, |host, cx| {
        host.with_workspace(cx, |workspace| {
            let DockNode::Tabs { items, selected } = workspace
                .graph()
                .node(root)
                .expect("root tabs should remain")
            else {
                panic!("root should stay as tabs");
            };
            (
                items.clone(),
                selected.clone(),
                workspace.panels().contains(&item("b")),
            )
        })
    });
    assert_eq!(items, vec![item("a")]);
    assert_eq!(selected.as_ref(), items.first());
    assert!(
        metadata_still_registered,
        "close should remove graph membership without discarding panel metadata"
    );
}

#[open_gpui::test]
fn non_closable_tab_omits_close_control_and_rejects_close_action(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["locked", "open"]);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel(
        item("locked"),
        DockPanel::new("Locked", test_view(cx, "A")).closable(false),
    );
    workspace.register_panel_view(item("open"), "Open", test_view(cx, "B"));
    let (_window, host, visual) = open_workspace(cx, workspace, size(px(400.0), px(240.0)));

    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::TabClose {
                tabs: root,
                item: item("locked"),
            },
        )
        .is_none(),
        "non-closable tab should not expose a rendered close affordance"
    );
    assert!(
        selector_for(
            &visual,
            &host,
            DockDebugRegion::TabClose {
                tabs: root,
                item: item("open"),
            },
        )
        .is_some(),
        "closable sibling should still expose a close affordance"
    );

    let changed = cx.update_entity(&host, |host, cx| {
        host.close_item_from_render(item("locked"), cx)
    });
    assert!(!changed);

    let items = cx.update_entity(&host, |host, cx| {
        host.with_workspace(cx, |workspace| {
            let DockNode::Tabs { items, .. } = workspace
                .graph()
                .node(root)
                .expect("root tabs should remain")
            else {
                panic!("root should stay as tabs");
            };
            items.clone()
        })
    });
    assert_eq!(items, vec![item("locked"), item("open")]);
}
