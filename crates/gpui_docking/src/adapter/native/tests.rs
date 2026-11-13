use dockspace::{
    model::{
        DockspaceLayout, DockspaceNode, DockspaceRootLayout, DockspaceSurfaceLayout, ItemId,
        RootId, SurfaceId,
    },
    policy::DockPolicy,
    runtime::{
        DockspaceSession, HostFrameReport, HostWindowToken, NativeHostCapabilities,
        NativeHostCapability, NativePointerHover, NativeReceiverAnswer, NativeSurfaceBinding,
        SurfaceUnavailableReason,
    },
};
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, Bounds, Context, DevicePixels, Empty, IntoElement,
    PlatformFrameDeliveryMode, PlatformWindowHitStack, PlatformWindowPresentOutcome, Render,
    Styled as _, TestAppContext, Window, WindowFrameSubmissionOutcome, WindowFrameSubmissionTicket,
    WindowOptions, div, point, size,
};

use super::{NativeBridge, NativeBridgeError, NativeCloseDecision, NativeReport, NativeTransport};

struct SubmissionView(WindowFrameSubmissionTicket);

impl Render for SubmissionView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let ticket = self.0.clone();
        div()
            .size_full()
            .bg(open_gpui::rgb(0x123456))
            .on_children_prepainted(move |_, window, _| {
                let ticket = ticket.clone();
                window.record_prepaint_focus_stable_commit(move |generation, window, cx| {
                    if matches!(
                        ticket.snapshot().outcome(),
                        WindowFrameSubmissionOutcome::Pending
                            | WindowFrameSubmissionOutcome::Accepted
                    ) {
                        window
                            .accept_exact_frame_submission_frame(&ticket, generation, cx)
                            .unwrap();
                    }
                });
            })
    }
}

fn layout() -> DockspaceLayout {
    DockspaceLayout::new([1, 2].map(|value| {
        DockspaceSurfaceLayout::new(
            SurfaceId::new(value),
            DockspaceRootLayout::new(
                RootId::new(value),
                DockspaceNode::tabs([ItemId::new(value)]),
            ),
        )
    }))
    .unwrap()
}

fn session() -> DockspaceSession {
    DockspaceSession::from_layout(layout(), DockPolicy::default()).unwrap()
}

fn commit(session: &mut DockspaceSession) -> HostFrameReport {
    let mut frame = session
        .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
        .unwrap();
    frame
        .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
        .unwrap();
    frame.commit().unwrap()
}

pub(super) fn fixture(
    cx: &mut App,
) -> (
    DockspaceSession,
    NativeBridge,
    [NativeSurfaceBinding; 2],
    [AnyWindowHandle; 2],
) {
    let mut session = session();
    let capabilities = NativeHostCapabilities::none_supported()
        .with(NativeHostCapability::AuthoritativeInventory)
        .with(NativeHostCapability::CloseCancellation);
    let mut bridge = NativeBridge::enable(
        &mut session,
        NativeTransport::new(|_| {}),
        capabilities,
        None,
    )
    .unwrap();
    for value in [1, 2] {
        session
            .register_native_root(SurfaceId::new(value), HostWindowToken::new(value))
            .unwrap();
    }
    let registered = commit(&mut session);
    let bindings: [NativeSurfaceBinding; 2] = registered.native_bindings().try_into().unwrap();
    let windows = [true, false].map(|anchor| {
        let window: AnyWindowHandle = cx
            .open_window_detailed(WindowOptions::default(), |_, cx| cx.new(|_| Empty))
            .unwrap()
            .into();
        let binding = bindings[usize::from(!anchor)].window();
        bridge.bind_window(binding, window, anchor, cx).unwrap();
        window
    });
    bridge.collect(&mut session).unwrap();
    let report = commit(&mut session);
    bridge.accept_report(report);
    (session, bridge, bindings, windows)
}

#[test]
fn managed_enrollment_rejects_an_existing_local_pointer_producer() {
    let mut session = session();
    session.enable_host_pointer().unwrap();
    let result = NativeBridge::enable(
        &mut session,
        NativeTransport::new(|_| {}),
        NativeHostCapabilities::none_supported(),
        None,
    );
    assert!(result.is_err());
}

#[test]
fn negative_physical_coordinates_are_preserved_without_a_dpi_round_trip() {
    let bounds = Bounds::new(
        point(DevicePixels(-1920), DevicePixels(-240)),
        size(DevicePixels(960), DevicePixels(720)),
    );
    let physical = super::input::physical_rect(bounds).unwrap();
    assert_eq!(physical.min().x(), -1920.0);
    assert_eq!(physical.min().y(), -240.0);
    assert_eq!(physical.width(), 960.0);
}

