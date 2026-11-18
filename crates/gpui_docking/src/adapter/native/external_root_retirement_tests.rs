use dockspace::{
    geometry::{LogicalSize, PhysicalPoint, PhysicalRect, ScaleFactor},
    model::{
        DockAnchor, DockPlacement, DockspaceLayout, DockspaceNode, DockspaceRootLayout,
        DockspaceSurfaceLayout, ItemId, RootId, SurfaceId,
    },
    policy::DockPolicy,
    runtime::{
        DockspaceHostFrame, DockspaceSession, HostFrameReport, HostWindowToken,
        NativeHostCapabilities, NativeHostCapability, NativeReceiverAnswer, NativeSurfaceBinding,
        NativeSurfacePlacement, NativeWindowFacts, NativeWorkAreaRoster, SurfaceUnavailableReason,
    },
};
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, Empty, PlatformFrameDeliveryMode, TestAppContext,
    WindowFrameSubmissionOutcome, WindowOptions,
};

use super::{NativeBridge, NativeBridgeError};
use crate::adapter::native::{NativeReport, NativeTransport, tests::fixture};

fn finish(mut frame: DockspaceHostFrame<'_>) -> HostFrameReport {
    frame
        .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
        .unwrap();
    frame.commit().unwrap()
}

fn collect_and_commit(
    session: &mut DockspaceSession,
    bridge: &mut NativeBridge,
) -> Vec<NativeReport> {
    bridge.collect(session).unwrap();
    let frame = session
        .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
        .unwrap();
    bridge.accept_report(finish(frame));
    bridge.take_reports()
}

#[open_gpui::test]
fn detached_external_root_close_waits_for_its_original_native_terminal(cx: &mut TestAppContext) {
    let (mut session, mut bridge, bindings, windows) = cx.update(fixture);
    cx.run_until_parked();
    collect_and_commit(&mut session, &mut bridge);
    let source = windows[1];
    let callbacks = bridge.callbacks(bindings[1].window()).unwrap();
    cx.set_window_frame_delivery_mode(source, PlatformFrameDeliveryMode::Deferred);
    cx.skip_drawing();
    let submission = cx
        .update_window(source, |_, window, cx| {
            let ticket = window.begin_exact_frame_submission(cx).unwrap();
            callbacks.watch_submission(&ticket, window).unwrap();
            ticket
        })
        .unwrap();
    assert_eq!(
        submission.snapshot().outcome(),
        WindowFrameSubmissionOutcome::Pending
    );
    let renderer = cx.hold_window_presentation_shutdown(source);
    let held = cx.hold_window_native_terminal(source);
    let mut frame = session
        .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
        .unwrap();
    frame
        .dock_item_current(
            ItemId::new(2),
            DockPlacement::Center(DockAnchor::Item(ItemId::new(1))),
        )
        .unwrap();
    let report = finish(frame);
    assert!(report.native_placement_detachments().is_empty());
    assert_eq!(report.native_bindings(), [bindings[0]]);
    bridge.accept_report(report);
    assert!(bridge.take_reports().is_empty());
    assert!(!session.is_current_native_binding(bindings[1]));
    assert!(session.recognizes_native_binding(bindings[1]));
    assert!(matches!(
        callbacks.seal(),
        Err(NativeBridgeError::NotQuiescent)
    ));
    assert!(matches!(
        bridge.quiesce_binding(&mut session, bindings[1]),
        Err(NativeBridgeError::NotQuiescent)
    ));

    let close = cx.simulate_window_close_request(source);
    assert!(
        close.logical_window_removed(),
        "the detached root must restore ordinary GPUI close"
    );
    assert!(!close.native_terminal_started());
    assert_eq!(
        submission.snapshot().outcome(),
        WindowFrameSubmissionOutcome::WindowTerminal
    );
    let retirement = cx
        .update(|cx| cx.native_window_retirement_ticket(source.window_id()))
        .unwrap();
    assert!(!retirement.snapshot().quiesced());
    assert!(!retirement.snapshot().native_terminal());
    let reports = collect_and_commit(&mut session, &mut bridge);
    assert_eq!(
        reports
            .iter()
            .filter(|report| matches!(report,
            NativeReport::LogicalClosed(binding) if *binding == bindings[1].window()))
            .count(),
        1,
        "{reports:?}, expected {bindings:?}"
    );
    assert!(!reports.iter().any(|report| matches!(
        report,
        NativeReport::NativeTerminal(_) | NativeReport::CloseRequested(_)
    )));
    assert!(matches!(
        callbacks.seal(),
        Err(NativeBridgeError::NotQuiescent)
    ));
    assert!(matches!(
        bridge.quiesce_binding(&mut session, bindings[1]),
        Err(NativeBridgeError::NotQuiescent)
    ));
    assert_eq!(
        bridge.binding_for_window(source.window_id()),
        Some(bindings[1].window())
    );

    renderer.release();
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(512));
    cx.run_until_parked();
    // Physical termination can be observed while the original callback is held.
    // The bridge still needs that callback's own terminal before sealing it.
    assert!(!callbacks.0.native_terminal.get());
    assert!(matches!(
        callbacks.seal(),
        Err(NativeBridgeError::NotQuiescent)
    ));
    assert!(held.release());
    cx.run_until_parked();
    let reports = collect_and_commit(&mut session, &mut bridge);
    assert_eq!(
        reports
            .iter()
            .filter(|report| matches!(report,
            NativeReport::NativeTerminal(binding) if *binding == bindings[1].window()))
            .count(),
        1,
        "{reports:?}"
    );
    assert!(retirement.snapshot().native_terminal() && retirement.snapshot().quiesced());
    assert!(bridge.callbacks(bindings[1].window()).is_some());
    callbacks.seal().unwrap();
    bridge.quiesce_binding(&mut session, bindings[1]).unwrap();
    collect_and_commit(&mut session, &mut bridge);
    assert!(!session.recognizes_native_binding(bindings[1]));
    assert!(bridge.callbacks_for_window(source.window_id()).is_none());
    assert!(session.is_current_native_binding(bindings[0]));
    assert!(cx.windows().contains(&windows[0]));
    assert!(!bridge.transport().is_frozen());
}

