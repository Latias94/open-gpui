use crate::{
    DockCentralRegion, DockController, DockFloatingContainer, DockGraph, DockGraphDropTarget,
    DockHost, DockItemId, DockNode, DockNodeId, DockOp, DockPanel, DockPanelDescriptor,
    DockPanelPlacement, DockSpaceId, DockSurface, DockSurfacePrimaryWindowOpenOutcome,
    DockSurfaceWindowSessionShutdownReason, DockViewportRuntimeHandle, DockWorkspace, DropZone,
    SplitAxis,
    debug::DockDebugRegion,
    divider_hit_map::{DockDividerHitMap, DockDividerHitTarget},
    drag::DockDragPayload,
    drop_scene_fact,
    drop_target::{DockDropResolveSource, DockResolvedDropTargetKind},
    host::DockHostWindowBinding,
    host_test_support::*,
    interaction::DockPayloadDropRelease,
    locked_drop_identity::DockLockedPayloadIdentity,
    surface::{
        DockSurfaceOwner,
        live_undock::{
            DockLiveUndockDragGeneration, DockLiveUndockEffect, DockLiveUndockFact,
            DockLiveUndockPhysicalBounds, DockLiveUndockPhysicalPoint,
            DockLiveUndockPromotionDestination, DockLiveUndockPromotionToken,
            DockLiveUndockRouteFeedback, DockLiveUndockRouteGeneration, DockLiveUndockSession,
            DockLiveUndockSourceSnapshot, DockLiveUndockTrigger,
        },
        payload_recovery::{
            DockPayloadRecoveryAuthority, DockPayloadRecoveryPresentationOrigin,
            DockPayloadRecoveryReason, DockPayloadRecoveryRestoreAction,
            DockPayloadRecoveryRestoreError,
        },
        with_root_transaction,
    },
    transition_geometry::DockVisualAffordanceTransitionKind,
    workspace_drop_transaction::DockWorkspaceDropPayload,
};
use open_gpui::{
    AnyView, AnyWindowHandle, AppContext as _, Bounds, Context, DevicePixels, Entity, Focusable,
    InteractiveElement, IntoElement, Modifiers, MouseButton, NativeCapturedDragReleaseTerminal,
    ParentElement, Pixels, PlatformNativeDragHysteresis, PlatformPointerCaptureReleaseOutcome,
    PlatformWindowDispatch, PlatformWindowHit, PlatformWindowHitStack,
    PlatformWindowPhysicalCoverage, PlatformWindowPhysicalGeometry, Point, PointerCancelReason,
    PointerCaptureHandle, Render, Size, Styled, Subscription, SubtreeTransform,
    SubtreeTransformExt, SubtreeTransformOrigin, TestAppContext, VisualTestContext, Window,
    WindowHandle, WindowMouseEvent, WindowMutationDomain, canvas, div, point, px, size,
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

struct ConditionalOccludedDockHostFixture {
    host: Entity<DockHost>,
    occluded: bool,
    occluded_frame_rendered: Rc<Cell<bool>>,
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

impl Render for ConditionalOccludedDockHostFixture {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut content = div()
            .relative()
            .size_full()
            .child(AnyView::from(self.host.clone()));
        if self.occluded {
            self.occluded_frame_rendered.set(true);
            content = content.child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .size_full()
                    .occlude(),
            );
        }
        content
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
    advertise_native_drag_hysteresis(cx);
    let source_bounds = Bounds::new(point(DevicePixels(0), DevicePixels(0)), source_size);
    cx.set_platform_window_physical_client_geometry(source_window, Some(source_bounds), 2.0);
    let sampled_point = point(DevicePixels(1800), DevicePixels(1800));
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available_open_desktop(sampled_point, Vec::new())
            .expect("desktop release observation should be valid"),
    );
}

fn advertise_native_window_hit_stack(cx: &TestAppContext) {
    advertise_native_drag_hysteresis(cx);
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available_open_desktop(
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
    advertise_native_drag_hysteresis(cx);
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

fn configure_native_source_window_hit(
    cx: &TestAppContext,
    source_window: AnyWindowHandle,
    source_point: Point<Pixels>,
) {
    advertise_native_drag_hysteresis(cx);
    let source_bounds = Bounds::new(
        point(DevicePixels(0), DevicePixels(0)),
        size(DevicePixels(720), DevicePixels(440)),
    );
    cx.set_platform_window_physical_client_geometry(source_window, Some(source_bounds), 2.0);
    let sampled_point = point(
        DevicePixels((source_point.x.as_f32() * 2.0).round() as i32),
        DevicePixels((source_point.y.as_f32() * 2.0).round() as i32),
    );
    let coverage = PlatformWindowPhysicalCoverage::try_new(source_bounds)
        .expect("source coverage should be representable");
    let geometry = PlatformWindowPhysicalGeometry::try_new(source_bounds, 2.0)
        .expect("source physical geometry should be representable");
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available(
            sampled_point,
            vec![PlatformWindowHit::RegisteredApplication {
                window: source_window,
                coverage,
                geometry,
            }],
        )
        .expect("registered source hit observation should be valid"),
    );
}

fn advertise_native_drag_hysteresis(cx: &TestAppContext) {
    cx.set_platform_native_drag_hysteresis(Some(
        PlatformNativeDragHysteresis::try_new(DevicePixels(4), DevicePixels(4))
            .expect("test native drag hysteresis must be positive"),
    ));
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
    pointer_cancellations: Rc<RefCell<Vec<NativeSourcePointerCancellation>>>,
    _pointer_event_interceptor: Subscription,
}

#[derive(Clone, Copy, Debug)]
struct NativeSourcePointerCancellation {
    reason: PointerCancelReason,
    live_presentation: bool,
    semantic_proxy: bool,
    transport_proxy: bool,
    execution_count: usize,
}

impl NativeSourcePointerCancellation {
    fn describe(&self) -> String {
        format!(
            "reason={:?}, live_presentation={}, semantic_proxy={}, transport_proxy={}, execution_count={}",
            self.reason,
            self.live_presentation,
            self.semantic_proxy,
            self.transport_proxy,
            self.execution_count,
        )
    }
}

fn assert_native_source_transport_capture(
    fixture: &NativeCapturedSourceFixture,
    expected: Option<PointerCaptureHandle>,
    cx: &mut TestAppContext,
) -> PointerCaptureHandle {
    let capture = fixture
        .source_window
        .update(cx, |_, window, _| window.captured_pointer())
        .expect("the source window should remain available")
        .expect("the source window should retain pointer capture");
    let handle = capture.handle();
    if let Some(expected) = expected {
        assert_eq!(
            handle, expected,
            "the source redraw must preserve the exact pointer-capture owner",
        );
    }
    let proxy = cx
        .read_entity(&fixture.source_host, |host, _| {
            host.native_drag_transport_proxy()
        })
        .expect("an active native drag must retain its transport proxy");
    assert_eq!(
        proxy.pointer_capture(),
        handle,
        "the transport proxy must bind the source's exact capture handle",
    );

    let cancellations = fixture.pointer_cancellations.borrow();
    if let Some(cancellation) = cancellations.first() {
        panic!(
            "the source capture was cancelled while its transport proxy remained responsible: {}; cancellation_count={}",
            cancellation.describe(),
            cancellations.len(),
        );
    }
    handle
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
    let pointer_cancellations = Rc::new(RefCell::new(Vec::new()));
    let pointer_event_interceptor = cx
        .update_window(source_window, {
            let pointer_cancellations = pointer_cancellations.clone();
            let source_host = source_host.downgrade();
            let owner = surface.owner().downgrade();
            move |_, window, _| {
                window.intercept_window_mouse_events(move |event, _, cx| {
                    let WindowMouseEvent::Cancel(event) = event else {
                        return;
                    };
                    let (live_presentation, semantic_proxy, transport_proxy) = source_host
                        .read_with(cx, |host, _| {
                            (
                                host.live_presentation_state().is_some(),
                                host.live_source_semantic_proxy().is_some(),
                                host.native_drag_transport_proxy().is_some(),
                            )
                        })
                        .unwrap_or_default();
                    let execution_count = owner
                        .read_with(cx, |owner, _| {
                            owner.live_undock_runtime().execution_count_for_test()
                        })
                        .unwrap_or_default();
                    pointer_cancellations
                        .borrow_mut()
                        .push(NativeSourcePointerCancellation {
                            reason: event.reason,
                            live_presentation,
                            semantic_proxy,
                            transport_proxy,
                            execution_count,
                        });
                })
            }
        })
        .expect("the source should install its pointer cancellation probe");
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
        pointer_cancellations,
        _pointer_event_interceptor: pointer_event_interceptor,
    }
}

fn begin_native_live_undock_with_released_source(
    fixture: &mut NativeCapturedSourceFixture,
    cx: &mut TestAppContext,
) {
    configure_native_desktop_release(
        cx,
        fixture.source_window,
        size(DevicePixels(720), DevicePixels(440)),
    );
    fixture.begin_drag(cx);
    fixture.source_visual.simulate_mouse_move(
        point(px(900.0), px(900.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let drained_without_advancing_time = (0..10_000).any(|_| !cx.background_executor.tick());
    assert!(
        drained_without_advancing_time,
        "the live-undock opening must quiesce without advancing its release deadline"
    );

    let (phase, execution_count) = cx.read_entity(fixture.surface.owner(), |owner, _| {
        (
            owner.live_undock_phase(),
            owner.live_undock_runtime().execution_count_for_test(),
        )
    });
    let route_facts =
        cx.read(|app| crate::native_captured_drag::active_live_undock_route_facts_for_test(app));
    let pointer_capture = fixture
        .source_window
        .update(cx, |_, window, _| window.captured_pointer())
        .ok()
        .flatten();
    assert_eq!(
        phase,
        crate::surface::live_undock::DockLiveUndockPhase::Bound,
        "the desktop move should bind one live-undock generation; execution_count={execution_count}, drag_session_active={}, route_facts={route_facts:?}, pointer_capture={pointer_capture:?}, cancellations={:?}",
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_some(),
        fixture.pointer_cancellations.borrow().as_slice(),
    );
    assert!(
        cx.read_entity(&fixture.source_host, |host, _| {
            matches!(
                host.live_presentation_state().map(|state| state.mode),
                Some(
                    crate::host::DockHostLivePresentationMode::SourceProjection {
                        phase: crate::host::DockHostLiveSourcePhase::Frozen
                            | crate::host::DockHostLiveSourcePhase::Retired,
                        ..
                    }
                )
            )
        }),
        "the source must publish its real release proxy before restoration is tested",
    );
    assert_native_source_transport_capture(
        fixture,
        pointer_capture.map(|capture| capture.handle()),
        cx,
    );
}

#[open_gpui::test]
fn live_source_projection_republishes_accepted_route_scene(cx: &mut TestAppContext) {
    let mut fixture = native_captured_source_fixture(cx);
    let source_window = fixture.source_window.window_id();
    let source_space = cx.read_entity(&fixture.source_host, |host, _| host.space().clone());
    let initial = fixture
        .runtime
        .runtime_status()
        .viewport_lifecycle
        .into_iter()
        .find(|record| record.space == source_space && record.window_id == source_window)
        .expect("the source viewport should publish initial route facts");
    assert_eq!(
        initial.route_status,
        crate::DockViewportRouteStatus::RouteReady
    );

    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture
        .source_visual
        .update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    let projected = fixture
        .runtime
        .runtime_status()
        .viewport_lifecycle
        .into_iter()
        .find(|record| record.space == source_space && record.window_id == source_window)
        .expect("the live source projection should retain its viewport registration");
    assert_eq!(
        projected.route_status,
        crate::DockViewportRouteStatus::RouteReady,
        "a live source projection must publish an accepted route scene instead of discarding the source route"
    );
    assert!(
        projected.facts_generation > initial.facts_generation,
        "the live source projection must replace the prior normal-host scene with a fresh generation"
    );
    assert!(
        cx.read_entity(&fixture.source_host, |host, _| host
            .interaction()
            .viewport_host_scene_frame()
            .is_some()),
        "the fresh source projection route must retain accepted-frame proof"
    );
}

#[open_gpui::test]
fn live_undock_release_back_to_source_host_restores_without_committing_unchanged_promotion(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_source_fixture(cx);
    let revision = cx.read(|app| fixture.surface.revision(app));
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);

    configure_native_source_window_hit(cx, fixture.source_window, fixture.start);
    fixture
        .source_visual
        .simulate_mouse_move(fixture.start, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let routed = fixture.runtime.runtime_status();
    assert!(matches!(
        routed.last_route.as_ref().map(|route| &route.target),
        Some(crate::DockViewportRouteTarget::Local {
            space,
            window_id,
            ..
        }) if space == &DockSpaceId::from("main")
            && *window_id == fixture.source_window.window_id()
    ));
    assert_eq!(
        routed
            .last_route
            .as_ref()
            .and_then(|route| route.selection_source),
        Some(crate::DockViewportRouteSelectionRecord::CapturedNativeHitStack),
        "the source no-op must be classified from exact captured-native route proof",
    );
    fixture
        .source_visual
        .simulate_mouse_up(fixture.start, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    cx.read_entity(fixture.surface.owner(), |owner, _| {
        assert_eq!(
            owner.live_undock_phase(),
            crate::surface::live_undock::DockLiveUndockPhase::Idle,
            "an unchanged source-host release must restore instead of crossing the promotion boundary",
        );
        assert_eq!(
            owner.live_undock_runtime().execution_count_for_test(),
            0,
            "source restoration must retire the rejected promotion execution",
        );
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::LostViewportRecovery,
            ),
            0,
            "a reversible no-op must restore the source without entering committed recovery",
        );
    });
    assert_eq!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload),
        None,
        "source restoration must release the exact payload drag session",
    );
    assert_eq!(
        cx.read(|app| fixture.surface.revision(app)),
        revision,
        "an unchanged source-host release must not publish a surface transaction",
    );
    assert_eq!(
        fixture.runtime.runtime_status().last_activation,
        None,
        "the no-op route must not publish its otherwise-valid focus activation",
    );
    assert_eq!(
        cx.windows(),
        vec![fixture.source_window],
        "the rejected promotion must not retain a provisional destination window",
    );
    cx.read_entity(&fixture.controller, |controller, _| {
        assert_eq!(
            controller.workspace().locked_payload_drop_commit_count(),
            0,
            "the no-op promotion must be rejected before the workspace commit ledger",
        );
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&DockSpaceId::from("main")),
            vec![item("a"), item("b")],
        );
    });
    cx.read_entity(&fixture.source_host, |host, _| {
        assert!(host.live_presentation_state().is_none());
        assert!(host.live_source_semantic_proxy().is_none());
        assert!(host.native_drag_transport_proxy().is_none());
    });
}

#[open_gpui::test]
fn revealed_live_destination_keeps_inert_source_route_ready(cx: &mut TestAppContext) {
    let mut fixture = native_captured_source_fixture(cx);
    let source_window = fixture.source_window.window_id();
    let source_space = cx.read_entity(&fixture.source_host, |host, _| host.space().clone());

    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    let _ = reveal_live_undock_provisional_destination(&fixture, cx);

    fixture
        .source_visual
        .update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    let projected = fixture
        .runtime
        .runtime_status()
        .viewport_lifecycle
        .into_iter()
        .find(|record| record.space == source_space && record.window_id == source_window)
        .expect("the revealed live source must retain its viewport registration");
    assert_eq!(
        projected.route_status,
        crate::DockViewportRouteStatus::RouteReady,
        "an inert source projection must retain non-interactive route geometry after destination reveal"
    );
    assert!(
        cx.read_entity(&fixture.source_host, |host, _| host
            .interaction()
            .viewport_host_scene_frame()
            .is_some()),
        "the inert source projection must retain accepted-frame route proof"
    );
}

#[open_gpui::test]
fn reveal_observing_advances_the_exact_destination_frame_on_cached_refresh(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_source_fixture(cx);
    cx.defer_next_window_frame_requests();
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);

    let destination_window = cx
        .windows()
        .into_iter()
        .find(|window| window.window_id() != fixture.source_window.window_id())
        .expect("live undock should retain one provisional destination window");
    let destination_host = destination_window
        .downcast::<DockHost>()
        .expect("the provisional destination should retain a DockHost root")
        .entity(cx)
        .expect("the provisional destination DockHost should remain live");
    let (key, preflight) = cx.read_entity(&destination_host, |host, _| {
        let state = host
            .live_presentation_state()
            .expect("the provisional destination must retain reveal authority");
        let crate::host::DockHostLivePresentationMode::DestinationProjection {
            phase: crate::host::DockHostLiveDestinationPhase::RevealObserving { presentation, .. },
            ..
        } = state.mode
        else {
            panic!("the destination must already be observing its exact native reveal");
        };
        (state.key, presentation)
    });
    let runtime = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner.live_undock_runtime()
    });
    let current_frame = |cx: &TestAppContext| {
        cx.read(|app| runtime.current_destination_presentation(key, preflight.mount(), app))
            .and_then(Result::ok)
            .expect("the destination must retain one accepted presentation")
            .frame_generation()
    };
    let first = current_frame(cx);
    let mut destination_visual = VisualTestContext::from_window(destination_window, cx);

    destination_visual.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear();
    });
    let second = current_frame(cx);
    destination_visual.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear();
    });
    let third = current_frame(cx);

    assert!(
        second > first,
        "the first cached reveal-observing refresh must advance the exact destination frame"
    );
    assert!(
        third > second,
        "every accepted reveal-observing frame must remain observable until native submission binds one generation"
    );
}

fn reveal_live_undock_provisional_destination(
    fixture: &NativeCapturedSourceFixture,
    cx: &mut TestAppContext,
) -> (AnyWindowHandle, Entity<DockHost>, DockSpaceId) {
    cx.run_until_parked();
    let destination_window = cx
        .windows()
        .into_iter()
        .find(|window| window.window_id() != fixture.source_window.window_id())
        .expect("live undock should retain one exact provisional destination window");
    let initial_facts = destination_window
        .update(cx, |_, window, _| window.presentation_facts())
        .expect("the provisional destination window should remain live");
    if !initial_facts.native_visible {
        assert!(
            cx.flush_window_mutation(destination_window, WindowMutationDomain::Placement),
            "the TestPlatform must settle the hidden target-display placement before reveal"
        );
    }
    let destination_host = destination_window
        .downcast::<DockHost>()
        .expect("the provisional destination should retain a DockHost root")
        .entity(cx)
        .expect("the provisional destination DockHost should remain live");
    let destination_space = cx.read_entity(&destination_host, |host, _| {
        assert!(
            host.is_provisional_viewport(),
            "the pre-release destination must still be provisional"
        );
        assert!(
            host.current_viewport_registration().is_none(),
            "a provisional destination must not publish a durable registration"
        );
        host.space().clone()
    });
    let facts = destination_window
        .update(cx, |_, window, _| window.presentation_facts())
        .expect("the provisional destination window should remain live");
    assert!(
        facts.native_visible,
        "the provisional destination must be visible before release"
    );
    assert!(
        facts.non_empty_presented_generation.is_some(),
        "the provisional destination must present non-empty content before release"
    );

    let reveal_already_settled = cx.read_entity(&destination_host, |host, cx| {
        let state = host
            .live_presentation_state()
            .expect("the provisional destination should retain live presentation authority");
        match state.mode {
            crate::host::DockHostLivePresentationMode::DestinationProjection {
                leases,
                phase: crate::host::DockHostLiveDestinationPhase::RevealSettled,
                ..
            } => {
                assert!(
                    open_gpui::view_presentation_window::presented_batch_receipt(cx, &leases)
                        .is_some(),
                    "a settled reveal must retain accepted-frame presentation proof"
                );
                true
            }
            crate::host::DockHostLivePresentationMode::DestinationProjection {
                phase: crate::host::DockHostLiveDestinationPhase::RevealObserving { .. },
                ..
            } => false,
            mode => panic!(
                "the provisional destination must be observing or have settled one exact reveal; mode={mode:?}"
            ),
        }
    });
    if reveal_already_settled {
        assert!(
            cx.read_entity(&fixture.source_host, |host, _| host
                .live_source_semantic_proxy()
                .is_some()),
            "a settled destination reveal must retain the source semantic proxy"
        );
        return (destination_window, destination_host, destination_space);
    }

    let mut destination_visual = VisualTestContext::from_window(destination_window, cx);
    let (
        reveal_key,
        reveal_ticket,
        initial_ticket_snapshot,
        reveal_candidate,
        reveal_preflight,
        initially_submitted_frame,
    ) = cx.read_entity(&destination_host, |host, _| {
        let state = host
            .live_presentation_state()
            .expect("the provisional destination should retain live presentation authority");
        let crate::host::DockHostLivePresentationMode::DestinationProjection {
            phase:
                crate::host::DockHostLiveDestinationPhase::RevealObserving {
                    presentation,
                    candidate_frame,
                    submitted_frame,
                    ticket,
                },
            ..
        } = state.mode
        else {
            panic!("the provisional destination must retain one exact reveal observation");
        };
        (
            state.key,
            ticket.clone(),
            ticket.snapshot(),
            candidate_frame,
            presentation,
            submitted_frame,
        )
    });
    assert!(
        initially_submitted_frame.is_none(),
        "accepted candidate frames must not publish submitted reveal authority"
    );
    assert_eq!(
        initial_ticket_snapshot.minimum_presentation_generation(),
        reveal_candidate.frame_generation(),
        "the reveal observer must retain the ticket's first eligible accepted generation"
    );
    let runtime = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner.live_undock_runtime()
    });
    let current_candidate = cx
        .read(|app| {
            runtime.current_destination_presentation(reveal_key, reveal_preflight.mount(), app)
        })
        .and_then(Result::ok)
        .expect("the live destination should retain one current candidate frame");
    assert_eq!(
        current_candidate.mount(),
        reveal_candidate.mount(),
        "candidate drift must remain inside one exact destination mount"
    );
    assert!(
        current_candidate.frame_generation() > reveal_candidate.frame_generation(),
        "the test must retain a later accepted candidate before the reveal ticket is submitted"
    );
    assert!(
        cx.update_entity(&destination_host, |host, cx| {
            host.begin_live_destination_reveal_observation(
                reveal_key,
                reveal_preflight,
                reveal_candidate,
                cx,
            )
        })
        .is_none(),
        "one reveal authority must never admit a second observer"
    );
    assert!(
        !cx.update_entity(&destination_host, |host, cx| {
            host.settle_live_destination_reveal(
                reveal_key,
                reveal_preflight,
                Some(reveal_candidate),
                cx,
            )
        }),
        "an accepted candidate cannot settle reveal before ticket submission binds it"
    );
    let drained =
        destination_visual.update(|window, cx| window.drain_next_frame_callbacks_for_test(cx));
    assert_ne!(
        drained, 0,
        "the exact reveal observer must retain a next-frame wakeup"
    );
    cx.run_until_parked();

    let submitted_ticket_snapshot = reveal_ticket.snapshot();
    let submitted_generation = submitted_ticket_snapshot
        .presentation_generation()
        .expect("the exact reveal ticket must bind one platform-submitted generation");
    assert!(
        submitted_generation > reveal_candidate.frame_generation(),
        "a deferred accepted candidate must not be mistaken for the later submitted reveal frame"
    );
    assert_eq!(
        submitted_generation,
        current_candidate.frame_generation(),
        "the reveal ticket must bind the later accepted frame that was actually submitted"
    );
    let final_batch_receipt = cx
        .read_entity(&destination_host, |host, cx| {
            let state = host
                .live_presentation_state()
                .expect("the revealed destination must retain its presentation state");
            let crate::host::DockHostLivePresentationMode::DestinationProjection {
                leases,
                phase: crate::host::DockHostLiveDestinationPhase::RevealSettled,
                ..
            } = state.mode
            else {
                panic!("the exact submitted reveal must settle destination authority");
            };
            open_gpui::view_presentation_window::presented_batch_receipt(cx, &leases)
        })
        .expect("the settled destination must retain its submitted batch receipt");
    assert!(
        final_batch_receipt.frame_generation() >= submitted_generation,
        "later accepted frames may advance the batch receipt, but never precede the submitted reveal"
    );

    let destination_revealed = cx.read_entity(&fixture.source_host, |host, _| {
        matches!(
            host.live_presentation_state().map(|state| state.mode),
            Some(
                crate::host::DockHostLivePresentationMode::SourceProjection {
                    phase: crate::host::DockHostLiveSourcePhase::Retired,
                    ..
                }
            )
        )
    });
    let destination_presentation =
        cx.read_entity(&destination_host, |host, _| host.live_presentation_state());
    let destination_window_facts = destination_window
        .update(cx, |_, window, _| window.presentation_facts())
        .ok();
    let owner_phase = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner.live_undock_phase()
    });
    assert!(
        destination_revealed,
        "the provisional destination must produce one exact native reveal; initial_ticket={initial_ticket_snapshot:?}, observer_candidate={reveal_candidate:?}, later_candidate={current_candidate:?}, submitted_ticket={submitted_ticket_snapshot:?}, final_batch={final_batch_receipt:?}, host={destination_presentation:?}, window={destination_window_facts:?}, owner={owner_phase:?}, windows={:?}",
        cx.windows(),
    );
    assert!(cx.read_entity(&fixture.source_host, |host, _| {
        host.live_source_semantic_proxy().is_some()
    }));

    (destination_window, destination_host, destination_space)
}

