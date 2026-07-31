use crate::{
    DockCentralRegion, DockController, DockFloatingContainer, DockGraph, DockHost, DockItemId,
    DockNode, DockNodeId, DockPanel, DockPanelDescriptor, DockPanelPlacement, DockSpaceId,
    DockSurface, DockSurfacePrimaryWindowOpenOutcome, DockSurfaceWindowSessionShutdownReason,
    DockViewportRuntimeHandle, DockWorkspace, DropZone, SplitAxis,
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
    AnyView, AnyWindowHandle, AppContext as _, Bounds, Context, DevicePixels, Entity, Focusable,
    InteractiveElement, IntoElement, Modifiers, MouseButton, ParentElement, Pixels,
    PlatformWindowHit, PlatformWindowHitStack, PlatformWindowPhysicalCoverage,
    PlatformWindowPhysicalGeometry, Point, Render, Size, Styled, SubtreeTransform,
    SubtreeTransformExt, SubtreeTransformOrigin, TestAppContext, VisualTestContext, Window,
    WindowMouseEvent, canvas, div, point, px, size,
};
use slotmap::Key;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    time::Duration,
};

struct OccludedDockHostFixture {
    host: Entity<DockHost>,
}

struct NativeSceneWorkContextSabotagePanel {
    owner: Rc<RefCell<Option<Entity<crate::surface::DockSurfaceOwner>>>>,
    armed: Rc<Cell<bool>>,
}

impl Render for NativeSceneWorkContextSabotagePanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let owner = self.owner.clone();
        let armed = self.armed.clone();
        canvas(
            |_, _, _| {},
            move |_, _, window, cx| {
                if !armed.replace(false) {
                    return;
                }
                let owner = owner
                    .borrow()
                    .clone()
                    .expect("the sabotage panel should be attached to its surface owner");
                let window_id = window.window_handle().window_id();
                owner.update(cx, |owner, _| {
                    let lease = owner
                        .window_session()
                        .active_lease()
                        .expect("the source scene candidate should use an active surface lease");
                    assert!(matches!(
                        owner.window_session_mut().begin_shutdown(
                            lease,
                            DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
                            [window_id],
                        ),
                        crate::surface::window_session::DockSurfaceWindowSessionBeginShutdownOutcome::Started { .. }
                    ));
                });
            },
        )
        .size_full()
    }
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

fn configure_native_desktop_release(
    cx: &TestAppContext,
    source_window: AnyWindowHandle,
    source_size: Size<DevicePixels>,
) {
    let source_bounds = Bounds::new(point(DevicePixels(0), DevicePixels(0)), source_size);
    cx.set_platform_window_physical_client_geometry(source_window, Some(source_bounds), 2.0);
    let sampled_point = point(DevicePixels(1800), DevicePixels(1800));
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available(sampled_point, Vec::new())
            .expect("desktop release observation should be valid"),
    );
}

fn advertise_native_window_hit_stack(cx: &TestAppContext) {
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available(
            point(DevicePixels(-1), DevicePixels(-1)),
            Vec::new(),
        )
        .expect("an empty point-scoped hit observation should be valid"),
    );
}

fn configure_native_registered_window_hit(
    cx: &TestAppContext,
    source_window: AnyWindowHandle,
    target_window: AnyWindowHandle,
    target_point: Point<Pixels>,
) {
    configure_native_registered_window_hit_with_target_size(
        cx,
        source_window,
        target_window,
        target_point,
        size(px(360.0), px(220.0)),
    );
}

fn configure_native_registered_window_hit_with_target_size(
    cx: &TestAppContext,
    source_window: AnyWindowHandle,
    target_window: AnyWindowHandle,
    target_point: Point<Pixels>,
    target_size: Size<Pixels>,
) {
    let source_bounds = Bounds::new(
        point(DevicePixels(0), DevicePixels(0)),
        size(DevicePixels(720), DevicePixels(440)),
    );
    let target_bounds = Bounds::new(
        point(DevicePixels(800), DevicePixels(0)),
        size(
            DevicePixels((target_size.width.as_f32() * 2.0).round() as i32),
            DevicePixels((target_size.height.as_f32() * 2.0).round() as i32),
        ),
    );
    cx.set_platform_window_physical_client_geometry(source_window, Some(source_bounds), 2.0);
    cx.set_platform_window_physical_client_geometry(target_window, Some(target_bounds), 2.0);
    let sampled_point = point(
        DevicePixels((target_point.x.as_f32() * 2.0).round() as i32),
        DevicePixels((target_point.y.as_f32() * 2.0).round() as i32),
    );
    let coverage = PlatformWindowPhysicalCoverage::try_new(target_bounds)
        .expect("target coverage should be representable");
    let geometry = PlatformWindowPhysicalGeometry::try_new(target_bounds, 2.0)
        .expect("target physical geometry should be representable");
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available(
            sampled_point,
            vec![PlatformWindowHit::RegisteredApplication {
                window: target_window,
                coverage,
                geometry,
            }],
        )
        .expect("registered target hit observation should be valid"),
    );
}

struct NativeCapturedSourceFixture {
    surface: DockSurface,
    controller: Entity<DockController>,
    runtime: DockViewportRuntimeHandle,
    source_window: AnyWindowHandle,
    source_host: Entity<DockHost>,
    source_visual: VisualTestContext,
    start: Point<Pixels>,
    threshold: Point<Pixels>,
    target: Point<Pixels>,
    payload: DockDragPayload,
}

