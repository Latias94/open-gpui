//! Physical preview ownership and the original core create/presentation pipeline.

use super::*;
use crate::adapter::{DockPane, GpuiDockspaceConfig};
use dockspace::{
    geometry::PhysicalPoint,
    model::{
        DockspaceLayout, DockspaceNode, DockspaceRootLayout, DockspaceSurfaceLayout, ItemId,
        NativeWindowPlacement, RootId,
    },
    policy::DockPolicy,
    runtime::{
        DockspaceSession, HostWorkAreaToken, NativeDesktopPointerLocation, NativeDesktopPosition,
        NativePointerButton, NativePointerEvent, NativePointerHover, NativePointerId,
        NativePointerInput, NativePointerOwner, PointerDeviceKind, PointerEventMetadata,
        SurfaceUnavailableReason,
    },
};
use open_gpui::{
    Bounds, DevicePixels, Empty, MouseButton, PlatformPhysicalDisplayObservation,
    PlatformWindowHitStack, PlatformWindowPhysicalGeometry, PlatformWindowPresentOutcome,
    RequestFrameOptions, TestAppContext, VisualTestContext, WindowMutationDomain, point, size,
};

const SOURCE: SurfaceId = SurfaceId::new(41);
const TARGET: SurfaceId = SurfaceId::new(42);
const REMAINING: ItemId = ItemId::new(43);

struct Fixture {
    dock: DockSurface,
    anchor: AnyWindowHandle,
    shell: AnyWindowHandle,
    state: Rc<RefCell<ProvisionalWindow>>,
    bridge: NativeBridge,
    pointer: PlatformNativePointerPhysicalFrame,
    drag: NativeDragTearOff,
}