#[open_gpui::test]
fn pending_reveal_observation_expires_without_next_frame_progress(cx: &mut TestAppContext) {
    let mut fixture = native_captured_source_fixture(cx);
    cx.defer_next_window_frame_requests();
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    cx.run_until_parked();

    let destination_window = cx
        .windows()
        .into_iter()
        .find(|window| window.window_id() != fixture.source_window.window_id())
        .expect("live undock should retain one pending provisional destination window");
    let destination_host = destination_window
        .downcast::<DockHost>()
        .expect("the provisional destination should retain a DockHost root")
        .entity(cx)
        .expect("the provisional destination DockHost should remain live");
    let reveal_ticket = cx.read_entity(&destination_host, |host, _| {
        let state = host
            .live_presentation_state()
            .expect("the pending destination must retain reveal authority");
        let crate::host::DockHostLivePresentationMode::DestinationProjection {
            phase:
                crate::host::DockHostLiveDestinationPhase::RevealObserving {
                    submitted_frame: None,
                    ticket,
                    ..
                },
            ..
        } = state.mode
        else {
            panic!("the destination must await one exact reveal observation");
        };
        ticket
    });
    cx.executor().advance_clock(
        crate::render::LIVE_UNDOCK_REVEAL_OBSERVATION_DEADLINE - Duration::from_millis(1),
    );
    cx.run_until_parked();
    assert_eq!(
        reveal_ticket.snapshot().outcome(),
        open_gpui::WindowProvisionalRevealOutcome::Pending
    );
    assert!(matches!(
        cx.read_entity(&destination_host, |host, _| host
            .live_presentation_state()
            .map(|state| state.mode)),
        Some(
            crate::host::DockHostLivePresentationMode::DestinationProjection {
                phase: crate::host::DockHostLiveDestinationPhase::RevealObserving { .. },
                ..
            }
        )
    ));

    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();

    let settled_state = cx.read_entity(&destination_host, |host, _| host.live_presentation_state());
    assert_eq!(
        reveal_ticket.snapshot().outcome(),
        open_gpui::WindowProvisionalRevealOutcome::Cancelled,
        "the deadline must win the exact GPUI reveal ticket before Dock publishes failure"
    );
    assert!(
        !matches!(
            settled_state.map(|state| state.mode),
            Some(
                crate::host::DockHostLivePresentationMode::DestinationProjection {
                    phase: crate::host::DockHostLiveDestinationPhase::RevealObserving { .. },
                    ..
                }
            )
        ),
        "the reveal deadline must retire the exact observer instead of refreshing forever"
    );

    assert!(
        !cx.windows().contains(&destination_window),
        "the cancelled provisional destination must leave the logical window registry"
    );
}

#[open_gpui::test]
fn native_reveal_winner_is_joined_by_the_deadline_before_the_next_observer_frame(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_source_fixture(cx);
    cx.defer_next_window_frame_requests();
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);

    let destination_window = cx
        .windows()
        .into_iter()
        .find(|window| window.window_id() != fixture.source_window.window_id())
        .expect("live undock should retain one pending provisional destination window");
    let destination_host = destination_window
        .downcast::<DockHost>()
        .expect("the provisional destination should retain a DockHost root")
        .entity(cx)
        .expect("the provisional destination DockHost should remain live");
    let reveal_ticket = cx.read_entity(&destination_host, |host, _| {
        let state = host
            .live_presentation_state()
            .expect("the pending destination must retain reveal authority");
        let crate::host::DockHostLivePresentationMode::DestinationProjection {
            phase:
                crate::host::DockHostLiveDestinationPhase::RevealObserving {
                    submitted_frame: None,
                    ticket,
                    ..
                },
            ..
        } = state.mode
        else {
            panic!("the destination must await one exact reveal observation");
        };
        ticket
    });
    assert!(cx.flush_window_mutation(destination_window, WindowMutationDomain::Placement));
    assert!(cx.step_deferred_window_frame_request(destination_window));
    assert_eq!(
        reveal_ticket.snapshot().outcome(),
        open_gpui::WindowProvisionalRevealOutcome::Revealed,
        "the native reveal must win before the held observer frame"
    );
    assert!(matches!(
        cx.read_entity(&destination_host, |host, _| host
            .live_presentation_state()
            .map(|state| state.mode)),
        Some(
            crate::host::DockHostLivePresentationMode::DestinationProjection {
                phase: crate::host::DockHostLiveDestinationPhase::RevealObserving {
                    submitted_frame: None,
                    ..
                },
                ..
            }
        )
    ));

    cx.executor()
        .advance_clock(crate::render::LIVE_UNDOCK_REVEAL_OBSERVATION_DEADLINE);
    cx.run_until_parked();

    assert!(matches!(
        cx.read_entity(&destination_host, |host, _| host
            .live_presentation_state()
            .map(|state| state.mode)),
        Some(
            crate::host::DockHostLivePresentationMode::DestinationProjection {
                phase: crate::host::DockHostLiveDestinationPhase::RevealSettled,
                ..
            }
        )
    ));
    let facts = destination_window
        .update(cx, |_, window, _| window.presentation_facts())
        .expect("the native reveal winner should keep its destination window live");
    assert!(facts.native_visible);
    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .live_undock_phase()),
        crate::surface::live_undock::DockLiveUndockPhase::Bound
    );
}

#[open_gpui::test]
fn native_reveal_winner_requests_frames_until_the_exact_submission_is_observed(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_source_fixture(cx);
    cx.defer_next_window_frame_requests();
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);

    let destination_window = cx
        .windows()
        .into_iter()
        .find(|window| window.window_id() != fixture.source_window.window_id())
        .expect("live undock should retain one pending provisional destination window");
    let destination_host = destination_window
        .downcast::<DockHost>()
        .expect("the provisional destination should retain a DockHost root")
        .entity(cx)
        .expect("the provisional destination DockHost should remain live");

    assert!(cx.flush_window_mutation(destination_window, WindowMutationDomain::Placement));
    assert!(cx.step_deferred_window_frame_request(destination_window));
    assert!(matches!(
        cx.read_entity(&destination_host, |host, _| host
            .live_presentation_state()
            .map(|state| state.mode)),
        Some(
            crate::host::DockHostLivePresentationMode::DestinationProjection {
                phase: crate::host::DockHostLiveDestinationPhase::RevealObserving { .. },
                ..
            }
        )
    ));

    let mut requested_followup = false;
    for _ in 0..4 {
        requested_followup |= cx.step_deferred_window_frame_request(destination_window);
        if matches!(
            cx.read_entity(&destination_host, |host, _| host
                .live_presentation_state()
                .map(|state| state.mode)),
            Some(
                crate::host::DockHostLivePresentationMode::DestinationProjection {
                    phase: crate::host::DockHostLiveDestinationPhase::RevealSettled,
                    ..
                }
            )
        ) {
            break;
        }
    }

    assert!(
        requested_followup,
        "an observer running before the exact native generation is accepted must request another platform frame"
    );
    assert!(matches!(
        cx.read_entity(&destination_host, |host, _| host
            .live_presentation_state()
            .map(|state| state.mode)),
        Some(
            crate::host::DockHostLivePresentationMode::DestinationProjection {
                phase: crate::host::DockHostLiveDestinationPhase::RevealSettled,
                ..
            }
        )
    ));
}

#[open_gpui::test]
fn logical_destination_close_settles_reveal_before_native_terminal(cx: &mut TestAppContext) {
    let mut fixture = native_captured_source_fixture(cx);
    cx.defer_next_window_frame_requests();
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);

    let destination_window = cx
        .windows()
        .into_iter()
        .find(|window| window.window_id() != fixture.source_window.window_id())
        .expect("live undock should retain one pending provisional destination window");
    let destination_host = destination_window
        .downcast::<DockHost>()
        .expect("the provisional destination should retain a DockHost root")
        .entity(cx)
        .expect("the provisional destination DockHost should remain live");
    let reveal_ticket = cx.read_entity(&destination_host, |host, _| {
        let state = host
            .live_presentation_state()
            .expect("the pending destination must retain reveal authority");
        let crate::host::DockHostLivePresentationMode::DestinationProjection {
            phase: crate::host::DockHostLiveDestinationPhase::RevealObserving { ticket, .. },
            ..
        } = state.mode
        else {
            panic!("the destination must await one exact reveal observation");
        };
        ticket
    });
    let destination_host = destination_host.downgrade();

    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available_open_desktop(
            point(DevicePixels(1880), DevicePixels(1880)),
            Vec::new(),
        )
        .expect("the desktop release observation should be valid"),
    );
    fixture.source_visual.simulate_mouse_up(
        point(px(940.0), px(940.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let drained_without_advancing_deadline = (0..10_000).any(|_| !cx.background_executor.tick());
    assert!(drained_without_advancing_deadline);
    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .live_undock_phase()),
        crate::surface::live_undock::DockLiveUndockPhase::Bound,
        "the logical-close test must exercise an uncommitted destination"
    );
    assert!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .live_undock_committed_destination_logical_close_authority(
                destination_window.window_id(),
            ))
        .is_none(),
        "the provisional destination must not delegate to committed close"
    );
    assert_eq!(
        reveal_ticket.snapshot().outcome(),
        open_gpui::WindowProvisionalRevealOutcome::Pending
    );
    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_some(),
        "the payload drag must remain live until logical close settles it"
    );

    let native_terminal = cx.hold_window_native_terminal(destination_window);
    destination_window
        .update(cx, |_, window, app| window.remove_window(app))
        .expect("the provisional destination should begin logical close");
    cx.run_until_parked();

    assert!(!cx.windows().contains(&destination_window));
    assert_eq!(
        reveal_ticket.snapshot().outcome(),
        open_gpui::WindowProvisionalRevealOutcome::WindowTerminal
    );
    assert!(
        destination_host.upgrade().is_some(),
        "the held native terminal should keep the window root alive and prove convergence did not rely on DockHost release"
    );
    assert!(matches!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .live_undock_phase()),
        crate::surface::live_undock::DockLiveUndockPhase::Restoring
            | crate::surface::live_undock::DockLiveUndockPhase::Idle
    ));
    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none(),
        "logical close must settle drag finalization before native terminal is released"
    );

    assert!(native_terminal.release());
    cx.run_until_parked();
}

#[open_gpui::test]
fn settled_reveal_observation_ignores_its_stale_deadline(cx: &mut TestAppContext) {
    let mut fixture = native_captured_source_fixture(cx);
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    let (destination_window, destination_host, _) =
        reveal_live_undock_provisional_destination(&fixture, cx);

    cx.executor()
        .advance_clock(crate::render::LIVE_UNDOCK_REVEAL_OBSERVATION_DEADLINE);
    cx.run_until_parked();

    assert!(cx.windows().contains(&destination_window));
    assert!(matches!(
        cx.read_entity(&destination_host, |host, _| host
            .live_presentation_state()
            .map(|state| state.mode)),
        Some(
            crate::host::DockHostLivePresentationMode::DestinationProjection {
                phase: crate::host::DockHostLiveDestinationPhase::RevealSettled,
                ..
            }
        )
    ));
    let facts = destination_window
        .update(cx, |_, window, _| window.presentation_facts())
        .expect("the revealed provisional destination should remain live");
    assert!(facts.native_visible);
    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .live_undock_phase()),
        crate::surface::live_undock::DockLiveUndockPhase::Bound,
        "a stale reveal deadline must not disturb the active live-undock session"
    );
}

#[open_gpui::test]
fn native_drag_transport_capture_survives_payload_title_refresh(cx: &mut TestAppContext) {
    let mut fixture = native_captured_source_fixture(cx);
    fixture.begin_drag(cx);
    let capture = assert_native_source_transport_capture(&fixture, None, cx);

    fixture.controller.update(cx, |controller, cx| {
        controller
            .workspace_mut()
            .register_panel_descriptor(item("a"), DockPanelDescriptor::new("Renamed Panel A"));
        cx.notify();
    });

    for _ in 0..2 {
        fixture
            .source_window
            .update(cx, |_, window, _| window.refresh())
            .expect("the source window should remain refreshable");
        cx.run_until_parked();
        assert_native_source_transport_capture(&fixture, Some(capture), cx);
        assert!(
            fixture
                .runtime
                .active_payload_drag_session(&fixture.payload)
                .is_some(),
            "a presentation-only title change must not replace the native drag session",
        );
    }
}

#[open_gpui::test]
fn native_drag_transport_capture_survives_floating_title_refresh(cx: &mut TestAppContext) {
    let (surface, runtime, source_window, floating) = cx.update(|cx| {
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
            .allow_floating(true)
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the floating captured-drag source surface should build");
        surface
            .float_panel_in_window("a", floating_bounds(24.0, 28.0, 240.0, 150.0), cx)
            .expect("Panel A should become a floating source");
        let controller = surface.controller(cx);
        let runtime = surface.viewport_runtime(cx);
        let floating = cx.read_entity(&controller, |controller, _| {
            controller
                .graph()
                .floating_containers(&DockSpaceId::from("main"))
                .first()
                .map(|container| container.node)
                .expect("the source surface should retain its floating container")
        });
        let source_window =
            match surface.open_primary_window(viewport_window_options(360.0, 240.0), cx) {
                DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
                outcome => {
                    panic!("the floating captured-drag source window should open, got {outcome:?}")
                }
            };
        (surface, runtime, source_window, floating)
    });
    cx.run_until_parked();

    let source_host = source_window
        .downcast::<DockHost>()
        .expect("the source window should retain a DockHost root")
        .entity(cx)
        .expect("the floating source DockHost should remain live");
    let mut source_visual = VisualTestContext::from_window(source_window, cx);
    let source_handle = selector_for(
        &source_visual,
        &source_host,
        DockDebugRegion::FloatingHandle { node: floating },
    )
    .expect("the floating source handle should be emitted");
    let start = debug_bounds(&mut source_visual, &source_handle).center();
    let threshold = point(start.x + px(24.0), start.y);
    let payload =
        DockDragPayload::new_floating(DockSpaceId::from("main"), floating, "Panel A".to_string());
    advertise_native_window_hit_stack(cx);

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(
        runtime.active_payload_drag_session(&payload).is_some(),
        "the floating title drag should establish one native payload session",
    );
    let capture = source_window
        .update(cx, |_, window, _| window.captured_pointer())
        .expect("the source window should remain available")
        .expect("the floating source should retain pointer capture")
        .handle();

    for _ in 0..2 {
        source_window
            .update(cx, |_, window, _| window.refresh())
            .expect("the floating source window should remain refreshable");
        cx.run_until_parked();
        let current_capture = source_window
            .update(cx, |_, window, _| window.captured_pointer())
            .expect("the source window should remain available")
            .expect("the refreshed floating source should retain pointer capture")
            .handle();
        assert_eq!(current_capture, capture);
        let proxy = cx
            .read_entity(&source_host, |host, _| host.native_drag_transport_proxy())
            .expect("the floating source should retain its native transport proxy");
        assert_eq!(proxy.pointer_capture(), capture);
    }

    drop(surface);
}