impl NativeCapturedSourceFixture {
    fn begin_drag(&mut self, cx: &mut TestAppContext) {
        activate_window_for_pointer_input(&mut self.source_visual);
        self.source_visual
            .simulate_mouse_down(self.start, MouseButton::Left, Modifiers::none());
        self.source_visual.simulate_mouse_move(
            self.threshold,
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.run_until_parked();
        assert!(
            self.runtime
                .active_payload_drag_session(&self.payload)
                .is_some()
        );
    }
}

fn native_captured_source_fixture(cx: &mut TestAppContext) -> NativeCapturedSourceFixture {
    let (surface, controller, runtime, source_window, source_tabs) = cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([
                DockPanelPlacement::center("a").selected(),
                DockPanelPlacement::stacked_with("b", "a"),
            ])
            .panel_factory("a", "Panel A", |cx| {
                cx.new(|cx| TestPanel::new("A", cx)).into()
            })
            .panel_factory("b", "Panel B", |cx| {
                cx.new(|cx| TestPanel::new("B", cx)).into()
            })
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the captured-drag source surface should build");
        let controller = surface.controller(cx);
        let runtime = surface.viewport_runtime(cx);
        let source_tabs = cx.read_entity(&controller, |controller, _| {
            controller
                .graph()
                .root(&DockSpaceId::from("main"))
                .expect("the source surface should retain its root tabs")
        });
        let source_window =
            match surface.open_primary_window(viewport_window_options(360.0, 220.0), cx) {
                DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
                outcome => panic!("the captured-drag source window should open, got {outcome:?}"),
            };
        (surface, controller, runtime, source_window, source_tabs)
    });
    cx.run_until_parked();

    let source_host = source_window
        .downcast::<DockHost>()
        .expect("the source window should retain a DockHost root")
        .entity(cx)
        .expect("the source DockHost should remain live");
    let mut source_visual = VisualTestContext::from_window(source_window, cx);
    let source_tab = selector_for(
        &source_visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("the source tab selector should be emitted");
    let target_tab = selector_for(
        &source_visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("b"),
        },
    )
    .expect("the target tab selector should be emitted");
    let start = debug_bounds(&mut source_visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let target = debug_bounds(&mut source_visual, &target_tab).center();
    let payload = DockDragPayload::new_item(
        DockSpaceId::from("main"),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    advertise_native_window_hit_stack(cx);

    NativeCapturedSourceFixture {
        surface,
        controller,
        runtime,
        source_window,
        source_host,
        source_visual,
        start,
        threshold,
        target,
        payload,
    }
}

struct NativeCapturedForeignFixture {
    source_space: DockSpaceId,
    source_tabs: DockNodeId,
    source_controller: Entity<DockController>,
    source_runtime: DockViewportRuntimeHandle,
    source_window: AnyWindowHandle,
    source_host: Entity<DockHost>,
    source_visual: VisualTestContext,
    target_space: DockSpaceId,
    target_controller: Entity<DockController>,
    target_runtime: DockViewportRuntimeHandle,
    target_window: AnyWindowHandle,
    target_host: Entity<DockHost>,
    target_global_from_source: Point<Pixels>,
    payload: DockDragPayload,
}

fn native_captured_foreign_preview_fixture(
    cx: &mut TestAppContext,
) -> NativeCapturedForeignFixture {
    let source_space = DockSpaceId::from("source");
    let mut source_graph = DockGraph::new();
    let source_tabs = source_graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    source_graph.set_root(source_space.clone(), source_tabs);
    let source_workspace = workspace_with_panels(
        cx,
        source_graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
    );
    let source_controller = cx.new(|_| DockController::new(source_workspace));
    let source_runtime = DockViewportRuntimeHandle::new(source_controller.clone());

    let target_space = DockSpaceId::from("target");
    let mut target_graph = DockGraph::new();
    let target_tabs = target_graph.insert_node(DockNode::Tabs {
        items: vec![item("x")],
        selected: Some(item("x")),
    });
    target_graph.set_root(target_space.clone(), target_tabs);
    let target_workspace = workspace_with_panels(cx, target_graph, &[("x", "Panel X", "X")]);
    let target_controller = cx.new(|_| DockController::new(target_workspace));
    let target_runtime = DockViewportRuntimeHandle::new(target_controller.clone());

    let (source_window, source_host, mut source_visual) = open_controller_space_with_runtime(
        cx,
        source_controller.clone(),
        source_runtime.clone(),
        source_space.clone(),
        size(px(360.0), px(220.0)),
    );
    let (target_window, target_host, mut target_visual) = open_controller_space_with_runtime(
        cx,
        target_controller.clone(),
        target_runtime.clone(),
        target_space.clone(),
        size(px(360.0), px(220.0)),
    );
    let source_window = source_window.into();
    let target_window = target_window.into();

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
    let target_local = debug_bounds(&mut target_visual, &target_tabs_selector).center();
    let target_global_from_source = point(px(400.0) + target_local.x, target_local.y);
    configure_native_registered_window_hit(
        cx,
        source_window,
        target_window,
        target_global_from_source,
    );

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(
        target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );

    NativeCapturedForeignFixture {
        source_space,
        source_tabs,
        source_controller,
        source_runtime,
        source_window,
        source_host,
        source_visual,
        target_space,
        target_controller,
        target_runtime,
        target_window,
        target_host,
        target_global_from_source,
        payload,
    }
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
    host.read_with(&visual, |host, _| {
        assert!(host.floating_drag().is_some());
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
        assert!(host.floating_drag().is_none());
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
    owner.read_with(&visual, |host, _| {
        assert!(host.active_payload_drag_session(&payload).is_some());
    });

    visual.deactivate_window();
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    assert!(visual.update(|_, cx| cx.active_drag_value::<DockDragPayload>().is_none()));
    owner.read_with(&visual, |host, _| {
        assert!(host.active_payload_drag_session(&payload).is_none());
        assert!(host.floating_drag().is_none());
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
    let target_global_from_source = point(px(400.0) + end.x, end.y);
    configure_native_registered_window_hit(
        cx,
        source_window.into(),
        target_window.into(),
        target_global_from_source,
    );

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(
        target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
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
    let source_feedback_visual = VisualTestContext::from_window(source_window.into(), cx);
    assert!(
        selector_for(
            &source_feedback_visual,
            &source_host,
            DockDebugRegion::DropRoutePreview {
                kind: crate::drop_preview::DockDropRoutePreviewKind::KnownViewport,
            },
        )
        .is_some(),
        "the native route should pair the target overlay with an exact source-side marker"
    );
    assert!(
        runtime
            .routed_drop_preview_for(&target_space, target_window.window_id())
            .is_some()
    );
    assert!(
        runtime
            .routed_drop_route_preview_for(&source_space, source_window.window_id())
            .is_some()
    );

    let (stale_binding, stale_registration, stale_frame) =
        cx.read_entity(&target_host, |host, _| {
            (
                host.current_window_binding()
                    .expect("the G1 target host should retain its exact window binding"),
                host.current_viewport_registration()
                    .expect("the G1 target host should retain its exact registration"),
                host.interaction()
                    .viewport_host_scene_frame()
                    .cloned()
                    .expect("the G1 target host should retain its committed scene frame"),
            )
        });
    let replacement_registration = runtime
        .borrow_mut()
        .replace_adapter_registration_for_test(target_space.clone(), target_window.into());
    target_host.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();

    let (current_binding, current_registration, current_frame) =
        cx.read_entity(&target_host, |host, _| {
            (
                host.current_window_binding()
                    .expect("the G2 target host should retain its exact window binding"),
                host.current_viewport_registration()
                    .expect("the G2 target host should retain its exact registration"),
                host.interaction()
                    .viewport_host_scene_frame()
                    .cloned()
                    .expect("the G2 target host should retain its committed scene frame"),
            )
        });
    assert_ne!(current_binding, stale_binding);
    assert_ne!(current_registration, stale_registration);
    assert_eq!(current_registration, replacement_registration);
    assert_ne!(current_frame, stale_frame);
    assert!(
        runtime
            .routed_drop_preview_for(&target_space, target_window.window_id())
            .is_some(),
        "publishing G2 should reproject the latest captured event without pointer motion"
    );

    cx.update(|app| {
        crate::native_captured_drag::clear_native_captured_host_scene(
            target_window.window_id(),
            &target_host.downgrade(),
            stale_binding,
            Some(&stale_frame),
            app,
        );
    });
    cx.run_until_parked();
    assert!(
        runtime
            .routed_drop_preview_for(&target_space, target_window.window_id())
            .is_some(),
        "a delayed G1 cleanup must not erase the current G2 target overlay"
    );
    assert!(
        runtime
            .routed_drop_route_preview_for(&source_space, source_window.window_id())
            .is_some(),
        "a delayed G1 cleanup must not erase the G2 paired source marker"
    );

    source_visual.simulate_mouse_up(
        target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
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
        !cx.windows().contains(&source_window.into()),
        "the vacated source viewport should close after the cross-window split"
    );
    assert_eq!(
        runtime.borrow().adapter().window_for_space(&source_space),
        None,
        "the vacated source space should no longer own a runtime window"
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
    let target_global_from_source = point(px(400.0) + end.x, end.y);
    configure_native_registered_window_hit_with_target_size(
        cx,
        source_window.into(),
        target_window.into(),
        target_global_from_source,
        size(px(480.0), px(260.0)),
    );

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(
        target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
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

    source_visual.simulate_mouse_up(
        target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
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
        !cx.windows().contains(&source_window.into()),
        "the vacated source viewport should close after the cross-window drop"
    );
    assert_eq!(
        runtime.borrow().adapter().window_for_space(&source_space),
        None,
        "the vacated source space should no longer own a runtime window"
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
    let target_global_from_source = point(px(400.0) + end.x, end.y);
    configure_native_registered_window_hit(
        cx,
        source_window.into(),
        target_window.into(),
        target_global_from_source,
    );

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(
        target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
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

    source_visual.simulate_mouse_up(
        target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
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
        !cx.windows().contains(&source_window.into()),
        "the vacated source viewport should close after the cross-window split"
    );
    assert_eq!(
        runtime.borrow().adapter().window_for_space(&source_space),
        None,
        "the vacated source space should no longer own a runtime window"
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
    let target_global_from_source = point(px(400.0) + end.x, end.y);
    configure_native_registered_window_hit(
        cx,
        source_window.into(),
        target_window.into(),
        target_global_from_source,
    );

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(
        target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    target_visual = VisualTestContext::from_window(target_window.into(), cx);
    let preview = selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview)
        .expect("empty target should render a host-level drop preview");
    assert!(debug_bounds(&mut target_visual, &preview).size.width > px(0.0));

    source_visual.simulate_mouse_up(
        target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
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
fn native_captured_release_does_not_retarget_to_a_scene_created_by_mouse_up_listener(
    cx: &mut TestAppContext,
) {
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
        empty_space.clone(),
        size(px(360.0), px(220.0)),
    );
    let source_window: AnyWindowHandle = source_window.into();
    let target_window: AnyWindowHandle = target_window.into();

    let source_tab = selector_for(
        &source_visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("the source tab selector should be emitted");
    let target_empty = selector_for(&target_visual, &target_host, DockDebugRegion::EmptySpace)
        .expect("the empty target selector should be emitted");
    let start = debug_bounds(&mut source_visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let end = debug_bounds(&mut target_visual, &target_empty).center();
    let target_global_from_source = point(px(400.0) + end.x, end.y);
    configure_native_registered_window_hit(
        cx,
        source_window,
        target_window,
        target_global_from_source,
    );

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(
        target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    target_visual = VisualTestContext::from_window(target_window, cx);
    assert!(
        selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview).is_some(),
        "G1 should be the visible release candidate before MouseUp"
    );
    let locked_frame = cx.read_entity(&target_host, |host, _| {
        host.interaction()
            .viewport_host_scene_frame()
            .cloned()
            .expect("G1 should have a committed target scene frame")
    });

    let listener_replaced_target = Rc::new(Cell::new(false));
    let _interceptor = cx
        .update_window(source_window, {
            let runtime = runtime.clone();
            let empty_space = empty_space.clone();
            let target_host = target_host.clone();
            let listener_replaced_target = listener_replaced_target.clone();
            move |_, window, _| {
                window.intercept_window_mouse_events(move |event, _, cx| {
                    if matches!(event, WindowMouseEvent::Up(_))
                        && !listener_replaced_target.replace(true)
                    {
                        runtime.borrow_mut().replace_adapter_registration_for_test(
                            empty_space.clone(),
                            target_window,
                        );
                        target_host.update(cx, |_, host_cx| host_cx.notify());
                    }
                })
            }
        })
        .expect("the source should install its MouseUp interceptor");

    source_visual.simulate_mouse_up(
        target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    assert!(listener_replaced_target.get());
    let replacement_frame = cx.read_entity(&target_host, |host, _| {
        host.interaction()
            .viewport_host_scene_frame()
            .cloned()
            .expect("the listener-created G2 scene should commit")
    });
    assert_ne!(replacement_frame, locked_frame);
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("a")],
            "a stale G1 release reservation must not remove the source item"
        );
        assert_eq!(
            controller.graph().root(&empty_space),
            None,
            "the MouseUp listener's G2 target did not exist when release was locked"
        );
    });
    assert_eq!(
        runtime.active_payload_drag_session(&DockDragPayload::new_item(
            source_space,
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        )),
        None,
        "the stale release must still retire its exact drag session"
    );
}

#[open_gpui::test]
fn native_captured_move_without_physical_frame_clears_preview_without_revision(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_source_fixture(cx);
    fixture.begin_drag(cx);
    let session = fixture
        .runtime
        .active_payload_drag_session(&fixture.payload)
        .expect("the source drag session should remain active");
    let revision = cx.read(|app| fixture.surface.revision(app));

    cx.set_platform_window_physical_client_geometry(fixture.source_window, None, 2.0);
    fixture
        .source_visual
        .simulate_mouse_move(fixture.target, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_some()
    );
    assert!(
        !fixture
            .runtime
            .has_routed_drop_preview_for_drag_session(Some(&session)),
        "a move without a callback-scoped physical frame must not retain a routed preview"
    );
    assert_eq!(fixture.runtime.runtime_status().last_drop_outcome, None);
    assert_eq!(cx.read(|app| fixture.surface.revision(app)), revision);
    cx.read_entity(&fixture.controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&DockSpaceId::from("main")),
            vec![item("a"), item("b")]
        );
    });

    let source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    assert!(
        selector_for(
            &source_visual,
            &fixture.source_host,
            DockDebugRegion::DropPreview,
        )
        .is_none()
    );
    fixture.source_visual.deactivate_window();
    cx.run_until_parked();
}

#[open_gpui::test]
fn native_captured_release_with_unavailable_hit_stack_does_not_drop_or_publish_revision(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_source_fixture(cx);
    fixture.begin_drag(cx);
    let session = fixture
        .runtime
        .active_payload_drag_session(&fixture.payload)
        .expect("the source drag session should remain active");
    let revision = cx.read(|app| fixture.surface.revision(app));
    cx.set_platform_window_physical_client_geometry(
        fixture.source_window,
        Some(Bounds::new(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(720), DevicePixels(440)),
        )),
        2.0,
    );
    cx.set_platform_window_hit_stack(PlatformWindowHitStack::Unavailable);

    fixture
        .source_visual
        .simulate_mouse_move(fixture.target, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.read(|app| fixture.surface.revision(app)), revision);
    fixture
        .source_visual
        .simulate_mouse_up(fixture.target, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    assert_eq!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload),
        None
    );
    assert!(
        !fixture
            .runtime
            .has_routed_drop_preview_for_drag_session(Some(&session)),
        "an unavailable release must retire every routed preview"
    );
    assert_eq!(cx.read(|app| fixture.surface.revision(app)), revision);
    cx.read_entity(&fixture.controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&DockSpaceId::from("main")),
            vec![item("a"), item("b")],
            "an unavailable native hit stack must not fall back to the source drop scene"
        );
    });
    let status = fixture.runtime.runtime_status();
    assert_eq!(status.last_activation, None);
    assert_eq!(status.last_tear_off, None);
}

#[open_gpui::test]
fn stale_native_scene_work_context_retires_source_route_without_another_pointer_event(
    cx: &mut TestAppContext,
) {
    let owner = Rc::new(RefCell::new(None));
    let armed = Rc::new(Cell::new(false));
    let panel_owner = owner.clone();
    let panel_armed = armed.clone();
    let (surface, source_window, source_tabs) = cx.update(|cx| {
        let surface = DockSurface::builder("main")
            .panel_placements([
                DockPanelPlacement::center("a").selected(),
                DockPanelPlacement::stacked_with("b", "a"),
            ])
            .panel_factory("a", "Panel A", move |cx| {
                let owner = panel_owner.clone();
                let armed = panel_armed.clone();
                cx.new(move |_| NativeSceneWorkContextSabotagePanel { owner, armed })
                    .into()
            })
            .panel_factory("b", "Panel B", |cx| {
                cx.new(|cx| TestPanel::new("B", cx)).into()
            })
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the managed source surface should build");
        owner.borrow_mut().replace(surface.owner().clone());
        let controller = surface.controller(cx);
        let source_tabs = cx.read_entity(&controller, |controller, _| {
            controller
                .graph()
                .root(&DockSpaceId::from("main"))
                .expect("the managed source should retain its root tabs")
        });
        let source_window =
            match surface.open_primary_window(viewport_window_options(360.0, 220.0), cx) {
                DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
                outcome => panic!("the managed source window should open, got {outcome:?}"),
            };
        (surface, source_window, source_tabs)
    });
    cx.run_until_parked();

    let source_host = source_window
        .downcast::<DockHost>()
        .expect("the managed source window should retain a DockHost root")
        .entity(cx)
        .expect("the managed source DockHost should remain live");
    let mut source_visual = VisualTestContext::from_window(source_window, cx);
    let source_tab = selector_for(
        &source_visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("the managed source tab selector should be emitted");
    let start = debug_bounds(&mut source_visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let payload = DockDragPayload::new_item(
        DockSpaceId::from("main"),
        source_tabs,
        item("a"),
        "Panel A".to_string(),
    );
    let runtime = surface.viewport_runtime(cx);
    advertise_native_window_hit_stack(cx);

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    assert!(runtime.active_payload_drag_session(&payload).is_some());
    assert!(cx.update(|cx| {
        crate::native_captured_drag::has_active_native_captured_drag_route_for_test(cx)
    }));

    armed.set(true);
    source_host.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();

    assert!(cx.read_entity(surface.owner(), |owner, _| {
        owner.window_session().active_lease().is_none()
    }));
    assert_eq!(runtime.active_payload_drag_session(&payload), None);
    assert!(!cx.update(|cx| {
        crate::native_captured_drag::has_active_native_captured_drag_route_for_test(cx)
    }));
    assert!(cx.read_entity(&source_host, |host, _| {
        host.payload_drag_anchor_position_from_render(&payload)
            .is_none()
    }));
}

#[open_gpui::test]
fn stale_native_source_scene_frame_cleanup_preserves_route_with_replacement(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_foreign_preview_fixture(cx);
    let (binding, stale_frame) = cx.read_entity(&fixture.source_host, |host, _| {
        (
            host.current_window_binding()
                .expect("the source host should have a window binding"),
            host.interaction()
                .viewport_host_scene_frame()
                .cloned()
                .expect("the source host should have a committed G1 scene frame"),
        )
    });

    cx.simulate_window_resize(fixture.source_window, size(px(420.0), px(240.0)));
    cx.run_until_parked();

    let current_frame = cx.read_entity(&fixture.source_host, |host, _| {
        assert_eq!(host.current_window_binding(), Some(binding));
        host.interaction()
            .viewport_host_scene_frame()
            .cloned()
            .expect("the resized source host should commit a G2 scene frame")
    });
    assert_eq!(
        current_frame.registration_key(),
        stale_frame.registration_key()
    );
    assert_ne!(current_frame, stale_frame);

    cx.update(|cx| {
        crate::native_captured_drag::clear_native_captured_host_scene(
            fixture.source_window.window_id(),
            &fixture.source_host.downgrade(),
            binding,
            Some(&stale_frame),
            cx,
        );
    });
    cx.run_until_parked();

    assert!(
        fixture
            .source_runtime
            .active_payload_drag_session(&fixture.payload)
            .is_some()
    );
    assert!(cx.update(|cx| {
        crate::native_captured_drag::has_active_native_captured_drag_route_for_test(cx)
    }));
    let target_visual = VisualTestContext::from_window(fixture.target_window, cx);
    assert!(
        selector_for(
            &target_visual,
            &fixture.target_host,
            DockDebugRegion::DropRoutePreview {
                kind: crate::drop_preview::DockDropRoutePreviewKind::Rejected,
            },
        )
        .is_some(),
        "a delayed source G1 cleanup must preserve the G2 route and its target feedback"
    );

    fixture.source_visual.deactivate_window();
    cx.run_until_parked();
}

#[open_gpui::test]
fn stale_native_scene_registration_cleanup_preserves_replacement_foreign_preview(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_foreign_preview_fixture(cx);
    let (stale_binding, stale_registration, stale_frame) =
        cx.read_entity(&fixture.target_host, |host, _| {
            (
                host.current_window_binding()
                    .expect("the G1 target host should have a window binding"),
                host.current_viewport_registration()
                    .expect("the G1 target host should have a registration"),
                host.interaction()
                    .viewport_host_scene_frame()
                    .cloned()
                    .expect("the G1 target host should have a committed scene frame"),
            )
        });
    let replacement_registration = fixture
        .target_runtime
        .borrow_mut()
        .replace_adapter_registration_for_test(fixture.target_space.clone(), fixture.target_window);

    fixture.target_host.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();

    let (current_binding, current_registration) =
        cx.read_entity(&fixture.target_host, |host, _| {
            (
                host.current_window_binding()
                    .expect("the G2 target host should have a window binding"),
                host.current_viewport_registration()
                    .expect("the G2 target host should have a registration"),
            )
        });
    assert_ne!(current_binding, stale_binding);
    assert_ne!(current_registration, stale_registration);
    assert_eq!(current_registration, replacement_registration);
    let mut target_visual = VisualTestContext::from_window(fixture.target_window, cx);
    assert!(
        selector_for(
            &target_visual,
            &fixture.target_host,
            DockDebugRegion::DropRoutePreview {
                kind: crate::drop_preview::DockDropRoutePreviewKind::Rejected,
            },
        )
        .is_some(),
        "publishing G2 should reproject the latest captured event without another pointer event"
    );

    cx.update(|cx| {
        crate::native_captured_drag::clear_native_captured_host_scene(
            fixture.target_window.window_id(),
            &fixture.target_host.downgrade(),
            stale_binding,
            Some(&stale_frame),
            cx,
        );
    });
    cx.run_until_parked();

    target_visual = VisualTestContext::from_window(fixture.target_window, cx);
    assert!(
        selector_for(
            &target_visual,
            &fixture.target_host,
            DockDebugRegion::DropRoutePreview {
                kind: crate::drop_preview::DockDropRoutePreviewKind::Rejected,
            },
        )
        .is_some(),
        "a delayed G1 cleanup must preserve the current G2 foreign preview"
    );
    assert!(
        fixture
            .source_runtime
            .active_payload_drag_session(&fixture.payload)
            .is_some()
    );

    fixture.source_visual.deactivate_window();
    cx.run_until_parked();
}

#[open_gpui::test]
fn stale_native_scene_frame_cleanup_preserves_newer_same_registration_foreign_preview(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_foreign_preview_fixture(cx);
    let (binding, stale_frame) = cx.read_entity(&fixture.target_host, |host, _| {
        (
            host.current_window_binding()
                .expect("the target host should have a window binding"),
            host.interaction()
                .viewport_host_scene_frame()
                .cloned()
                .expect("the target host should have a committed G1 scene frame"),
        )
    });

    cx.simulate_window_resize(fixture.target_window, size(px(360.0), px(220.0)));
    cx.run_until_parked();

    let current_frame = cx.read_entity(&fixture.target_host, |host, _| {
        assert_eq!(host.current_window_binding(), Some(binding));
        host.interaction()
            .viewport_host_scene_frame()
            .cloned()
            .expect("the same-layout target host should commit a complete G2 scene frame")
    });
    assert_eq!(
        current_frame.registration_key(),
        stale_frame.registration_key()
    );
    assert_ne!(current_frame, stale_frame);
    let mut target_visual = VisualTestContext::from_window(fixture.target_window, cx);
    assert!(
        selector_for(
            &target_visual,
            &fixture.target_host,
            DockDebugRegion::DropRoutePreview {
                kind: crate::drop_preview::DockDropRoutePreviewKind::Rejected,
            },
        )
        .is_some(),
        "a same-semantic G2 frame must reproject the latest event without another pointer event"
    );

    cx.update(|cx| {
        crate::native_captured_drag::clear_native_captured_host_scene(
            fixture.target_window.window_id(),
            &fixture.target_host.downgrade(),
            binding,
            Some(&stale_frame),
            cx,
        );
    });
    cx.run_until_parked();

    target_visual = VisualTestContext::from_window(fixture.target_window, cx);
    assert!(
        selector_for(
            &target_visual,
            &fixture.target_host,
            DockDebugRegion::DropRoutePreview {
                kind: crate::drop_preview::DockDropRoutePreviewKind::Rejected,
            },
        )
        .is_some(),
        "a delayed G1 frame cleanup must preserve the current G2 frame and preview"
    );
    assert!(
        fixture
            .source_runtime
            .active_payload_drag_session(&fixture.payload)
            .is_some()
    );

    fixture.source_visual.deactivate_window();
    cx.run_until_parked();
}

#[open_gpui::test]
fn runtime_native_captured_foreign_surface_projects_rejection_without_delivery(
    cx: &mut TestAppContext,
) {
    let source_space = DockSpaceId::from("source");
    let mut source_graph = DockGraph::new();
    let source_tabs = source_graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    source_graph.set_root(source_space.clone(), source_tabs);
    let source_workspace = workspace_with_panels(
        cx,
        source_graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
    );
    let source_controller = cx.new(|_| DockController::new(source_workspace));
    let source_runtime = DockViewportRuntimeHandle::new(source_controller.clone());

    let target_space = DockSpaceId::from("target");
    let mut target_graph = DockGraph::new();
    let target_tabs = target_graph.insert_node(DockNode::Tabs {
        items: vec![item("x")],
        selected: Some(item("x")),
    });
    target_graph.set_root(target_space.clone(), target_tabs);
    let target_workspace = workspace_with_panels(cx, target_graph, &[("x", "Panel X", "X")]);
    let target_controller = cx.new(|_| DockController::new(target_workspace));
    let target_runtime = DockViewportRuntimeHandle::new(target_controller.clone());

    let (source_window, source_host, mut source_visual) = open_controller_space_with_runtime(
        cx,
        source_controller.clone(),
        source_runtime.clone(),
        source_space.clone(),
        size(px(360.0), px(220.0)),
    );
    let (target_window, target_host, mut target_visual) = open_controller_space_with_runtime(
        cx,
        target_controller.clone(),
        target_runtime.clone(),
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
    let target_local = debug_bounds(&mut target_visual, &target_tabs_selector).center();
    let target_global_from_source = point(px(400.0) + target_local.x, target_local.y);
    configure_native_registered_window_hit(
        cx,
        source_window.into(),
        target_window.into(),
        target_global_from_source,
    );

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(
        target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    let target_visual = VisualTestContext::from_window(target_window.into(), cx);
    let source_feedback_visual = VisualTestContext::from_window(source_window.into(), cx);
    assert!(
        selector_for(
            &source_feedback_visual,
            &source_host,
            DockDebugRegion::DropRoutePreview {
                kind: crate::drop_preview::DockDropRoutePreviewKind::Rejected,
            },
        )
        .is_some(),
        "the source surface should render rejected feedback from its own current route proof"
    );
    assert!(
        selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::DropRoutePreview {
                kind: crate::drop_preview::DockDropRoutePreviewKind::Rejected,
            },
        )
        .is_some(),
        "the foreign target runtime should render a rejected route marker without receiving raw pointer input"
    );
    assert!(
        selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview).is_none(),
        "a foreign runtime must not construct a payload-bearing target preview"
    );
    let source_status = source_runtime.runtime_status();
    assert!(matches!(
        source_status.last_route.as_ref().map(|route| &route.target),
        Some(crate::DockViewportRouteTarget::Rejected {
            reason: crate::DockViewportRouteRejectionRecord::ForeignSurface,
        })
    ));
    assert_eq!(
        target_runtime.runtime_status().last_route,
        None,
        "the target runtime validates and renders foreign feedback but does not own route diagnostics"
    );

    source_visual.simulate_mouse_up(
        target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let target_visual = VisualTestContext::from_window(target_window.into(), cx);
    assert!(
        selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::DropRoutePreview {
                kind: crate::drop_preview::DockDropRoutePreviewKind::Rejected,
            },
        )
        .is_none(),
        "foreign release should clear the exact rejected marker"
    );
    let source_status = source_runtime.runtime_status();
    assert!(matches!(
        source_status.last_route.as_ref().map(|route| &route.target),
        Some(crate::DockViewportRouteTarget::Rejected {
            reason: crate::DockViewportRouteRejectionRecord::ForeignSurface,
        })
    ));
    assert_eq!(
        source_status
            .last_drop_outcome
            .as_ref()
            .and_then(|outcome| outcome.error.as_ref()),
        Some(&crate::DockActionApplyError::DropTargetUnavailable)
    );
    assert_eq!(source_status.last_activation, None);
    cx.read_entity(&source_controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("a"), item("b")],
            "foreign release must not mutate the source graph or promote to a desktop tear-off"
        );
    });
    cx.read_entity(&target_controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&target_space),
            vec![item("x")],
            "foreign release must not mutate the target graph"
        );
    });
}