#[test]
fn missing_native_hover_is_unknown_and_an_unmatched_sample_cannot_prove_desktop() {
    let point = point(DevicePixels(-20), DevicePixels(30));
    let resolve = |_| None;
    assert_eq!(
        super::input::native_hover(&PlatformWindowHitStack::Unavailable, Some(point), resolve),
        NativePointerHover::Unknown
    );
    let desktop = PlatformWindowHitStack::try_available_open_desktop(point, Vec::new()).unwrap();
    assert_eq!(
        super::input::native_hover(&desktop, Some(point), resolve),
        NativePointerHover::OutsideAll
    );
    assert_eq!(
        super::input::native_hover(&desktop, None, resolve),
        NativePointerHover::Unknown
    );
}

#[open_gpui::test]
fn shared_root_continuation_retires_each_placement_once(cx: &mut TestAppContext) {
    use dockspace::{
        geometry::{LogicalSize, PhysicalPoint, PhysicalRect, ScaleFactor},
        runtime::{NativeSurfacePlacement, NativeWindowFacts, NativeWorkAreaRoster},
    };

    cx.defer_next_window_frame_requests();
    let (mut session, mut bridge, original, window) = cx.update(|cx| {
        let mut session = session();
        let mut bridge = NativeBridge::enable(
            &mut session,
            NativeTransport::new(|_| {}),
            NativeHostCapabilities::none_supported()
                .with(NativeHostCapability::AuthoritativeInventory),
            None,
        )
        .unwrap();
        session
            .register_native_root(SurfaceId::new(1), HostWindowToken::new(1))
            .unwrap();
        let registered = commit(&mut session);
        let original = registered.native_bindings()[0].window();
        let window: AnyWindowHandle = cx
            .open_window_detailed(WindowOptions::default(), |_, cx| cx.new(|_| Empty))
            .unwrap()
            .into();
        bridge.bind_window(original, window, true, cx).unwrap();
        bridge.collect(&mut session).unwrap();
        commit(&mut session);
        (session, bridge, original, window)
    });
    cx.run_until_parked();
    bridge.collect(&mut session).unwrap();
    commit(&mut session);
    session
        .report_managed_native_snapshot(
            [(
                original,
                NativeWindowFacts::live()
                    .with_content_bounds(PhysicalRect::new(0.0, 0.0, 800.0, 600.0).unwrap())
                    .with_native_scale_factor(ScaleFactor::new(1.0).unwrap())
                    .with_presentation_scale_factor(ScaleFactor::new(1.0).unwrap()),
            )],
            NativeWorkAreaRoster::Unknown,
        )
        .unwrap();
    commit(&mut session);
    let placements = [1_u32, 2].map(|id| {
        let x = f64::from(id - 1) * 400.0;
        NativeSurfacePlacement::new(
            SurfaceId::new(u64::from(id)),
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
        .set_native_surface_placements(original, &placements)
        .unwrap();
    frame
        .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
        .unwrap();
    let composed = frame.commit().unwrap();
    bridge.accept_report(composed);
    bridge.take_reports();
    cx.set_window_frame_delivery_mode(window, PlatformFrameDeliveryMode::Deferred);
    let ticket = cx
        .update_window(window, |_, window, cx| {
            let ticket = window.begin_exact_frame_submission(cx).unwrap();
            bridge
                .callbacks(original)
                .unwrap()
                .watch_submission(&ticket, window)
                .unwrap();
            window.replace_root(cx, |_, _| SubmissionView(ticket.clone()));
            ticket
        })
        .unwrap();
    assert!(
        open_gpui::VisualTestContext::from_window(window, cx).simulate_frame(
            open_gpui::RequestFrameOptions {
                require_presentation: true,
                force_render: true,
                ..Default::default()
            }
        )
    );
    let submission = cx
        .pending_window_frame_submission(window)
        .expect("the original exact frame must reach the real renderer submission boundary");
    assert_eq!(
        ticket.snapshot().outcome(),
        WindowFrameSubmissionOutcome::Pending
    );
    assert_eq!(ticket.snapshot().accepted_frame_generation(), None);
    let reset = session.prepare_layout_reset(layout()).unwrap();
    let mut frame = session
        .begin_native_layout_reset_frame(&reset, |_| NativeReceiverAnswer::Unknown)
        .unwrap();
    frame
        .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
        .unwrap();
    let report = frame.commit().unwrap();
    let pairs = report.native_root_continuations().to_vec();
    assert_eq!(pairs.len(), 2);
    let successor = pairs[0].successor().window();
    assert!(
        pairs
            .iter()
            .all(|pair| pair.predecessor().window() == original)
    );
    assert!(
        pairs
            .iter()
            .all(|pair| pair.successor().window() == successor)
    );
    bridge.accept_report(report);
    let retired: Vec<_> = bridge
        .take_reports()
        .into_iter()
        .filter_map(|report| match report {
            NativeReport::RetirePlacement(binding) => Some(binding),
            _ => None,
        })
        .collect();
    assert_eq!(retired.len(), 2);
    assert_eq!(
        retired
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        pairs.iter().map(|pair| pair.predecessor()).collect()
    );
    assert_eq!(
        bridge.binding_for_window(window.window_id()),
        Some(successor)
    );
    assert_eq!(bridge.anchor, Some(successor));
    assert!(bridge.callbacks(original).is_none());
    let callbacks = bridge.callbacks(successor).unwrap();
    assert_eq!(callbacks.binding(), successor);
    assert_eq!(callbacks.window_id(), Some(window.window_id()));
    assert!(bridge.take_reports().is_empty());
    assert!(submission.complete(PlatformWindowPresentOutcome::Submitted));
    cx.run_until_parked();
    let submitted = ticket.snapshot();
    assert_eq!(submitted.outcome(), WindowFrameSubmissionOutcome::Submitted);
    assert_eq!(
        submitted.accepted_frame_generation(),
        Some(submission.frame_generation())
    );
    assert_eq!(
        submitted.submitted_frame_generation(),
        Some(submission.frame_generation())
    );
    bridge.collect(&mut session).unwrap();
    commit(&mut session);
    assert!(bridge.transport.is_empty());
    let late = bridge.take_reports();
    assert!(!late.is_empty());
    assert_eq!(
        late.iter()
            .filter(|report| matches!(report,
                NativeReport::Submission { binding, snapshot }
                    if *binding == original && *snapshot == submitted))
            .count(),
        1,
        "{late:?}"
    );
    assert!(
        late.iter().all(|report| match report {
            NativeReport::StaleCallback(binding) => *binding == original,
            NativeReport::Submission { binding, snapshot } => {
                *binding == original && *snapshot == submitted
            }
            _ => false,
        }),
        "{late:?}"
    );
    assert!(!session.has_pending_native_input());
    cx.update_window(window, |_, window, _| {
        assert!(window.retire_exact_frame_submission(&ticket));
    })
    .unwrap();
    for pair in pairs {
        bridge
            .quiesce_binding(&mut session, pair.predecessor())
            .unwrap();
        commit(&mut session);
        assert!(!session.recognizes_native_binding(pair.predecessor()));
        assert!(session.is_current_native_binding(pair.successor()));
    }
    assert!(bridge.retired_placements.is_empty());
    cx.update(|cx| assert!(cx.windows().contains(&window)));
}

#[open_gpui::test]
fn failed_opens_release_staging_and_transfer_original_rollback_custody(cx: &mut TestAppContext) {
    use std::rc::Rc;

    use dockspace::{
        geometry::{PhysicalRect, ScaleFactor},
        model::NativeWindowPlacement,
        runtime::{
            NativeEffectOperation, NativeWindowFacts, NativeWindowPresentationState,
            NativeWorkAreaRoster,
        },
    };

    use super::NativeGeometry;

    let (mut session, mut bridge, bindings) = cx.update(|cx| {
        let layout = DockspaceLayout::new((1..=4).map(|value| {
            DockspaceSurfaceLayout::new(
                SurfaceId::new(value),
                DockspaceRootLayout::new(
                    RootId::new(value),
                    DockspaceNode::central_tabs([ItemId::new(value)]),
                ),
            )
        }))
        .unwrap();
        let mut policy = DockPolicy::default();
        policy.set_allow_native_surfaces(true);
        let mut session = DockspaceSession::from_layout(layout, policy).unwrap();
        let capabilities = NativeHostCapabilities::none_supported()
            .with(NativeHostCapability::AuthoritativeInventory)
            .with(NativeHostCapability::NativeWindowLifecycle)
            .with(NativeHostCapability::GlobalWindowPlacement);
        let mut bridge = NativeBridge::enable(
            &mut session,
            NativeTransport::new(|_| {}),
            capabilities,
            Some(Rc::new(NativeGeometry::new(|_| None, |_, _| None))),
        )
        .unwrap();
        session
            .register_native_root(SurfaceId::new(1), HostWindowToken::new(1))
            .unwrap();
        let root = commit(&mut session).native_bindings()[0];
        let window: AnyWindowHandle = cx
            .open_window_detailed(WindowOptions::default(), |_, cx| cx.new(|_| Empty))
            .unwrap()
            .into();
        bridge.bind_window(root.window(), window, true, cx).unwrap();
        bridge.collect(&mut session).unwrap();
        commit(&mut session);
        let bounds = PhysicalRect::new(0.0, 0.0, 800.0, 600.0).unwrap();
        let scale = ScaleFactor::new(1.0).unwrap();
        session
            .report_managed_native_snapshot(
                [(
                    root.window(),
                    NativeWindowFacts::live()
                        .with_content_bounds(bounds)
                        .with_outer_bounds(bounds)
                        .with_native_scale_factor(scale)
                        .with_presentation_scale_factor(scale)
                        .with_presentation(NativeWindowPresentationState::Visible),
                )],
                NativeWorkAreaRoster::Unknown,
            )
            .unwrap();
        commit(&mut session);
        let mut frame = session
            .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
            .unwrap();
        for target in 2..=4 {
            frame
                .create_native_surface_current(
                    SurfaceId::new(target),
                    root.surface(),
                    NativeWindowPlacement::new(bounds),
                )
                .unwrap();
        }
        let mut report = frame.commit().unwrap();
        let effects = report.take_native_effects();
        let bindings: Vec<_> = effects
            .iter()
            .map(|request| match request.operation() {
                NativeEffectOperation::CreateWindow { binding, .. } => *binding,
                operation => panic!("each logical surface must request a window: {operation:?}"),
            })
            .collect();
        assert_eq!(bindings.len(), 3, "{:?}", report.inputs());
        bridge.accept_report(report);
        bridge.enqueue_effects(effects);
        (session, bridge, bindings)
    });
    cx.fail_next_window_map("original pre-root native rollback");
    let mut original_rollback = None;
    let mut opened = None;
    let mut dispatched = Vec::new();
    cx.update(|cx| {
        let count = bridge
            .dispatch_effects(
                &mut |request, callbacks, cx| {
                    assert!(callbacks.window_id().is_none());
                    dispatched.push(request.binding);
                    if request.binding == bindings[0] {
                        return cx
                            .open_window_with_platform_window(
                                WindowOptions::default(),
                                |_, _, _| {
                                    Err(std::io::Error::other("before native ownership").into())
                                },
                                |_, cx| cx.new(|_| Empty),
                            )
                            .map(Into::into)
                            .map_err(Into::into);
                    }
                    let result = callbacks.open(
                        WindowOptions::default(),
                        |_, cx| {
                            assert_eq!(request.binding, bindings[2]);
                            cx.new(|_| Empty)
                        },
                        cx,
                    );
                    match &result {
                        Err(error) => {
                            let ticket = error.retirement_ticket.as_ref().unwrap();
                            assert!(ticket.same_authority(
                                error.source.as_ref().unwrap().retirement_ticket().unwrap()
                            ));
                            original_rollback = Some(ticket.clone());
                        }
                        Ok(handle) => opened = Some(*handle),
                    }
                    result
                },
                cx,
            )
            .unwrap();
        assert_eq!(count, 3);
        assert_eq!(dispatched, bindings);
        assert!(bridge.opening.is_none());
        assert!(bridge.callbacks(bindings[0].window()).is_none());
        let ticket = original_rollback.as_ref().unwrap();
        assert!(!ticket.snapshot().native_terminal());
        let failed = bridge
            .callbacks_for_window(ticket.snapshot().window_id())
            .expect("a real rollback owner must be indexed before returning to the event loop");
        assert_eq!(failed.binding(), bindings[1].window());
        assert!(failed.handle().is_none());
        assert!(matches!(
            failed.seal(),
            Err(NativeBridgeError::NotQuiescent)
        ));
        let opened = opened.unwrap();
        assert_eq!(
            bridge.binding_for_window(opened.window_id()),
            Some(bindings[2].window())
        );
        assert_eq!(
            bridge.callbacks(bindings[2].window()).unwrap().handle(),
            Some(opened)
        );
    });
    let reports = bridge.take_reports();
    assert!(reports.iter().any(|report| matches!(report,
        NativeReport::OpenFailed { binding, retirement_ticket: None, .. }
            if *binding == bindings[0])));
    let ticket = original_rollback.unwrap();
    assert!(reports.iter().any(|report| matches!(report,
        NativeReport::OpenFailed { binding, retirement_ticket: Some(original), .. }
            if *binding == bindings[1] && original.same_authority(&ticket))));
    cx.run_until_parked();
    bridge.collect(&mut session).unwrap();
    commit(&mut session);
    assert!(ticket.snapshot().quiesced() && ticket.snapshot().native_terminal());
    assert!(bridge.take_reports().iter().any(|report| matches!(report,
        NativeReport::NativeTerminal(binding) if *binding == bindings[1].window())));
    bridge
        .callbacks(bindings[1].window())
        .unwrap()
        .seal()
        .unwrap();
    assert_eq!(
        bridge.binding_for_window(ticket.snapshot().window_id()),
        Some(bindings[1].window()),
        "native terminal alone cannot retire the original callback and output owners"
    );
}

#[open_gpui::test]
fn native_close_defer_and_veto_preserve_the_original_real_window(cx: &mut TestAppContext) {
    let (mut session, mut bridge, _, windows) = cx.update(fixture);
    assert!(!cx.simulate_window_close(windows[1]));
    bridge.collect(&mut session).unwrap();
    let report = commit(&mut session);
    bridge.accept_report(report);
    let close = bridge
        .take_reports()
        .into_iter()
        .find_map(|report| match report {
            NativeReport::CloseRequested(close) => Some(close),
            _ => None,
        })
        .expect("the committed native close edge reaches application policy");
    let before = session.native_input_recorded_count();
    bridge
        .decide_close(&mut session, close, NativeCloseDecision::Defer)
        .unwrap();
    assert_eq!(session.native_input_recorded_count(), before);
    bridge
        .decide_close(&mut session, close, NativeCloseDecision::Veto)
        .unwrap();
    let report = commit(&mut session);
    bridge.accept_report(report);
    cx.update(|cx| {
        bridge
            .dispatch_effects(
                &mut |_, _, _| panic!("a close veto cannot create a window"),
                cx,
            )
            .unwrap();
    });
    bridge.collect(&mut session).unwrap();
    commit(&mut session);
    cx.update(|cx| assert!(cx.windows().contains(&windows[1])));
    assert!(!bridge.transport().is_frozen());
}

#[open_gpui::test]
fn two_window_shutdown_waits_for_true_terminal_and_preserves_a_sibling_group(
    cx: &mut TestAppContext,
) {
    let (mut session, mut bridge, bindings, windows) = cx.update(fixture);
    let sibling = cx.update(|cx| {
        cx.open_window_detailed(WindowOptions::default(), |_, cx| cx.new(|_| Empty))
            .unwrap()
    });
    let held = cx.hold_window_native_terminal(windows[1]);
    assert!(!cx.simulate_window_close(windows[0]));
    cx.update(|cx| bridge.drive_shutdown(cx).unwrap());
    bridge.collect(&mut session).unwrap();
    commit(&mut session);
    let reports = bridge.take_reports();
    assert!(reports.iter().any(|report| matches!(report, NativeReport::LogicalClosed(binding) if *binding == bindings[1].window())));
    assert!(!reports.iter().any(|report| matches!(report, NativeReport::NativeTerminal(binding) if *binding == bindings[1].window())));
    assert!(!bridge.is_quiescent());
    let callbacks = bridge.callbacks(bindings[1].window()).unwrap();
    assert!(matches!(
        callbacks.seal(),
        Err(NativeBridgeError::NotQuiescent)
    ));
    cx.update(|cx| {
        assert!(cx.windows().contains(&windows[0]));
        assert!(cx.windows().contains(&sibling.into()));
    });
    assert!(held.release());
    cx.run_until_parked();
    bridge.collect(&mut session).unwrap();
    commit(&mut session);
    cx.update(|cx| bridge.drive_shutdown(cx).unwrap());
    cx.run_until_parked();
    bridge.collect(&mut session).unwrap();
    commit(&mut session);
    bridge.take_reports();
    for binding in bindings {
        bridge.callbacks(binding.window()).unwrap().seal().unwrap();
        bridge.quiesce_binding(&mut session, binding).unwrap();
        commit(&mut session);
    }
    assert!(bridge.is_quiescent());
    cx.update(|cx| assert!(cx.windows().contains(&sibling.into())));
}