#[open_gpui::test]
fn moving_presentation_failure_retires_transport_before_source_restoration(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_source_fixture(cx);
    cx.defer_next_window_frame_requests();
    configure_native_desktop_release(
        cx,
        fixture.source_window,
        size(DevicePixels(720), DevicePixels(440)),
    );
    fixture.begin_drag(cx);
    fixture.source_visual.simulate_mouse_move(
        point(px(900.0), px(900.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let live_runtime = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner.live_undock_runtime()
    });

    let injected = (0..10_000).any(|_| {
        let source_projection_and_transport = cx.read_entity(&fixture.source_host, |host, _| {
            host.live_presentation_state().is_some() && host.native_drag_transport_proxy().is_some()
        });
        if source_projection_and_transport
            && cx.update(|cx| live_runtime.fail_current_presentation_for_test(cx))
        {
            true
        } else {
            cx.background_executor.tick();
            false
        }
    });
    assert!(
        injected,
        "the moving presentation pipeline should expose one exact failure stage",
    );
    cx.run_until_parked();

    cx.read_entity(&fixture.source_host, |host, _| {
        assert!(host.native_drag_transport_proxy().is_none());
        assert!(
            !host.has_native_drag_transport_proxy_slot_for_test(),
            "source restoration must remove the transport slot, not merely hide an inactive lease",
        );
    });
    for _ in 0..2 {
        fixture
            .source_window
            .update(cx, |_, window, _| window.refresh())
            .expect("the restored source should remain refreshable");
        cx.run_until_parked();
        assert!(cx.read_entity(&fixture.source_host, |host, _| {
            !host.has_native_drag_transport_proxy_slot_for_test()
        }));
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
    let deferred_replaced_target = Rc::new(Cell::new(false));
    let _interceptor = cx
        .update_window(source_window, {
            let controller = controller.clone();
            let empty_space = empty_space.clone();
            let target_host = target_host.clone();
            let listener_replaced_target = listener_replaced_target.clone();
            let deferred_replaced_target = deferred_replaced_target.clone();
            move |_, window, _| {
                window.intercept_window_mouse_events(move |event, _, cx| {
                    if matches!(event, WindowMouseEvent::Up(_))
                        && !listener_replaced_target.replace(true)
                    {
                        let controller = controller.clone();
                        let empty_space = empty_space.clone();
                        let target_host = target_host.clone();
                        let deferred_replaced_target = deferred_replaced_target.clone();
                        cx.defer(move |cx| {
                            controller.update(cx, |controller, controller_cx| {
                                let mut graph = controller.graph().clone();
                                let replacement_tabs = graph.insert_node(DockNode::Tabs {
                                    items: vec![item("b")],
                                    selected: Some(item("b")),
                                });
                                graph.set_root(empty_space, replacement_tabs);
                                controller.workspace_mut().set_graph(graph);
                                controller_cx.notify();
                            });
                            target_host.update(cx, |_, host_cx| host_cx.notify());
                            deferred_replaced_target.set(true);
                        });
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
    assert!(deferred_replaced_target.get());
    let replacement_frame = cx.read_entity(&target_host, |host, _| {
        host.interaction()
            .viewport_host_scene_frame()
            .cloned()
            .expect("the listener-created G2 scene should commit")
    });
    assert_ne!(replacement_frame, locked_frame);
    assert_eq!(
        replacement_frame.registration_key(),
        locked_frame.registration_key(),
        "the deferred G2 scene should replace G1 without replacing its viewport registration"
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("a")],
            "a stale G1 release reservation must not remove the source item"
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&empty_space),
            vec![item("b")],
            "the MouseUp listener's G2 target may appear, but it cannot receive the frozen G1 release"
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
fn native_captured_release_fails_closed_when_mouse_up_listener_occludes_locked_target(
    cx: &mut TestAppContext,
) {
    let source_space = space();
    let empty_space = DockSpaceId::from("empty");
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
    let target_host = cx.new(|cx| {
        DockHost::from_controller(controller.clone(), empty_space.clone(), runtime.clone(), cx)
    });
    let occluded_frame_rendered = Rc::new(Cell::new(false));
    let target_window = cx.open_window(size(px(360.0), px(220.0)), {
        let target_host = target_host.clone();
        let occluded_frame_rendered = occluded_frame_rendered.clone();
        move |_, _| ConditionalOccludedDockHostFixture {
            host: target_host,
            occluded: false,
            occluded_frame_rendered,
        }
    });
    let target_root = target_window
        .root(cx)
        .expect("the target window should expose its conditional root");
    cx.run_until_parked();
    let mut target_visual = VisualTestContext::from_window(target_window.into(), cx);
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
    let locked_window_frame_revision = cx
        .update_window(target_window, |_, window, _| {
            window.rendered_frame_revision()
        })
        .expect("the target window should expose its locked frame revision");

    let listener_occluded_target = Rc::new(Cell::new(false));
    let deferred_occluded_target = Rc::new(Cell::new(false));
    let _interceptor = cx
        .update_window(source_window, {
            let target_root = target_root.clone();
            let listener_occluded_target = listener_occluded_target.clone();
            let deferred_occluded_target = deferred_occluded_target.clone();
            move |_, window, _| {
                window.intercept_window_mouse_events(move |event, _, cx| {
                    if matches!(event, WindowMouseEvent::Up(_))
                        && !listener_occluded_target.replace(true)
                    {
                        let target_root = target_root.clone();
                        let deferred_occluded_target = deferred_occluded_target.clone();
                        cx.defer(move |cx| {
                            target_root.update(cx, |root, root_cx| {
                                root.occluded = true;
                                root_cx.notify();
                            });
                            deferred_occluded_target.set(true);
                        });
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

    assert!(listener_occluded_target.get());
    assert!(deferred_occluded_target.get());
    assert!(
        occluded_frame_rendered.get(),
        "the deferred BlockMouse overlay should render before terminal release settles"
    );
    let occluded_window_frame_revision = cx
        .update_window(target_window, |_, window, _| {
            window.rendered_frame_revision()
        })
        .expect("the target window should expose its occluded frame revision");
    assert!(
        occluded_window_frame_revision > locked_window_frame_revision,
        "the foreground BlockMouse overlay should commit a newer target-window frame"
    );
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("a")],
            "the now-occluded G1 host must not consume the source item"
        );
        assert!(
            controller
                .graph()
                .collect_items_in_space(&empty_space)
                .is_empty(),
            "the release must not pass through the foreground blocker into the empty host"
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
        "the rejected release must still retire its exact drag session"
    );
}

fn assert_native_captured_release_uses_locked_policy(
    cx: &mut TestAppContext,
    allowed_at_mouse_up: bool,
    allowed_in_listener: bool,
    expect_drop: bool,
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
    workspace
        .policy_mut()
        .set_allow_center_merge(allowed_at_mouse_up);
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
    let target_panel = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::Panel { item: item("b") },
    )
    .expect("the target panel selector should be emitted");
    let start = debug_bounds(&mut source_visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let target_position = debug_bounds(&mut target_visual, &target_panel).center();
    let target_global_from_source = point(px(400.0) + target_position.x, target_position.y);
    configure_native_registered_window_hit(
        cx,
        source_window,
        target_window,
        target_global_from_source,
    );

    let listener_ran = Rc::new(Cell::new(false));
    let _interceptor = cx
        .update_window(source_window, {
            let controller = controller.clone();
            let listener_ran = listener_ran.clone();
            move |_, window, _| {
                window.intercept_window_mouse_events(move |event, _, cx| {
                    if matches!(event, WindowMouseEvent::Up(_)) && !listener_ran.replace(true) {
                        controller.update(cx, |controller, _| {
                            controller
                                .policy_mut()
                                .set_allow_center_merge(allowed_in_listener);
                        });
                    }
                })
            }
        })
        .expect("the source should install its MouseUp policy interceptor");

    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(
        target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    source_visual.simulate_mouse_up(
        target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    assert!(listener_ran.get());
    cx.read_entity(&controller, |controller, _| {
        let source_items = controller.graph().collect_items_in_space(&source_space);
        let target_items = controller.graph().collect_items_in_space(&target_space);
        if expect_drop {
            assert!(source_items.is_empty());
            assert_eq!(target_items.len(), 2);
            assert!(target_items.contains(&item("a")));
            assert!(target_items.contains(&item("b")));
        } else {
            assert_eq!(source_items, vec![item("a")]);
            assert_eq!(target_items, vec![item("b")]);
        }
    });
}

#[open_gpui::test]
fn native_captured_release_keeps_mouse_up_rejection_when_listener_relaxes_policy(
    cx: &mut TestAppContext,
) {
    assert_native_captured_release_uses_locked_policy(cx, false, true, false);
}

#[open_gpui::test]
fn native_captured_release_keeps_mouse_up_acceptance_when_listener_tightens_policy(
    cx: &mut TestAppContext,
) {
    assert_native_captured_release_uses_locked_policy(cx, true, false, true);
}

#[open_gpui::test]
fn native_captured_release_rejects_tabs_payload_changed_by_mouse_up_listener(
    cx: &mut TestAppContext,
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
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("x", "Panel X", "X"),
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
        runtime,
        target_space.clone(),
        size(px(360.0), px(220.0)),
    );
    let source_window: AnyWindowHandle = source_window.into();
    let target_window: AnyWindowHandle = target_window.into();

    let source_stack = selector_for(
        &source_visual,
        &source_host,
        DockDebugRegion::Tabs { node: source_tabs },
    )
    .expect("the source tabs selector should be emitted");
    let target_stack = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::Tabs { node: target_tabs },
    )
    .expect("the target tabs selector should be emitted");
    let source_bounds = debug_bounds(&mut source_visual, &source_stack);
    let start = point(
        source_bounds.origin.x + source_bounds.size.width - px(8.0),
        source_bounds.origin.y + px(12.0),
    );
    let threshold = point(start.x + px(24.0), start.y);
    let target_position = debug_bounds(&mut target_visual, &target_stack).center();
    let target_global_from_source = point(px(400.0) + target_position.x, target_position.y);
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
        "the MouseUp locker should observe a valid Tabs center-drop candidate"
    );

    let listener_opened_item = Rc::new(Cell::new(false));
    let _interceptor = cx
        .update_window(source_window, {
            let controller = controller.clone();
            let source_space = source_space.clone();
            let listener_opened_item = listener_opened_item.clone();
            move |_, window, _| {
                window.intercept_window_mouse_events(move |event, _, cx| {
                    if matches!(event, WindowMouseEvent::Up(_))
                        && !listener_opened_item.replace(true)
                    {
                        controller.update(cx, |controller, _| {
                            controller
                                .open_item(source_space.clone(), Some(source_tabs), item("x"), None)
                                .expect(
                                    "the MouseUp listener should add Panel X to the source tabs",
                                );
                        });
                    }
                })
            }
        })
        .expect("the source should install its MouseUp payload interceptor");

    source_visual.simulate_mouse_up(
        target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    assert!(listener_opened_item.get());
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("a"), item("x")],
            "a stale frozen Tabs payload must leave both the original and listener-added items at the source"
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&target_space),
            vec![item("b")],
            "a changed Tabs payload must not partially or fully commit into the target"
        );
    });
}

#[open_gpui::test]
fn native_captured_release_rejects_floating_title_target_reparented_by_mouse_up_listener(
    cx: &mut TestAppContext,
) {
    let source_space = DockSpaceId::from("source");
    let target_space = DockSpaceId::from("target");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let target_root_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let floating_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    let floating = graph.insert_node(DockNode::Floating {
        child: floating_tabs,
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_root_tabs);
    graph
        .floating_containers_mut(target_space.clone())
        .push(DockFloatingContainer {
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
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace.policy_mut().set_allow_floating(true);
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
        runtime,
        target_space.clone(),
        size(px(420.0), px(260.0)),
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
    let floating_handle = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::FloatingHandle { node: floating },
    )
    .expect("the target floating handle selector should be emitted");
    let start = debug_bounds(&mut source_visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let target_position = debug_bounds(&mut target_visual, &floating_handle).center();
    let target_global_from_source = point(px(400.0) + target_position.x, target_position.y);
    configure_native_registered_window_hit_with_target_size(
        cx,
        source_window,
        target_window,
        target_global_from_source,
        size(px(420.0), px(260.0)),
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
        "the MouseUp locker should observe the floating title-bar candidate"
    );

    let listener_reparented_target = Rc::new(Cell::new(false));
    let _interceptor = cx
        .update_window(source_window, {
            let controller = controller.clone();
            let target_space = target_space.clone();
            let listener_reparented_target = listener_reparented_target.clone();
            move |_, window, _| {
                window.intercept_window_mouse_events(move |event, _, cx| {
                    if matches!(event, WindowMouseEvent::Up(_))
                        && !listener_reparented_target.replace(true)
                    {
                        controller.update(cx, |controller, _| {
                            let plan = controller
                                .graph()
                                .edge_dock_plan(
                                    &target_space,
                                    target_root_tabs,
                                    DropZone::Right,
                                )
                                .expect("the main target root should admit an edge move");
                            controller
                                .workspace_mut()
                                .commit_floating_move(
                                    &target_space,
                                    floating,
                                    &target_space,
                                    DockGraphDropTarget::edge(plan),
                                )
                                .expect(
                                    "the MouseUp listener should dock the floating target into the main tree",
                                );
                        });
                    }
                })
            }
        })
        .expect("the source should install its MouseUp target interceptor");

    source_visual.simulate_mouse_up(
        target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    assert!(listener_reparented_target.get());
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("a")],
            "the source item must remain when the frozen floating-title owner is stale"
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&target_space),
            vec![item("b"), item("c")],
            "the locked release must not redirect into the reparented floating tabs"
        );
        assert!(
            controller
                .graph()
                .floating_containers(&target_space)
                .is_empty(),
            "the listener mutation should have removed the original floating owner"
        );
        assert!(
            matches!(
                controller.graph().node(floating_tabs),
                Some(DockNode::Tabs { items, .. }) if items == &vec![item("c")]
            ),
            "the listener should preserve the original tabs node while reparenting it into the main tree"
        );
    });
}

#[open_gpui::test]
fn live_rehost_session_checkout_restores_authority_after_unwind(cx: &mut TestAppContext) {
    let mut fixture = native_captured_source_fixture(cx);
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    let runtime = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner.live_undock_runtime()
    });

    let panic = catch_unwind(AssertUnwindSafe(|| {
        runtime.panic_with_current_rehost_session_checked_out_for_test();
    }));
    assert!(
        panic.is_err(),
        "the test must exercise unwind while checked out"
    );
    assert!(
        runtime.current_rehost_session_is_active_for_test(),
        "RAII checkout must restore the exact rehost session after unwind"
    );

    let session = fixture
        .runtime
        .active_payload_drag_session(&fixture.payload)
        .expect("the drag session must remain recoverable after the injected panic");
    cx.update(|app| {
        crate::native_captured_drag::cancel_native_captured_drag_route(
            fixture.runtime.identity(),
            Some(&session),
            Some(&fixture.payload),
            &fixture.source_host.downgrade(),
            None,
            PointerCancelReason::CaptureRevoked,
            app,
        );
    });
    cx.run_until_parked();
    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .live_undock_phase()),
        crate::surface::live_undock::DockLiveUndockPhase::Idle
    );
    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none(),
        "the restored session must still converge through ordinary cancellation"
    );
}

#[open_gpui::test]
fn native_source_restoration_activates_only_after_the_visible_receipt(cx: &mut TestAppContext) {
    let mut fixture = native_captured_source_fixture(cx);
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    let session = fixture
        .runtime
        .active_payload_drag_session(&fixture.payload)
        .expect("the source should retain its exact live-undock drag session");
    let activation = cx.window_activation_probe(fixture.source_window);
    let activation_before_cancel = activation.count();
    let observed_before_visible_receipt = Rc::new(Cell::new(false));
    cx.set_platform_focused_window_available(false);

    cx.update({
        let activation = activation.clone();
        let observed_before_visible_receipt = observed_before_visible_receipt.clone();
        let owner = fixture.surface.owner().clone();
        let source_host = fixture.source_host.clone();
        |app| {
            crate::native_captured_drag::cancel_native_captured_drag_route(
                fixture.runtime.identity(),
                Some(&session),
                Some(&fixture.payload),
                &fixture.source_host.downgrade(),
                None,
                PointerCancelReason::CaptureRevoked,
                app,
            );
            app.defer(move |app| {
                assert_eq!(
                    app.read_entity(&owner, |owner, _| owner.live_undock_phase()),
                    crate::surface::live_undock::DockLiveUndockPhase::Restoring,
                );
                app.read_entity(&source_host, |host, _| {
                    assert!(matches!(
                        host.live_presentation_state().map(|state| state.mode),
                        Some(
                            crate::host::DockHostLivePresentationMode::SourceRestoration {
                                phase: crate::host::DockHostLiveSourceRestorationPhase::Staging,
                                ..
                            }
                        )
                    ));
                    assert!(
                        host.live_source_semantic_proxy().is_some(),
                        "restoration staging must preserve semantic continuity",
                    );
                    assert!(
                        host.native_drag_transport_proxy().is_none(),
                        "route cancellation must retire transport before ordinary source rendering resumes",
                    );
                });
                assert_eq!(
                    activation.count(),
                    activation_before_cancel,
                    "staging source restoration cannot activate before its visible receipt",
                );
                observed_before_visible_receipt.set(true);
            });
        }
    });

    assert!(
        observed_before_visible_receipt.get(),
        "the FIFO probe must run between source-restoration install and its first frame",
    );

    cx.run_until_parked();
    cx.set_platform_focused_window_available(true);

    assert_eq!(
        activation.count(),
        activation_before_cancel + 1,
        "the accepted visible restoration receipt must activate the source exactly once",
    );
    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .live_undock_phase()),
        crate::surface::live_undock::DockLiveUndockPhase::Idle,
    );
    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none(),
        "terminal restoration must release the exact drag session",
    );
}

#[open_gpui::test]
fn live_source_restoration_host_loss_after_finish_converges_through_orphan_recovery(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_source_fixture(cx);
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    let session = fixture
        .runtime
        .active_payload_drag_session(&fixture.payload)
        .expect("the source should retain its exact live-undock drag session");
    let owner = fixture.surface.owner().clone();
    let live_runtime = cx.read_entity(&owner, |owner, _| owner.live_undock_runtime());
    live_runtime.replace_source_host_after_finish_once_for_test();
    let source_window = fixture.source_window;
    let source_host = fixture.source_host.downgrade();
    let panel_entity = cx.read_entity(&fixture.controller, |controller, _| {
        controller
            .workspace()
            .panels()
            .resolved_render_view(&item("a"))
            .expect("the live payload should retain its resolved panel view")
            .entity_id()
    });
    drop(fixture.source_host);

    cx.update(|app| {
        crate::native_captured_drag::cancel_native_captured_drag_route(
            fixture.runtime.identity(),
            Some(&session),
            Some(&fixture.payload),
            &source_host,
            None,
            PointerCancelReason::CaptureRevoked,
            app,
        );
    });
    cx.run_until_parked();

    assert!(source_host.upgrade().is_none());
    assert!(
        source_window
            .update(cx, |_, window, _| window.presentation_facts())
            .is_ok(),
        "source Host loss must not imply source-window terminal"
    );
    assert!(
        cx.read(|app| {
            open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                panel_entity,
                source_window.window_id(),
            )
        })
        .is_none(),
        "Host loss before visible confirmation must release the exact restored stable lease"
    );
    cx.read_entity(&owner, |owner, _| {
        assert_eq!(
            owner.live_undock_phase(),
            crate::surface::live_undock::DockLiveUndockPhase::Idle,
            "source Host loss must not strand live undock in Restoring"
        );
        assert_eq!(owner.live_undock_runtime().execution_count_for_test(), 0);
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::PreCommitOrphan,
            ),
            1,
            "presentation authority loss must preserve the payload as a pre-commit recovery record"
        );
    });
    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none(),
        "terminal orphan recovery must release the exact drag session"
    );
}

#[open_gpui::test]
fn orphan_recovery_replays_cleanup_after_durable_recovery_commit(cx: &mut TestAppContext) {
    let mut fixture = native_captured_source_fixture(cx);
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    let session = fixture
        .runtime
        .active_payload_drag_session(&fixture.payload)
        .expect("the source should retain its exact live-undock drag session");
    let owner = fixture.surface.owner().clone();
    let live_runtime = cx.read_entity(&owner, |owner, _| owner.live_undock_runtime());
    live_runtime.replace_source_host_after_finish_once_for_test();
    live_runtime.interrupt_orphan_cleanup_after_recovery_commit_once_for_test();
    let source_host = fixture.source_host.downgrade();
    drop(fixture.source_host);

    cx.update(|app| {
        crate::native_captured_drag::cancel_native_captured_drag_route(
            fixture.runtime.identity(),
            Some(&session),
            Some(&fixture.payload),
            &source_host,
            None,
            PointerCancelReason::CaptureRevoked,
            app,
        );
    });
    cx.run_until_parked();

    cx.read_entity(&owner, |owner, _| {
        assert_eq!(
            owner.live_undock_phase(),
            crate::surface::live_undock::DockLiveUndockPhase::RecoveringOrphan,
            "an interrupted cleanup must retain the reducer and runtime execution"
        );
        assert_eq!(owner.live_undock_runtime().execution_count_for_test(), 1);
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::PreCommitOrphan,
            ),
            1,
            "recovery authority must become durable before destructive cleanup"
        );
    });

    cx.executor().advance_clock(Duration::from_millis(16));
    cx.run_until_parked();

    cx.read_entity(&owner, |owner, _| {
        assert_eq!(
            owner.live_undock_phase(),
            crate::surface::live_undock::DockLiveUndockPhase::Idle,
            "the retry must replay cleanup from the committed recovery receipt"
        );
        assert_eq!(owner.live_undock_runtime().execution_count_for_test(), 0);
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::PreCommitOrphan,
            ),
            1,
            "cleanup replay must not duplicate the durable recovery record"
        );
    });
    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none(),
        "accepted orphan recovery must release the exact drag generation"
    );
}

#[open_gpui::test]
fn source_restoration_shutdown_before_visible_receipt_never_activates(cx: &mut TestAppContext) {
    let mut fixture = native_captured_source_fixture(cx);
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    let session = fixture
        .runtime
        .active_payload_drag_session(&fixture.payload)
        .expect("the source should retain its exact live-undock drag session");
    let activation = cx.window_activation_probe(fixture.source_window);
    let activation_before_cancel = activation.count();
    let shutdown_started_before_visible_receipt = Rc::new(Cell::new(false));
    let runtime_identity = fixture.runtime.identity();
    let payload = fixture.payload.clone();
    let source_host_weak = fixture.source_host.downgrade();
    let owner = fixture.surface.owner().clone();
    let live_runtime = cx.read_entity(&owner, |owner, _| owner.live_undock_runtime());
    let cleanup_authority = Rc::new(RefCell::new(None));
    live_runtime.reject_orphan_recovery_records_for_test();

    cx.update({
        let activation = activation.clone();
        let owner = owner.clone();
        let source_host = fixture.source_host.clone();
        let source_window = fixture.source_window;
        let live_runtime = live_runtime.clone();
        let cleanup_authority = cleanup_authority.clone();
        let shutdown_started_before_visible_receipt =
            shutdown_started_before_visible_receipt.clone();
        move |app| {
            crate::native_captured_drag::cancel_native_captured_drag_route(
                runtime_identity,
                Some(&session),
                Some(&payload),
                &source_host_weak,
                None,
                PointerCancelReason::CaptureRevoked,
                app,
            );
            app.defer(move |app| {
                assert_eq!(
                    app.read_entity(&owner, |owner, _| owner.live_undock_phase()),
                    crate::surface::live_undock::DockLiveUndockPhase::Restoring,
                );
                app.read_entity(&source_host, |host, _| {
                    assert!(matches!(
                        host.live_presentation_state().map(|state| state.mode),
                        Some(
                            crate::host::DockHostLivePresentationMode::SourceRestoration {
                                phase: crate::host::DockHostLiveSourceRestorationPhase::Staging,
                                ..
                            }
                        )
                    ));
                    assert!(host.live_source_semantic_proxy().is_some());
                    assert!(host.native_drag_transport_proxy().is_none());
                });
                assert_eq!(activation.count(), activation_before_cancel);
                *cleanup_authority.borrow_mut() = Some(
                    live_runtime
                        .orphan_cleanup_authority_for_test()
                        .expect("source restoration must retain exact cleanup authority"),
                );
                source_window
                    .update(app, |_, window, app| window.remove_window(app))
                    .expect(
                        "the source anchor should remain removable before its restoration frame",
                    );
                shutdown_started_before_visible_receipt.set(true);
            });
        }
    });
    assert!(shutdown_started_before_visible_receipt.get());
    cx.run_until_parked();
    let (prepared_rehost, retained_visual, source_transport) = cleanup_authority
        .borrow_mut()
        .take()
        .expect("the pre-shutdown cleanup authority must be captured");

    assert_eq!(
        activation.count(),
        activation_before_cancel,
        "surface shutdown must revoke pending source-focus authority",
    );
    assert!(cx.windows().is_empty());
    let status = cx.update(|app| fixture.surface.window_session_status(app));
    let convergence = (status.phase() != crate::DockSurfaceWindowSessionPhase::Closed).then(|| {
        cx.read_entity(fixture.surface.owner(), |owner, _| {
            let lease = owner
                .window_session()
                .shutting_down_lease()
                .expect("a non-closed status must retain its shutdown lease");
            (
                owner.window_session().pending_terminal_window_ids(lease),
                owner.window_session().has_pending_dependencies(lease),
                owner.live_undock_phase(),
                owner.live_undock_runtime().execution_count_for_test(),
            )
        })
    });
    assert_eq!(
        status.phase(),
        crate::DockSurfaceWindowSessionPhase::Closed,
        "source-restoration shutdown must fully converge: {status:?}; convergence={convergence:?}"
    );
    assert_eq!(status.pending_terminal_ticket_count(), 0);
    assert_eq!(status.runtime_empty(), Some(true));
    assert_eq!(
        cx.read_entity(&owner, |owner, _| owner.live_undock_phase()),
        crate::surface::live_undock::DockLiveUndockPhase::Idle,
        "shutdown cleanup must publish one terminal fact before the surface closes"
    );
    assert_eq!(
        live_runtime.execution_count_for_test(),
        0,
        "terminal publication must remove the exact live-undock execution"
    );
    let recovery_count = cx.read_entity(&owner, |owner, _| {
        owner.visible_payload_recovery_count_for_test(
            crate::surface::payload_recovery::DockPayloadRecoveryReason::PreCommitOrphan,
        )
    });
    let rehost_absent = cx.read(|app| prepared_rehost.authority_is_retired(app));
    assert!(
        rehost_absent,
        "shutdown fallback must retire exact rehost authority; captured_generation={}; recovery_count={recovery_count}; transport_active={}",
        prepared_rehost.generation(),
        source_transport.is_active(),
    );
    assert_eq!(
        retained_visual.source_window(),
        prepared_rehost.source().window_id()
    );
    assert!(!source_transport.is_active());
    assert!(fixture.source_window.update(cx, |_, _, _| ()).is_err());
    assert_eq!(
        recovery_count, 0,
        "shutdown fallback cleanup must not fabricate a durable recovery record"
    );
}

#[open_gpui::test]
fn surface_owned_native_captured_desktop_release_promotes_exact_visible_provisional_window(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_source_fixture(cx);
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    let (destination_window, destination_host, destination_space) =
        reveal_live_undock_provisional_destination(&fixture, cx);
    let accepted_geometry = cx.read_entity(&destination_host, |host, _| {
        host.live_destination_geometry_for_test()
    });
    let destination_window_facts = destination_window
        .update(cx, |_, window, app| {
            crate::DockViewportWindowFacts::from_window(window, app)
        })
        .expect("the provisional destination should retain current platform facts");
    assert!(
        accepted_geometry.is_some(),
        "the visible provisional must retain accepted host geometry before release; window_facts={destination_window_facts:?}"
    );
    let window_count_before_release = cx.windows().len();
    let revision_before_release = cx.read(|app| fixture.surface.revision(app));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let surface = fixture.surface.clone();
    let _change_subscription = cx.update(|app| {
        surface.subscribe_changes(app, {
            let changes = changes.clone();
            move |event, _| changes.borrow_mut().push(event.clone())
        })
    });
    {
        let runtime = fixture.runtime.borrow();
        assert_eq!(runtime.adapter().window_for_space(&destination_space), None);
        assert_eq!(
            runtime
                .adapter()
                .space_for_window_id(destination_window.window_id()),
            None
        );
    }
    cx.read_entity(&fixture.controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&DockSpaceId::from("main")),
            vec![item("a"), item("b")]
        );
        assert!(
            controller
                .graph()
                .collect_items_in_space(&destination_space)
                .is_empty()
        );
    });

    cx.set_next_window_placement_dispatch(destination_window, PlatformWindowDispatch::Queued);
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available_open_desktop(
            point(DevicePixels(1880), DevicePixels(1880)),
            Vec::new(),
        )
        .expect("the moved desktop release observation should be valid"),
    );
    fixture.source_visual.simulate_mouse_up(
        point(px(940.0), px(940.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let drained_without_advancing_deadline = (0..10_000).any(|_| !cx.background_executor.tick());
    assert!(
        drained_without_advancing_deadline,
        "the queued placement must quiesce without consuming the release deadline"
    );
    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .live_undock_phase()),
        crate::surface::live_undock::DockLiveUndockPhase::Bound,
        "a queued placement is not promotion evidence"
    );
    assert_eq!(
        cx.read(|app| fixture.surface.revision(app)),
        revision_before_release,
        "queued placement must not publish topology"
    );
    cx.read_entity(&fixture.controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&DockSpaceId::from("main")),
            vec![item("a"), item("b")]
        );
    });
    assert!(cx.flush_window_mutation(destination_window, WindowMutationDomain::Placement));
    cx.run_until_parked();

    let (mapped_window, mapped_space, registration) = {
        let runtime = fixture.runtime.borrow();
        (
            runtime.adapter().window_for_space(&destination_space),
            runtime
                .adapter()
                .space_for_window_id(destination_window.window_id())
                .cloned(),
            runtime.adapter().registration_key(&destination_space),
        )
    };
    assert_eq!(mapped_window, Some(destination_window));
    assert_eq!(mapped_space, Some(destination_space.clone()));
    assert_eq!(
        cx.windows().len(),
        window_count_before_release,
        "same-window promotion must not open a replacement HWND"
    );
    let committed_host = destination_window
        .downcast::<DockHost>()
        .expect("the committed destination should retain a DockHost root")
        .entity(cx)
        .expect("the committed destination DockHost should remain live");
    assert_eq!(committed_host.entity_id(), destination_host.entity_id());
    cx.read_entity(&committed_host, |host, _| {
        assert!(!host.is_provisional_viewport());
        assert_eq!(host.current_viewport_registration(), registration);
        assert!(host.live_presentation_state().is_none());
        assert!(host.live_destination_semantics().is_none());
    });
    cx.read_entity(&fixture.controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&DockSpaceId::from("main")),
            vec![item("b")]
        );
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&destination_space),
            vec![item("a")]
        );
    });
    assert_eq!(
        changes.borrow().len(),
        1,
        "one same-window promotion must publish one transaction, got {:?}",
        changes.borrow().as_slice()
    );
    assert_eq!(
        changes.borrow()[0].categories(),
        [
            crate::DockSurfaceChangeCategory::Layout,
            crate::DockSurfaceChangeCategory::Selection,
            crate::DockSurfaceChangeCategory::PanelLifecycle,
            crate::DockSurfaceChangeCategory::ViewportTopology,
            crate::DockSurfaceChangeCategory::ObservedViewportPlacement,
        ]
    );
    assert_eq!(
        cx.read(|app| fixture.surface.revision(app)),
        revision_before_release + 1
    );
    cx.read_entity(fixture.surface.owner(), |owner, _| {
        assert_eq!(
            owner.live_undock_phase(),
            crate::surface::live_undock::DockLiveUndockPhase::Idle
        );
        assert_eq!(owner.live_undock_runtime().execution_count_for_test(), 0);
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::LostViewportRecovery,
            ),
            0
        );
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::PreCommitOrphan,
            ),
            0
        );
    });
    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none()
    );
    cx.read_entity(&fixture.source_host, |host, _| {
        assert!(host.live_presentation_state().is_none());
        assert!(host.live_source_semantic_proxy().is_none());
        assert!(host.native_drag_transport_proxy().is_none());
    });
    let destination_facts = destination_window
        .update(cx, |_, window, _| window.presentation_facts())
        .expect("the promoted destination window should remain live");
    assert!(destination_facts.native_visible);
    assert!(destination_facts.non_empty_presented_generation.is_some());
}