#[open_gpui::test]
fn runtime_native_captured_unavailable_release_replaces_foreign_route_diagnostics(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_foreign_preview_fixture(cx);
    assert!(matches!(
        fixture
            .source_runtime
            .runtime_status()
            .last_route
            .as_ref()
            .map(|route| &route.target),
        Some(crate::DockViewportRouteTarget::Rejected {
            reason: crate::DockViewportRouteRejectionRecord::ForeignSurface,
        })
    ));

    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available(
            point(DevicePixels(-1), DevicePixels(-1)),
            Vec::new(),
        )
        .expect("an empty point-scoped hit observation should be valid"),
    );
    fixture.source_visual.simulate_mouse_up(
        fixture.target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    let status = fixture.source_runtime.runtime_status();
    let route = status
        .last_route
        .expect("unavailable release must replace the previous foreign route record");
    assert_eq!(route.target, crate::DockViewportRouteTarget::Unavailable);
    assert_eq!(
        route.unavailable_reason,
        Some(crate::DockViewportReleaseUnavailableRecord::NoViewportRouteSelection)
    );
    assert_eq!(
        status
            .last_drop_outcome
            .as_ref()
            .and_then(|outcome| outcome.error.as_ref()),
        Some(&crate::DockActionApplyError::DropTargetUnavailable)
    );
    assert_eq!(status.last_activation, None);
    assert_eq!(status.last_tear_off, None);
    cx.read_entity(&fixture.source_controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&fixture.source_space),
            vec![item("a"), item("b")]
        );
    });
    let target_visual = VisualTestContext::from_window(fixture.target_window, cx);
    assert!(
        selector_for(
            &target_visual,
            &fixture.target_host,
            DockDebugRegion::DropRoutePreview {
                kind: crate::drop_preview::DockDropRoutePreviewKind::Rejected,
            },
        )
        .is_none(),
        "unavailable release must clear the prior exact foreign marker"
    );
}

