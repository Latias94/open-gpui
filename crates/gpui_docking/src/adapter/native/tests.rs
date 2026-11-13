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

fn session() -> DockspaceSession {
    let layout = DockspaceLayout::new([1, 2].map(|value| {
        DockspaceSurfaceLayout::new(
            SurfaceId::new(value),
            DockspaceRootLayout::new(
                RootId::new(value),
                DockspaceNode::tabs([ItemId::new(value)]),
            ),
        )
    }))
    .unwrap();
    DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap()
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