#[open_gpui::test]
fn same_window_post_swap_graph_replacement_recovers_before_semantics(cx: &mut TestAppContext) {
    let mut fixture = native_captured_source_fixture(cx);
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    let (destination_window, _destination_host, destination_space) =
        reveal_live_undock_provisional_destination(&fixture, cx);

    let live_runtime = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner.live_undock_runtime()
    });
    live_runtime.after_same_window_provider_commit_for_test({
        let controller = fixture.controller.clone();
        let destination_space = destination_space.clone();
        move |cx| {
            cx.update_entity(&controller, |controller, controller_cx| {
                let mut graph = controller.graph().clone();
                let replacement = graph.insert_node(DockNode::Tabs {
                    items: vec![item("c")],
                    selected: Some(item("c")),
                });
                graph.set_root(destination_space, replacement);
                controller.workspace_mut().set_graph(graph);
                controller_cx.notify();
            });
        }
    });

    cx.set_next_window_placement_dispatch(destination_window, PlatformWindowDispatch::Queued);
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available_open_desktop(
            point(DevicePixels(1880), DevicePixels(1880)),
            Vec::new(),
        )
        .expect("the moved desktop release observation should be valid"),
    );
    fixture.source_visual.simulate_mouse_up(
        point(px(940.0), px(940.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let drained_without_advancing_deadline = (0..10_000).any(|_| !cx.background_executor.tick());
    assert!(drained_without_advancing_deadline);
    assert!(cx.flush_window_mutation(destination_window, WindowMutationDomain::Placement));
    cx.run_until_parked();

    assert!(
        destination_window.update(cx, |_, _, _| ()).is_err(),
        "a successor graph that replaces the payload must prevent destination semantics"
    );
    cx.read_entity(&fixture.controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&DockSpaceId::from("main")),
            vec![item("b")],
            "the atomic promotion must commit before the queued graph transaction"
        );
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&destination_space),
            vec![item("c")],
            "the later graph transaction must remain the final topology authority"
        );
    });
    cx.read_entity(fixture.surface.owner(), |owner, _| {
        let phase = owner.live_undock_phase();
        let execution_count = owner.live_undock_runtime().execution_count_for_test();
        assert_eq!(
            phase,
            crate::surface::live_undock::DockLiveUndockPhase::Idle,
            "the superseded promotion must settle through committed-destination recovery; execution_count={execution_count}"
        );
        assert_eq!(
            execution_count,
            0,
            "post-commit publication must retire the exact promotion execution"
        );
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::LostViewportRecovery,
            ),
            1,
            "the replaced payload projection must remain discoverable through recovery"
        );
    });
    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none(),
        "post-commit settlement must release the exact payload drag session"
    );
    cx.read_entity(&fixture.source_host, |host, _| {
        assert!(
            host.live_presentation_state().is_none(),
            "the atomic final swap must retire the frozen source presentation"
        );
        assert!(host.live_source_semantic_proxy().is_none());
        assert!(host.native_drag_transport_proxy().is_none());
    });
}

#[open_gpui::test]
fn same_window_missing_graph_receipt_before_semantics_recovers_committed_destination(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_source_fixture(cx);
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    let (destination_window, _destination_host, destination_space) =
        reveal_live_undock_provisional_destination(&fixture, cx);

    let live_runtime = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner.live_undock_runtime()
    });
    live_runtime.retire_next_same_window_graph_commit_before_semantics_ack_for_test();

    cx.set_next_window_placement_dispatch(destination_window, PlatformWindowDispatch::Queued);
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available_open_desktop(
            point(DevicePixels(1880), DevicePixels(1880)),
            Vec::new(),
        )
        .expect("the moved desktop release observation should be valid"),
    );
    fixture.source_visual.simulate_mouse_up(
        point(px(940.0), px(940.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let drained_without_advancing_deadline = (0..10_000).any(|_| !cx.background_executor.tick());
    assert!(drained_without_advancing_deadline);
    assert!(cx.flush_window_mutation(destination_window, WindowMutationDomain::Placement));
    cx.run_until_parked();

    assert!(
        destination_window.update(cx, |_, _, _| ()).is_err(),
        "a missing graph receipt must never open the destination for interaction"
    );
    cx.read_entity(&fixture.controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&DockSpaceId::from("main")),
            vec![item("b")]
        );
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&destination_space),
            vec![item("a")],
            "recovery must not roll back the committed graph topology"
        );
    });
    cx.read_entity(fixture.surface.owner(), |owner, _| {
        assert_eq!(
            owner.live_undock_phase(),
            crate::surface::live_undock::DockLiveUndockPhase::Idle
        );
        assert_eq!(owner.live_undock_runtime().execution_count_for_test(), 0);
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::LostViewportRecovery,
            ),
            1,
            "a missing graph receipt must enter committed-destination recovery"
        );
    });
    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none(),
        "post-commit settlement must release the exact payload drag generation"
    );
}

#[open_gpui::test]
fn same_window_superseded_graph_before_interaction_admission_recovers_committed_destination(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_source_fixture(cx);
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    let (destination_window, _destination_host, destination_space) =
        reveal_live_undock_provisional_destination(&fixture, cx);

    let live_runtime = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner.live_undock_runtime()
    });
    let interaction_activated = Rc::new(Cell::new(false));
    live_runtime.install_before_destination_interaction_activation_hook_for_test({
        let interaction_activated = interaction_activated.clone();
        move |_| interaction_activated.set(true)
    });
    live_runtime.install_before_destination_interaction_admission_hook_for_test({
        let controller = fixture.controller.clone();
        let destination_space = destination_space.clone();
        move |cx| {
            cx.update_entity(&controller, |controller, controller_cx| {
                let mut graph = controller.graph().clone();
                let replacement = graph.insert_node(DockNode::Tabs {
                    items: vec![item("c")],
                    selected: Some(item("c")),
                });
                graph.set_root(destination_space, replacement);
                controller.workspace_mut().set_graph(graph);
                controller_cx.notify();
            });
        }
    });

    cx.set_next_window_placement_dispatch(destination_window, PlatformWindowDispatch::Queued);
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available_open_desktop(
            point(DevicePixels(1880), DevicePixels(1880)),
            Vec::new(),
        )
        .expect("the moved desktop release observation should be valid"),
    );
    fixture.source_visual.simulate_mouse_up(
        point(px(940.0), px(940.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let drained_without_advancing_deadline = (0..10_000).any(|_| !cx.background_executor.tick());
    assert!(drained_without_advancing_deadline);
    assert!(cx.flush_window_mutation(destination_window, WindowMutationDomain::Placement));
    cx.run_until_parked();

    assert!(
        destination_window.update(cx, |_, _, _| ()).is_err(),
        "a graph superseded immediately before admission must not publish interaction"
    );
    assert!(
        !interaction_activated.get(),
        "a non-exact graph projection must fail before destination interaction activation"
    );
    cx.read_entity(&fixture.controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&DockSpaceId::from("main")),
            vec![item("b")]
        );
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&destination_space),
            vec![item("c")],
            "the successor graph must remain the topology authority"
        );
    });
    cx.read_entity(fixture.surface.owner(), |owner, _| {
        assert_eq!(
            owner.live_undock_phase(),
            crate::surface::live_undock::DockLiveUndockPhase::Idle
        );
        assert_eq!(owner.live_undock_runtime().execution_count_for_test(), 0);
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::LostViewportRecovery,
            ),
            1,
            "admission must recover the committed payload after graph loss"
        );
    });
    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none(),
        "committed-destination recovery must release the exact payload drag generation"
    );
}

#[open_gpui::test]
fn same_window_logical_close_after_final_swap_uses_durable_authority(cx: &mut TestAppContext) {
    let mut fixture = native_captured_source_fixture(cx);
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    let (destination_window, _destination_host, destination_space) =
        reveal_live_undock_provisional_destination(&fixture, cx);

    let ordinary_close_applied = Rc::new(Cell::new(false));
    fixture.runtime.install_window_close_apply_hook_for_test({
        let ordinary_close_applied = ordinary_close_applied.clone();
        move |_| ordinary_close_applied.set(true)
    });
    let durable_authority_observed = Rc::new(Cell::new(false));
    let live_runtime = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner.live_undock_runtime()
    });
    live_runtime.after_same_window_viewport_commit_for_test({
        let owner = fixture.surface.owner().clone();
        let durable_authority_observed = durable_authority_observed.clone();
        move |cx| {
            let authority = cx.read_entity(&owner, |owner, _| {
                owner.live_undock_committed_destination_logical_close_authority(
                    destination_window.window_id(),
                )
            });
            assert!(matches!(
                authority,
                Some(
                    crate::surface::live_undock_runtime::DockLiveUndockLogicalCloseAuthority::Durable(_)
                )
            ));
            durable_authority_observed.set(true);
            destination_window
                .update(cx, |_, window, cx| window.remove_window(cx))
                .expect("the durable destination should begin logical close");
        }
    });

    cx.set_next_window_placement_dispatch(destination_window, PlatformWindowDispatch::Queued);
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available_open_desktop(
            point(DevicePixels(1880), DevicePixels(1880)),
            Vec::new(),
        )
        .expect("the moved desktop release observation should be valid"),
    );
    fixture.source_visual.simulate_mouse_up(
        point(px(940.0), px(940.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let drained_without_advancing_deadline = (0..10_000).any(|_| !cx.background_executor.tick());
    assert!(drained_without_advancing_deadline);
    assert!(cx.flush_window_mutation(destination_window, WindowMutationDomain::Placement));
    cx.run_until_parked();

    assert!(durable_authority_observed.get());
    assert!(
        !ordinary_close_applied.get(),
        "a durable destination must not enter the ordinary provisional close path"
    );
    assert!(
        destination_window.update(cx, |_, _, _| ()).is_err(),
        "committed-destination recovery must settle the closed destination"
    );
    cx.read_entity(&fixture.controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&DockSpaceId::from("main")),
            vec![item("b")]
        );
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&destination_space),
            vec![item("a")],
            "logical close after the final swap must not roll committed topology back"
        );
    });
    cx.read_entity(fixture.surface.owner(), |owner, _| {
        assert_eq!(
            owner.live_undock_phase(),
            crate::surface::live_undock::DockLiveUndockPhase::Idle
        );
        assert_eq!(owner.live_undock_runtime().execution_count_for_test(), 0);
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::LostViewportRecovery,
            ),
            1
        );
    });
    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none()
    );
}

#[open_gpui::test]
fn same_window_post_commit_retries_refresh_after_surface_subscriber_panic(cx: &mut TestAppContext) {
    let mut fixture = native_captured_source_fixture(cx);
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    let (destination_window, _destination_host, destination_space) =
        reveal_live_undock_provisional_destination(&fixture, cx);
    let revision_before_release = cx.read(|app| fixture.surface.revision(app));
    let subscriber_calls = Rc::new(Cell::new(0_u32));
    let surface = fixture.surface.clone();
    let _subscription = cx.update(|app| {
        surface.subscribe_changes(app, {
            let subscriber_calls = subscriber_calls.clone();
            move |_event, _| {
                subscriber_calls.set(subscriber_calls.get() + 1);
                panic!("injected promotion surface subscriber panic");
            }
        })
    });
    cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner
            .live_undock_runtime()
            .panic_next_same_window_post_commit_refresh_for_test();
    });

    cx.set_next_window_placement_dispatch(destination_window, PlatformWindowDispatch::Queued);
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available_open_desktop(
            point(DevicePixels(1880), DevicePixels(1880)),
            Vec::new(),
        )
        .expect("the moved desktop release observation should be valid"),
    );
    fixture.source_visual.simulate_mouse_up(
        point(px(940.0), px(940.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let drained_without_advancing_deadline = (0..10_000).any(|_| !cx.background_executor.tick());
    assert!(drained_without_advancing_deadline);

    let panic = catch_unwind(AssertUnwindSafe(|| {
        assert!(cx.flush_window_mutation(destination_window, WindowMutationDomain::Placement));
        cx.run_until_parked();
    }));
    assert!(panic.is_err());
    assert_eq!(subscriber_calls.get(), 0);
    cx.read_entity(fixture.surface.owner(), |owner, _| {
        assert_eq!(
            owner
                .live_undock_runtime()
                .same_window_post_commit_refresh_attempts_for_test(),
            1
        );
        assert_eq!(
            owner.live_undock_runtime().execution_count_for_test(),
            1,
            "the failed post-commit refresh must retain the exact execution for replay"
        );
    });

    // The retry replays only the incomplete refresh. Surface publication remains at-most-once.
    let publication_panic = catch_unwind(AssertUnwindSafe(|| {
        cx.executor().advance_clock(Duration::from_millis(16));
        cx.run_until_parked();
    }));
    assert!(publication_panic.is_err());

    assert_eq!(
        subscriber_calls.get(),
        1,
        "the committed event is at-most-once"
    );
    cx.read_entity(fixture.surface.owner(), |owner, _| {
        assert_eq!(
            owner
                .live_undock_runtime()
                .same_window_post_commit_refresh_attempts_for_test(),
            1
        );
        assert_eq!(owner.live_undock_runtime().execution_count_for_test(), 1);
    });

    cx.executor().advance_clock(Duration::from_millis(16));
    cx.run_until_parked();
    cx.read_entity(fixture.surface.owner(), |owner, _| {
        assert_eq!(
            owner
                .live_undock_runtime()
                .same_window_post_commit_refresh_attempts_for_test(),
            2,
            "the failed post-commit refresh must be replayed before terminal authority settles"
        );
        assert_eq!(owner.live_undock_runtime().execution_count_for_test(), 0);
    });
    assert_eq!(
        cx.read(|app| fixture.surface.revision(app)),
        revision_before_release + 1
    );
    cx.read_entity(&fixture.controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&DockSpaceId::from("main")),
            vec![item("b")]
        );
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&destination_space),
            vec![item("a")]
        );
    });
    cx.read_entity(fixture.surface.owner(), |owner, _| {
        assert_eq!(
            owner.live_undock_phase(),
            crate::surface::live_undock::DockLiveUndockPhase::Idle
        );
        assert_eq!(owner.live_undock_runtime().execution_count_for_test(), 0);
    });
    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none()
    );
}

#[open_gpui::test]
fn promoted_same_window_destination_adopts_dynamic_close_policy_and_merge_back_lifecycle(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_source_fixture(cx);
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    let (destination_window, _destination_host, destination_space) =
        reveal_live_undock_provisional_destination(&fixture, cx);
    let revision_before_release = cx.read(|app| fixture.surface.revision(app));

    cx.set_next_window_placement_dispatch(destination_window, PlatformWindowDispatch::Queued);
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available_open_desktop(
            point(DevicePixels(1880), DevicePixels(1880)),
            Vec::new(),
        )
        .expect("the moved desktop release observation should be valid"),
    );
    fixture.source_visual.simulate_mouse_up(
        point(px(940.0), px(940.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let drained_without_advancing_deadline = (0..10_000).any(|_| !cx.background_executor.tick());
    assert!(
        drained_without_advancing_deadline,
        "the queued placement must quiesce without consuming the release deadline"
    );
    assert!(cx.flush_window_mutation(destination_window, WindowMutationDomain::Placement));
    cx.run_until_parked();

    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .live_undock_phase()),
        crate::surface::live_undock::DockLiveUndockPhase::Idle,
        "the same-HWND promotion must finish before ordinary close policy takes ownership"
    );
    assert_eq!(
        fixture
            .runtime
            .borrow()
            .adapter()
            .window_for_space(&destination_space),
        Some(destination_window)
    );

    fixture
        .runtime
        .set_close_policy(crate::DockViewportClosePolicy::Prevent);
    let mut destination_visual = VisualTestContext::from_window(destination_window, cx);
    assert!(
        !destination_visual.simulate_close(),
        "a promoted same-HWND viewport must adopt the runtime should-close hook"
    );
    assert_eq!(
        fixture
            .runtime
            .borrow()
            .adapter()
            .window_for_space(&destination_space),
        Some(destination_window),
        "Prevent must preserve the committed destination registration"
    );

    fixture
        .runtime
        .set_close_policy(crate::DockViewportClosePolicy::MergeBack {
            target_space: DockSpaceId::from("main"),
        });
    assert!(
        destination_visual.simulate_close(),
        "MergeBack must allow the promoted committed viewport to close"
    );
    cx.run_until_parked();

    assert!(
        destination_window.update(cx, |_, _, _| ()).is_err(),
        "the promoted destination should reach ordinary logical and native terminal"
    );
    {
        let runtime = fixture.runtime.borrow();
        assert_eq!(runtime.adapter().window_for_space(&destination_space), None);
        assert_eq!(
            runtime
                .adapter()
                .space_for_window_id(destination_window.window_id()),
            None,
            "ordinary close must clear both sides of the promoted registration"
        );
    }
    cx.read_entity(&fixture.controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&DockSpaceId::from("main")),
            vec![item("b"), item("a")],
            "MergeBack must move the promoted payload into the configured fallback space"
        );
        assert!(
            controller
                .graph()
                .collect_items_in_space(&destination_space)
                .is_empty(),
            "ordinary MergeBack close must retire the destination topology"
        );
    });
    assert_eq!(
        cx.read(|app| fixture.surface.revision(app)),
        revision_before_release + 2,
        "promotion and ordinary MergeBack close must publish one transaction each"
    );
}

#[open_gpui::test]
fn destination_close_during_terminal_interaction_activation_uses_ordinary_close_cleanup(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_source_fixture(cx);
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    let (destination_window, _destination_host, destination_space) =
        reveal_live_undock_provisional_destination(&fixture, cx);
    let revision_before_release = cx.read(|app| fixture.surface.revision(app));

    let live_runtime = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner.live_undock_runtime()
    });
    let ordinary_close_applied = Rc::new(Cell::new(false));
    fixture.runtime.install_window_close_apply_hook_for_test({
        let ordinary_close_applied = ordinary_close_applied.clone();
        move |_| ordinary_close_applied.set(true)
    });
    live_runtime.install_before_destination_interaction_activation_hook_for_test({
        let owner = fixture.surface.owner().clone();
        move |cx| {
            cx.read_entity(&owner, |owner, _| {
                assert_eq!(
                    owner.live_undock_phase(),
                    crate::surface::live_undock::DockLiveUndockPhase::Idle,
                    "destination interaction activation must run after reducer terminal acceptance"
                );
                assert!(
                    owner
                        .live_undock_committed_destination_logical_close_authority(
                            destination_window.window_id(),
                        )
                        .is_none(),
                    "an owner-terminal live-undock execution must no longer delegate logical close"
                );
            });
            destination_window
                .update(cx, |_, window, cx| window.remove_window(cx))
                .expect("the committed destination should still be live at interaction activation");
        }
    });
    cx.set_next_window_placement_dispatch(destination_window, PlatformWindowDispatch::Queued);
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available_open_desktop(
            point(DevicePixels(1880), DevicePixels(1880)),
            Vec::new(),
        )
        .expect("the moved desktop release observation should be valid"),
    );
    fixture.source_visual.simulate_mouse_up(
        point(px(940.0), px(940.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let drained_without_advancing_deadline = (0..10_000).any(|_| !cx.background_executor.tick());
    assert!(
        drained_without_advancing_deadline,
        "the queued placement must quiesce without consuming the release deadline"
    );
    assert!(cx.flush_window_mutation(destination_window, WindowMutationDomain::Placement));
    cx.run_until_parked();

    assert!(
        destination_window.update(cx, |_, _, _| ()).is_err(),
        "the reentrant ordinary close must reach the destination's logical and native terminal"
    );
    assert!(
        ordinary_close_applied.get(),
        "owner-terminal reentry must execute the ordinary viewport close transaction"
    );
    {
        let runtime = fixture.runtime.borrow();
        assert_eq!(runtime.adapter().window_for_space(&destination_space), None);
        assert_eq!(
            runtime
                .adapter()
                .space_for_window_id(destination_window.window_id()),
            None,
            "ordinary close must clear both sides of the committed registration"
        );
    }
    cx.read_entity(&fixture.controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&DockSpaceId::from("main")),
            vec![item("b")],
            "a direct logical close has no should-close plan and must preserve the source topology"
        );
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&destination_space),
            vec![item("a")],
            "ordinary RetainLayout cleanup must preserve the committed destination topology"
        );
    });
    cx.read_entity(fixture.surface.owner(), |owner, _| {
        assert_eq!(
            owner.live_undock_phase(),
            crate::surface::live_undock::DockLiveUndockPhase::Idle
        );
        assert_eq!(owner.live_undock_runtime().execution_count_for_test(), 0);
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::LostViewportRecovery,
            ),
            0,
            "an ordinary post-terminal close must not be reclassified as live-undock loss"
        );
    });
    assert_eq!(
        cx.read(|app| fixture.surface.revision(app)),
        revision_before_release + 2,
        "promotion and reentrant ordinary topology cleanup must publish one transaction each"
    );
}

#[open_gpui::test]
fn same_window_promotion_can_commit_failure_restores_without_durable_swap(cx: &mut TestAppContext) {
    let mut fixture = native_captured_source_fixture(cx);
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    let (destination_window, _destination_host, destination_space) =
        reveal_live_undock_provisional_destination(&fixture, cx);
    let revision_before_release = cx.read(|app| fixture.surface.revision(app));

    fixture
        .runtime
        .reject_next_live_undock_promotion_commit_for_test();
    cx.set_next_window_placement_dispatch(destination_window, PlatformWindowDispatch::Queued);
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available_open_desktop(
            point(DevicePixels(1880), DevicePixels(1880)),
            Vec::new(),
        )
        .expect("the moved desktop release observation should be valid"),
    );
    fixture.source_visual.simulate_mouse_up(
        point(px(940.0), px(940.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let drained_without_advancing_deadline = (0..10_000).any(|_| !cx.background_executor.tick());
    assert!(drained_without_advancing_deadline);
    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .live_undock_phase()),
        crate::surface::live_undock::DockLiveUndockPhase::Bound
    );

    assert!(cx.flush_window_mutation(destination_window, WindowMutationDomain::Placement));
    cx.run_until_parked();

    assert_eq!(
        cx.read(|app| fixture.surface.revision(app)),
        revision_before_release
    );
    cx.read_entity(&fixture.controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&DockSpaceId::from("main")),
            vec![item("a"), item("b")]
        );
        assert!(
            controller
                .graph()
                .collect_items_in_space(&destination_space)
                .is_empty()
        );
    });
    {
        let runtime = fixture.runtime.borrow();
        assert_eq!(runtime.adapter().window_for_space(&destination_space), None);
        assert_eq!(
            runtime
                .adapter()
                .space_for_window_id(destination_window.window_id()),
            None
        );
    }
    assert!(
        destination_window.update(cx, |_, _, _| ()).is_err(),
        "pre-swap compensation must retire the provisional destination"
    );
    assert_eq!(cx.windows(), vec![fixture.source_window]);
    cx.read_entity(fixture.surface.owner(), |owner, _| {
        assert_eq!(
            owner.live_undock_phase(),
            crate::surface::live_undock::DockLiveUndockPhase::Idle
        );
        assert_eq!(owner.live_undock_runtime().execution_count_for_test(), 0);
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::LostViewportRecovery,
            ),
            0
        );
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::PreCommitOrphan,
            ),
            0
        );
    });
    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none()
    );
    cx.read_entity(&fixture.source_host, |host, _| {
        assert!(host.live_presentation_state().is_none());
        assert!(host.live_source_semantic_proxy().is_none());
        assert!(host.native_drag_transport_proxy().is_none());
    });
}