#[open_gpui::test]
fn runtime_native_captured_target_close_clears_preview_without_retiring_source_route(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_foreign_preview_fixture(cx);
    let target_visual = VisualTestContext::from_window(fixture.target_window, cx);
    assert!(
        selector_for(
            &target_visual,
            &fixture.target_host,
            DockDebugRegion::DropRoutePreview {
                kind: crate::drop_preview::DockDropRoutePreviewKind::Rejected,
            },
        )
        .is_some()
    );
    let source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    assert!(
        selector_for(
            &source_visual,
            &fixture.source_host,
            DockDebugRegion::DropRoutePreview {
                kind: crate::drop_preview::DockDropRoutePreviewKind::Rejected,
            },
        )
        .is_some(),
        "foreign feedback should be projected back into the source host"
    );
    assert!(
        fixture
            .source_runtime
            .active_payload_drag_session(&fixture.payload)
            .is_some()
    );

    fixture
        .target_window
        .update(cx, |_, window, cx| window.remove_window(cx))
        .expect("the foreign target window should close");
    cx.run_until_parked();

    assert!(fixture.target_window.update(cx, |_, _, _| ()).is_err());
    let source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    assert!(
        selector_for(
            &source_visual,
            &fixture.source_host,
            DockDebugRegion::DropRoutePreview {
                kind: crate::drop_preview::DockDropRoutePreviewKind::Rejected,
            },
        )
        .is_none(),
        "target close must clear the exact source-side foreign feedback projection"
    );
    assert!(
        fixture
            .source_runtime
            .active_payload_drag_session(&fixture.payload)
            .is_some(),
        "closing only the current target must retain the source-owned drag route"
    );
    fixture.source_visual.deactivate_window();
    cx.run_until_parked();
    assert!(
        fixture
            .source_runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none()
    );
    assert!(
        fixture
            .source_visual
            .update(|_, cx| { cx.active_drag_value::<DockDragPayload>().is_none() })
    );
}

