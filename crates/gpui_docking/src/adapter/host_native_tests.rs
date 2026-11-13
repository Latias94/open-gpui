//! Root registration failures retain original GPUI and core lifetime authority.

use dockspace::{
    model::{
        DockspaceLayout, DockspaceNode, DockspaceRootLayout, DockspaceSurfaceLayout, ItemId,
        RootId, SurfaceId,
    },
    policy::DockPolicy,
    runtime::{
        DockspaceRuntimeErrorKind, DockspaceSession, HostFrameReport, HostInputOutcome,
        NativeHostErrorKind, NativeReceiverAnswer, SurfaceUnavailableReason,
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

fn commit_native(session: &mut DockspaceSession) -> HostFrameReport {
    let mut frame = session
        .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
        .unwrap();
    frame
        .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
        .unwrap();
    frame.commit().unwrap()
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
        bridge.collect(&mut host.session).unwrap();
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
    cx.update(|_| {
        let mut host = dock.host.borrow_mut();
        bridge.collect(&mut host.session).unwrap();
        let report = commit_native(&mut host.session);
        bridge.accept_report(report);
    });
    assert!(held.release());
    cx.run_until_parked();
    cx.update(|cx| {
        let mut host = dock.host.borrow_mut();
        bridge.collect(&mut host.session).unwrap();
        let report = commit_native(&mut host.session);
        bridge.accept_report(report);
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