#[open_gpui::test]
fn same_window_destination_semantics_can_commit_after_more_than_four_watchdog_wakes(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_source_fixture(cx);
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    let (destination_window, _destination_host, destination_space) =
        reveal_live_undock_provisional_destination(&fixture, cx);
    let live_runtime = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner.live_undock_runtime()
    });
    live_runtime.suppress_same_window_destination_semantics_frames_for_test(u32::MAX);

    cx.set_next_window_placement_dispatch(destination_window, PlatformWindowDispatch::Queued);
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available_open_desktop(
            point(DevicePixels(1880), DevicePixels(1880)),
            Vec::new(),
        )
        .expect("the moved desktop release observation should be valid"),
    );
    fixture.source_visual.simulate_mouse_up(
        point(px(940.0), px(940.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let drained_without_advancing_deadline = (0..10_000).any(|_| !cx.background_executor.tick());
    assert!(drained_without_advancing_deadline);
    assert!(cx.flush_window_mutation(destination_window, WindowMutationDomain::Placement));
    cx.run_until_parked();

    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .live_undock_phase()),
        crate::surface::live_undock::DockLiveUndockPhase::Bound,
        "a missing destination-semantics frame must remain visibly gated before the watchdog expires"
    );
    for _ in 0..5 {
        cx.executor().advance_clock(
            crate::surface::live_undock_runtime::LIVE_UNDOCK_DESTINATION_SEMANTICS_WATCHDOG_INTERVAL,
        );
        cx.run_until_parked();
    }

    assert!(
        destination_window.update(cx, |_, _, _| ()).is_ok(),
        "elapsed watchdog wakes must not close a destination whose exact authority remains live"
    );
    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .live_undock_phase()),
        crate::surface::live_undock::DockLiveUndockPhase::Bound,
        "the destination must remain gated while semantics are pending"
    );

    live_runtime.suppress_same_window_destination_semantics_frames_for_test(0);
    destination_window
        .update(cx, |_, window, _| window.refresh())
        .expect("the exact destination must remain refreshable after delayed semantics");
    for _ in 0..8 {
        cx.executor().advance_clock(
            crate::surface::live_undock_runtime::LIVE_UNDOCK_DESTINATION_SEMANTICS_WATCHDOG_INTERVAL,
        );
        cx.run_until_parked();
        if cx.read_entity(fixture.surface.owner(), |owner, _| {
            owner.live_undock_phase()
        }) == crate::surface::live_undock::DockLiveUndockPhase::Idle
        {
            break;
        }
    }

    assert!(destination_window.update(cx, |_, _, _| ()).is_ok());
    cx.read_entity(&fixture.controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&destination_space),
            vec![item("a")]
        );
    });
    cx.read_entity(fixture.surface.owner(), |owner, _| {
        assert_eq!(
            owner.live_undock_phase(),
            crate::surface::live_undock::DockLiveUndockPhase::Idle
        );
        assert_eq!(owner.live_undock_runtime().execution_count_for_test(), 0);
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::LostViewportRecovery,
            ),
            0,
            "a delayed but healthy destination must not create a recovery entry"
        );
    });
    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none(),
        "successful delayed semantics must settle the exact payload drag authority"
    );
}

#[open_gpui::test]
fn stale_selected_same_window_registration_falls_back_to_ordinary_close_before_recovery(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_source_fixture(cx);
    let _window_closed_observer = cx.update(|app| fixture.runtime.observe_window_closed(app));
    fixture
        .runtime
        .set_close_policy(crate::DockViewportClosePolicy::MergeBack {
            target_space: DockSpaceId::from("main"),
        });
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    let (destination_window, _destination_host, destination_space) =
        reveal_live_undock_provisional_destination(&fixture, cx);
    let revision_before_release = cx.read(|app| fixture.surface.revision(app));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let surface = fixture.surface.clone();
    let _change_subscription = cx.update(|app| {
        surface.subscribe_changes(app, {
            let changes = changes.clone();
            move |event, _| changes.borrow_mut().push(event.clone())
        })
    });

    let live_runtime = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner.live_undock_runtime()
    });
    let replacement_registration = Rc::new(RefCell::new(None));
    fixture
        .runtime
        .install_live_undock_logical_close_selection_hook_for_test({
            let runtime = fixture.runtime.clone();
            let destination_space = destination_space.clone();
            let replacement_registration = replacement_registration.clone();
            move |_| {
                let replacement = runtime.borrow_mut().replace_adapter_registration_for_test(
                    destination_space,
                    destination_window.into(),
                );
                *replacement_registration.borrow_mut() = Some(replacement);
            }
        });
    live_runtime.terminate_next_same_window_destination_before_semantics_ack_for_test();
    cx.set_next_window_placement_dispatch(destination_window, PlatformWindowDispatch::Queued);
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available_open_desktop(
            point(DevicePixels(1880), DevicePixels(1880)),
            Vec::new(),
        )
        .expect("the moved desktop release observation should be valid"),
    );
    fixture.source_visual.simulate_mouse_up(
        point(px(940.0), px(940.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let drained_without_advancing_deadline = (0..10_000).any(|_| !cx.background_executor.tick());
    assert!(
        drained_without_advancing_deadline,
        "the queued placement must quiesce without consuming the release deadline"
    );
    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .live_undock_phase()),
        crate::surface::live_undock::DockLiveUndockPhase::Bound,
        "a queued placement is not promotion evidence"
    );

    assert!(cx.flush_window_mutation(destination_window, WindowMutationDomain::Placement));
    cx.run_until_parked();

    assert!(
        destination_window.update(cx, |_, _, _| ()).is_err(),
        "the injected terminal must retire the exact promoted provisional window"
    );
    assert_eq!(cx.windows(), vec![fixture.source_window]);
    assert!(
        replacement_registration.borrow().is_some(),
        "the regression must replace the selected live-undock registration before exact settlement"
    );
    {
        let runtime = fixture.runtime.borrow();
        assert_eq!(runtime.adapter().window_for_space(&destination_space), None);
        assert_eq!(
            runtime
                .adapter()
                .space_for_window_id(destination_window.window_id()),
            None,
            "a terminal destination must not retain registry authority"
        );
    }
    cx.read_entity(&fixture.controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&DockSpaceId::from("main")),
            vec![item("b")]
        );
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&destination_space),
            vec![item("a")],
            "post-swap failure must never roll durable topology back"
        );
    });
    cx.read_entity(fixture.surface.owner(), |owner, _| {
        assert_eq!(
            owner.live_undock_phase(),
            crate::surface::live_undock::DockLiveUndockPhase::Idle
        );
        assert_eq!(owner.live_undock_runtime().execution_count_for_test(), 0);
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::LostViewportRecovery,
            ),
            1,
            "the committed payload must remain discoverable after destination terminal"
        );
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::PreCommitOrphan,
            ),
            0,
            "a post-swap loss must not be misclassified as a pre-commit orphan"
        );
    });
    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none(),
        "terminal recovery must release the exact payload drag session"
    );
    cx.read_entity(&fixture.source_host, |host, _| {
        assert!(host.live_presentation_state().is_none());
        assert!(host.live_source_semantic_proxy().is_none());
        assert!(host.native_drag_transport_proxy().is_none());
    });

    let changes = changes.borrow();
    assert_eq!(
        changes.len(),
        3,
        "promotion, stale-selection fallback, and viewport-loss recovery must publish three ordered transactions: {changes:?}"
    );
    assert_eq!(
        changes[0].categories(),
        [
            crate::DockSurfaceChangeCategory::Layout,
            crate::DockSurfaceChangeCategory::Selection,
            crate::DockSurfaceChangeCategory::PanelLifecycle,
            crate::DockSurfaceChangeCategory::ViewportTopology,
            crate::DockSurfaceChangeCategory::ObservedViewportPlacement,
        ]
    );
    assert!(changes[0].transitions().is_empty());
    assert_eq!(
        changes[1].categories(),
        [crate::DockSurfaceChangeCategory::ViewportTopology],
        "stale special-close selection must fall through to one ordinary topology cleanup"
    );
    assert!(changes[1].transitions().is_empty());
    assert_eq!(
        changes[2].categories(),
        [
            crate::DockSurfaceChangeCategory::PanelLifecycle,
            crate::DockSurfaceChangeCategory::ViewportTopology,
        ]
    );
    assert_eq!(
        changes[2].transitions(),
        [crate::DockSurfaceTransition::ViewportLostAfterPromotion]
    );
    assert_eq!(
        cx.read(|app| fixture.surface.revision(app)),
        revision_before_release + 3
    );
}

fn trigger_committed_destination_admission_failure(
    fixture: &mut NativeCapturedSourceFixture,
    cx: &mut TestAppContext,
    reject_recovery: bool,
) -> (AnyWindowHandle, DockSpaceId) {
    begin_native_live_undock_with_released_source(fixture, cx);
    fixture.source_visual = VisualTestContext::from_window(fixture.source_window, cx);
    let (destination_window, _destination_host, destination_space) =
        reveal_live_undock_provisional_destination(fixture, cx);

    let live_runtime = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner.live_undock_runtime()
    });
    live_runtime.reject_next_destination_interaction_admission_for_test();
    if reject_recovery {
        live_runtime.reject_committed_destination_recovery_records_for_test();
    }
    cx.set_next_window_placement_dispatch(destination_window, PlatformWindowDispatch::Queued);
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available_open_desktop(
            point(DevicePixels(1880), DevicePixels(1880)),
            Vec::new(),
        )
        .expect("the moved desktop release observation should be valid"),
    );
    fixture.source_visual.simulate_mouse_up(
        point(px(940.0), px(940.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let drained_without_advancing_deadline = (0..10_000).any(|_| !cx.background_executor.tick());
    assert!(
        drained_without_advancing_deadline,
        "the queued placement must quiesce without consuming the release deadline"
    );
    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .live_undock_phase()),
        crate::surface::live_undock::DockLiveUndockPhase::Bound,
        "a queued placement is not promotion evidence"
    );
    assert!(cx.flush_window_mutation(destination_window, WindowMutationDomain::Placement));
    cx.run_until_parked();
    (destination_window, destination_space)
}

#[open_gpui::test]
fn committed_destination_admission_failure_recovers_before_retiring_its_window(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_source_fixture(cx);
    let (destination_window, destination_space) =
        trigger_committed_destination_admission_failure(&mut fixture, cx, false);

    let (phase, execution_count, recovery_count) =
        cx.read_entity(fixture.surface.owner(), |owner, _| {
            (
                owner.live_undock_phase(),
                owner.live_undock_runtime().execution_count_for_test(),
                owner.visible_payload_recovery_count_for_test(
                    crate::surface::payload_recovery::DockPayloadRecoveryReason::LostViewportRecovery,
                ),
            )
        });
    assert_eq!(
        phase,
        crate::surface::live_undock::DockLiveUndockPhase::Idle,
        "live-undock recovery must settle after exact destination terminal"
    );
    assert_eq!(execution_count, 0);
    assert_eq!(
        recovery_count, 1,
        "the durable payload must remain discoverable after its destination is retired"
    );
    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none(),
        "terminal recovery must release the exact payload drag session"
    );
    cx.read_entity(&fixture.source_host, |host, _| {
        assert!(
            host.live_source_semantic_proxy().is_none(),
            "the recovery record must replace the stale source semantic owner"
        );
        assert!(host.native_drag_transport_proxy().is_none());
    });
    assert!(
        destination_window.update(cx, |_, _, _| ()).is_err(),
        "the promoted provisional destination must reach native terminal"
    );
    cx.read_entity(&fixture.controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&DockSpaceId::from("main")),
            vec![item("b")]
        );
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&destination_space),
            vec![item("a")],
            "the committed destination topology must survive native window retirement",
        );
    });
}

#[open_gpui::test]
fn shutdown_committed_destination_recovery_failure_closes_with_failure_evidence(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_source_fixture(cx);
    let (destination_window, _destination_space) =
        trigger_committed_destination_admission_failure(&mut fixture, cx, true);
    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .live_undock_phase()),
        crate::surface::live_undock::DockLiveUndockPhase::RecoveringCommittedDestination,
        "a permanent recovery rejection must wait for the explicit shutdown failure path"
    );

    let close = cx.simulate_window_close_request(fixture.source_window);
    assert!(!close.native_close_allowed());
    assert!(close.terminal_transition_started());
    cx.run_until_parked();

    let status = cx.update(|app| fixture.surface.window_session_status(app));
    assert_eq!(status.phase(), crate::DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(status.pending_terminal_ticket_count(), 0);
    assert_eq!(status.failed_terminal_ticket_count(), 1);
    assert_eq!(status.runtime_empty(), Some(true));
    assert_eq!(cx.windows().len(), 0);
    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| {
            (
                owner.live_undock_phase(),
                owner.live_undock_runtime().execution_count_for_test(),
                owner.visible_payload_recovery_count_for_test(
                    crate::surface::payload_recovery::DockPayloadRecoveryReason::LostViewportRecovery,
                ),
            )
        }),
        (
            crate::surface::live_undock::DockLiveUndockPhase::ShutdownCleanupFailed,
            0,
            0,
        )
    );
    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none(),
        "failure-terminal shutdown must settle the exact drag finalizer before closing the anchor"
    );
    assert!(
        destination_window.update(cx, |_, _, _| ()).is_err(),
        "failure-terminal shutdown must still retire the provisional destination HWND"
    );
}

#[open_gpui::test]
fn lost_source_host_converges_through_payload_recovery_without_retrying_forever(
    cx: &mut TestAppContext,
) {
    let mut fixture = native_captured_source_fixture(cx);
    begin_native_live_undock_with_released_source(&mut fixture, cx);
    let session = fixture
        .runtime
        .active_payload_drag_session(&fixture.payload)
        .expect("the source should retain its exact live-undock drag session");
    let source_host = fixture.source_host.downgrade();

    fixture
        .source_window
        .update(cx, |_, window, app| {
            window.replace_root(app, |_, _| open_gpui::Empty);
        })
        .expect("the source window should allow replacing its DockHost root");
    drop(fixture.source_host);
    assert!(
        source_host.upgrade().is_none(),
        "the runtime must observe the original source Host as unavailable",
    );

    cx.update(|app| {
        crate::native_captured_drag::cancel_native_captured_drag_route(
            fixture.runtime.identity(),
            Some(&session),
            Some(&fixture.payload),
            &source_host,
            None,
            PointerCancelReason::CaptureRevoked,
            app,
        );
    });
    cx.run_until_parked();

    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .live_undock_phase()),
        crate::surface::live_undock::DockLiveUndockPhase::Idle,
        "source Host authority loss must terminate through orphan recovery, not a timer retry",
    );
    assert!(
        fixture
            .runtime
            .active_payload_drag_session(&fixture.payload)
            .is_none(),
        "orphan recovery must finalize the exact drag generation",
    );
    cx.read_entity(&fixture.controller, |controller, _| {
        assert_eq!(
            controller
                .graph()
                .collect_items_in_space(&DockSpaceId::from("main")),
            vec![item("a"), item("b")],
            "payload recovery must preserve the source topology",
        );
    });

    fixture
        .source_window
        .update(cx, |_, window, app| window.remove_window(app))
        .expect("the recovered surface should remain normally closeable");
    cx.run_until_parked();
    let live_windows = cx
        .windows()
        .into_iter()
        .map(|window| window.window_id())
        .collect::<Vec<_>>();
    let status = cx.update(|app| fixture.surface.window_session_status(app));
    let convergence = (status.phase() != crate::DockSurfaceWindowSessionPhase::Closed).then(|| {
        cx.read_entity(fixture.surface.owner(), |owner, _| {
            let lease = owner
                .window_session()
                .shutting_down_lease()
                .expect("a non-closed status must retain its shutdown lease");
            (
                owner.window_session().pending_terminal_window_ids(lease),
                owner.window_session().has_pending_dependencies(lease),
                owner.live_undock_phase(),
                owner.live_undock_runtime().execution_count_for_test(),
            )
        })
    });
    assert_eq!(
        status.phase(),
        crate::DockSurfaceWindowSessionPhase::Closed,
        "lost-source recovery shutdown must fully converge: {status:?}; convergence={convergence:?}; live_windows={live_windows:?}"
    );
    assert_eq!(status.pending_terminal_ticket_count(), 0);
    assert_eq!(status.runtime_empty(), Some(true));
}

#[open_gpui::test]
fn failed_retired_native_release_is_claimed_by_surface_shutdown(cx: &mut TestAppContext) {
    let mut fixture = native_captured_source_fixture(cx);
    fixture.begin_drag(cx);
    let session = fixture
        .runtime
        .active_payload_drag_session(&fixture.payload)
        .expect("the source should own an exact native drag session");
    let lease = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner
            .window_session()
            .active_lease()
            .expect("the managed surface should expose its exact active lease")
    });
    let release_attempts = Rc::new(Cell::new(0));
    cx.set_pointer_capture_release_callback(fixture.source_window, {
        let release_attempts = release_attempts.clone();
        move |_| {
            release_attempts.set(release_attempts.get() + 1);
            PlatformPointerCaptureReleaseOutcome::Rejected
        }
    });

    cx.update(|app| {
        crate::native_captured_drag::cancel_native_captured_drag_route(
            fixture.runtime.identity(),
            Some(&session),
            Some(&fixture.payload),
            &fixture.source_host.downgrade(),
            None,
            PointerCancelReason::CaptureRevoked,
            app,
        );
    });
    cx.background_executor
        .advance_clock(Duration::from_millis(8));
    cx.background_executor
        .advance_clock(Duration::from_millis(32));
    cx.background_executor
        .advance_clock(Duration::from_millis(128));
    cx.run_until_parked();

    assert_eq!(release_attempts.get(), 4);
    assert!(cx.read(|app| {
        crate::native_captured_drag::has_failed_native_captured_release_for_surface_for_test(
            fixture.runtime.identity(),
            lease,
            app,
        )
    }));

    let panic = catch_unwind(AssertUnwindSafe(|| {
        fixture
            .source_window
            .update(cx, |_, window, app| window.remove_window(app))
            .expect("the source anchor should remain removable after release failure");
        cx.run_until_parked();
    }))
    .expect_err("surface shutdown must report the previously failed native release");
    panic
        .downcast::<crate::surface::DockSurfaceCaptureReleaseFailure>()
        .expect(
            "the persisted release failure must reach the surface coordinator as typed failure",
        );

    cx.run_until_parked();
    assert!(cx.windows().is_empty());
    let status = cx.update(|app| fixture.surface.window_session_status(app));
    assert_eq!(status.phase(), crate::DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(status.pending_terminal_ticket_count(), 0);
    assert_eq!(status.runtime_empty(), Some(true));
}

#[open_gpui::test]
fn reentrant_surface_claim_attaches_to_delivering_native_release(cx: &mut TestAppContext) {
    let mut fixture = native_captured_source_fixture(cx);
    let native_generation = Rc::new(Cell::new(None));
    let _native_observer = cx.update({
        let native_generation = native_generation.clone();
        move |app| {
            app.observe_native_captured_drag(move |event, _| {
                native_generation.set(Some(event.generation()));
            })
        }
    });
    fixture.begin_drag(cx);
    fixture.source_visual.simulate_mouse_move(
        point(fixture.threshold.x + px(1.0), fixture.threshold.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let native_generation = native_generation
        .get()
        .expect("the started native drag should publish its generation");
    let session = fixture
        .runtime
        .active_payload_drag_session(&fixture.payload)
        .expect("the source should own an exact native drag session");
    let lease = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner
            .window_session()
            .active_lease()
            .expect("the managed surface should expose its exact active lease")
    });
    cx.set_pointer_capture_release_callback(fixture.source_window, |_| {
        PlatformPointerCaptureReleaseOutcome::Released
    });
    let release_outcome = Rc::new(Cell::new(None));
    let runtime_identity = fixture.runtime.identity();
    let source_window = fixture.source_window;
    let source_host = fixture.source_host.downgrade();
    let payload = fixture.payload.clone();

    let barrier = cx
        .update({
            let release_outcome = release_outcome.clone();
            move |app| {
                let barrier = app.cancel_native_captured_drag_with_release_barrier(
                    source_window.window_id(),
                    native_generation,
                    PointerCancelReason::WindowClosed,
                    move |_, terminal, app| {
                        assert_eq!(terminal, NativeCapturedDragReleaseTerminal::Released);
                        crate::native_captured_drag::cancel_native_captured_drag_route_for_surface(
                            runtime_identity,
                            lease,
                            move |release, _, _| release_outcome.set(Some(release.outcome())),
                            app,
                        );
                    },
                );
                crate::native_captured_drag::cancel_native_captured_drag_route(
                    runtime_identity,
                    Some(&session),
                    Some(&payload),
                    &source_host,
                    None,
                    PointerCancelReason::CaptureRevoked,
                    app,
                );
                barrier
            }
        })
        .expect("the exact native drag must reserve a release barrier");
    cx.run_until_parked();

    assert_eq!(barrier.source_window(), source_window.window_id());
    assert_eq!(
        release_outcome.get(),
        Some(crate::native_captured_drag::DockNativeCapturedSurfaceReleaseOutcome::Released),
        "a surface claim during completion delivery must observe the real release terminal",
    );
    assert!(cx.read(|app| {
        !crate::native_captured_drag::has_failed_native_captured_release_for_surface_for_test(
            runtime_identity,
            lease,
            app,
        )
    }));

    fixture
        .source_window
        .update(cx, |_, window, app| window.remove_window(app))
        .expect("the source anchor should remain removable after release");
    cx.run_until_parked();
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
    let runtime = surface.viewport_runtime(cx);

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
        PlatformWindowHitStack::try_available_open_desktop(
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
fn runtime_native_captured_live_undock_host_transfer_replays_window_effects_after_panic(
    cx: &mut TestAppContext,
) {
    let source_space = DockSpaceId::from("source-live-host");
    let target_space = DockSpaceId::from("target-live-host");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_tabs);

    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    workspace.policy_mut().set_allow_platform_viewports(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let surface = cx.update(|app| DockSurface::from_controller(controller.clone(), app));
    let source_window = cx.update(|app| {
        match surface.open_primary_window(viewport_window_options(360.0, 220.0), app) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("source primary window should open, got {outcome:?}"),
        }
    });
    cx.run_until_parked();
    let target_window = cx.update(|app| {
        match surface.open_viewport(
            target_space.clone(),
            viewport_window_options(360.0, 220.0),
            app,
        ) {
            crate::DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("target managed viewport should open, got {outcome:?}"),
        }
    });
    let source_host = source_window
        .downcast::<DockHost>()
        .expect("source window should render DockHost")
        .entity(cx)
        .expect("source host should remain live");
    let target_host = target_window
        .downcast::<DockHost>()
        .expect("target window should render DockHost")
        .entity(cx)
        .expect("target host should remain live");
    cx.run_until_parked();
    let mut source_visual = VisualTestContext::from_window(source_window, cx);
    let mut target_visual = VisualTestContext::from_window(target_window, cx);

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
    let target_global = point(px(400.0) + target_local.x, target_local.y);

    configure_native_desktop_release(
        cx,
        source_window.into(),
        size(DevicePixels(720), DevicePixels(440)),
    );
    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    source_visual.simulate_mouse_move(
        point(px(900.0), px(900.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let drained_without_advancing_deadline = (0..10_000).any(|_| !cx.background_executor.tick());
    assert!(
        drained_without_advancing_deadline,
        "the live-undock opening should quiesce before its release deadline"
    );

    assert_eq!(
        cx.read_entity(surface.owner(), |owner, _| owner.live_undock_phase()),
        crate::surface::live_undock::DockLiveUndockPhase::Bound,
        "crossing the desktop must bind the live-undock session before host release",
    );

    configure_native_registered_window_hit(
        cx,
        source_window.into(),
        target_window.into(),
        target_global,
    );
    source_visual.simulate_mouse_move(target_global, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let target_visual = VisualTestContext::from_window(target_window.into(), cx);
    assert!(
        selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview).is_some(),
        "the existing target host must receive the live route preview before release",
    );

    let runtime = cx.read_entity(surface.owner(), |owner, _| owner.runtime());
    let window_effect_attempts = Rc::new(Cell::new(0_u32));
    runtime.install_window_close_apply_hook_for_test({
        let window_effect_attempts = window_effect_attempts.clone();
        move |_| {
            window_effect_attempts.set(window_effect_attempts.get() + 1);
            panic!("injected committed host window-effect panic");
        }
    });
    let panic = catch_unwind(AssertUnwindSafe(|| {
        source_visual.simulate_mouse_up(target_global, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
    }));
    assert!(panic.is_err());
    assert_eq!(window_effect_attempts.get(), 1);
    cx.executor().advance_clock(Duration::from_millis(16));
    cx.run_until_parked();

    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("b")],
            "a valid host release must remove the payload from the source space",
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&target_space),
            vec![item("c"), item("a")],
            "a valid host release must commit into the existing target host",
        );
    });
    let target_visual = VisualTestContext::from_window(target_window.into(), cx);
    assert!(
        selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::Tab {
                tabs: target_tabs,
                item: item("a"),
            },
        )
        .is_some(),
        "the committed payload must render in the existing target host",
    );
    cx.read_entity(&source_host, |host, _| {
        assert!(
            host.live_presentation_state().is_none(),
            "host promotion must retire the source live-presentation authority",
        );
        assert!(
            host.live_source_semantic_proxy().is_none(),
            "host promotion must retire the source semantic proxy",
        );
        assert!(
            host.native_drag_transport_proxy().is_none(),
            "host promotion must retire the source transport proxy",
        );
    });
    cx.read_entity(&target_host, |host, _| {
        assert!(
            host.live_presentation_state().is_none(),
            "an existing Host drop must not leave provisional presentation state behind",
        );
    });
    cx.read_entity(surface.owner(), |owner, _| {
        assert_eq!(
            owner.live_undock_phase(),
            crate::surface::live_undock::DockLiveUndockPhase::Idle,
            "host transfer must settle the live-undock session",
        );
        assert_eq!(
            owner.live_undock_runtime().execution_count_for_test(),
            0,
            "host transfer must retire the runtime execution",
        );
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::LostViewportRecovery,
            ),
            0,
            "a successful Host transfer must not manufacture a recovery record",
        );
    });
    assert_eq!(
        cx.windows().len(),
        2,
        "the provisional window must retire after the existing Host becomes authoritative",
    );
}