#[open_gpui::test]
fn runtime_native_captured_source_close_retires_route_and_foreign_preview(cx: &mut TestAppContext) {
    let fixture = native_captured_foreign_preview_fixture(cx);
    fixture
        .source_window
        .update(cx, |_, window, cx| window.remove_window(cx))
        .expect("the source window should close");
    cx.run_until_parked();

    assert!(fixture.source_window.update(cx, |_, _, _| ()).is_err());
    assert!(
        fixture
            .source_runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none(),
        "source close must retire the exact runtime drag session"
    );
    let target_visual = VisualTestContext::from_window(fixture.target_window, cx);
    assert!(
        selector_for(
            &target_visual,
            &fixture.target_host,
            DockDebugRegion::DropRoutePreview {
                kind: crate::drop_preview::DockDropRoutePreviewKind::Rejected,
            },
        )
        .is_none(),
        "source close must clear the foreign route preview"
    );
    assert!(
        fixture
            .target_window
            .update(cx, |_, _, cx| {
                cx.active_drag_value::<DockDragPayload>().is_none()
            })
            .expect("the independent target window should remain live")
    );
    cx.read_entity(&fixture.source_controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&fixture.source_space),
            vec![item("a"), item("b")]
        );
    });
    cx.read_entity(&fixture.target_controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&fixture.target_space),
            vec![item("x")]
        );
    });
}