fn shared_fixture(
    cx: &mut App,
) -> (
    DockspaceSession,
    NativeBridge,
    [NativeSurfaceBinding; 2],
    AnyWindowHandle,
) {
    let layout = DockspaceLayout::new([1, 2, 3].map(|value| {
        let items = [ItemId::new(value)];
        DockspaceSurfaceLayout::new(
            SurfaceId::new(value),
            DockspaceRootLayout::new(
                RootId::new(value),
                if value == 3 {
                    DockspaceNode::central_tabs(items)
                } else {
                    DockspaceNode::tabs(items)
                },
            ),
        )
    }))
    .unwrap();
    let mut session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
    let mut bridge = NativeBridge::enable(
        &mut session,
        NativeTransport::new(|_| {}),
        NativeHostCapabilities::none_supported()
            .with(NativeHostCapability::AuthoritativeInventory)
            .with(NativeHostCapability::CloseCancellation),
        None,
    )
    .unwrap();
    session
        .register_native_root(SurfaceId::new(1), HostWindowToken::new(1))
        .unwrap();
    let frame = session
        .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
        .unwrap();
    let registered = finish(frame);
    let binding = registered.native_bindings()[0].window();
    bridge.accept_report(registered);
    let window = cx
        .open_window_detailed(WindowOptions::default(), |_, cx| cx.new(|_| Empty))
        .unwrap()
        .into();
    bridge.bind_window(binding, window, false, cx).unwrap();
    collect_and_commit(&mut session, &mut bridge);
    session
        .report_managed_native_snapshot(
            [(
                binding,
                NativeWindowFacts::live()
                    .with_content_bounds(PhysicalRect::new(0.0, 0.0, 800.0, 600.0).unwrap())
                    .with_native_scale_factor(ScaleFactor::new(1.0).unwrap())
                    .with_presentation_scale_factor(ScaleFactor::new(1.0).unwrap()),
            )],
            NativeWorkAreaRoster::Unknown,
        )
        .unwrap();
    let placements = [1_u32, 2].map(|value| {
        let x = f64::from(value - 1) * 400.0;
        NativeSurfacePlacement::new(
            SurfaceId::new(u64::from(value)),
            LogicalSize::new(400.0, 600.0).unwrap(),
            PhysicalPoint::new(x, 0.0).unwrap(),
            ScaleFactor::new(1.0).unwrap(),
            PhysicalRect::new(x, 0.0, 400.0, 600.0).unwrap(),
        )
    });
    let mut frame = session
        .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
        .unwrap();
    frame
        .set_native_surface_placements(binding, &placements)
        .unwrap();
    let report = finish(frame);
    let bindings = report.native_bindings().try_into().unwrap();
    bridge.accept_report(report);
    bridge.take_reports();
    (session, bridge, bindings, window)
}

fn vacate_shared_member(session: &mut DockspaceSession, bridge: &mut NativeBridge) {
    let mut frame = session
        .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
        .unwrap();
    frame
        .dock_item_current(
            ItemId::new(2),
            DockPlacement::Center(DockAnchor::Central(RootId::new(3))),
        )
        .unwrap();
    bridge.accept_report(finish(frame));
}

