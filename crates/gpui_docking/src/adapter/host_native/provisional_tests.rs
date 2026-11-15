//! Physical preview ownership and the original core create/presentation pipeline.

use super::*;
use crate::adapter::{DockPane, GpuiDockspaceConfig};
use dockspace::{
    model::{
        DockspaceLayout, DockspaceNode, DockspaceRootLayout, DockspaceSurfaceLayout, ItemId,
        NativeWindowPlacement, RootId,
    },
    policy::DockPolicy,
    runtime::DockspaceSession,
};
use open_gpui::{
    Bounds, DevicePixels, Empty, PlatformPhysicalDisplayObservation,
    PlatformWindowPhysicalGeometry, PlatformWindowPresentOutcome, RequestFrameOptions,
    TestAppContext, VisualTestContext, WindowMutationDomain, point, size,
};

const SOURCE: SurfaceId = SurfaceId::new(41);
const TARGET: SurfaceId = SurfaceId::new(42);

struct Fixture {
    dock: DockSurface,
    anchor: AnyWindowHandle,
    shell: AnyWindowHandle,
    state: Rc<RefCell<ProvisionalWindow>>,
    bridge: NativeBridge,
    pointer: PlatformNativePointerPhysicalFrame,
}

impl Fixture {
    fn new(cx: &mut TestAppContext) -> Self {
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
                        DockspaceNode::tabs([ItemId::new(surface.get())]),
                    ),
                )
            }))
            .unwrap();
            let mut policy = DockPolicy::default();
            policy.set_allow_native_surfaces(true);
            let session = DockspaceSession::from_layout(layout, policy).unwrap();
            let panes = [SOURCE, TARGET].map(|surface| {
                DockPane::new(
                    ItemId::new(surface.get()),
                    "Original View",
                    cx.new(|_| Empty),
                )
            });
            DockSurface::new_managed(session, panes, GpuiDockspaceConfig::default(), cx).unwrap()
        });
        cx.defer_next_window_frame_requests();
        let anchor = cx
            .update(|cx| dock.open_native_root(SOURCE, true, WindowOptions::default(), cx))
            .unwrap();
        let mut source = VisualTestContext::from_window(anchor, cx);
        for _ in 0..4 {
            frame(&mut source, &dock);
        }
        let bridge = dock
            .host
            .borrow_mut()
            .native
            .as_mut()
            .unwrap()
            .bridge
            .take()
            .unwrap();
        let bounds = Bounds::new(
            point(DevicePixels(100), DevicePixels(100)),
            size(DevicePixels(600), DevicePixels(450)),
        );
        let pointer = PlatformNativePointerPhysicalFrame::new(
            point(DevicePixels(180), DevicePixels(180)),
            PlatformWindowPhysicalGeometry::try_new(bounds, 2.0).unwrap(),
        )
        .with_target_display(display)
        .unwrap();
        let preview = Preview {
            host_surface: SOURCE,
            target_surface: TARGET,
            placement: PhysicalRect::new(100.0, 100.0, 600.0, 450.0).unwrap(),
        };
        cx.defer_next_window_frame_requests();
        let state = cx
            .update(|cx| {
                open_preview(
                    &dock.host,
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
        }
    }

    fn request_create(&mut self, cx: &mut TestAppContext, inside_shell: bool) {
        self.state.borrow_mut().release_frame = Some(self.pointer);
        cx.update(|cx| {
            self.dock
                .update(
                    |frame| {
                        frame.create_native_surface_current(
                            TARGET,
                            SOURCE,
                            NativeWindowPlacement::new(PhysicalRect::new(
                                130.0, 140.0, 600.0, 450.0,
                            )?),
                        )?;
                        Ok(())
                    },
                    cx,
                )
                .unwrap();
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
            self.bridge
                .dispatch_effects(
                    &mut |request, callbacks, cx| {
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

fn frame(visual: &mut VisualTestContext, dock: &DockSurface) {
    assert!(visual.simulate_frame(RequestFrameOptions {
        force_render: true,
        require_presentation: true,
        ..Default::default()
    }));
    assert!(dock.error().is_none(), "{:?}", dock.error());
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
    assert_eq!(
        original_create.outer_bounds,
        PhysicalRect::new(130.0, 140.0, 600.0, 450.0).unwrap()
    );
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
    assert_eq!(
        original_placement.client_bounds(),
        Bounds::new(
            point(DevicePixels(130), DevicePixels(140)),
            size(DevicePixels(600), DevicePixels(450)),
        )
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
        crate::adapter::presentation::observe(&dock.host, &resumed, serial, window);
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