#[open_gpui::test]
fn managed_source_host_drop_into_primary_anchor_preserves_surface_shutdown_hook(
    cx: &mut TestAppContext,
) {
    let source_space = DockSpaceId::from("managed-source-live-host");
    let target_space = DockSpaceId::from("primary-target-live-host");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_tabs);

    let mut workspace = DockWorkspace::new(target_space.clone(), graph);
    workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    workspace.policy_mut().set_allow_platform_viewports(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let surface = cx.update(|app| DockSurface::from_controller(controller.clone(), app));
    let target_window = cx.update(|app| {
        match surface.open_primary_window(viewport_window_options(360.0, 220.0), app) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("target primary window should open, got {outcome:?}"),
        }
    });
    cx.run_until_parked();
    let source_window = cx.update(|app| {
        match surface.open_viewport(
            source_space.clone(),
            viewport_window_options(360.0, 220.0),
            app,
        ) {
            crate::DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("managed source viewport should open, got {outcome:?}"),
        }
    });
    let source_host = source_window
        .downcast::<DockHost>()
        .expect("managed source window should render DockHost")
        .entity(cx)
        .expect("managed source host should remain live");
    let target_host = target_window
        .downcast::<DockHost>()
        .expect("primary target window should render DockHost")
        .entity(cx)
        .expect("primary target host should remain live");
    cx.run_until_parked();
    let mut source_visual = VisualTestContext::from_window(source_window, cx);
    let mut target_visual = VisualTestContext::from_window(target_window, cx);

    let source_tab = selector_for(
        &source_visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("managed source tab selector should be emitted");
    let target_tabs_selector = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::Tabs { node: target_tabs },
    )
    .expect("primary target tabs selector should be emitted");
    let start = debug_bounds(&mut source_visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let target_local = debug_bounds(&mut target_visual, &target_tabs_selector).center();
    let target_global = point(px(400.0) + target_local.x, target_local.y);

    configure_native_desktop_release(
        cx,
        source_window.into(),
        size(DevicePixels(720), DevicePixels(440)),
    );
    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    source_visual.simulate_mouse_move(
        point(px(900.0), px(900.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let drained_without_advancing_deadline = (0..10_000).any(|_| !cx.background_executor.tick());
    assert!(
        drained_without_advancing_deadline,
        "the live-undock opening should quiesce before its release deadline"
    );
    assert_eq!(
        cx.read_entity(surface.owner(), |owner, _| owner.live_undock_phase()),
        crate::surface::live_undock::DockLiveUndockPhase::Bound,
        "crossing the desktop must bind the live-undock session before primary-host release",
    );

    configure_native_registered_window_hit(
        cx,
        source_window.into(),
        target_window.into(),
        target_global,
    );
    source_visual.simulate_mouse_move(target_global, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let target_visual = VisualTestContext::from_window(target_window, cx);
    assert!(
        selector_for(&target_visual, &target_host, DockDebugRegion::DropPreview).is_some(),
        "the primary target host must receive the live route preview before release",
    );

    source_visual.simulate_mouse_up(target_global, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("b")],
            "a valid primary-host release must remove the payload from the managed source",
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&target_space),
            vec![item("c"), item("a")],
            "a valid primary-host release must commit into the anchor Host",
        );
    });
    cx.read_entity(surface.owner(), |owner, _| {
        assert_eq!(
            owner.live_undock_phase(),
            crate::surface::live_undock::DockLiveUndockPhase::Idle,
        );
        assert_eq!(owner.live_undock_runtime().execution_count_for_test(), 0);
        assert!(
            owner
                .window_session()
                .active_lease_for_anchor(target_window.window_id())
                .is_some(),
            "the Host transfer must preserve the primary anchor's active surface lease",
        );
    });

    let close = cx.simulate_window_close_request(target_window.into());
    assert!(
        !close.native_close_allowed(),
        "the PrimaryAnchor hook must veto direct native close and own the coordinated shutdown",
    );
    assert!(
        close.terminal_transition_started(),
        "the vetoed native request must still begin the surface-owned terminal transition",
    );
    assert!(
        cx.windows().is_empty(),
        "surface shutdown must retire the managed source before completing anchor teardown",
    );
    let status = cx.update(|app| surface.window_session_status(app));
    assert_eq!(status.phase(), crate::DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(status.pending_terminal_ticket_count(), 0);
    assert_eq!(status.runtime_empty(), Some(true));
}

#[open_gpui::test]
fn runtime_existing_host_binding_loss_after_final_swap_retries_recovery_and_terminates(
    cx: &mut TestAppContext,
) {
    let source_space = DockSpaceId::from("source-live-host-recovery");
    let target_space = DockSpaceId::from("target-live-host-recovery");
    let mut graph = DockGraph::new();
    let source_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    let target_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(target_space.clone(), target_tabs);

    let panel_a = test_view(cx, "A");
    let panel_a_focus = cx.read_entity(&panel_a, |panel, cx| panel.focus_handle(cx));
    let mut workspace = DockWorkspace::new(source_space.clone(), graph);
    workspace.register_focusable_panel_view(item("a"), "Panel A", panel_a);
    workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    workspace.policy_mut().set_allow_platform_viewports(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let surface = cx.update(|app| DockSurface::from_controller(controller.clone(), app));
    let source_window = cx.update(|app| {
        match surface.open_primary_window(viewport_window_options(360.0, 220.0), app) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("source primary window should open, got {outcome:?}"),
        }
    });
    cx.run_until_parked();
    let target_window = cx.update(|app| {
        match surface.open_viewport(
            target_space.clone(),
            viewport_window_options(360.0, 220.0),
            app,
        ) {
            crate::DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("target managed viewport should open, got {outcome:?}"),
        }
    });
    let source_host = source_window
        .downcast::<DockHost>()
        .expect("source window should render DockHost")
        .entity(cx)
        .expect("source host should remain live");
    let target_host = target_window
        .downcast::<DockHost>()
        .expect("target window should render DockHost")
        .entity(cx)
        .expect("target host should remain live");
    cx.run_until_parked();
    let mut source_visual = VisualTestContext::from_window(source_window, cx);
    let target_visual = VisualTestContext::from_window(target_window, cx);

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
    let mut target_visual = target_visual;
    let target_local = debug_bounds(&mut target_visual, &target_tabs_selector).center();
    let target_global = point(px(400.0) + target_local.x, target_local.y);

    configure_native_desktop_release(
        cx,
        source_window.into(),
        size(DevicePixels(720), DevicePixels(440)),
    );
    activate_window_for_pointer_input(&mut source_visual);
    source_visual.update(|window, cx| panel_a_focus.focus(window, cx));
    assert_eq!(
        source_visual.update(|window, cx| window.focused(cx)),
        Some(panel_a_focus.clone()),
        "the live-undock source should capture exact payload focus evidence",
    );
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    source_visual.simulate_mouse_move(
        point(px(900.0), px(900.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let drained_without_advancing_deadline = (0..10_000).any(|_| !cx.background_executor.tick());
    assert!(
        drained_without_advancing_deadline,
        "the live-undock opening should quiesce before its release deadline"
    );

    configure_native_registered_window_hit(
        cx,
        source_window.into(),
        target_window.into(),
        target_global,
    );
    source_visual.simulate_mouse_move(target_global, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let live_runtime = cx.read_entity(surface.owner(), |owner, _| owner.live_undock_runtime());
    let durable_authority_observed = Rc::new(Cell::new(false));
    live_runtime.after_host_drop_commit_for_test({
        let owner = surface.owner().clone();
        let target_host = target_host.clone();
        let durable_authority_observed = durable_authority_observed.clone();
        move |cx| {
            let authority = cx.read_entity(&owner, |owner, _| {
                owner.live_undock_committed_destination_logical_close_authority(
                    target_window.window_id(),
                )
            });
            assert!(matches!(
                authority,
                Some(
                    crate::surface::live_undock_runtime::DockLiveUndockLogicalCloseAuthority::Durable(_)
                )
            ));
            durable_authority_observed.set(true);
            cx.update_entity(&target_host, |host, _| {
                host.invalidate_window_binding_for_test();
            });
            target_window
                .update(cx, |_, window, cx| window.remove_window(cx))
                .expect("the durable Host destination should begin logical close");
        }
    });
    live_runtime.panic_next_committed_destination_recovery_attempt_for_test();

    let panic = catch_unwind(AssertUnwindSafe(|| {
        source_visual.simulate_mouse_up(target_global, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
    }));
    assert!(
        panic.is_err(),
        "the first committed-destination recovery attempt should expose the injected panic"
    );
    cx.executor().advance_clock(Duration::from_millis(16));
    cx.run_until_parked();

    assert!(
        durable_authority_observed.get(),
        "a committed Host drop must publish durable logical-close authority before post-commit callbacks"
    );

    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("b")],
            "committed recovery must not roll the payload back into its source",
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&target_space),
            vec![item("c"), item("a")],
            "committed recovery must preserve the single durable payload location",
        );
    });
    cx.read_entity(surface.owner(), |owner, _| {
        assert_eq!(
            owner.live_undock_phase(),
            crate::surface::live_undock::DockLiveUndockPhase::Idle,
            "Host binding-loss recovery must reach a terminal live-undock phase",
        );
        assert_eq!(
            owner.live_undock_runtime().execution_count_for_test(),
            0,
            "Host recovery must retire its execution graph",
        );
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::LostViewportRecovery,
            ),
            1,
            "the durable payload must remain discoverable through one recovery record",
        );
    });
    let payload = DockDragPayload::new_item(
        source_space.clone(),
        source_tabs,
        item("a"),
        "Panel A".to_owned(),
    );
    assert!(
        cx.read_entity(&source_host, |host, _| host
            .active_payload_drag_session(&payload)
            .is_none()),
        "terminal Host recovery must release the exact payload drag session",
    );
    cx.read_entity(&source_host, |host, _| {
        assert!(host.live_presentation_state().is_none());
        assert!(host.live_source_semantic_proxy().is_none());
        assert!(host.native_drag_transport_proxy().is_none());
    });
    cx.read_entity(&target_host, |host, _| {
        assert!(host.live_presentation_state().is_none());
    });
    assert_eq!(
        cx.windows().len(),
        1,
        "the provisional and terminal target must retire while the source anchor remains",
    );

    assert_eq!(
        cx.read_entity(surface.owner(), |owner, _| owner
            .visible_payload_recovery_entries()
            .len()),
        1,
        "the active anchor should project the durable recovery record before rendering",
    );
    assert_eq!(
        source_host.update(cx, |host, host_cx| host
            .visible_payload_recovery_entries(host_cx)
            .len()),
        1,
        "the primary DockHost should admit the active-anchor recovery projection",
    );
    let recovery_focus = cx.read_entity(surface.owner(), |owner, _| {
        owner
            .visible_payload_recovery_entries()
            .into_iter()
            .next()
            .expect("the visible recovery entry should retain one focus handle")
            .focus_handle()
            .clone()
    });
    assert_eq!(
        source_visual.update(|window, cx| window.focused(cx)),
        Some(recovery_focus),
        "lost focused payload content should hand focus to its visible recovery entry",
    );

    assert!(cx.activate_accessibility(source_window.into()));
    let recovery_tree = cx
        .latest_accessibility_tree_update(source_window.into())
        .expect("the primary anchor should publish a recovery accessibility tree");
    let (recovery_node_id, recovery_node) = recovery_tree
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Lost viewport recovery for Panel A"))
        .map(|(id, node)| (*id, node))
        .unwrap_or_else(|| {
            let labels = recovery_tree
                .nodes
                .iter()
                .filter_map(|(_, node)| node.label())
                .collect::<Vec<_>>();
            panic!(
                "the primary anchor should expose one visible payload recovery entry; labels={labels:?}"
            );
        });
    assert_eq!(recovery_node.role(), open_gpui::accesskit::Role::Group);
    let recovery_actions = open_gpui::test::ACCESSKIT_ACTIONS
        .iter()
        .copied()
        .filter(|action| recovery_node.supports_action(*action))
        .collect::<Vec<_>>();
    assert_eq!(
        recovery_actions,
        vec![open_gpui::AccessibleAction::Click],
        "the recovery group should expose exactly one semantic Restore action",
    );
    let mut recovery_descendants = recovery_node.children().to_vec();
    while let Some(descendant_id) = recovery_descendants.pop() {
        let descendant = recovery_tree
            .nodes
            .iter()
            .find_map(|(id, node)| (*id == descendant_id).then_some(node))
            .expect("every recovery descendant should exist in the same committed tree");
        assert_ne!(
            descendant.role(),
            open_gpui::accesskit::Role::TabPanel,
            "the recovery group must not own a duplicate payload tab-panel subtree",
        );
        assert_ne!(
            descendant.label(),
            Some("Panel A panel"),
            "the recovery group must not own the lost payload panel",
        );
        recovery_descendants.extend_from_slice(descendant.children());
    }
    assert!(
        recovery_tree
            .nodes
            .iter()
            .all(|(_, node)| node.label() != Some("Panel A panel")),
        "the recovery entry must not mount a second payload panel subtree",
    );

    assert!(cx.dispatch_accessibility_action(
        source_window.into(),
        open_gpui::accesskit::ActionRequest {
            action: open_gpui::AccessibleAction::Click,
            target_tree: open_gpui::accesskit::TreeId::ROOT,
            target_node: recovery_node_id,
            data: None,
        },
    ));
    cx.run_until_parked();

    cx.read_entity(&controller, |controller, _| {
        assert_eq!(
            controller.graph().collect_items_in_space(&source_space),
            vec![item("b"), item("a")],
            "Restore should re-home the payload into the primary recovery tab group",
        );
        assert_eq!(
            controller.graph().collect_items_in_space(&target_space),
            vec![item("c")],
            "Restore should remove the payload from the lost destination exactly once",
        );
    });
    cx.read_entity(surface.owner(), |owner, _| {
        assert_eq!(
            owner.visible_payload_recovery_count_for_test(
                crate::surface::payload_recovery::DockPayloadRecoveryReason::LostViewportRecovery,
            ),
            0,
            "the exact recovery action should consume its durable record",
        );
    });
    let restored_focus_state = source_visual.update(|window, cx| {
        (
            window.focused(cx),
            window.is_focus_handle_rendered(&panel_a_focus),
        )
    });
    assert_eq!(
        restored_focus_state.0,
        Some(panel_a_focus),
        "Restore should return focus to the exact surviving payload focus handle; rendered={} ",
        restored_focus_state.1,
    );
    assert!(
        restored_focus_state.1,
        "the restored focus handle must belong to a panel root rendered by the primary Host",
    );
    assert!(
        selector_for(
            &source_visual,
            &source_host,
            DockDebugRegion::Panel { item: item("a") },
        )
        .is_some(),
        "the primary Host must render Panel A after recovery commits",
    );
    assert!(
        selector_for(
            &target_visual,
            &target_host,
            DockDebugRegion::Panel { item: item("a") },
        )
        .is_none(),
        "the former target Host must retire Panel A after recovery commits",
    );
    let restored_tree = cx
        .latest_accessibility_tree_update(source_window.into())
        .expect("Restore should publish a replacement accessibility tree");
    assert!(
        restored_tree
            .nodes
            .iter()
            .all(|(_, node)| node.label() != Some("Lost viewport recovery for Panel A")),
        "a consumed recovery action must remove its visible entry",
    );
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

struct PayloadRecoveryHostFixture {
    surface: DockSurface,
    payload: DockLockedPayloadIdentity,
    lost_space: DockSpaceId,
    lost_tabs: DockNodeId,
    primary_window: AnyWindowHandle,
    primary_host: Entity<DockHost>,
    source_window: AnyWindowHandle,
    source_host: Entity<DockHost>,
    panel_a: Entity<TestPanel>,
    panel_b: Entity<TestPanel>,
}

fn payload_recovery_host_fixture(cx: &mut TestAppContext) -> PayloadRecoveryHostFixture {
    let original_space = DockSpaceId::from("payload-recovery-original");
    let lost_space = DockSpaceId::from("payload-recovery-lost");
    let primary_space = DockSpaceId::from("payload-recovery-primary");
    let mut graph = DockGraph::new();
    let original_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    let primary_tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("home")],
        selected: Some(item("home")),
    });
    graph.set_root(original_space.clone(), original_tabs);
    graph.set_root(primary_space.clone(), primary_tabs);
    let payload = DockLockedPayloadIdentity::capture(
        &graph,
        &original_space,
        DockWorkspaceDropPayload::Tabs {
            source_tabs: original_tabs,
        },
    )
    .expect("the tabs payload should lock before promotion");
    graph
        .apply_op_checked(&DockOp::MoveTabs {
            source_space: original_space,
            source_tabs: original_tabs,
            target_space: lost_space.clone(),
            target: DockGraphDropTarget::empty_space(),
        })
        .expect("the tabs payload should move into the lost viewport");
    let lost_tabs = graph
        .root(&lost_space)
        .expect("the lost viewport should receive one tabs root");

    let panel_a = test_view(cx, "A");
    let panel_b = test_view(cx, "B");
    let mut workspace = DockWorkspace::new(primary_space, graph);
    workspace.register_panel_view(item("a"), "Panel A", panel_a.clone());
    workspace.register_panel_view(item("b"), "Panel B", panel_b.clone());
    workspace.register_panel_view(item("home"), "Home", test_view(cx, "Home"));
    workspace.policy_mut().set_allow_platform_viewports(true);
    let controller = cx.new(|_| DockController::new(workspace));
    let surface = cx.update(|app| DockSurface::from_controller(controller, app));
    let primary_window = cx.update(|app| {
        match surface.open_primary_window(viewport_window_options(360.0, 220.0), app) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the recovery primary window should open, got {outcome:?}"),
        }
    });
    cx.run_until_parked();
    let source_window = cx.update(|app| {
        match surface.open_viewport(
            lost_space.clone(),
            viewport_window_options(360.0, 220.0),
            app,
        ) {
            crate::DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the recovery source viewport should open, got {outcome:?}"),
        }
    });
    cx.run_until_parked();

    let primary_host = primary_window
        .downcast::<DockHost>()
        .expect("the recovery primary should render DockHost")
        .entity(cx)
        .expect("the recovery primary Host should remain live");
    let source_host = source_window
        .downcast::<DockHost>()
        .expect("the recovery source should render DockHost")
        .entity(cx)
        .expect("the recovery source Host should remain live");
    let initial_leases = cx.read(|app| {
        (
            open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                panel_a.entity_id(),
                source_window.window_id(),
            ),
            open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                panel_b.entity_id(),
                source_window.window_id(),
            ),
            open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                panel_b.entity_id(),
                primary_window.window_id(),
            ),
        )
    });
    assert!(
        initial_leases.0.is_some(),
        "the selected source root must own stable presentation authority"
    );
    assert_eq!(
        initial_leases.1, None,
        "the hidden source root must remain ungoverned"
    );
    assert_eq!(
        initial_leases.2, None,
        "the hidden root must not leak into the recovery destination"
    );

    PayloadRecoveryHostFixture {
        surface,
        payload,
        lost_space,
        lost_tabs,
        primary_window,
        primary_host,
        source_window,
        source_host,
        panel_a,
        panel_b,
    }
}

struct PayloadRecoveryRestoreStart {
    owner: Entity<DockSurfaceOwner>,
    primary_window: WindowHandle<DockHost>,
    primary_binding: DockHostWindowBinding,
    action: DockPayloadRecoveryRestoreAction,
}

fn prepare_payload_recovery_host_restore(
    fixture: &PayloadRecoveryHostFixture,
    cx: &mut TestAppContext,
) -> PayloadRecoveryRestoreStart {
    let source_window = fixture
        .source_window
        .downcast::<DockHost>()
        .expect("the recovery source should retain its typed window");
    let (source_binding, source_registration) = cx.read_entity(&fixture.source_host, |host, _| {
        (
            host.current_window_binding()
                .expect("the recovery source should retain its window binding"),
            host.current_viewport_registration()
                .expect("the recovery source should retain its viewport registration"),
        )
    });
    let origin = DockPayloadRecoveryPresentationOrigin::new(
        source_window,
        source_binding,
        source_registration,
    )
    .expect("the recovery origin should bind one exact source endpoint");
    prepare_payload_recovery_host_restore_with_origin(fixture, origin, cx)
}

fn prepare_payload_recovery_host_restore_with_origin(
    fixture: &PayloadRecoveryHostFixture,
    origin: DockPayloadRecoveryPresentationOrigin,
    cx: &mut TestAppContext,
) -> PayloadRecoveryRestoreStart {
    let source_window = fixture
        .source_window
        .downcast::<DockHost>()
        .expect("the recovery source should retain its typed window");
    let source_binding = cx.read_entity(&fixture.source_host, |host, _| {
        host.current_window_binding()
            .expect("the recovery source should retain its window binding")
    });
    let anchor_lease = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner
            .window_session()
            .active_lease()
            .expect("the recovery destination must own the active surface lease")
    });
    let trigger = DockLiveUndockTrigger::new(
        DockLiveUndockDragGeneration::new(1)
            .expect("the synthetic recovery drag generation should be non-zero"),
        DockLiveUndockSourceSnapshot::new(source_window.window_id(), source_binding.generation()),
        DockLiveUndockRouteGeneration::new(1)
            .expect("the synthetic recovery route generation should be non-zero"),
        DockLiveUndockRouteFeedback::Desktop,
        DockLiveUndockPhysicalPoint::new(50, 50),
        DockLiveUndockPhysicalBounds::new(DockLiveUndockPhysicalPoint::new(0, 0), 640, 480)
            .expect("synthetic recovery bounds must be non-empty"),
    )
    .expect("the synthetic recovery trigger should be valid");
    let identity = DockLiveUndockSession::new()
        .apply(DockLiveUndockFact::Trigger {
            lease: anchor_lease,
            trigger,
        })
        .into_iter()
        .find_map(|effect| match effect {
            DockLiveUndockEffect::OpenProvisional { identity, .. } => Some(identity),
            _ => None,
        })
        .expect("the synthetic trigger should mint one live-undock identity");
    let authority = DockPayloadRecoveryAuthority::committed_destination(
        identity,
        DockLiveUndockPromotionToken::new(1)
            .expect("the synthetic promotion token should be non-zero"),
        DockLiveUndockPromotionDestination::SameWindowDesktop {
            window_id: origin.window().window_id(),
        },
    );
    let owner = fixture.surface.owner().clone();
    let recovery = with_root_transaction(&owner, cx, |owner, transaction, owner_cx| {
        let prepared = owner
            .prepare_payload_recovery_with_origin(
                authority,
                &fixture.payload,
                DockPayloadRecoveryReason::LostViewportRecovery,
                origin,
                owner_cx,
            )
            .expect("the lost payload should prepare one durable recovery record");
        owner
            .commit_payload_recovery(transaction, &prepared, owner_cx)
            .expect("the lost payload should commit one durable recovery record")
    });
    let action = cx.read_entity(&owner, |owner, _| {
        owner
            .payload_recovery_restore_action(recovery)
            .expect("the active anchor should expose one exact restore action")
    });
    let primary_window = fixture
        .primary_window
        .downcast::<DockHost>()
        .expect("the recovery destination should retain its typed window");
    let primary_binding = cx.read_entity(&fixture.primary_host, |host, _| {
        host.current_window_binding()
            .expect("the recovery destination should retain its window binding")
    });
    PayloadRecoveryRestoreStart {
        owner,
        primary_window,
        primary_binding,
        action,
    }
}