#[open_gpui::test]
fn shared_external_root_keeps_its_successor_and_native_close_policy(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (mut session, mut bridge, bindings, window) = cx.update(shared_fixture);
    cx.run_until_parked();
    collect_and_commit(&mut session, &mut bridge);
    vacate_shared_member(&mut session, &mut bridge);
    let reports = bridge.take_reports();
    assert_eq!(reports.len(), 1, "{reports:?}");
    assert!(matches!(reports[0], NativeReport::RetirePlacement(binding) if binding == bindings[1]));
    assert!(session.is_current_native_binding(bindings[0]));
    let callbacks = bridge.callbacks(bindings[0].window()).unwrap();
    assert!(matches!(
        callbacks.seal(),
        Err(NativeBridgeError::NotQuiescent)
    ));

    let pending_callback = cx.update(|cx| bridge.transport().wake_after_callback(cx));
    assert!(matches!(
        bridge.quiesce_binding(&mut session, bindings[1]),
        Err(NativeBridgeError::NotQuiescent)
    ));
    drop(pending_callback);
    bridge.quiesce_binding(&mut session, bindings[1]).unwrap();
    collect_and_commit(&mut session, &mut bridge);
    assert!(!session.recognizes_native_binding(bindings[1]));
    assert_eq!(
        bridge.binding_for_window(window.window_id()),
        Some(bindings[0].window())
    );
    assert!(cx.windows().contains(&window));
    assert!(!cx.simulate_window_close(window));
    let reports = collect_and_commit(&mut session, &mut bridge);
    assert!(
        reports.iter().any(|report| matches!(report,
        NativeReport::CloseRequested(close) if close.binding().window() == bindings[0].window()
            && close.binding().surface() == bindings[0].surface())),
        "{reports:?}, expected {bindings:?}"
    );
    assert!(!reports.iter().any(|report| matches!(
        report,
        NativeReport::LogicalClosed(_) | NativeReport::NativeTerminal(_)
    )));
    assert!(cx.windows().contains(&window));
    assert!(!bridge.transport().is_frozen());
}

#[open_gpui::test]
fn retired_shared_placement_waits_for_terminal_when_its_last_successor_detaches(
    cx: &mut TestAppContext,
) {
    cx.defer_next_window_frame_requests();
    let (mut session, mut bridge, bindings, window) = cx.update(shared_fixture);
    cx.run_until_parked();
    collect_and_commit(&mut session, &mut bridge);
    vacate_shared_member(&mut session, &mut bridge);
    bridge.take_reports();
    let callbacks = bridge.callbacks(bindings[0].window()).unwrap();
    let held = cx.hold_window_native_terminal(window);
    let mut frame = session
        .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
        .unwrap();
    frame
        .dock_item_current(
            ItemId::new(1),
            DockPlacement::Center(DockAnchor::Central(RootId::new(3))),
        )
        .unwrap();
    let report = finish(frame);
    assert!(report.native_placement_detachments().is_empty());
    assert!(report.native_bindings().is_empty());
    bridge.accept_report(report);
    assert!(bridge.take_reports().is_empty());
    assert!(bridge.retired_placements.contains(&bindings[1]));
    assert!(matches!(
        bridge.quiesce_binding(&mut session, bindings[1]),
        Err(NativeBridgeError::NotQuiescent)
    ));
    assert!(cx.windows().contains(&window));
    assert!(cx.simulate_window_close(window));
    let reports = collect_and_commit(&mut session, &mut bridge);
    assert!(reports.iter().any(|report| matches!(report,
        NativeReport::LogicalClosed(binding) if *binding == bindings[0].window())));
    assert!(
        !reports
            .iter()
            .any(|report| matches!(report, NativeReport::NativeTerminal(_)))
    );
    assert!(matches!(
        bridge.quiesce_binding(&mut session, bindings[1]),
        Err(NativeBridgeError::NotQuiescent)
    ));
    assert!(held.release());
    cx.run_until_parked();
    collect_and_commit(&mut session, &mut bridge);
    assert!(matches!(
        bridge.quiesce_binding(&mut session, bindings[1]),
        Err(NativeBridgeError::NotQuiescent)
    ));
    callbacks.seal().unwrap();
    for binding in bindings {
        bridge.quiesce_binding(&mut session, binding).unwrap();
        collect_and_commit(&mut session, &mut bridge);
        assert!(!session.recognizes_native_binding(binding));
    }
    assert!(bridge.is_quiescent());
}