impl Fixture {
    fn new(cx: &mut TestAppContext) -> Self {
        cx.set_platform_mouse_button_is_pressed(MouseButton::Left, Some(false));
        cx.set_platform_window_hit_stack(
            PlatformWindowHitStack::try_available_open_desktop(
                point(DevicePixels(0), DevicePixels(0)),
                vec![],
            )
            .unwrap(),
        );
        let display = cx.update(|cx| {
            let snapshot = cx.display_snapshot();
            let logical = snapshot.displays()[0].clone();
            let display = PlatformPhysicalDisplayObservation::try_new(
                snapshot.generation().unwrap(),
                logical.id(),
                logical.bounds().to_device_pixels(2.0),
                logical.visible_bounds().to_device_pixels(2.0),
                2.0,
            )
            .unwrap();
            display
        });
        cx.simulate_physical_display_roster(Some(vec![display]));
        let dock = cx.update(|cx| {
            let layout = DockspaceLayout::new([SOURCE, TARGET].map(|surface| {
                DockspaceSurfaceLayout::new(
                    surface,
                    DockspaceRootLayout::new(
                        RootId::new(surface.get()),
                        DockspaceNode::tabs(if surface == SOURCE {
                            vec![ItemId::new(surface.get()), REMAINING]
                        } else {
                            vec![ItemId::new(surface.get())]
                        }),
                    ),
                )
            }))
            .unwrap();
            let mut policy = DockPolicy::default();
            policy.set_allow_native_surfaces(true);
            let session = DockspaceSession::from_layout(layout, policy).unwrap();
            let mut panes = Vec::from([SOURCE, TARGET].map(|surface| {
                DockPane::new(
                    ItemId::new(surface.get()),
                    "Original View",
                    cx.new(|_| Empty),
                )
            }));
            panes.push(DockPane::new(
                REMAINING,
                "Remaining View",
                cx.new(|_| Empty),
            ));
            DockSurface::new_managed(session, panes, GpuiDockspaceConfig::default(), cx).unwrap()
        });
        cx.defer_next_window_frame_requests();
        let anchor = cx
            .update(|cx| dock.open_native_root(SOURCE, true, WindowOptions::default(), cx))
            .unwrap();
        let bounds = Bounds::new(
            point(DevicePixels(100), DevicePixels(100)),
            size(DevicePixels(600), DevicePixels(450)),
        );
        cx.set_platform_window_physical_client_geometry(anchor, Some(bounds), 2.0);
        cx.simulate_window_scale_factor_change(anchor, 2.0);
        cx.update_window(anchor, |_, window, cx| window.bounds_changed(cx))
            .unwrap();
        let mut source = VisualTestContext::from_window(anchor, cx);
        for _ in 0..4 {
            frame(&mut source, &dock);
        }
        let mut bridge = dock
            .host
            .borrow_mut()
            .native
            .as_mut()
            .unwrap()
            .bridge
            .take()
            .unwrap();
        let geometry = PlatformWindowPhysicalGeometry::try_new(bounds, 2.0).unwrap();
        let pointer = PlatformNativePointerPhysicalFrame::new(
            point(DevicePixels(900), DevicePixels(180)),
            geometry,
        )
        .with_target_display(display)
        .unwrap();
        let pressed = source.debug_bounds("dockspace:tab:41").unwrap().center();
        let pressed = geometry.local_to_global(pressed).unwrap();
        let binding = bridge.binding_for_window(anchor.window_id()).unwrap();
        let work_area = dock
            .host
            .borrow()
            .session
            .native_work_area(HostWorkAreaToken::new(u64::from(display.display_id())))
            .unwrap();
        let input = |event, point: open_gpui::Point<DevicePixels>, hover, at| {
            NativePointerInput::new(
                NativePointerId::new(1),
                event,
                NativeDesktopPointerLocation::new(
                    NativeDesktopPosition::Exact(
                        PhysicalPoint::new(f64::from(point.x.0), f64::from(point.y.0)).unwrap(),
                    ),
                    hover,
                    (hover == NativePointerHover::OutsideAll).then_some(work_area),
                ),
                NativePointerOwner::Native(binding),
                NativePointerOwner::Native(binding),
            )
            .with_metadata(PointerEventMetadata::new(
                PointerDeviceKind::Mouse,
                std::time::Duration::from_millis(at),
            ))
        };
        cx.update(|cx| {
            dock.host
                .borrow_mut()
                .session
                .record_native_pointer_batch([
                    input(
                        NativePointerEvent::ButtonPressed(NativePointerButton::Primary),
                        pressed,
                        NativePointerHover::Dock(binding),
                        10,
                    ),
                    input(
                        NativePointerEvent::Moved,
                        pointer.global_position(),
                        NativePointerHover::OutsideAll,
                        20,
                    ),
                ])
                .unwrap();
            commit_input(&dock, cx);
        });
        for _ in 0..4 {
            frame(&mut source, &dock);
            collect(&dock, &mut bridge, cx);
        }
        cx.update(|cx| {
            dock.host
                .borrow_mut()
                .session
                .record_native_pointer(input(
                    NativePointerEvent::Moved,
                    pointer.global_position(),
                    NativePointerHover::OutsideAll,
                    25,
                ))
                .unwrap();
            commit_input(&dock, cx);
        });
        for _ in 0..4 {
            frame(&mut source, &dock);
            collect(&dock, &mut bridge, cx);
        }
        let (drag, preview) = {
            let host = dock.host.borrow();
            (
                host.session
                    .native_drag_tear_off()
                    .expect("the original committed desktop drag"),
                host.mounts[&anchor.window_id()].regions[&SOURCE]
                    .packet
                    .as_ref()
                    .unwrap()
                    .native_preview
                    .expect("the original core preview reserves a surface"),
            )
        };
        while let Some(report) = dock
            .host
            .borrow_mut()
            .native
            .as_mut()
            .unwrap()
            .pending_reports
            .pop_front()
        {
            bridge.accept_report(report);
        }
        cx.defer_next_window_frame_requests();
        let state = cx
            .update(|cx| {
                open_preview(
                    &dock.host,
                    drag,
                    preview,
                    pointer,
                    None,
                    vec![anchor.window_id()],
                    1,
                    cx,
                )
            })
            .unwrap();
        let shell = state.borrow().window.unwrap();
        dock.host
            .borrow_mut()
            .native
            .as_mut()
            .unwrap()
            .provisional
            .opening = Some(state.clone());
        cx.set_platform_window_physical_client_geometry(shell, Some(bounds), 2.0);
        let mut visual = VisualTestContext::from_window(shell, cx);
        for _ in 0..3 {
            frame(&mut visual, &dock);
        }
        assert!(cx.flush_window_mutation(shell, WindowMutationDomain::Placement));
        for _ in 0..3 {
            frame(&mut visual, &dock);
        }
        assert_eq!(
            state.borrow().reveal.as_ref().unwrap().snapshot().outcome(),
            WindowProvisionalRevealOutcome::Revealed
        );
        assert!(state.borrow().binding.is_none());
        assert!(
            state.borrow().failure.is_none(),
            "{:?}",
            state.borrow().failure
        );
        assert!(
            !dock.host.borrow().mounts.contains_key(&shell.window_id()),
            "a visual shell cannot mount the reserved core surface"
        );
        Self {
            dock,
            anchor,
            shell,
            state,
            bridge,
            pointer,
            drag,
        }
    }