#[open_gpui::test]
fn runtime_native_captured_drag_start_panic_rolls_back_before_g2(cx: &mut TestAppContext) {
    let source_space = DockSpaceId::from("source");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    graph.set_root(source_space.clone(), source_tabs);
    let workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);
    let controller = cx.new(|_| DockController::new(workspace));
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let (window, host, mut visual) = open_controller_space_with_runtime(
        cx,
        controller,
        runtime.clone(),
        source_space.clone(),
        size(px(360.0), px(220.0)),
    );
    let source_tab = selector_for(
        &visual,
        &host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("source tab selector should be emitted");
    let start = debug_bounds(&mut visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let payload =
        DockDragPayload::new_item(source_space, source_tabs, item("a"), "Panel A".to_string());
    advertise_native_window_hit_stack(cx);

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.update(|cx| {
        crate::native_captured_drag::panic_next_native_captured_drag_for_test(
            crate::native_captured_drag::DockNativeCapturedDragTestPanic::BeginRouteAfterInstall,
            cx,
        );
    });
    let panic = catch_unwind(AssertUnwindSafe(|| {
        visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    }));
    assert!(panic.is_err());

    assert_eq!(runtime.active_payload_drag_session(&payload), None);
    assert!(cx.read_entity(&host, |host, _| {
        host.payload_drag_anchor_position_from_render(&payload)
            .is_none()
    }));
    assert!(!cx.update(|cx| {
        crate::native_captured_drag::has_active_native_captured_drag_route_for_test(cx)
    }));
    assert!(visual.update(|_, cx| { cx.active_drag_value::<DockDragPayload>().is_none() }));

    cx.run_until_parked();
    visual = VisualTestContext::from_window(window.into(), cx);
    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    assert!(runtime.active_payload_drag_session(&payload).is_some());
    assert!(cx.update(|cx| {
        crate::native_captured_drag::has_active_native_captured_drag_route_for_test(cx)
    }));
    visual.deactivate_window();
    cx.run_until_parked();
    assert_eq!(runtime.active_payload_drag_session(&payload), None);
}

#[open_gpui::test]
fn runtime_native_captured_resolver_panic_retires_g1_and_accepts_g2(cx: &mut TestAppContext) {
    let mut fixture = native_captured_foreign_preview_fixture(cx);
    let session = fixture
        .source_runtime
        .active_payload_drag_session(&fixture.payload)
        .expect("G1 should have an active runtime session");
    let listener_finished_session = Rc::new(Cell::new(false));
    let _interceptor = cx
        .update_window(fixture.source_window, {
            let runtime = fixture.source_runtime.clone();
            let listener_finished_session = listener_finished_session.clone();
            move |_, window, _| {
                window.intercept_window_mouse_events(move |event, _, cx| {
                    if matches!(event, WindowMouseEvent::Up(_))
                        && !listener_finished_session.replace(true)
                    {
                        assert!(runtime.finish_payload_drag_with_app(&session, cx));
                    }
                })
            }
        })
        .expect("the source window should remain open");
    cx.update(|cx| {
        crate::native_captured_drag::panic_next_native_captured_drag_for_test(
            crate::native_captured_drag::DockNativeCapturedDragTestPanic::ResolveTarget,
            cx,
        );
    });

    let panic = catch_unwind(AssertUnwindSafe(|| {
        fixture.source_visual.simulate_mouse_up(
            fixture.target_global_from_source,
            MouseButton::Left,
            Modifiers::none(),
        );
    }));
    assert!(panic.is_err());
    assert!(
        listener_finished_session.get(),
        "the MouseUp listener must invalidate G1 after its release reservation is locked"
    );
    cx.run_until_parked();

    assert!(
        fixture
            .source_runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none(),
        "a panicking G1 resolver must retire its exact runtime session"
    );
    let target_visual = VisualTestContext::from_window(fixture.target_window, cx);
    assert!(
        selector_for(
            &target_visual,
            &fixture.target_host,
            DockDebugRegion::DropRoutePreview {
                kind: crate::drop_preview::DockDropRoutePreviewKind::Rejected,
            },
        )
        .is_none(),
        "panic cleanup must retire the exact G1 foreign preview"
    );

    let mut source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    let source_tab = selector_for(
        &source_visual,
        &fixture.source_host,
        DockDebugRegion::Tab {
            tabs: fixture.source_tabs,
            item: item("a"),
        },
    )
    .expect("the source tab should remain available for G2");
    let start = debug_bounds(&mut source_visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(
        fixture.target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    let target_visual = VisualTestContext::from_window(fixture.target_window, cx);
    assert!(
        selector_for(
            &target_visual,
            &fixture.target_host,
            DockDebugRegion::DropRoutePreview {
                kind: crate::drop_preview::DockDropRoutePreviewKind::Rejected,
            },
        )
        .is_some(),
        "the persistent native captured-drag consumer must accept G2 after G1 panics"
    );
    source_visual.deactivate_window();
    cx.run_until_parked();
}

#[open_gpui::test]
fn runtime_native_captured_desktop_release_tears_off_tab(cx: &mut TestAppContext) {
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
    configure_native_desktop_release(
        cx,
        opened.window().into(),
        size(DevicePixels(720), DevicePixels(440)),
    );
    let source_registration = runtime
        .borrow()
        .adapter()
        .registration_key(&source_space)
        .expect("source viewport should have an exact registration before release");

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(outside_window, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let desktop_feedback_visual = VisualTestContext::from_window(opened.window(), cx);
    assert!(
        selector_for(
            &desktop_feedback_visual,
            &source_host,
            DockDebugRegion::DropRoutePreview {
                kind: crate::drop_preview::DockDropRoutePreviewKind::TearOff,
            },
        )
        .is_some(),
        "desktop movement should project tear-off feedback into the exact source host"
    );
    assert!(
        selector_for(
            &desktop_feedback_visual,
            &source_host,
            DockDebugRegion::DropPreview,
        )
        .is_none(),
        "desktop feedback must not forge a target-host payload overlay"
    );
    visual.simulate_mouse_up(outside_window, MouseButton::Left, Modifiers::none());
    {
        let runtime = runtime.borrow();
        assert_eq!(
            runtime.adapter().registration_key(&source_space),
            Some(source_registration.clone()),
            "source-only release must retain the exact source registration"
        );
        assert!(
            runtime
                .adapter()
                .snapshot(&source_space)
                .is_some_and(|snapshot| snapshot.is_route_ready()),
            "source-only release must not resample its currently borrowed source window as unavailable"
        );
    }
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
            .expect("captured desktop release should create a detached viewport space");
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
    configure_native_desktop_release(
        cx,
        opened.window().into(),
        size(DevicePixels(1280), DevicePixels(840)),
    );

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
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
            .expect("captured desktop release should create a detached viewport space");
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            vec![item("bottom")]
        );
        detached_space
    });
    let detached_window = runtime
        .borrow()
        .adapter()
        .window_for_space(&detached_space)
        .expect("detached space should have a runtime window");
    let detached_bounds = detached_window
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
    configure_native_desktop_release(
        cx,
        opened.window().into(),
        size(DevicePixels(720), DevicePixels(440)),
    );

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_up(outside_window, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    let (detached_space, detached_tabs) = cx.read_entity(&controller, |controller, _| {
        let detached_space = controller
            .graph()
            .spaces()
            .into_iter()
            .find(|space| space.as_str().starts_with("source:tear-off:a:"))
            .expect("captured desktop release should create a detached viewport space");
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
    let detached_window: AnyWindowHandle = detached_window.into();
    let mut detached_visual = VisualTestContext::from_window(detached_window, cx);

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
    let target_global_from_detached = point(px(400.0) + end.x, end.y);
    configure_native_registered_window_hit(
        cx,
        detached_window,
        opened.window(),
        target_global_from_detached,
    );

    activate_window_for_pointer_input(&mut detached_visual);
    detached_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    detached_visual.simulate_mouse_move(
        point(start.x + px(24.0), start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    detached_visual.simulate_mouse_move(
        target_global_from_detached,
        MouseButton::Left,
        Modifiers::none(),
    );
    detached_visual.simulate_mouse_up(
        target_global_from_detached,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

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
fn runtime_secondary_single_tab_native_release_creates_detached_viewport(cx: &mut TestAppContext) {
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
    configure_native_desktop_release(
        cx,
        secondary_any_window,
        size(DevicePixels(720), DevicePixels(440)),
    );

    activate_window_for_pointer_input(&mut visual);
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
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
            .expect("captured desktop release should create a detached viewport space");
        assert_eq!(
            controller.graph().collect_items_in_space(&detached_space),
            vec![item("b")]
        );
        assert_eq!(
            runtime.registered_viewport_spaces(),
            vec![primary_space.clone(), detached_space.clone()],
            "captured desktop release should detach the tab and vacate the empty source viewport"
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
}

#[open_gpui::test]
fn runtime_native_captured_desktop_release_rejects_when_platform_viewports_disabled(
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
    configure_native_desktop_release(
        cx,
        opened.window().into(),
        size(DevicePixels(720), DevicePixels(440)),
    );

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
        "rejected captured release should clear the drop preview"
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
        "a non-runtime source release should leave the source panel active"
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