fn start_payload_recovery_host_restore(
    fixture: &PayloadRecoveryHostFixture,
    cx: &mut TestAppContext,
) {
    let start = prepare_payload_recovery_host_restore(fixture, cx);
    cx.update(|app| {
        crate::surface::payload_recovery_executor::start_payload_recovery_restore(
            start.owner.clone(),
            fixture.primary_host.downgrade(),
            start.primary_window,
            start.primary_binding,
            start.action,
            app,
        )
    })
    .expect("the recovery executor should accept the exact source and destination endpoints");
}

fn provider_terminal_payload_recovery_origin(
    fixture: &PayloadRecoveryHostFixture,
    cx: &mut TestAppContext,
) -> (
    AnyWindowHandle,
    open_gpui::view_presentation_window::LeaseBatch,
    DockPayloadRecoveryPresentationOrigin,
) {
    let endpoint: AnyWindowHandle = cx
        .open_window(size(px(240.0), px(160.0)), |_, _| open_gpui::Empty)
        .into();
    let roots = [
        AnyView::from(fixture.panel_a.clone()),
        AnyView::from(fixture.panel_b.clone()),
    ];
    let leases = cx.update(|app| {
        let source_leases = roots
            .iter()
            .filter_map(|root| {
                open_gpui::view_presentation_window::stable_lease_for_window(
                    app,
                    root.entity_id(),
                    fixture.source_window.window_id(),
                )
            })
            .collect::<Vec<_>>();
        open_gpui::view_presentation_window::release_stable_leases_after_endpoint_loss(
            app,
            &source_leases,
        );
        open_gpui::view_presentation_window::claim_batch(app, &roots, endpoint.window_id())
            .expect("the synthetic provider endpoint should claim the exact recovered roots")
    });
    let origin = DockPayloadRecoveryPresentationOrigin::provider_terminal(
        WindowHandle::<DockHost>::new(endpoint.window_id()),
        leases.clone(),
    )
    .expect("the provider terminal must identify its exact lease window");
    (endpoint, leases, origin)
}

#[open_gpui::test]
fn provider_terminal_payload_recovery_rejects_a_live_old_endpoint(cx: &mut TestAppContext) {
    let fixture = payload_recovery_host_fixture(cx);
    let (endpoint, leases, origin) = provider_terminal_payload_recovery_origin(&fixture, cx);
    let start = prepare_payload_recovery_host_restore_with_origin(&fixture, origin, cx);

    let result = cx.update(|app| {
        crate::surface::payload_recovery_executor::start_payload_recovery_restore(
            start.owner.clone(),
            fixture.primary_host.downgrade(),
            start.primary_window,
            start.primary_binding,
            start.action,
            app,
        )
    });

    assert_eq!(
        result,
        Err(DockPayloadRecoveryRestoreError::PresentationEndpointUnavailable)
    );
    assert!(endpoint.update(cx, |_, _, _| ()).is_ok());
    assert!(cx.read(|app| {
        leases.leases().iter().all(|lease| {
            open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                lease.entity_id(),
                endpoint.window_id(),
            ) == Some(*lease)
        })
    }));
    assert_eq!(
        cx.read_entity(&start.owner, |owner, _| owner
            .visible_payload_recovery_count_for_test(
                DockPayloadRecoveryReason::LostViewportRecovery,
            )),
        1,
        "a live old endpoint must leave the exact recovery action retryable"
    );
}

#[open_gpui::test]
fn provider_terminal_payload_recovery_preserves_third_window_replacement_authority(
    cx: &mut TestAppContext,
) {
    let fixture = payload_recovery_host_fixture(cx);
    let (endpoint, _leases, origin) = provider_terminal_payload_recovery_origin(&fixture, cx);
    let start = prepare_payload_recovery_host_restore_with_origin(&fixture, origin, cx);

    fixture
        .source_window
        .update(cx, |_, window, app| {
            window.replace_root(app, |_, _| open_gpui::Empty);
        })
        .expect("the original Host should stop rendering before the provider endpoint closes");
    endpoint
        .update(cx, |_, window, app| window.remove_window(app))
        .expect("the synthetic provider endpoint should close");
    cx.run_until_parked();
    assert!(endpoint.update(cx, |_, _, _| ()).is_err());

    let replacement: AnyWindowHandle = cx
        .open_window(size(px(240.0), px(160.0)), |_, _| open_gpui::Empty)
        .into();
    let replacement_root = AnyView::from(fixture.panel_a.clone());
    let replacement_lease = cx.update(|app| {
        open_gpui::view_presentation_window::claim(app, &replacement_root, replacement.window_id())
            .expect("the third window should claim a replacement presentation generation")
    });

    let result = cx.update(|app| {
        crate::surface::payload_recovery_executor::start_payload_recovery_restore(
            start.owner.clone(),
            fixture.primary_host.downgrade(),
            start.primary_window,
            start.primary_binding,
            start.action,
            app,
        )
    });

    assert!(matches!(
        result,
        Err(DockPayloadRecoveryRestoreError::PresentationPrepare(
            open_gpui::view_presentation_window::ResolvedViewRehostError::UnexpectedWindow {
                current,
            },
        )) if current == replacement_lease
    ));
    assert_eq!(
        cx.read(|app| {
            open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                fixture.panel_a.entity_id(),
                replacement.window_id(),
            )
        }),
        Some(replacement_lease),
        "exact old-lease cleanup must not release a replacement generation"
    );
    assert!(cx.read(|app| {
        open_gpui::view_presentation_window::stable_lease_for_window(
            app,
            fixture.panel_a.entity_id(),
            fixture.primary_window.window_id(),
        )
        .is_none()
    }));
    assert_eq!(
        cx.read_entity(&start.owner, |owner, _| owner
            .payload_recovery_committed_restore_count_for_test()),
        0,
        "third-window authority must prevent the no-rehost recovery commit"
    );
}

#[open_gpui::test]
fn payload_recovery_finalization_resumes_after_each_committed_stage_panics(
    cx: &mut TestAppContext,
) {
    use crate::surface::payload_recovery_executor::DockPayloadRecoveryFinalizationPanicStage;

    for stage in [
        DockPayloadRecoveryFinalizationPanicStage::Provider,
        DockPayloadRecoveryFinalizationPanicStage::SourceHost,
        DockPayloadRecoveryFinalizationPanicStage::DestinationHost,
        DockPayloadRecoveryFinalizationPanicStage::Owner,
    ] {
        let fixture = payload_recovery_host_fixture(cx);
        let owner = fixture.surface.owner().clone();
        cx.update_entity(&owner, |owner, _| {
            owner.pause_before_payload_recovery_finalization_once_for_test();
            owner.panic_after_payload_recovery_finalization_stage_once_for_test(stage);
        });
        start_payload_recovery_host_restore(&fixture, cx);
        cx.run_until_parked();
        let execution = cx
            .read_entity(&owner, |owner, _| {
                owner.payload_recovery_execution_snapshot_for_test()
            })
            .expect("the paused finalization must retain its execution")
            .0;

        cx.update(|app| {
            crate::surface::payload_recovery_executor::resume_payload_recovery_finalization_for_test(
                owner.clone(),
                execution,
                app,
            );
        });
        cx.run_until_parked();
        assert!(
            cx.read_entity(&owner, |owner, _| owner
                .payload_recovery_execution_snapshot_for_test())
                .is_none(),
            "the retained continuation must finish after {stage:?} retry"
        );
        assert_eq!(
            cx.read_entity(&owner, |owner, _| owner
                .visible_payload_recovery_count_for_test(
                    DockPayloadRecoveryReason::LostViewportRecovery,
                )),
            0,
            "the exact durable recovery record must be consumed after {stage:?} retry"
        );
        assert_eq!(
            cx.read_entity(&owner, |owner, _| owner
                .payload_recovery_committed_restore_count_for_test()),
            0,
            "the finalization journal must retire its registry tombstone after {stage:?}"
        );
        assert!(
            cx.read(|app| {
                open_gpui::view_presentation_window::stable_lease_for_window(
                    app,
                    fixture.panel_a.entity_id(),
                    fixture.primary_window.window_id(),
                )
            })
            .is_some(),
            "the destination must retain stable presentation after {stage:?} retry"
        );
    }
}

#[open_gpui::test]
fn payload_recovery_finalization_exhausts_persistent_panic_without_starving_effects(
    cx: &mut TestAppContext,
) {
    use crate::surface::payload_recovery_executor::DockPayloadRecoveryFinalizationPanicStage;

    let fixture = payload_recovery_host_fixture(cx);
    let start = prepare_payload_recovery_host_restore(&fixture, cx);
    cx.update_entity(&start.owner, |owner, _| {
        owner.panic_after_payload_recovery_finalization_stage_for_test(
            DockPayloadRecoveryFinalizationPanicStage::Provider,
            2,
        );
    });

    cx.update(|app| {
        crate::surface::payload_recovery_executor::start_payload_recovery_restore(
            start.owner.clone(),
            fixture.primary_host.downgrade(),
            start.primary_window,
            start.primary_binding,
            start.action,
            app,
        )
    })
    .expect("the recovery executor should admit the persistent-panic sequence");
    cx.run_until_parked();

    assert!(
        cx.read_entity(&start.owner, |owner, _| owner
            .payload_recovery_execution_snapshot_for_test())
            .is_none(),
        "retry exhaustion must retire the terminal executor instead of self-deferring forever"
    );
    assert_eq!(
        cx.read_entity(&start.owner, |owner, _| owner
            .visible_payload_recovery_count_for_test(
                DockPayloadRecoveryReason::LostViewportRecovery,
            )),
        1,
        "a provider-only terminal must preserve the durable recovery action"
    );
    assert_eq!(
        cx.read_entity(&start.owner, |owner, _| owner
            .payload_recovery_committed_restore_count_for_test()),
        0
    );
    assert!(cx.read(|app| {
        open_gpui::view_presentation_window::stable_lease_for_window(
            app,
            fixture.panel_a.entity_id(),
            fixture.primary_window.window_id(),
        )
        .is_none()
    }));

    cx.update(|app| {
        crate::surface::payload_recovery_executor::start_payload_recovery_restore(
            start.owner,
            fixture.primary_host.downgrade(),
            start.primary_window,
            start.primary_binding,
            start.action,
            app,
        )
    })
    .expect("terminal compensation must leave the durable recovery immediately retryable");
    cx.run_until_parked();
    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .visible_payload_recovery_count_for_test(
                DockPayloadRecoveryReason::LostViewportRecovery,
            )),
        0
    );
    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .payload_recovery_committed_restore_count_for_test()),
        0
    );
}

#[open_gpui::test]
fn payload_recovery_finalization_uses_source_checkpoint_after_source_host_rebind(
    cx: &mut TestAppContext,
) {
    use crate::surface::payload_recovery_executor::DockPayloadRecoveryFinalizationPanicStage;

    let fixture = payload_recovery_host_fixture(cx);
    let owner = fixture.surface.owner().clone();
    cx.update_entity(&owner, |owner, _| {
        owner.pause_before_payload_recovery_finalization_once_for_test();
        owner.pause_payload_recovery_finalization_retry_once_for_test();
        owner.panic_after_payload_recovery_finalization_stage_once_for_test(
            DockPayloadRecoveryFinalizationPanicStage::SourceHost,
        );
    });
    start_payload_recovery_host_restore(&fixture, cx);
    cx.run_until_parked();
    let execution = cx
        .read_entity(&owner, |owner, _| {
            owner.payload_recovery_execution_snapshot_for_test()
        })
        .expect("the paused finalization must retain its execution")
        .0;

    cx.update(|app| {
        crate::surface::payload_recovery_executor::resume_payload_recovery_finalization_for_test(
            owner.clone(),
            execution,
            app,
        );
    });
    assert!(
        cx.read_entity(&owner, |owner, _| owner
            .payload_recovery_execution_snapshot_for_test())
            .is_some(),
        "the test retry pause must retain the committed source checkpoint"
    );
    fixture
        .source_window
        .update(cx, |_, window, app| {
            window.replace_root(app, |_, _| open_gpui::Empty);
        })
        .expect("the source Host should be replaceable between finalization attempts");

    cx.update(|app| {
        crate::surface::payload_recovery_executor::resume_payload_recovery_finalization_for_test(
            owner.clone(),
            execution,
            app,
        );
    });
    cx.run_until_parked();
    assert!(
        cx.read_entity(&owner, |owner, _| owner
            .payload_recovery_execution_snapshot_for_test())
            .is_none()
    );
    assert_eq!(
        cx.read_entity(&owner, |owner, _| owner
            .visible_payload_recovery_count_for_test(
                DockPayloadRecoveryReason::LostViewportRecovery,
            )),
        0
    );
    assert!(cx.read(|app| {
        open_gpui::view_presentation_window::stable_lease_for_window(
            app,
            fixture.panel_a.entity_id(),
            fixture.primary_window.window_id(),
        )
        .is_some()
    }));
}

#[open_gpui::test]
fn payload_recovery_finalization_bounds_destination_loss_before_owner_commit(
    cx: &mut TestAppContext,
) {
    use crate::surface::payload_recovery_executor::DockPayloadRecoveryFinalizationPanicStage;

    let fixture = payload_recovery_host_fixture(cx);
    let owner = fixture.surface.owner().clone();
    cx.update_entity(&owner, |owner, _| {
        owner.pause_before_payload_recovery_finalization_once_for_test();
        owner.pause_payload_recovery_finalization_retry_once_for_test();
        owner.panic_after_payload_recovery_finalization_stage_once_for_test(
            DockPayloadRecoveryFinalizationPanicStage::DestinationHost,
        );
    });
    start_payload_recovery_host_restore(&fixture, cx);
    cx.run_until_parked();
    let execution = cx
        .read_entity(&owner, |owner, _| {
            owner.payload_recovery_execution_snapshot_for_test()
        })
        .expect("the paused finalization must retain its execution")
        .0;

    cx.update(|app| {
        crate::surface::payload_recovery_executor::resume_payload_recovery_finalization_for_test(
            owner.clone(),
            execution,
            app,
        );
    });
    fixture
        .primary_window
        .update(cx, |_, window, app| {
            window.replace_root(app, |_, _| open_gpui::Empty);
        })
        .expect("the destination Host should be replaceable between finalization attempts");

    cx.update(|app| {
        crate::surface::payload_recovery_executor::resume_payload_recovery_finalization_for_test(
            owner.clone(),
            execution,
            app,
        );
    });
    cx.run_until_parked();
    assert!(
        cx.read_entity(&owner, |owner, _| owner
            .payload_recovery_execution_snapshot_for_test())
            .is_none(),
        "destination loss must retire the terminal executor after its retry budget"
    );
    assert_eq!(
        cx.read_entity(&owner, |owner, _| owner
            .visible_payload_recovery_count_for_test(
                DockPayloadRecoveryReason::LostViewportRecovery,
            )),
        1,
        "destination loss before owner commit must preserve the durable recovery action"
    );
    assert_eq!(
        cx.read_entity(&owner, |owner, _| owner
            .payload_recovery_committed_restore_count_for_test()),
        0
    );
    assert!(cx.read(|app| {
        open_gpui::view_presentation_window::stable_lease_for_window(
            app,
            fixture.panel_a.entity_id(),
            fixture.primary_window.window_id(),
        )
        .is_none()
    }));
}

#[open_gpui::test]
fn payload_recovery_installation_never_drops_rehost_authority_during_unwind(
    cx: &mut TestAppContext,
) {
    use crate::surface::payload_recovery_executor::DockPayloadRecoveryInstallationPanicStage;

    for stage in [
        DockPayloadRecoveryInstallationPanicStage::PreparedSession,
        DockPayloadRecoveryInstallationPanicStage::DestinationHost,
        DockPayloadRecoveryInstallationPanicStage::SourceHost,
        DockPayloadRecoveryInstallationPanicStage::Executor,
    ] {
        let fixture = payload_recovery_host_fixture(cx);
        let start = prepare_payload_recovery_host_restore(&fixture, cx);
        cx.update_entity(&start.owner, |owner, _| {
            owner.panic_after_payload_recovery_installation_stage_once_for_test(stage);
        });

        let start_result = catch_unwind(AssertUnwindSafe(|| {
            cx.update(|app| {
                crate::surface::payload_recovery_executor::start_payload_recovery_restore(
                    start.owner.clone(),
                    fixture.primary_host.downgrade(),
                    start.primary_window,
                    start.primary_binding,
                    start.action,
                    app,
                )
            })
        }));

        if stage == DockPayloadRecoveryInstallationPanicStage::Executor {
            assert!(
                start_result
                    .expect("executor-owned installation panic must be contained")
                    .is_ok(),
                "executor admission owns enough state to continue after unwind"
            );
        } else {
            assert!(
                start_result.is_err(),
                "pre-admission {stage:?} panic must propagate after compensation"
            );
            cx.run_until_parked();
            assert!(
                cx.read_entity(&start.owner, |owner, _| owner
                    .payload_recovery_execution_snapshot_for_test())
                    .is_none(),
                "compensation must retire the reservation after {stage:?}"
            );
            assert!(cx.read_entity(&fixture.source_host, |host, _| {
                host.payload_recovery_presentation_state().is_none()
            }));
            assert!(cx.read_entity(&fixture.primary_host, |host, _| {
                host.payload_recovery_presentation_state().is_none()
            }));
            cx.update(|app| {
                crate::surface::payload_recovery_executor::start_payload_recovery_restore(
                    start.owner.clone(),
                    fixture.primary_host.downgrade(),
                    start.primary_window,
                    start.primary_binding,
                    start.action,
                    app,
                )
            })
            .expect("a compensated installation must be immediately retryable");
        }

        cx.run_until_parked();
        assert!(
            cx.read_entity(&start.owner, |owner, _| owner
                .payload_recovery_execution_snapshot_for_test())
                .is_none(),
            "the recovered installation must converge after {stage:?}"
        );
        assert_eq!(
            cx.read_entity(&start.owner, |owner, _| owner
                .visible_payload_recovery_count_for_test(
                    DockPayloadRecoveryReason::LostViewportRecovery,
                )),
            0
        );
    }
}

#[open_gpui::test]
fn payload_recovery_source_close_before_release_waits_for_native_terminal(cx: &mut TestAppContext) {
    let fixture = payload_recovery_host_fixture(cx);
    let start = prepare_payload_recovery_host_restore(&fixture, cx);
    let terminal = cx.hold_window_native_terminal(fixture.source_window);
    let initial = cx.update(|app| {
        crate::surface::payload_recovery_executor::start_payload_recovery_restore(
            start.owner,
            fixture.primary_host.downgrade(),
            start.primary_window,
            start.primary_binding,
            start.action,
            app,
        )
        .expect("the recovery executor should accept the exact endpoints");
        let initial = app
            .read_entity(fixture.surface.owner(), |owner, _| {
                owner.payload_recovery_execution_snapshot_for_test()
            })
            .expect("the recovery executor should retain one transfer");
        fixture
            .source_window
            .update(app, |_, window, app| window.remove_window(app))
            .expect("the recovery source should begin logical close");
        initial
    });
    assert_eq!(
        initial.1,
        open_gpui::view_presentation_window::RehostPhase::AwaitingSourceRelease
    );
    assert!(!initial.2);
    assert_eq!(initial.3, fixture.source_window.window_id());
    assert_eq!(initial.4, fixture.primary_window.window_id());

    cx.run_until_parked();

    let pending = cx
        .read_entity(fixture.surface.owner(), |owner, _| {
            owner.payload_recovery_execution_snapshot_for_test()
        })
        .expect("logical close must not retire recovery before native terminal");
    assert_eq!(pending.0, initial.0);
    assert_eq!(
        pending.1,
        open_gpui::view_presentation_window::RehostPhase::Invalidated
    );
    assert!(!pending.2);
    assert!(pending.5);
    let prepared = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner
            .payload_recovery_transfer(pending.0)
            .expect("the native-terminal barrier should retain the transfer")
            .projection()
            .clone()
    });

    assert!(terminal.release());
    cx.run_until_parked();
    assert!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .payload_recovery_execution_snapshot_for_test())
            .is_none(),
        "the exact native terminal should retire the recovery execution"
    );
    assert!(cx.read(|app| prepared.authority_is_retired(app)));
    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .visible_payload_recovery_count_for_test(
                DockPayloadRecoveryReason::LostViewportRecovery,
            )),
        1,
        "native source loss must preserve the durable recovery entry"
    );
    assert!(cx.read_entity(&fixture.primary_host, |host, _| {
        host.payload_recovery_presentation_state().is_none()
    }));
}

#[open_gpui::test]
fn payload_recovery_endpoint_loss_releases_stable_authority_before_restore(
    cx: &mut TestAppContext,
) {
    let fixture = payload_recovery_host_fixture(cx);
    let start = prepare_payload_recovery_host_restore(&fixture, cx);
    let owner = fixture.surface.owner().clone();
    let source_window = fixture.source_window;
    let source_host = fixture.source_host.downgrade();

    drop(fixture.source_host);
    source_window
        .update(cx, |_, window, app| {
            window.replace_root(app, |_, _| open_gpui::Empty);
        })
        .expect("the live source window should allow replacing its DockHost root");
    cx.run_until_parked();
    assert!(source_host.upgrade().is_none());
    assert!(
        cx.read(|app| {
            open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                fixture.panel_a.entity_id(),
                source_window.window_id(),
            )
        })
        .is_some(),
        "recovery admission must observe the exact stale source authority it will release"
    );

    cx.update(|app| {
        crate::surface::payload_recovery_executor::start_payload_recovery_restore(
            start.owner.clone(),
            fixture.primary_host.downgrade(),
            start.primary_window,
            start.primary_binding,
            start.action,
            app,
        )
    })
    .expect("endpoint cleanup must make the durable recovery immediately restorable");
    cx.run_until_parked();
    assert_eq!(
        cx.read_entity(&owner, |owner, _| owner
            .visible_payload_recovery_count_for_test(
                DockPayloadRecoveryReason::LostViewportRecovery,
            )),
        0
    );
    assert!(
        cx.read(|app| {
            open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                fixture.panel_a.entity_id(),
                source_window.window_id(),
            )
        })
        .is_none(),
        "the first recovery attempt must release stale source authority"
    );
    assert!(
        cx.read(|app| {
            open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                fixture.panel_a.entity_id(),
                fixture.primary_window.window_id(),
            )
        })
        .is_some()
    );
}