    fn release(&mut self, cx: &mut TestAppContext) {
        self.state.borrow_mut().release_frame = Some(self.pointer);
        cx.update(|cx| {
            let binding = self
                .bridge
                .binding_for_window(self.anchor.window_id())
                .unwrap();
            let work_area = self
                .dock
                .host
                .borrow()
                .session
                .native_work_area(HostWorkAreaToken::new(u64::from(
                    self.pointer.target_display().unwrap().display_id(),
                )))
                .unwrap();
            self.dock
                .host
                .borrow_mut()
                .session
                .record_native_pointer(
                    NativePointerInput::new(
                        NativePointerId::new(1),
                        NativePointerEvent::ButtonReleased(NativePointerButton::Primary),
                        NativeDesktopPointerLocation::new(
                            NativeDesktopPosition::Exact(
                                PhysicalPoint::new(
                                    f64::from(self.pointer.global_position().x.0),
                                    f64::from(self.pointer.global_position().y.0),
                                )
                                .unwrap(),
                            ),
                            NativePointerHover::Foreign,
                            None,
                        ),
                        NativePointerOwner::Native(binding),
                        NativePointerOwner::None,
                    )
                    .with_native_tear_off(work_area, Some(self.drag))
                    .with_metadata(PointerEventMetadata::new(
                        PointerDeviceKind::Mouse,
                        std::time::Duration::from_millis(30),
                    )),
                )
                .unwrap();
            commit_input(&self.dock, cx);
        });
    }

    fn enqueue_create(&mut self, cx: &mut TestAppContext) {
        self.release(cx);
        let mut source = VisualTestContext::from_window(self.anchor, cx);
        for _ in 0..4 {
            frame(&mut source, &self.dock);
            collect(&self.dock, &mut self.bridge, cx);
        }
        while let Some(report) = self
            .dock
            .host
            .borrow_mut()
            .native
            .as_mut()
            .unwrap()
            .pending_reports
            .pop_front()
        {
            self.bridge.accept_report(report);
        }
    }

    fn request_create(&mut self, cx: &mut TestAppContext, inside_shell: bool) {
        self.enqueue_create(cx);
        cx.update(|cx| {
            self.bridge
                .dispatch_effects(
                    &mut |request, callbacks, cx| {
                        assert_eq!(request.drag_tear_off, Some(self.drag));
                        assert!(adopt(
                            &self.dock,
                            NativeWindowRequest { drag_tear_off: None, ..request },
                            callbacks,
                            cx,
                        ).is_none(), "a create without the original key cannot adopt even the same reserved surface");
                        assert!(Rc::ptr_eq(
                            self.dock.host.borrow().native.as_ref().unwrap().provisional.opening.as_ref().unwrap(),
                            &self.state,
                        ));
                        if inside_shell {
                            self.shell
                                .update(cx, |_, _, cx| {
                                    super::super::default_factory(
                                        &self.dock, request, callbacks, cx,
                                    )
                                })
                                .unwrap()
                        } else {
                            super::super::default_factory(&self.dock, request, callbacks, cx)
                        }
                    },
                    cx,
                )
                .unwrap();
        });
        if inside_shell {
            return;
        }
        assert_eq!(
            self.dock
                .native_callbacks_for_window(self.shell.window_id())
                .unwrap()
                .window_id(),
            Some(self.shell.window_id())
        );
        assert_eq!(
            cx.update(|cx| cx.windows().len()),
            2,
            "adoption must keep the original physical owner"
        );
    }
}

fn commit_input(dock: &DockSurface, cx: &mut App) {
    let mut host = dock.host.borrow_mut();
    let HostState {
        session,
        mounts,
        native,
        ..
    } = &mut *host;
    let mut candidate =
        super::super::begin_frame(session, mounts, native.as_ref(), None, cx).unwrap();
    candidate
        .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
        .unwrap();
    let report = candidate.commit().unwrap();
    host.apply_report(report);
}

fn collect(dock: &DockSurface, bridge: &mut NativeBridge, cx: &mut TestAppContext) {
    cx.update(|cx| {
        let mut deferred = super::super::DeferredHostWork::default();
        super::super::collect_host(&mut dock.host.borrow_mut(), bridge, None, &mut deferred, cx)
            .unwrap();
        deferred.flush_pre_dispatch(&dock.host, bridge, cx);
    });
}

