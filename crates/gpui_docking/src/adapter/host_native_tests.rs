//! Root registration failures retain original GPUI and core lifetime authority.

#[path = "presentation_tests.rs"]
mod presentation_order;

use dockspace::{
    model::{
        DockspaceLayout, DockspaceNode, DockspaceRootLayout, DockspaceSurfaceLayout, ItemId,
        RootId, SurfaceId,
    },
    policy::DockPolicy,
    runtime::{
        DockspaceManipulationStatus, DockspaceRuntimeErrorKind, DockspaceSession, HostFrameReport,
        HostInputOutcome, NativeHostErrorKind, NativeReceiverAnswer, SurfaceUnavailableReason,
    },
};
use open_gpui::{
    App, AppContext as _, Empty, TestAppContext, WindowOpenFailureStage, WindowOptions,
};

use super::{DockSurface, NativeBridgeError, NativeCloseDecision, NativeOpenFailure, NativeReport};
use crate::adapter::{DockPane, GpuiDockspaceConfig};

const FIRST: SurfaceId = SurfaceId::new(1);
const SECOND: SurfaceId = SurfaceId::new(2);

fn layout() -> DockspaceLayout {
    DockspaceLayout::new([FIRST, SECOND].map(|surface| {
        DockspaceSurfaceLayout::new(
            surface,
            DockspaceRootLayout::new(
                RootId::new(surface.get()),
                DockspaceNode::tabs([ItemId::new(surface.get())]),
            ),
        )
    }))
    .unwrap()
}

fn workspace(cx: &mut App) -> DockSurface {
    let session = DockspaceSession::from_layout(layout(), DockPolicy::default()).unwrap();
    let panes = [FIRST, SECOND].map(|surface| {
        DockPane::new(
            ItemId::new(surface.get()),
            "Original View",
            cx.new(|_| Empty),
        )
    });
    DockSurface::new_managed(session, panes, GpuiDockspaceConfig::default(), cx).unwrap()
}

fn original_ticket(error: &NativeOpenFailure) -> open_gpui::WindowPresentationShutdownTicket {
    let source = error.source.as_ref().expect("original typed GPUI error");
    let ticket = error
        .retirement_ticket
        .as_ref()
        .expect("original native rollback owner");
    assert!(ticket.same_authority(source.retirement_ticket().unwrap()));
    ticket.clone()
}

fn finish(cx: &mut TestAppContext, dock: &DockSurface) {
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(dock.error().is_none(), "{:?}", dock.error());
}

pub(in crate::adapter) fn capture_feedback(
    cx: &mut TestAppContext,
    dock: &DockSurface,
    dispatch: impl FnOnce(&mut TestAppContext),
) -> Vec<DockspaceManipulationStatus> {
    // Let GPUI route its genuine native callbacks while retaining the bridge so
    // the test can read the core feedback before the adapter consumes the report.
    let mut bridge = dock
        .host
        .borrow_mut()
        .native
        .as_mut()
        .unwrap()
        .bridge
        .take()
        .unwrap();
    dispatch(cx);
    cx.run_until_parked();
    let feedback = cx.update(|cx| {
        let mut host = dock.host.borrow_mut();
        let mut feedback = Vec::new();
        for _ in 0..4 {
            bridge.collect(&mut host.session).unwrap();
            if host.session.has_pending_native_input() {
                let super::HostState {
                    session,
                    mounts,
                    native,
                    ..
                } = &mut *host;
                let report = super::begin_frame(session, mounts, native.as_ref(), None, cx)
                    .unwrap()
                    .commit()
                    .unwrap();
                feedback.extend(
                    report
                        .manipulation_feedback()
                        .iter()
                        .map(|entry| entry.status()),
                );
                host.invalidate_report(&report);
                bridge.accept_report(report);
            }
            if bridge.transport().is_empty() {
                break;
            }
        }
        assert!(bridge.transport().is_empty());
        host.native.as_mut().unwrap().bridge = Some(bridge);
        feedback
    });
    finish(cx, dock);
    feedback
}

