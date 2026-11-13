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
    AnyWindowHandle, App, AppContext as _, Bounds, DevicePixels, Empty, PlatformWindowHitStack,
    TestAppContext, WindowOptions, point, size,
};

use super::{NativeBridge, NativeBridgeError, NativeCloseDecision, NativeReport, NativeTransport};

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
    let mut report = commit(&mut session);
    bridge.accept_report(&mut report);
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
    let mut composed = frame.commit().unwrap();
    bridge.accept_report(&mut composed);
    bridge.take_reports();
    let reset = session.prepare_layout_reset(layout()).unwrap();
    let mut frame = session
        .begin_native_layout_reset_frame(&reset, |_| NativeReceiverAnswer::Unknown)
        .unwrap();
    frame
        .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
        .unwrap();
    let mut report = frame.commit().unwrap();
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
    bridge.accept_report(&mut report);
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
    bridge.accept_report(&mut report);
    assert!(bridge.take_reports().is_empty());
    bridge.collect(&mut session).unwrap();
    commit(&mut session);
    assert!(bridge.transport.is_empty());
    let late = bridge.take_reports();
    assert!(!late.is_empty());
    assert!(
        late.iter().all(|report| matches!(report,
            NativeReport::StaleCallback(binding) if *binding == original)),
        "{late:?}"
    );
    assert!(!session.has_pending_native_input());
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
fn native_close_defer_and_veto_preserve_the_original_real_window(cx: &mut TestAppContext) {
    let (mut session, mut bridge, _, windows) = cx.update(fixture);
    assert!(!cx.simulate_window_close(windows[1]));
    bridge.collect(&mut session).unwrap();
    let mut report = commit(&mut session);
    bridge.accept_report(&mut report);
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
    let mut report = commit(&mut session);
    bridge.accept_report(&mut report);
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