fn frame(visual: &mut VisualTestContext, dock: &DockSurface) {
    assert!(visual.simulate_frame(RequestFrameOptions {
        force_render: true,
        require_presentation: true,
        ..Default::default()
    }));
    assert!(dock.error().is_none(), "{:?}", dock.error());
}

#[open_gpui::test]
fn release_waiting_for_preview_keeps_the_original_provisional_window(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx);
    fixture.pointer = PlatformNativePointerPhysicalFrame::new(
        point(DevicePixels(980), DevicePixels(220)),
        fixture.pointer.source_geometry(),
    )
    .with_target_display(fixture.pointer.target_display().unwrap())
    .unwrap();
    fixture.release(cx);
    {
        let host = fixture.dock.host.borrow();
        assert!(!host.session.has_active_gesture());
        assert!(host.session.has_pending_presentation());
        assert!(!host.session.has_pending_native_create());
        assert!(host.session.native_drag_tear_off().is_none());
    }
    cx.update(|cx| {
        drive(&fixture.dock.host, &fixture.bridge, cx);
        assert!(cx.windows().contains(&fixture.shell));
    });
    assert!(Rc::ptr_eq(
        fixture
            .dock
            .host
            .borrow()
            .native
            .as_ref()
            .unwrap()
            .provisional
            .opening
            .as_ref()
            .unwrap(),
        &fixture.state,
    ));
    fixture
        .dock
        .host
        .borrow_mut()
        .native
        .as_mut()
        .unwrap()
        .bridge = Some(fixture.bridge);
    cx.update(|cx| fixture.dock.begin_native_shutdown(cx))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| fixture.dock.pump_native(cx));
    cx.run_until_parked();
    assert!(
        fixture.dock.native_is_quiescent(),
        "{:?}",
        fixture.dock.inspection()
    );
}

#[open_gpui::test]
fn another_drag_key_cannot_adopt_the_same_reserved_surface(cx: &mut TestAppContext) {
    let mut other_context = cx.new_app();
    let other = Fixture::new(&mut other_context);
    let other_drag = other.drag;
    other_context.quit();
    let mut fixture = Fixture::new(cx);
    assert_ne!(fixture.drag, other_drag);
    fixture.enqueue_create(cx);
    cx.update(|cx| {
        fixture
            .bridge
            .dispatch_effects(
                &mut |request, callbacks, cx| {
                    assert_eq!(request.drag_tear_off, Some(fixture.drag));
                    assert!(
                        adopt(
                            &fixture.dock,
                            NativeWindowRequest {
                                drag_tear_off: Some(other_drag),
                                ..request
                            },
                            callbacks,
                            cx,
                        )
                        .is_none()
                    );
                    assert!(Rc::ptr_eq(
                        fixture
                            .dock
                            .host
                            .borrow()
                            .native
                            .as_ref()
                            .unwrap()
                            .provisional
                            .opening
                            .as_ref()
                            .unwrap(),
                        &fixture.state,
                    ));
                    super::super::default_factory(&fixture.dock, request, callbacks, cx)
                },
                cx,
            )
            .unwrap();
    });
    assert_eq!(
        fixture.state.borrow().binding.unwrap().surface(),
        fixture.state.borrow().preview.target_surface
    );
    assert_eq!(fixture.state.borrow().window, Some(fixture.shell));
    fixture
        .dock
        .host
        .borrow_mut()
        .native
        .as_mut()
        .unwrap()
        .bridge = Some(fixture.bridge);
    cx.update(|cx| fixture.dock.begin_native_shutdown(cx))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| fixture.dock.pump_native(cx));
    cx.run_until_parked();
    assert!(
        fixture.dock.native_is_quiescent(),
        "{:?}",
        fixture.dock.inspection()
    );
}