#[open_gpui::test]
fn payload_recovery_source_install_rejection_compensates_destination_before_retry(
    cx: &mut TestAppContext,
) {
    let fixture = payload_recovery_host_fixture(cx);
    let start = prepare_payload_recovery_host_restore(&fixture, cx);
    cx.update_entity(&fixture.source_host, |host, _| {
        host.reject_next_payload_recovery_source_install_for_test();
    });

    let first = cx.update(|app| {
        crate::surface::payload_recovery_executor::start_payload_recovery_restore(
            start.owner.clone(),
            fixture.primary_host.downgrade(),
            start.primary_window,
            start.primary_binding,
            start.action,
            app,
        )
    });
    assert_eq!(
        first,
        Err(DockPayloadRecoveryRestoreError::PresentationInstallRejected)
    );
    assert!(
        cx.read_entity(&start.owner, |owner, _| owner
            .payload_recovery_execution_snapshot_for_test())
            .is_none(),
        "a rejected source installation must release the executor reservation"
    );
    assert!(cx.read_entity(&fixture.source_host, |host, _| {
        host.payload_recovery_presentation_state().is_none()
    }));
    assert!(cx.read_entity(&fixture.primary_host, |host, _| {
        host.payload_recovery_presentation_state().is_none()
    }));
    assert!(cx.read(|app| {
        open_gpui::view_presentation_window::stable_lease_for_window(
            app,
            fixture.panel_a.entity_id(),
            fixture.source_window.window_id(),
        )
        .is_some()
            && open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                fixture.panel_a.entity_id(),
                fixture.primary_window.window_id(),
            )
            .is_none()
    }));
    assert_eq!(
        cx.read_entity(&start.owner, |owner, _| owner
            .visible_payload_recovery_count_for_test(
                DockPayloadRecoveryReason::LostViewportRecovery,
            )),
        1
    );

    cx.update(|app| {
        crate::surface::payload_recovery_executor::start_payload_recovery_restore(
            start.owner.clone(),
            fixture.primary_host.downgrade(),
            start.primary_window,
            start.primary_binding,
            start.action,
            app,
        )
    })
    .expect("destination compensation must leave the exact recovery immediately retryable");
    cx.run_until_parked();
    assert_eq!(
        cx.read_entity(&start.owner, |owner, _| owner
            .visible_payload_recovery_count_for_test(
                DockPayloadRecoveryReason::LostViewportRecovery,
            )),
        0
    );
    assert!(cx.read(|app| {
        open_gpui::view_presentation_window::stable_lease_for_window(
            app,
            fixture.panel_a.entity_id(),
            fixture.source_window.window_id(),
        )
        .is_none()
            && open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                fixture.panel_a.entity_id(),
                fixture.primary_window.window_id(),
            )
            .is_some()
    }));
}

#[open_gpui::test]
fn payload_recovery_owner_transfer_rejection_compensates_both_hosts_before_retry(
    cx: &mut TestAppContext,
) {
    let fixture = payload_recovery_host_fixture(cx);
    let start = prepare_payload_recovery_host_restore(&fixture, cx);
    cx.update_entity(&start.owner, |owner, _| {
        owner.reject_next_payload_recovery_transfer_install_for_test();
    });

    let first = cx.update(|app| {
        crate::surface::payload_recovery_executor::start_payload_recovery_restore(
            start.owner.clone(),
            fixture.primary_host.downgrade(),
            start.primary_window,
            start.primary_binding,
            start.action,
            app,
        )
    });
    assert_eq!(
        first,
        Err(DockPayloadRecoveryRestoreError::PresentationInstallRejected)
    );
    assert!(
        cx.read_entity(&start.owner, |owner, _| owner
            .payload_recovery_execution_snapshot_for_test())
            .is_none(),
        "owner handoff rejection must release the executor reservation"
    );
    assert!(cx.read_entity(&fixture.source_host, |host, _| {
        host.payload_recovery_presentation_state().is_none()
    }));
    assert!(cx.read_entity(&fixture.primary_host, |host, _| {
        host.payload_recovery_presentation_state().is_none()
    }));
    assert!(cx.read(|app| {
        open_gpui::view_presentation_window::stable_lease_for_window(
            app,
            fixture.panel_a.entity_id(),
            fixture.source_window.window_id(),
        )
        .is_some()
            && open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                fixture.panel_a.entity_id(),
                fixture.primary_window.window_id(),
            )
            .is_none()
    }));
    assert_eq!(
        cx.read_entity(&start.owner, |owner, _| owner
            .visible_payload_recovery_count_for_test(
                DockPayloadRecoveryReason::LostViewportRecovery,
            )),
        1
    );

    cx.update(|app| {
        crate::surface::payload_recovery_executor::start_payload_recovery_restore(
            start.owner.clone(),
            fixture.primary_host.downgrade(),
            start.primary_window,
            start.primary_binding,
            start.action,
            app,
        )
    })
    .expect("two-host compensation must leave the exact recovery immediately retryable");
    cx.run_until_parked();
    assert_eq!(
        cx.read_entity(&start.owner, |owner, _| owner
            .visible_payload_recovery_count_for_test(
                DockPayloadRecoveryReason::LostViewportRecovery,
            )),
        0
    );
    assert!(cx.read(|app| {
        open_gpui::view_presentation_window::stable_lease_for_window(
            app,
            fixture.panel_a.entity_id(),
            fixture.source_window.window_id(),
        )
        .is_none()
            && open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                fixture.panel_a.entity_id(),
                fixture.primary_window.window_id(),
            )
            .is_some()
    }));
}

#[open_gpui::test]
fn payload_recovery_source_close_while_restoring_waits_for_native_terminal(
    cx: &mut TestAppContext,
) {
    let fixture = payload_recovery_host_fixture(cx);
    cx.update_entity(fixture.surface.owner(), |owner, _| {
        owner.pause_payload_recovery_after_source_release_once_for_test();
        owner.pause_payload_recovery_after_source_restoration_once_for_test();
    });
    start_payload_recovery_host_restore(&fixture, cx);
    cx.run_until_parked();

    let owner = fixture.surface.owner().clone();
    let (transfer, phase) = cx
        .read_entity(&owner, |owner, _| {
            let snapshot = owner.payload_recovery_execution_snapshot_for_test()?;
            owner
                .payload_recovery_transfer(snapshot.0)
                .map(|transfer| (transfer, snapshot.1))
        })
        .expect("the paused recovery should retain one admitted transfer");
    assert_eq!(
        phase,
        open_gpui::view_presentation_window::RehostPhase::DestinationAdmitted
    );
    let prepared = transfer.projection().clone();
    cx.update(|app| {
        crate::surface::payload_recovery_executor::payload_recovery_presentation_failed(
            owner.downgrade(),
            transfer.destination().host().clone(),
            transfer.destination_presentation(),
            app,
        );
    });
    cx.run_until_parked();

    let restoring = cx
        .read_entity(&owner, |owner, _| {
            owner.payload_recovery_execution_snapshot_for_test()
        })
        .expect("the restoration pause should retain one transfer");
    assert_eq!(
        restoring.1,
        open_gpui::view_presentation_window::RehostPhase::RestoringSource
    );

    let terminal = cx.hold_window_native_terminal(fixture.source_window);
    fixture
        .source_window
        .update(cx, |_, window, app| window.remove_window(app))
        .expect("the restoring source should begin logical close");
    cx.run_until_parked();
    let pending = cx
        .read_entity(&owner, |owner, _| {
            owner.payload_recovery_execution_snapshot_for_test()
        })
        .expect("logical close during restoration must retain the terminal barrier");
    assert_eq!(pending.0, restoring.0);
    assert_eq!(
        pending.1,
        open_gpui::view_presentation_window::RehostPhase::Invalidated
    );
    assert!(!pending.2);
    assert!(pending.5);
    assert!(cx.read_entity(&fixture.primary_host, |host, _| {
        host.payload_recovery_presentation_state().is_none()
    }));

    assert!(terminal.release());
    cx.run_until_parked();
    assert!(
        cx.read_entity(&owner, |owner, _| owner
            .payload_recovery_execution_snapshot_for_test())
            .is_none()
    );
    assert!(cx.read(|app| prepared.authority_is_retired(app)));
}

#[open_gpui::test]
fn payload_recovery_source_host_release_retires_execution_while_window_stays_live(
    cx: &mut TestAppContext,
) {
    let fixture = payload_recovery_host_fixture(cx);
    let start = prepare_payload_recovery_host_restore(&fixture, cx);
    let owner = fixture.surface.owner().clone();
    let source_window = fixture.source_window;
    let source_host = fixture.source_host.downgrade();
    drop(fixture.source_host);
    let (initial, prepared) = cx.update(|app| {
        crate::surface::payload_recovery_executor::start_payload_recovery_restore(
            start.owner.clone(),
            fixture.primary_host.downgrade(),
            start.primary_window,
            start.primary_binding,
            start.action,
            app,
        )
        .expect("the recovery executor should accept the exact endpoints");
        let initial = app
            .read_entity(&owner, |owner, _| {
                owner.payload_recovery_execution_snapshot_for_test()
            })
            .expect("the recovery executor should retain one transfer");
        let prepared = app.read_entity(&owner, |owner, _| {
            owner
                .payload_recovery_transfer(initial.0)
                .expect("the live Host release should begin with one transfer")
                .projection()
                .clone()
        });
        source_window
            .update(app, |_, window, app| {
                window.replace_root(app, |_, _| open_gpui::Empty);
            })
            .expect("the live source window should allow replacing its DockHost root");
        (initial, prepared)
    });
    assert_eq!(
        initial.1,
        open_gpui::view_presentation_window::RehostPhase::AwaitingSourceRelease
    );
    cx.run_until_parked();

    assert!(
        source_host.upgrade().is_none(),
        "the replaced DockHost should reach entity terminal while its window remains live"
    );
    assert!(source_window.update(cx, |_, _, _| ()).is_ok());
    assert!(
        cx.read_entity(&owner, |owner, _| owner
            .payload_recovery_execution_snapshot_for_test())
            .is_none(),
        "Host endpoint release must not leave a Busy recovery executor"
    );
    assert!(cx.read(|app| prepared.authority_is_retired(app)));
    assert!(
        cx.read(|app| {
            open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                fixture.panel_a.entity_id(),
                source_window.window_id(),
            )
        })
        .is_none(),
        "a released source Host must not retain stable authority in its live window"
    );
    assert_eq!(
        cx.read_entity(&owner, |owner, _| owner
            .visible_payload_recovery_count_for_test(
                DockPayloadRecoveryReason::LostViewportRecovery,
            )),
        1,
        "Host endpoint loss must preserve the durable recovery record"
    );
    assert!(cx.read_entity(&fixture.primary_host, |host, _| {
        host.payload_recovery_presentation_state().is_none()
    }));

    cx.update(|app| {
        crate::surface::payload_recovery_executor::start_payload_recovery_restore(
            start.owner,
            fixture.primary_host.downgrade(),
            start.primary_window,
            start.primary_binding,
            start.action,
            app,
        )
    })
    .expect("the durable recovery action should remain retryable after source Host loss");
    cx.run_until_parked();
    assert_eq!(
        cx.read_entity(&owner, |owner, _| owner
            .visible_payload_recovery_count_for_test(
                DockPayloadRecoveryReason::LostViewportRecovery,
            )),
        0,
        "the retry should consume the exact durable recovery action"
    );
    assert!(
        cx.read(|app| {
            open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                fixture.panel_a.entity_id(),
                fixture.primary_window.window_id(),
            )
        })
        .is_some(),
        "the retry should establish stable authority in the primary Host"
    );
}

#[open_gpui::test]
fn payload_recovery_source_host_loss_after_finish_releases_stable_batch_before_retry(
    cx: &mut TestAppContext,
) {
    let fixture = payload_recovery_host_fixture(cx);
    let start = prepare_payload_recovery_host_restore(&fixture, cx);
    let owner = fixture.surface.owner().clone();
    cx.update_entity(&owner, |owner, _| {
        owner.pause_payload_recovery_after_source_release_once_for_test();
        owner.replace_payload_recovery_source_host_after_finish_once_for_test();
    });
    let source_window = fixture.source_window;
    let source_host = fixture.source_host.downgrade();
    drop(fixture.source_host);
    cx.update(|app| {
        crate::surface::payload_recovery_executor::start_payload_recovery_restore(
            start.owner.clone(),
            fixture.primary_host.downgrade(),
            start.primary_window,
            start.primary_binding,
            start.action,
            app,
        )
    })
    .expect("the recovery executor should accept the exact endpoints");
    cx.run_until_parked();

    let (transfer, phase) = cx
        .read_entity(&owner, |owner, _| {
            let snapshot = owner.payload_recovery_execution_snapshot_for_test()?;
            owner
                .payload_recovery_transfer(snapshot.0)
                .map(|transfer| (transfer, snapshot.1))
        })
        .expect("the source-release pause should retain one admitted transfer");
    assert_eq!(
        phase,
        open_gpui::view_presentation_window::RehostPhase::DestinationAdmitted
    );
    let prepared = transfer.projection().clone();
    cx.update(|app| {
        crate::surface::payload_recovery_executor::payload_recovery_presentation_failed(
            owner.downgrade(),
            transfer.destination().host().clone(),
            transfer.destination_presentation(),
            app,
        );
    });
    cx.run_until_parked();

    assert!(cx.read(|app| prepared.authority_is_retired(app)));

    assert!(source_host.upgrade().is_none());
    assert!(
        cx.read(|app| {
            open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                fixture.panel_a.entity_id(),
                source_window.window_id(),
            )
        })
        .is_none(),
        "Host loss before its visible checkpoint must release the exact restored stable batch"
    );
    assert!(
        cx.read_entity(&owner, |owner, _| owner
            .payload_recovery_execution_snapshot_for_test())
            .is_none()
    );
    assert_eq!(
        cx.read_entity(&owner, |owner, _| owner
            .visible_payload_recovery_count_for_test(
                DockPayloadRecoveryReason::LostViewportRecovery,
            )),
        1
    );

    cx.update(|app| {
        crate::surface::payload_recovery_executor::start_payload_recovery_restore(
            start.owner,
            fixture.primary_host.downgrade(),
            start.primary_window,
            start.primary_binding,
            start.action,
            app,
        )
    })
    .expect("the first retry should consume the durable recovery after source Host loss");
    cx.run_until_parked();
    assert_eq!(
        cx.read_entity(&owner, |owner, _| owner
            .visible_payload_recovery_count_for_test(
                DockPayloadRecoveryReason::LostViewportRecovery,
            )),
        0
    );
    assert!(
        cx.read(|app| {
            open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                fixture.panel_a.entity_id(),
                fixture.primary_window.window_id(),
            )
        })
        .is_some()
    );
}

#[open_gpui::test]
fn payload_recovery_source_close_after_release_preserves_ungoverned_hidden_root(
    cx: &mut TestAppContext,
) {
    let fixture = payload_recovery_host_fixture(cx);
    cx.update_entity(fixture.surface.owner(), |owner, _| {
        owner.pause_payload_recovery_after_source_release_once_for_test();
    });
    start_payload_recovery_host_restore(&fixture, cx);
    cx.run_until_parked();

    let released = cx
        .read_entity(fixture.surface.owner(), |owner, _| {
            owner.payload_recovery_execution_snapshot_for_test()
        })
        .expect("the paused recovery executor should retain one transfer");
    assert_eq!(
        released.1,
        open_gpui::view_presentation_window::RehostPhase::DestinationAdmitted
    );
    assert!(!released.2);
    let rehost_generation = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner
            .payload_recovery_transfer(released.0)
            .expect("the paused recovery executor should retain its transfer")
            .projection()
            .generation()
    });
    let source_state = cx
        .read_entity(&fixture.source_host, |host, _| {
            host.payload_recovery_presentation_state()
        })
        .expect("the released source should retain its frozen projection");
    assert_eq!(source_state.key.rehost_generation(), rehost_generation);
    assert!(matches!(
        source_state.mode,
        crate::host::DockHostRecoveryPresentationMode::SourceProjection {
            phase: crate::host::DockHostRecoverySourcePhase::Frozen,
            ..
        }
    ));
    let destination_state = cx
        .read_entity(&fixture.primary_host, |host, _| {
            host.payload_recovery_presentation_state()
        })
        .expect("the paused destination should retain its pre-arm projection");
    assert_eq!(destination_state.key.rehost_generation(), rehost_generation);
    assert!(matches!(
        destination_state.mode,
        crate::host::DockHostRecoveryPresentationMode::DestinationProjection {
            phase: crate::host::DockHostRecoveryDestinationPhase::AwaitingSourceRelease,
            ..
        }
    ));
    let hidden_leases = cx.read(|app| {
        (
            open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                fixture.panel_b.entity_id(),
                fixture.source_window.window_id(),
            ),
            open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                fixture.panel_b.entity_id(),
                fixture.primary_window.window_id(),
            ),
        )
    });
    assert_eq!(hidden_leases, (None, None));

    let terminal = cx.hold_window_native_terminal(fixture.source_window);
    fixture
        .source_window
        .update(cx, |_, window, app| window.remove_window(app))
        .expect("the released recovery source should begin logical close");
    cx.run_until_parked();
    let pending = cx
        .read_entity(fixture.surface.owner(), |owner, _| {
            owner.payload_recovery_execution_snapshot_for_test()
        })
        .expect("post-release logical close must still await native terminal");
    assert_eq!(pending.0, released.0);
    assert_eq!(
        pending.1,
        open_gpui::view_presentation_window::RehostPhase::DestinationAdmitted
    );
    assert!(!pending.2);
    assert!(pending.5);
    let prepared = cx.read_entity(fixture.surface.owner(), |owner, _| {
        owner
            .payload_recovery_transfer(pending.0)
            .expect("the native-terminal barrier should retain the released transfer")
            .projection()
            .clone()
    });

    assert!(terminal.release());
    cx.run_until_parked();
    assert!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .payload_recovery_execution_snapshot_for_test())
            .is_none()
    );
    assert!(cx.read(|app| prepared.authority_is_retired(app)));
    assert_eq!(
        cx.read_entity(fixture.surface.owner(), |owner, _| owner
            .visible_payload_recovery_count_for_test(
                DockPayloadRecoveryReason::LostViewportRecovery,
            )),
        1,
        "post-release source loss must preserve the durable recovery entry"
    );
    assert!(cx.read_entity(&fixture.primary_host, |host, _| {
        host.payload_recovery_presentation_state().is_none()
    }));

    let reopened = cx.update(|app| {
        match fixture.surface.open_viewport(
            fixture.lost_space.clone(),
            viewport_window_options(360.0, 220.0),
            app,
        ) {
            crate::DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the retained recovery source should reopen, got {outcome:?}"),
        }
    });
    cx.run_until_parked();
    let reopened_host = reopened
        .downcast::<DockHost>()
        .expect("the reopened recovery source should render DockHost")
        .entity(cx)
        .expect("the reopened recovery source Host should remain live");
    let mut visual = VisualTestContext::from_window(reopened, cx);
    let tab_b = selector_for(
        &visual,
        &reopened_host,
        DockDebugRegion::Tab {
            tabs: fixture.lost_tabs,
            item: item("b"),
        },
    )
    .expect("the formerly hidden root should render after reopening its source");
    let tab_b_bounds = debug_bounds(&mut visual, &tab_b);
    visual.simulate_click(tab_b_bounds.center(), Modifiers::none());
    cx.run_until_parked();

    assert!(
        selector_for(
            &visual,
            &reopened_host,
            DockDebugRegion::Panel { item: item("b") },
        )
        .is_some(),
        "the ungoverned hidden root must remain selectable after source recovery"
    );
    let final_leases = cx.read(|app| {
        (
            open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                fixture.panel_b.entity_id(),
                reopened.window_id(),
            ),
            open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                fixture.panel_b.entity_id(),
                fixture.primary_window.window_id(),
            ),
        )
    });
    assert!(final_leases.0.is_some());
    assert_eq!(final_leases.1, None);
    assert!(
        cx.read(
            |app| open_gpui::view_presentation_window::stable_lease_for_window(
                app,
                fixture.panel_a.entity_id(),
                fixture.primary_window.window_id(),
            )
        )
        .is_none(),
        "the recovery destination must not retain any source-root authority"
    );
}

#[open_gpui::test]
fn payload_recovery_destination_close_then_source_close_waits_for_source_native_terminal(
    cx: &mut TestAppContext,
) {
    let fixture = payload_recovery_host_fixture(cx);
    cx.update_entity(fixture.surface.owner(), |owner, _| {
        owner.pause_payload_recovery_after_source_release_once_for_test();
    });
    start_payload_recovery_host_restore(&fixture, cx);
    cx.run_until_parked();

    let owner = fixture.surface.owner().clone();
    let admitted = cx
        .read_entity(&owner, |owner, _| {
            owner.payload_recovery_execution_snapshot_for_test()
        })
        .expect("the paused recovery should retain one admitted transfer");
    assert_eq!(
        admitted.1,
        open_gpui::view_presentation_window::RehostPhase::DestinationAdmitted
    );
    let prepared = cx.read_entity(&owner, |owner, _| {
        owner
            .payload_recovery_transfer(admitted.0)
            .expect("the admitted recovery should retain its exact transfer")
            .projection()
            .clone()
    });
    let source_terminal = cx.hold_window_native_terminal(fixture.source_window);

    fixture
        .primary_window
        .update(cx, |_, window, app| window.remove_window(app))
        .expect("the recovery destination should begin logical close");
    let _ = fixture
        .source_window
        .update(cx, |_, window, app| window.remove_window(app));
    cx.run_until_parked();

    let pending = cx
        .read_entity(&owner, |owner, _| {
            owner.payload_recovery_execution_snapshot_for_test()
        })
        .expect("source logical close must remain sticky after destination invalidation");
    assert_eq!(pending.0, admitted.0);
    assert!(!pending.2);
    assert!(pending.5);
    assert!(cx.read_entity(
        &owner,
        |owner, _| owner.payload_recovery_source_close_state(pending.0) == Some((true, false))
    ));

    assert!(source_terminal.release());
    cx.run_until_parked();
    assert!(
        cx.read_entity(&owner, |owner, _| owner
            .payload_recovery_execution_snapshot_for_test())
            .is_none()
    );
    assert!(cx.read(|app| prepared.authority_is_retired(app)));
}

#[open_gpui::test]
fn payload_recovery_released_source_host_release_abandons_transfer(cx: &mut TestAppContext) {
    let fixture = payload_recovery_host_fixture(cx);
    cx.update_entity(fixture.surface.owner(), |owner, _| {
        owner.pause_payload_recovery_after_source_release_once_for_test();
    });
    start_payload_recovery_host_restore(&fixture, cx);
    cx.run_until_parked();

    let owner = fixture.surface.owner().clone();
    let source_window = fixture.source_window;
    let source_host = fixture.source_host.downgrade();
    let transfer = cx
        .read_entity(&owner, |owner, _| {
            owner.payload_recovery_execution_snapshot_for_test()
        })
        .expect("the paused recovery executor should retain one transfer");
    assert_eq!(
        transfer.1,
        open_gpui::view_presentation_window::RehostPhase::DestinationAdmitted
    );
    let prepared = cx.read_entity(&owner, |owner, _| {
        owner
            .payload_recovery_transfer(transfer.0)
            .expect("the released Host should retain one prepared transfer")
            .projection()
            .clone()
    });

    drop(fixture.source_host);
    source_window
        .update(cx, |_, window, app| {
            window.replace_root(app, |_, _| open_gpui::Empty);
        })
        .expect("the released source window should allow replacing its DockHost root");
    cx.run_until_parked();

    assert!(source_host.upgrade().is_none());
    assert!(source_window.update(cx, |_, _, _| ()).is_ok());
    assert!(
        cx.read_entity(&owner, |owner, _| owner
            .payload_recovery_execution_snapshot_for_test())
            .is_none(),
        "post-release Host loss must retire the recovery executor"
    );
    assert!(cx.read(|app| prepared.authority_is_retired(app)));
    assert!(cx.read_entity(&fixture.primary_host, |host, _| {
        host.payload_recovery_presentation_state().is_none()
    }));
    assert_eq!(
        cx.read_entity(&owner, |owner, _| owner
            .visible_payload_recovery_count_for_test(
                DockPayloadRecoveryReason::LostViewportRecovery,
            )),
        1,
        "post-release Host loss must preserve the durable recovery record"
    );
}
