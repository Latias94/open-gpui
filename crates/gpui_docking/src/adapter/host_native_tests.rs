//! Root registration failures retain original GPUI and core lifetime authority.

use dockspace::{
    model::{
        DockspaceLayout, DockspaceNode, DockspaceRootLayout, DockspaceSurfaceLayout, ItemId,
        RootId, SurfaceId,
    },
    policy::DockPolicy,
    runtime::{DockspaceRuntimeErrorKind, DockspaceSession},
};
use open_gpui::{
    App, AppContext as _, Empty, TestAppContext, WindowOpenFailureStage, WindowOptions,
};

use super::{DockSurface, NativeBridgeError, NativeOpenFailure};
use crate::adapter::{DockPane, GpuiDockspaceConfig};

const FIRST: SurfaceId = SurfaceId::new(1);
const SECOND: SurfaceId = SurfaceId::new(2);

fn workspace(cx: &mut App) -> DockSurface {
    let layout = DockspaceLayout::new([FIRST, SECOND].map(|surface| {
        DockspaceSurfaceLayout::new(
            surface,
            DockspaceRootLayout::new(
                RootId::new(surface.get()),
                DockspaceNode::tabs([ItemId::new(surface.get())]),
            ),
        )
    }))
    .unwrap();
    let session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
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