#[open_gpui::test]
fn stationary_desktop_intent_preserves_foreign_hover_and_requires_fresh_work_area(
    cx: &mut TestAppContext,
) {
    use open_gpui::{
        PlatformNativePointerCaptureOwner, PlatformWindowHit, PlatformWindowHitStack,
        PlatformWindowPhysicalCoverage,
    };

    let mut fixture = Fixture::new(cx);
    let mut source = VisualTestContext::from_window(fixture.anchor, cx);
    let target = source.debug_bounds("dockspace:tab:43").unwrap().center();
    let position = fixture
        .pointer
        .source_geometry()
        .local_to_global(target)
        .unwrap();
    let binding = fixture
        .bridge
        .binding_for_window(fixture.anchor.window_id())
        .unwrap();
    fixture
        .dock
        .host
        .borrow_mut()
        .session
        .record_native_pointer(
            NativePointerInput::new(
                NativePointerId::new(1),
                NativePointerEvent::Moved,
                NativeDesktopPointerLocation::new(
                    NativeDesktopPosition::Exact(
                        PhysicalPoint::new(f64::from(position.x.0), f64::from(position.y.0))
                            .unwrap(),
                    ),
                    NativePointerHover::Dock(binding),
                    None,
                ),
                NativePointerOwner::Native(binding),
                NativePointerOwner::Native(binding),
            )
            .with_metadata(PointerEventMetadata::new(
                PointerDeviceKind::Mouse,
                std::time::Duration::from_millis(40),
            )),
        )
        .unwrap();
    cx.update(|cx| commit_input(&fixture.dock, cx));
    for _ in 0..4 {
        frame(&mut source, &fixture.dock);
        collect(&fixture.dock, &mut fixture.bridge, cx);
    }
    let host = fixture.dock.host.borrow();
    let request = host
        .session
        .native_pointer_continuation()
        .expect("the original desktop drag waits over the inactive tab");
    assert!(request.local_coordinates().is_none());
    let metadata = PointerEventMetadata::new(PointerDeviceKind::Mouse, request.deadline());
    let frame = fixture
        .pointer
        .with_capture_owner(PlatformNativePointerCaptureOwner::Window(
            fixture.anchor.window_id(),
        ));
    let stack = PlatformWindowHitStack::try_available(
        frame.global_position(),
        vec![PlatformWindowHit::OpaqueBarrier {
            coverage: PlatformWindowPhysicalCoverage::try_new(Bounds::new(
                point(DevicePixels(850), DevicePixels(100)),
                size(DevicePixels(300), DevicePixels(300)),
            ))
            .unwrap(),
        }],
    )
    .unwrap();
    let expected = NativePointerInput::new(
        request.pointer(),
        request.event(),
        NativeDesktopPointerLocation::new(
            NativeDesktopPosition::Exact(
                PhysicalPoint::new(
                    f64::from(frame.global_position().x.0),
                    f64::from(frame.global_position().y.0),
                )
                .unwrap(),
            ),
            NativePointerHover::Foreign,
            None,
        ),
        NativePointerOwner::Native(binding),
        NativePointerOwner::Native(binding),
    )
    .with_metadata(metadata);
    let area = host
        .session
        .native_work_area(HostWorkAreaToken::new(u64::from(
            frame.target_display().unwrap().display_id(),
        )))
        .unwrap();
    assert_eq!(
        fixture
            .bridge
            .continuation_input(&host.session, request, frame, &stack, metadata),
        Some(expected.with_native_tear_off(area, None))
    );
    assert!(
        fixture
            .bridge
            .continuation_input(
                &host.session,
                request,
                frame.with_capture_owner(PlatformNativePointerCaptureOwner::UnregisteredWindow),
                &stack,
                metadata
            )
            .is_none()
    );
    let stale_display = PlatformPhysicalDisplayObservation::try_new(
        frame.target_display().unwrap().topology_generation() + 1,
        frame.target_display().unwrap().display_id(),
        frame.target_display().unwrap().bounds(),
        frame.target_display().unwrap().visible_bounds(),
        frame.target_display().unwrap().scale_factor(),
    )
    .unwrap();
    let stale_frame =
        PlatformNativePointerPhysicalFrame::new(frame.global_position(), frame.source_geometry())
            .with_target_display(stale_display)
            .unwrap()
            .with_capture_owner(frame.capture_owner());
    assert_eq!(
        fixture
            .bridge
            .continuation_input(&host.session, request, stale_frame, &stack, metadata),
        Some(expected)
    );
}