#[open_gpui::test]
fn bounded_observations_preserve_original_deferred_close_decisions(cx: &mut TestAppContext) {
    let dock = cx.update(workspace);
    dock.set_native_close_policy(NativeCloseDecision::Defer)
        .unwrap();
    cx.update(|cx| dock.open_native_root(SECOND, true, WindowOptions::default(), cx))
        .unwrap();
    let window = cx
        .update(|cx| dock.open_native_root(FIRST, false, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    dock.take_native_reports();
    assert!(!cx.simulate_window_close(window));
    finish(cx, &dock);
    let close = {
        let mut host = dock.host.borrow_mut();
        let reports = &mut host.native.as_mut().unwrap().reports;
        let close = reports
            .iter()
            .find_map(|report| match report {
                NativeReport::CloseRequested(close) => Some(*close),
                _ => None,
            })
            .expect("original deferred close");
        for _ in 0..1024 {
            reports.push_deferred_close(close);
        }
        for _ in 0..1024 {
            reports.push_back(NativeReport::Unsupported {
                operation: "history pressure",
            });
        }
        assert_eq!(reports.len(), 257);
        close
    };
    assert!(
        dock.inspection()
            .unwrap()
            .native
            .unwrap()
            .discarded_report_count
            > 0
    );
    let reports = dock.take_native_reports();
    assert!(
        matches!(reports.first(), Some(NativeReport::CloseRequested(retained)) if *retained == close)
    );
    cx.update(|cx| dock.decide_native_close(close, NativeCloseDecision::Veto, cx))
        .unwrap();
    finish(cx, &dock);
    assert!(cx.update_window(window, |_, _, _| ()).is_ok());
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    finish(cx, &dock);
    assert!(dock.native_is_quiescent());
}

#[open_gpui::test]
fn primary_failure_rejects_new_native_roots_before_creating_physical_owners(
    cx: &mut TestAppContext,
) {
    let dock = cx.update(workspace);
    dock.host.borrow_mut().failure = Some(crate::DockHostFailure {
        message: "original protocol rejection".into(),
        runtime_kind: Some(DockspaceRuntimeErrorKind::HostProtocol),
        native_kind: None,
    });
    let before = dock.inspection().unwrap();
    cx.update(|cx| {
        let failure = dock
            .open_native_root(FIRST, true, WindowOptions::default(), cx)
            .unwrap_err();
        assert!(matches!(
            failure.root_binding,
            Some(NativeBridgeError::Admission(_))
        ));
        assert!(failure.source.is_none());
        assert!(failure.retirement_ticket.is_none());
        assert!(cx.windows().is_empty());
    });
    assert_eq!(dock.inspection().unwrap(), before);
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    assert!(
        dock.native_is_quiescent(),
        "the admission error must not block original cleanup"
    );
    assert_eq!(
        dock.failure().unwrap().message,
        "original protocol rejection"
    );
}

#[open_gpui::test]
fn quiescence_notification_waits_for_original_native_terminal_and_runs_outside_the_borrow(
    cx: &mut TestAppContext,
) {
    let dock = cx.update(workspace);
    let window = cx
        .update(|cx| dock.open_native_root(FIRST, true, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    let held = cx.hold_window_native_terminal(window);
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let observed = calls.clone();
    let weak = std::rc::Rc::downgrade(&dock.host);
    let terminations = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let observed_terminations = terminations.clone();
    cx.update(|cx| {
        dock.when_native_terminated(
            move |disposition, _| observed_terminations.borrow_mut().push(disposition),
            cx,
        )
        .unwrap();
        dock.when_native_quiescent(
            move |_| {
                let dock = DockSurface {
                    host: weak.upgrade().unwrap(),
                };
                assert!(dock.native_is_quiescent());
                assert!(dock.inspection().unwrap().native.unwrap().quiescent);
                observed.set(observed.get() + 1);
            },
            cx,
        )
        .unwrap();
        dock.begin_native_shutdown(cx).unwrap();
    });
    cx.run_until_parked();
    assert_eq!(calls.get(), 0);
    assert!(terminations.borrow().is_empty());
    assert!(!dock.native_is_quiescent());
    assert!(held.release());
    finish(cx, &dock);
    assert_eq!(calls.get(), 1);
    assert_eq!(
        *terminations.borrow(),
        [crate::DockNativeTermination::Drained]
    );
    cx.update(|cx| dock.pump_native(cx));
    assert_eq!(calls.get(), 1);
}

#[open_gpui::test]
fn failure_termination_requires_an_original_failure_and_does_not_freeze_a_healthy_host(
    cx: &mut TestAppContext,
) {
    let dock = cx.update(workspace);
    let window = cx
        .update(|cx| dock.open_native_root(FIRST, true, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    let before = dock.inspection().unwrap();
    cx.update(|cx| dock.begin_failed_native_shutdown(cx))
        .unwrap_err();
    assert_eq!(dock.inspection().unwrap(), before);
    cx.update_window(window, |_, _, _| ()).unwrap();
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    finish(cx, &dock);
}

#[open_gpui::test]
fn failed_anchor_rollback_cannot_block_retirement_of_its_existing_root_dependency(
    cx: &mut TestAppContext,
) {
    let dock = cx.update(workspace);
    let root = cx
        .update(|cx| dock.open_native_root(FIRST, false, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    cx.set_next_window_present_outcome(open_gpui::PlatformWindowPresentOutcome::Rejected);
    let ticket = cx.update(|cx| {
        let error = dock
            .open_native_root(SECOND, true, WindowOptions::default(), cx)
            .unwrap_err();
        let ticket = original_ticket(&error);
        assert!(!ticket.snapshot().native_terminal());
        dock.host.borrow_mut().failure = Some(crate::DockHostFailure {
            message: "original failed anchor generation".into(),
            runtime_kind: Some(DockspaceRuntimeErrorKind::HostProtocol),
            native_kind: None,
        });
        dock.begin_failed_native_shutdown(cx).unwrap();
        ticket
    });
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(ticket.snapshot().quiesced() && ticket.snapshot().native_terminal());
    assert!(cx.update_window(root, |_, _, _| ()).is_err());
    assert!(matches!(
        dock.native_termination(),
        Some(crate::DockNativeTermination::Failed(_))
    ));
    assert!(!dock.native_is_quiescent());
}

#[open_gpui::test]
fn unbound_root_destroyed_before_retirement_does_not_block_failed_shutdown(
    cx: &mut TestAppContext,
) {
    let dock = cx.update(workspace);
    let anchor = cx
        .update(|cx| dock.open_native_root(FIRST, true, WindowOptions::default(), cx))
        .unwrap();
    let root = cx
        .update(|cx| {
            cx.open_window_detailed(WindowOptions::default(), |window, cx| {
                // Exercise the state after registration was recorded but before
                // a native callback installation could claim the window.
                let handle = window.window_handle();
                let mut host = dock.host.borrow_mut();
                let native = host.native.as_mut().unwrap();
                native.registrations.insert(
                    handle.window_id(),
                    super::RootRegistration {
                        surface: SECOND,
                        window: handle,
                        anchor: false,
                        retirement: super::termination::UnboundWindowRetirement::observe(
                            handle.window_id(),
                            &native.transport,
                            cx,
                        ),
                    },
                );
                cx.new(|_| Empty)
            })
        })
        .unwrap()
        .into();
    // Keep pump callbacks from recovering the registration before its window
    // goes away. The failed native generation still owns the same registration.
    let bridge = dock
        .host
        .borrow_mut()
        .native
        .as_mut()
        .unwrap()
        .bridge
        .take()
        .unwrap();
    let owner = cx
        .update_window(root, |_, window, cx| {
            let owner = window.window_handle().window_id();
            window.remove_window(cx);
            owner
        })
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(!cx.windows().contains(&root));
        assert!(cx.native_window_retirement_ticket(owner).is_none());
        let mut host = dock.host.borrow_mut();
        assert!(
            host.native
                .as_ref()
                .unwrap()
                .registrations
                .contains_key(&owner)
        );
        host.native.as_mut().unwrap().bridge = Some(bridge);
        host.fail(&NativeBridgeError::EffectReport(
            NativeHostErrorKind::InvalidFacts,
        ));
        drop(host);
        dock.begin_failed_native_shutdown(cx).unwrap();
    });
    cx.run_until_parked();
    assert!(matches!(
        dock.native_termination(),
        Some(crate::DockNativeTermination::Failed(_))
    ));
    assert!(cx.update_window(anchor, |_, _, _| ()).is_err());
}

#[open_gpui::test]
fn failed_shutdown_retries_the_original_window_borrow_without_another_external_wake(
    cx: &mut TestAppContext,
) {
    let dock = cx.update(workspace);
    let window = cx
        .update(|cx| dock.open_native_root(FIRST, true, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    dock.host.borrow_mut().failure = Some(crate::DockHostFailure {
        message: "original failed generation".into(),
        runtime_kind: Some(DockspaceRuntimeErrorKind::HostProtocol),
        native_kind: None,
    });
    cx.update_window(window, |_, _, cx| dock.begin_failed_native_shutdown(cx))
        .unwrap()
        .unwrap();
    cx.run_until_parked();
    assert!(matches!(
        dock.native_termination(),
        Some(crate::DockNativeTermination::Failed(_))
    ));
    assert!(!dock.native_is_quiescent());
}

#[open_gpui::test]
fn failed_generation_retires_children_and_anchor_without_claiming_semantic_drain(
    cx: &mut TestAppContext,
) {
    use dockspace::{
        geometry::PhysicalRect,
        model::NativeWindowPlacement,
        runtime::{
            NativeDesktopPointerLocation, NativeDesktopPosition, NativePointerButton,
            NativePointerEvent, NativePointerHover, NativePointerId, NativePointerInput,
            NativePointerOwner,
        },
    };

    let dock = cx.update(|cx| {
        let mut policy = DockPolicy::default();
        policy.set_allow_native_surfaces(true);
        let session = DockspaceSession::from_layout(layout(), policy).unwrap();
        DockSurface::new_managed(
            session,
            [FIRST, SECOND].map(|surface| {
                DockPane::new(
                    ItemId::new(surface.get()),
                    "Original View",
                    cx.new(|_| Empty),
                )
            }),
            GpuiDockspaceConfig::default(),
            cx,
        )
        .unwrap()
    });
    let sibling = cx.update(workspace);
    let sibling_window = cx
        .update(|cx| sibling.open_native_root(FIRST, true, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &sibling);
    let anchor = cx
        .update(|cx| dock.open_native_root(FIRST, true, WindowOptions::default(), cx))
        .unwrap();
    cx.update(|cx| {
        dock.update(
            |frame| {
                frame.create_native_surface_current(
                    SECOND,
                    FIRST,
                    NativeWindowPlacement::new(PhysicalRect::new(100.0, 100.0, 600.0, 400.0)?),
                )?;
                Ok(())
            },
            cx,
        )
        .unwrap();
    });
    finish(cx, &dock);
    let child = cx.update(|cx| {
        cx.windows()
            .into_iter()
            .find(|window| *window != anchor && *window != sibling_window)
            .unwrap()
    });
    let binding = dock
        .native_callbacks_for_window(child.window_id())
        .unwrap()
        .binding();
    let renderer = cx.hold_window_presentation_shutdown(child);
    let native_terminal = cx.hold_window_native_terminal(child);
    let terminated = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let observed = terminated.clone();
    let weak = std::rc::Rc::downgrade(&dock.host);
    cx.update(|cx| {
        dock.when_native_terminated(
            move |disposition, _| {
                assert!(
                    weak.upgrade().unwrap().try_borrow_mut().is_ok(),
                    "termination runs outside host borrows"
                );
                observed.borrow_mut().push(disposition);
            },
            cx,
        )
        .unwrap();
    });
    {
        let mut host = dock.host.borrow_mut();
        for event in [
            NativePointerEvent::ButtonPressed(NativePointerButton::Primary),
            NativePointerEvent::ButtonPressed(NativePointerButton::Primary),
        ] {
            host.session
                .record_native_pointer(NativePointerInput::new(
                    NativePointerId::new(1),
                    event,
                    NativeDesktopPointerLocation::new(
                        NativeDesktopPosition::Unknown,
                        NativePointerHover::Unknown,
                        None,
                    ),
                    NativePointerOwner::Native(binding),
                    NativePointerOwner::None,
                ))
                .unwrap();
        }
    }
    cx.update(|cx| dock.pump_native(cx));
    assert!(dock.host.borrow().session.has_pending_native_input());
    assert!(dock.failure().unwrap().is_primary_protocol_failure());
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(cx.windows().contains(&anchor)));
    assert!(dock.native_termination().is_none());
    let original_failure = dock.failure().unwrap();
    let recorded = dock.host.borrow().session.native_input_recorded_count();
    cx.update(|cx| dock.begin_failed_native_shutdown(cx))
        .unwrap();
    cx.run_until_parked();
    assert!(terminated.borrow().is_empty());
    assert!(dock.native_termination().is_none());
    renderer.release();
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(512));
    cx.run_until_parked();
    assert!(terminated.borrow().is_empty());
    assert!(
        dock.native_termination().is_none(),
        "renderer quiescence cannot replace native terminal"
    );
    assert!(native_terminal.release());
    cx.run_until_parked();
    cx.update(|cx| {
        dock.pump_native(cx);
        assert!(
            !cx.windows().contains(&child),
            "the child physically retires"
        );
        assert!(
            !cx.windows().contains(&anchor),
            "the anchor must not wait for an irreparable semantic prefix"
        );
    });
    assert!(!dock.native_is_quiescent());
    assert!(dock.host.borrow().session.has_pending_native_input());
    assert_eq!(
        dock.host.borrow().session.native_input_recorded_count(),
        recorded
    );
    let disposition = crate::DockNativeTermination::Failed(original_failure);
    assert_eq!(dock.native_termination(), Some(disposition.clone()));
    assert_eq!(*terminated.borrow(), [disposition]);
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert_eq!(terminated.borrow().len(), 1, "termination is reported once");

    // Application-owned panes and committed topology acquire entirely new
    // session/provider authority. No queued native records enter the new host.
    let fresh = cx.update(|cx| {
        let old = dock.host.borrow();
        let session = DockspaceSession::from_layout(
            old.session.view().to_layout(),
            old.session.policy().clone(),
        )
        .unwrap();
        DockSurface::new_managed(
            session,
            old.panels.values().cloned(),
            old.config.clone(),
            cx,
        )
        .unwrap()
    });
    let fresh_anchor = cx
        .update(|cx| fresh.open_native_root(FIRST, true, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &fresh);
    assert_ne!(
        fresh
            .native_callbacks_for_window(fresh_anchor.window_id())
            .unwrap()
            .binding(),
        binding
    );
    assert!(!fresh.host.borrow().session.has_pending_native_input());
    {
        let old = dock.host.borrow();
        let new = fresh.host.borrow();
        for (item, pane) in &old.panels {
            assert_eq!(pane.view.entity_id(), new.panels[item].view.entity_id());
        }
    }
    cx.update(|cx| fresh.begin_native_shutdown(cx)).unwrap();
    finish(cx, &fresh);
    assert_eq!(
        fresh.native_termination(),
        Some(crate::DockNativeTermination::Drained)
    );
    assert!(cx.update_window(sibling_window, |_, _, _| ()).is_ok());
    assert!(!sibling.native_is_quiescent());
    cx.update(|cx| sibling.begin_native_shutdown(cx)).unwrap();
    finish(cx, &sibling);
}

fn commit_native(session: &mut DockspaceSession) -> HostFrameReport {
    let mut frame = session
        .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
        .unwrap();
    frame
        .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
        .unwrap();
    frame.commit().unwrap()
}

fn collect_without_report_dispatch(
    host: &mut super::HostState,
    bridge: &mut super::NativeBridge,
    cx: &mut App,
) {
    // These tests isolate report-policy failures. Commit the real ingress and
    // presentation barriers while leaving their reports for process_reports.
    loop {
        bridge.collect(&mut host.session).unwrap();
        if host.session.has_pending_native_input() {
            let report = commit_native(&mut host.session);
            host.invalidate_report(&report);
            bridge.accept_report(report);
        }
        let Some(terminal) = bridge.transport().take_presentation() else {
            break;
        };
        crate::adapter::presentation::settle_native(host, terminal, None, cx).unwrap();
        while let Some(report) = host.native.as_mut().unwrap().pending_reports.pop_front() {
            bridge.accept_report(report);
        }
    }
    assert!(bridge.transport().is_empty());
}

#[open_gpui::test]
fn shutdown_rejects_application_updates_before_invoking_their_callback(cx: &mut TestAppContext) {
    let dock = cx.update(workspace);
    cx.update(|cx| {
        dock.begin_native_shutdown(cx).unwrap();
        let before = dock.inspection().unwrap();
        let mut invoked = false;
        let error = dock
            .update(
                |_| {
                    invoked = true;
                    Ok(())
                },
                cx,
            )
            .expect_err("shutdown must reject new application intent");
        assert!(!invoked, "rejected intent must not enter application code");
        assert!(matches!(
            error.downcast_ref::<NativeBridgeError>(),
            Some(NativeBridgeError::Draining)
        ));
        assert_eq!(dock.inspection().unwrap(), before);
    });
    finish(cx, &dock);
    assert!(dock.native_is_quiescent());
}

#[open_gpui::test]
fn shutdown_rejects_pane_registration_without_changing_the_host(cx: &mut TestAppContext) {
    let dock = cx.update(workspace);
    cx.update(|cx| {
        dock.begin_native_shutdown(cx).unwrap();
        let before = dock.inspection().unwrap();
        let item = ItemId::new(3);
        let pane = DockPane::new(item, "Rejected View", cx.new(|_| Empty));
        let error = dock
            .open_pane(
                pane,
                dockspace::model::DockPlacement::Center(dockspace::model::DockAnchor::Item(
                    ItemId::new(1),
                )),
                cx,
            )
            .expect_err("shutdown must reject new pane registration");
        assert!(matches!(
            error.downcast_ref::<NativeBridgeError>(),
            Some(NativeBridgeError::Draining)
        ));
        assert_eq!(dock.inspection().unwrap(), before);
        assert!(!dock.host.borrow().panels.contains_key(&item));
    });
    finish(cx, &dock);
    assert!(dock.native_is_quiescent());
}

#[open_gpui::test]
fn a_close_retired_by_the_same_native_reset_does_not_lose_the_terminal_suffix(
    cx: &mut TestAppContext,
) {
    let dock = cx.update(workspace);
    cx.defer_next_window_frame_requests();
    cx.update(|cx| dock.open_native_root(FIRST, true, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    cx.defer_next_window_frame_requests();
    let second = cx
        .update(|cx| dock.open_native_root(SECOND, false, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    dock.take_native_reports();
    let mut bridge = dock
        .host
        .borrow_mut()
        .native
        .as_mut()
        .unwrap()
        .bridge
        .take()
        .unwrap();
    assert!(!cx.simulate_window_close(second));
    let close = cx.update(|_| {
        let mut host = dock.host.borrow_mut();
        bridge.collect(&mut host.session).unwrap();
        let reset = host.session.prepare_layout_reset(layout()).unwrap();
        let mut frame = host
            .session
            .begin_native_layout_reset_frame(&reset, |_| NativeReceiverAnswer::Unknown)
            .unwrap();
        frame
            .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
            .unwrap();
        let report = frame.commit().unwrap();
        let requests: Vec<_> = report
            .inputs()
            .iter()
            .filter_map(|input| match input {
                HostInputOutcome::NativePlatformSnapshotApplied { close_requests }
                | HostInputOutcome::NativeCloseObservationApplied { close_requests } => {
                    Some(close_requests)
                }
                _ => None,
            })
            .flatten()
            .copied()
            .collect();
        assert_eq!(requests.len(), 1, "{:?}", report.inputs());
        let close = requests[0];
        assert_eq!(close.surface(), SECOND);
        assert!(!host.session.is_current_native_binding(close.binding()));
        assert!(host.session.recognizes_native_binding(close.binding()));
        assert_eq!(
            host.session
                .cancel_native_surface_close(close)
                .unwrap_err()
                .native_kind(),
            Some(NativeHostErrorKind::StaleBinding)
        );
        host.invalidate_report(&report);
        bridge.accept_report(report);
        let inventory = bridge
            .snapshot(&host.session, None)
            .expect("continuing physical roots retain exact inventory before a new draw");
        assert_eq!(
            inventory.len(),
            host.session.native_window_bindings().count()
        );
        assert!(!inventory.is_empty());
        assert!(
            inventory
                .iter()
                .all(|(_, facts)| { *facts == dockspace::runtime::NativeWindowFacts::live() })
        );
        close
    });
    cx.update_window(second, |_, window, cx| window.remove_window(cx))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let mut host = dock.host.borrow_mut();
        collect_without_report_dispatch(&mut host, &mut bridge, cx);
        let terminal = bridge.binding_for_window(second.window_id()).unwrap();
        let report = commit_native(&mut host.session);
        host.invalidate_report(&report);
        bridge.accept_report(report);
        let retired: Vec<_> = host
            .session
            .retained_native_surface_bindings_for_window(terminal)
            .collect();
        assert!(!retired.is_empty());
        let result = super::process_reports(&mut host, &mut bridge, cx);
        assert!(result.is_ok(), "{result:?}");
        let native = host.native.as_mut().unwrap();
        assert!(native.reports.iter().any(|report| matches!(
            report,
            NativeReport::StaleCallback(binding) if *binding == close.binding().window()
        )));
        assert_eq!(
            native
                .reports
                .iter()
                .filter(|report| matches!(
                    report,
                    NativeReport::NativeTerminal(binding) if *binding == terminal
                ))
                .count(),
            1
        );
        assert!(
            retired
                .iter()
                .all(|binding| native.retired.contains(binding))
        );
        native.bridge = Some(bridge);
    });
    finish(cx, &dock);
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    finish(cx, &dock);
    assert!(dock.native_is_quiescent());
}

#[open_gpui::test]
fn a_failed_close_policy_preserves_the_current_report_and_terminal_suffix(cx: &mut TestAppContext) {
    let dock = cx.update(workspace);
    dock.set_native_close_policy(NativeCloseDecision::Veto)
        .unwrap();
    cx.defer_next_window_frame_requests();
    let first = cx
        .update(|cx| dock.open_native_root(FIRST, false, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    cx.defer_next_window_frame_requests();
    let second = cx
        .update(|cx| dock.open_native_root(SECOND, false, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    dock.take_native_reports();
    let mut bridge = dock
        .host
        .borrow_mut()
        .native
        .as_mut()
        .unwrap()
        .bridge
        .take()
        .unwrap();
    let terminal = bridge.binding_for_window(second.window_id()).unwrap();
    let held = cx.hold_window_native_terminal(second);
    cx.update_window(second, |_, window, cx| window.remove_window(cx))
        .unwrap();
    cx.run_until_parked();
    assert!(!cx.simulate_window_close(first));
    cx.update(|cx| {
        let mut host = dock.host.borrow_mut();
        collect_without_report_dispatch(&mut host, &mut bridge, cx);
    });
    assert!(held.release());
    cx.run_until_parked();
    cx.update(|cx| {
        let mut host = dock.host.borrow_mut();
        collect_without_report_dispatch(&mut host, &mut bridge, cx);
        let retired: Vec<_> = host
            .session
            .retained_native_surface_bindings_for_window(terminal)
            .collect();
        assert!(!retired.is_empty());
        let unavailable = DockspaceSession::from_layout(layout(), DockPolicy::default()).unwrap();
        let original = std::mem::replace(&mut host.session, unavailable);
        let result = super::process_reports(&mut host, &mut bridge, cx);
        host.session = original;
        let error = result.unwrap_err();
        assert!(matches!(
            error.downcast_ref::<NativeBridgeError>(),
            Some(NativeBridgeError::Core(error))
                if error.native_kind() == Some(NativeHostErrorKind::NotEnabled)
        ));
        let native = host.native.as_ref().unwrap();
        assert_eq!(native.reports.len(), 1);
        assert!(matches!(
            native.reports.front(),
            Some(NativeReport::LogicalClosed(binding)) if *binding == terminal
        ));
        assert!(native.retired.is_empty());
        super::process_reports(&mut host, &mut bridge, cx).unwrap();
        let native = host.native.as_mut().unwrap();
        assert_eq!(native.reports.len(), 3);
        assert!(matches!(
            native.reports.get(1),
            Some(NativeReport::CloseRequested(close)) if close.surface() == FIRST
        ));
        assert!(matches!(
            native.reports.back(),
            Some(NativeReport::NativeTerminal(binding)) if *binding == terminal
        ));
        assert!(
            retired
                .iter()
                .all(|binding| native.retired.contains(binding))
        );
        native.bridge = Some(bridge);
    });
    finish(cx, &dock);
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    finish(cx, &dock);
    assert!(dock.native_is_quiescent());
}

#[open_gpui::test]
fn a_busy_candidate_rejects_root_registration_before_appending_and_allows_retry(
    cx: &mut TestAppContext,
) {
    let dock = cx.update(workspace);
    cx.defer_next_window_frame_requests();
    let first = cx
        .update(|cx| dock.open_native_root(FIRST, true, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    cx.defer_next_window_frame_requests();
    let ticket = cx
        .update_window(first, |_, window, cx| {
            let update = {
                let mut host = dock.host.borrow_mut();
                let super::HostState {
                    session,
                    mounts,
                    native,
                    ..
                } = &mut *host;
                super::begin_frame(session, mounts, native.as_ref(), Some(window), cx)
                    .unwrap()
                    .suspend()
            };
            let recorded = dock.host.borrow().session.native_input_recorded_count();
            let error = dock
                .open_native_root(SECOND, false, WindowOptions::default(), cx)
                .unwrap_err();
            assert!(
                matches!(error.root_binding.as_ref(), Some(NativeBridgeError::Core(error))
            if error.kind() == DockspaceRuntimeErrorKind::UpdateInProgress)
            );
            assert_eq!(error.stage, Some(WindowOpenFailureStage::ClosedDuringBuild));
            let ticket = original_ticket(&error);
            assert!(!ticket.snapshot().native_terminal());
            assert_eq!(
                dock.host.borrow().session.native_input_recorded_count(),
                recorded,
                "a root failure cannot leave a registration behind the original candidate"
            );
            assert!(
                dock.host
                    .borrow()
                    .native
                    .as_ref()
                    .unwrap()
                    .registrations
                    .is_empty()
            );
            assert!(
                dock.host.borrow().native.as_ref().unwrap().retirements
                    [&ticket.snapshot().window_id()]
                    .same_authority(&ticket)
            );
            drop(update);
            ticket
        })
        .unwrap();
    finish(cx, &dock);
    assert!(
        ticket.snapshot().quiesced() && ticket.snapshot().native_terminal(),
        "{:?}",
        ticket.snapshot()
    );
    assert_eq!(
        dock.host.borrow().session.native_window_bindings().count(),
        1
    );
    cx.defer_next_window_frame_requests();
    let second = cx
        .update(|cx| dock.open_native_root(SECOND, false, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    assert_ne!(second.window_id(), ticket.snapshot().window_id());
    assert_eq!(
        dock.host.borrow().session.native_window_bindings().count(),
        2
    );
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    finish(cx, &dock);
    assert!(dock.native_is_quiescent());
}

#[open_gpui::test]
fn a_second_anchor_failure_preserves_both_errors_and_the_original_retirement_ticket(
    cx: &mut TestAppContext,
) {
    let dock = cx.update(workspace);
    cx.defer_next_window_frame_requests();
    let first = cx
        .update(|cx| dock.open_native_root(FIRST, true, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    let recorded = dock.host.borrow().session.native_input_recorded_count();
    cx.defer_next_window_frame_requests();
    let ticket = cx.update(|cx| {
        let error = dock
            .open_native_root(SECOND, true, WindowOptions::default(), cx)
            .unwrap_err();
        assert!(matches!(
            error.root_binding,
            Some(NativeBridgeError::WindowAlreadyBound)
        ));
        assert_eq!(error.stage, Some(WindowOpenFailureStage::ClosedDuringBuild));
        let ticket = original_ticket(&error);
        assert!(!ticket.snapshot().native_terminal());
        ticket
    });
    finish(cx, &dock);
    assert_eq!(
        dock.host.borrow().session.native_input_recorded_count(),
        recorded
    );
    assert!(ticket.snapshot().quiesced() && ticket.snapshot().native_terminal());
    cx.update(|cx| assert!(cx.windows().contains(&first)));
    cx.defer_next_window_frame_requests();
    cx.update(|cx| dock.open_native_root(SECOND, false, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    finish(cx, &dock);
    assert!(dock.native_is_quiescent());
}

#[open_gpui::test]
fn a_root_failure_after_registration_retires_semantics_before_same_surface_retry(
    cx: &mut TestAppContext,
) {
    let dock = cx.update(workspace);
    cx.defer_next_window_frame_requests();
    cx.update(|cx| dock.open_native_root(FIRST, true, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    cx.set_next_window_present_outcome(open_gpui::PlatformWindowPresentOutcome::Rejected);
    cx.defer_next_window_frame_requests();
    let ticket = cx.update(|cx| {
        let error = dock
            .open_native_root(SECOND, false, WindowOptions::default(), cx)
            .unwrap_err();
        assert!(
            error.root_binding.is_none(),
            "registration succeeded before native presentation failed"
        );
        assert_eq!(
            error.stage,
            Some(WindowOpenFailureStage::BeforeVisibilityPresentation)
        );
        let ticket = original_ticket(&error);
        assert!(
            dock.native_callbacks_for_window(ticket.snapshot().window_id())
                .is_some()
        );
        ticket
    });
    finish(cx, &dock);
    assert!(ticket.snapshot().quiesced() && ticket.snapshot().native_terminal());
    assert!(
        dock.native_callbacks_for_window(ticket.snapshot().window_id())
            .is_none()
    );
    assert!(
        dock.host
            .borrow()
            .native
            .as_ref()
            .unwrap()
            .registrations
            .is_empty()
    );
    cx.defer_next_window_frame_requests();
    cx.update(|cx| dock.open_native_root(SECOND, false, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    finish(cx, &dock);
    assert!(dock.native_is_quiescent());
}