#[open_gpui::test]
fn adoption_shares_the_original_semantics_submission_and_waits_for_first_live(
    cx: &mut TestAppContext,
) {
    let mut fixture = Fixture::new(cx);
    fixture.request_create(cx, false);
    let original_placement = fixture
        .state
        .borrow()
        .placement
        .as_ref()
        .unwrap()
        .snapshot();
    let callbacks = fixture
        .dock
        .native_callbacks_for_window(fixture.shell.window_id())
        .unwrap();
    let original_create = callbacks.create_request().unwrap();
    assert_eq!(original_create.binding.window(), callbacks.binding());
    assert_eq!(original_create.drag_tear_off, Some(fixture.drag));
    assert!(
        fixture
            .dock
            .native_provisional_placement_for_test(fixture.shell.window_id())
            .is_none(),
        "an adopted final placement is owned by callbacks, not the live-route inspector"
    );
    assert_eq!(callbacks.provisional_placement(), Some(original_placement));
    assert_eq!(original_placement.window_id(), fixture.shell.window_id());
    assert_eq!(
        original_placement.purpose(),
        WindowProvisionalPlacementPurpose::FinalRelease
    );
    assert_eq!(
        original_placement.anchor_point(),
        fixture.pointer.global_position()
    );
    assert_eq!(
        Some(original_placement.target_display()),
        fixture.pointer.target_display()
    );
    let client = original_placement.client_bounds();
    assert_eq!(
        [
            client.origin.x.0,
            client.origin.y.0,
            client.size.width.0,
            client.size.height.0
        ]
        .map(f64::from),
        [
            original_create.outer_bounds.x(),
            original_create.outer_bounds.y(),
            original_create.outer_bounds.width(),
            original_create.outer_bounds.height()
        ]
        .map(f64::round),
    );
    let Fixture {
        dock,
        shell,
        state,
        mut bridge,
        ..
    } = fixture;
    cx.update(|cx| {
        bridge
            .dispatch_effects(
                &mut |_, _, _| panic!("the affine create effect cannot run twice"),
                cx,
            )
            .unwrap();
    });
    let mut visual = VisualTestContext::from_window(shell, cx);
    visual.set_window_present_outcome(shell, PlatformWindowPresentOutcome::Deferred);
    assert!(cx.flush_window_mutation(shell, WindowMutationDomain::Placement));
    cx.update(|cx| {
        bridge
            .dispatch_effects(
                &mut |_, _, _| panic!("placement retry cannot create another window"),
                cx,
            )
            .unwrap();
    });
    dock.host.borrow_mut().native.as_mut().unwrap().bridge = Some(bridge);
    for _ in 0..4 {
        assert!(
            cx.update(|cx| cx.windows().contains(&shell)),
            "shell retired before its deferred destination frame: failure={:?}, session={:?}, diagnostics={:?}, reports={:?}",
            state.borrow().failure,
            state.borrow().session.snapshot(),
            dock.inspection(),
            dock.take_native_reports()
        );
        frame(&mut visual, &dock);
    }
    let ticket = state
        .borrow()
        .semantics
        .as_ref()
        .unwrap()
        .frame_submission()
        .clone();
    assert_eq!(
        ticket.snapshot().outcome(),
        WindowFrameSubmissionOutcome::Accepted
    );
    {
        let host = dock.host.borrow();
        let pending = host.mounts[&shell.window_id()].pending.as_ref().unwrap();
        assert!(pending.ticket.same_authority(&ticket));
        assert!(!state.borrow().admitted);
    }
    assert_eq!(
        state.borrow().session.snapshot().phase(),
        WindowProvisionalSessionPhase::DestinationSemanticsAccepted
    );
    cx.update_window(shell, |_, window, cx| {
        // Candidate withdrawal removes Window custody and callbacks without
        // changing the original ticket's Pending/Accepted outcome.
        assert!(window.retire_exact_frame_submission(&ticket));
        let resumed = begin_submission(&dock.host.borrow(), window, cx).unwrap();
        assert!(resumed.same_authority(&ticket));
        super::super::watch_submission(&dock.host.borrow(), &resumed, window).unwrap();
        let serial = dock.host.borrow().mounts[&shell.window_id()].serial;
        crate::adapter::presentation::observe_presented(&dock.host, &resumed, serial, window);
    })
    .unwrap();
    frame(&mut visual, &dock);
    visual.set_window_present_outcome(shell, PlatformWindowPresentOutcome::Submitted);
    for _ in 0..8 {
        frame(&mut visual, &dock);
    }
    assert_eq!(
        ticket.snapshot().outcome(),
        WindowFrameSubmissionOutcome::Submitted
    );
    assert!(
        state.borrow().admitted,
        "only the exact core admission can release the framework gate"
    );
    assert_eq!(
        state.borrow().session.snapshot().phase(),
        WindowProvisionalSessionPhase::Promoted
    );
    let settled = callbacks.provisional_placement().unwrap();
    assert_eq!(
        callbacks.create_request().unwrap().outer_bounds,
        original_create.outer_bounds
    );
    assert_eq!(
        callbacks.create_request().unwrap().binding,
        original_create.binding
    );
    assert_eq!(settled.client_bounds(), original_placement.client_bounds());
    assert_eq!(
        settled.target_display(),
        original_placement.target_display()
    );
    assert_eq!(
        settled.placement_generation(),
        original_placement.placement_generation()
    );
    assert_eq!(
        settled.outcome(),
        WindowProvisionalPlacementOutcome::Settled
    );
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(dock.native_is_quiescent(), "{:?}", dock.inspection());
}

#[open_gpui::test]
fn unadopted_preview_retains_shutdown_authority_and_blocks_anchor_retirement(
    cx: &mut TestAppContext,
) {
    let Fixture {
        dock,
        anchor,
        shell,
        bridge,
        ..
    } = Fixture::new(cx);
    let terminals = Rc::new(RefCell::new(Vec::new()));
    let observed = terminals.clone();
    let _terminal = cx.update(|cx| {
        cx.on_window_native_terminal(move |_, owner| observed.borrow_mut().push(owner))
    });
    let hold = cx.hold_window_native_terminal(shell);
    let renderer = cx.hold_window_presentation_shutdown(shell);
    dock.host.borrow_mut().native.as_mut().unwrap().bridge = Some(bridge);
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    let ticket = dock
        .host
        .borrow()
        .native
        .as_ref()
        .unwrap()
        .provisional
        .retirements[&shell.window_id()]
        .clone();
    assert!(!ticket.snapshot().quiesced());
    assert!(!ticket.snapshot().native_terminal());
    assert!(!dock.native_is_quiescent());
    assert!(
        !terminals.borrow().contains(&anchor.window_id()),
        "the anchor must retain native authority until the dependent terminal"
    );
    renderer.release();
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(512));
    cx.run_until_parked();
    assert!(ticket.snapshot().quiesced());
    // TestWindow's terminal hold delays callback delivery after actual native
    // destruction. Its original shutdown ticket can already observe terminal.
    assert!(ticket.snapshot().native_terminal());
    assert!(!terminals.borrow().contains(&anchor.window_id()));
    assert!(hold.release());
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(ticket.snapshot().native_terminal());
    assert_eq!(*terminals.borrow(), [shell.window_id(), anchor.window_id()]);
    assert!(dock.error().is_none(), "{:?}", dock.error());
    assert!(dock.native_is_quiescent());
}

#[open_gpui::test]
fn a_custom_factory_retires_the_default_preview_and_keeps_creation_ownership(
    cx: &mut TestAppContext,
) {
    let Fixture {
        dock,
        shell,
        bridge,
        ..
    } = Fixture::new(cx);
    let hold = cx.hold_window_native_terminal(shell);
    let renderer = cx.hold_window_presentation_shutdown(shell);
    let calls = Rc::new(std::cell::Cell::new(0));
    let observed = calls.clone();
    dock.set_native_window_factory(move |_, _, callbacks, cx| {
        observed.set(observed.get() + 1);
        callbacks.open(WindowOptions::default(), |_, cx| cx.new(|_| Empty), cx)
    })
    .unwrap();
    dock.host.borrow_mut().native.as_mut().unwrap().bridge = Some(bridge);
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.create_native_surface_current(
                    TARGET,
                    SOURCE,
                    NativeWindowPlacement::new(PhysicalRect::new(130.0, 140.0, 600.0, 450.0)?),
                )?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    cx.run_until_parked();
    assert_eq!(calls.get(), 1);
    let ticket = dock
        .host
        .borrow()
        .native
        .as_ref()
        .unwrap()
        .provisional
        .retirements[&shell.window_id()]
        .clone();
    assert!(!ticket.snapshot().native_terminal());
    assert!(
        dock.native_callbacks_for_window(shell.window_id())
            .is_none(),
        "an application factory cannot accidentally adopt a default visual shell"
    );
    renderer.release();
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(512));
    cx.run_until_parked();
    assert!(hold.release());
    cx.run_until_parked();
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(ticket.snapshot().native_terminal());
    assert!(dock.error().is_none(), "{:?}", dock.error());
    assert!(dock.native_is_quiescent());
}

#[open_gpui::test]
fn preview_live_placement_failure_retires_only_the_visual_shell(cx: &mut TestAppContext) {
    let Fixture {
        dock,
        anchor,
        shell,
        state,
        bridge,
        pointer,
        drag,
    } = Fixture::new(cx);
    let preview = state.borrow().preview;
    let updated = Preview {
        placement: PhysicalRect::new(
            preview.placement.x() + 24.0,
            preview.placement.y(),
            preview.placement.width(),
            preview.placement.height(),
        )
        .unwrap(),
        ..preview
    };
    dock.host
        .borrow_mut()
        .native
        .as_mut()
        .unwrap()
        .provisional
        .preview = Some(AcceptedPreview {
        owner: anchor.window_id(),
        drag,
        visual: updated,
        route: Some(PlatformNativePointerPhysicalFrame::new(
            pointer.global_position(),
            pointer.source_geometry(),
        )),
        capture: None,
    });
    cx.update(|cx| {
        assert!(dock.has_active_gesture());
        drive(&dock.host, &bridge, cx);
        assert!(state.borrow().failure.is_some());
        assert!(dock.error().is_none(), "{:?}", dock.error());
        drive(&dock.host, &bridge, cx);
    });
    cx.run_until_parked();
    assert!(dock.has_active_gesture());
    assert!(!cx.update(|cx| cx.windows().contains(&shell)));
    assert!(cx.update(|cx| cx.windows().contains(&anchor)));
}

#[open_gpui::test]
fn preview_destroyed_before_retirement_does_not_block_failed_shutdown(cx: &mut TestAppContext) {
    let Fixture {
        dock,
        shell,
        state,
        bridge,
        ..
    } = Fixture::new(cx);
    cx.update_window(shell, |_, window, cx| window.remove_window(cx))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(!cx.windows().contains(&shell));
        assert!(
            cx.native_window_retirement_ticket(shell.window_id())
                .is_none()
        );
        assert_eq!(
            state.borrow().session.snapshot().phase(),
            WindowProvisionalSessionPhase::Terminal
        );
        let mut host = dock.host.borrow_mut();
        host.native.as_mut().unwrap().bridge = Some(bridge);
        host.fail(&crate::adapter::native::NativeBridgeError::EffectReport(
            dockspace::runtime::NativeHostErrorKind::InvalidFacts,
        ));
        drop(host);
        dock.begin_failed_native_shutdown(cx).unwrap();
    });
    cx.run_until_parked();
    assert!(matches!(
        dock.native_termination(),
        Some(crate::DockNativeTermination::Failed(_))
    ));
    assert_eq!(
        dock.inspection()
            .unwrap()
            .native
            .unwrap()
            .provisional_retirement_count,
        0
    );
}

#[open_gpui::test]
fn preview_close_retries_after_the_original_window_borrow_is_released(cx: &mut TestAppContext) {
    let Fixture {
        dock,
        shell,
        state,
        bridge,
        ..
    } = Fixture::new(cx);
    dock.host
        .borrow_mut()
        .native
        .as_mut()
        .unwrap()
        .provisional
        .opening
        .take();
    cx.update_window(shell, |_, _, cx| {
        retire(&dock.host, state.clone(), cx);
        let host = dock.host.borrow();
        let provisional = &host.native.as_ref().unwrap().provisional;
        assert!(
            provisional
                .awaiting_retirement
                .contains_key(&shell.window_id())
        );
        assert!(!provisional.retirements.contains_key(&shell.window_id()));
    })
    .unwrap();
    dock.host.borrow_mut().native.as_mut().unwrap().bridge = Some(bridge);
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(!cx.update(|cx| cx.windows().contains(&shell)));
    assert_eq!(
        state.borrow().session.snapshot().phase(),
        WindowProvisionalSessionPhase::Terminal
    );
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(dock.error().is_none(), "{:?}", dock.error());
    assert!(dock.native_is_quiescent());
}

#[open_gpui::test]
fn adoption_inside_the_borrowed_shell_keeps_its_original_retirement_intent(
    cx: &mut TestAppContext,
) {
    let mut fixture = Fixture::new(cx);
    fixture.request_create(cx, true);
    let Fixture {
        dock,
        shell,
        state,
        bridge,
        ..
    } = fixture;
    assert!(
        dock.host
            .borrow()
            .native
            .as_ref()
            .unwrap()
            .provisional
            .awaiting_retirement
            .contains_key(&shell.window_id())
    );
    assert!(
        dock.native_callbacks_for_window(shell.window_id())
            .is_none()
    );
    dock.host.borrow_mut().native.as_mut().unwrap().bridge = Some(bridge);
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(!cx.update(|cx| cx.windows().contains(&shell)));
    assert_eq!(
        state.borrow().session.snapshot().phase(),
        WindowProvisionalSessionPhase::Terminal
    );
    assert!(
        dock.take_native_reports()
            .iter()
            .any(|report| matches!(report, super::super::NativeReport::OpenFailed { .. }))
    );
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(dock.error().is_none(), "{:?}", dock.error());
    assert!(dock.native_is_quiescent());
}
