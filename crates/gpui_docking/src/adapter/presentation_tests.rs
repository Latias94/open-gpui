//! Original renderer terminals cannot overtake native callback prefixes.

use super::*;
use open_gpui::{PlatformWindowPresentOutcome, RequestFrameOptions, VisualTestContext};

fn frames(visual: &mut VisualTestContext, dock: &DockSurface) {
    for _ in 0..4 {
        assert!(visual.simulate_frame(RequestFrameOptions {
            require_presentation: true,
            force_render: true,
            ..Default::default()
        }));
        assert!(dock.error().is_none(), "{:?}", dock.error());
    }
}

#[open_gpui::test]
fn a_rejected_native_head_retains_its_type_and_the_original_submitted_output(
    cx: &mut TestAppContext,
) {
    use dockspace::runtime::{
        NativeLocalPointerLocation, NativePointerEvent, NativePointerId, NativePointerInput,
        NativePointerOwner,
    };

    cx.defer_next_window_frame_requests();
    let dock = cx.update(workspace);
    let foreign = cx.update(workspace);
    let window = cx
        .update(|cx| dock.open_native_root(FIRST, true, WindowOptions::default(), cx))
        .unwrap();
    let foreign_window = cx
        .update(|cx| foreign.open_native_root(FIRST, true, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    finish(cx, &foreign);
    let foreign_binding = foreign
        .native_binding_for_window(foreign_window.window_id())
        .unwrap();
    let mut visual = VisualTestContext::from_window(window, cx);
    frames(&mut visual, &dock);
    let original_artifact = dock.host.borrow().mounts[&window.window_id()].regions[&FIRST]
        .presented
        .unwrap()
        .1;
    visual.set_window_present_outcome(window, PlatformWindowPresentOutcome::Deferred);
    dock.host
        .borrow_mut()
        .mounts
        .get_mut(&window.window_id())
        .unwrap()
        .regions
        .get_mut(&FIRST)
        .unwrap()
        .dirty = true;
    frames(&mut visual, &dock);
    let (ticket, transport, bridge) = {
        let mut host = dock.host.borrow_mut();
        let pending = host.mounts[&window.window_id()].pending.as_ref().unwrap();
        assert!(pending.update.is_none());
        let ticket = pending.ticket.clone();
        let native = host.native.as_mut().unwrap();
        (
            ticket,
            native.transport.clone(),
            native.bridge.take().unwrap(),
        )
    };
    visual.update(|_, cx| {
        transport.pointer(
            NativePointerInput::new_local(
                NativePointerId::new(1),
                NativePointerEvent::Moved,
                NativeLocalPointerLocation::Unknown,
                NativePointerOwner::Native(foreign_binding),
                NativePointerOwner::Unknown,
            ),
            cx,
        );
    });
    visual.set_window_present_outcome(window, PlatformWindowPresentOutcome::Submitted);
    assert!(visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        force_render: true,
        ..Default::default()
    }));
    assert_eq!(
        ticket.snapshot().outcome(),
        open_gpui::WindowFrameSubmissionOutcome::Submitted
    );
    dock.host.borrow_mut().native.as_mut().unwrap().bridge = Some(bridge);
    visual.update(|_, cx| dock.pump_native(cx));
    let primary = dock.failure().expect("the invalid provider is rejected");
    assert_eq!(
        primary.runtime_kind,
        Some(DockspaceRuntimeErrorKind::HostProtocol)
    );
    assert_eq!(primary.native_kind, Some(NativeHostErrorKind::InvalidFacts));
    assert!(!transport.is_empty());
    assert_eq!(
        dock.host.borrow().mounts[&window.window_id()].regions[&FIRST]
            .presented
            .unwrap()
            .1,
        original_artifact,
        "the terminal cannot publish past a permanently rejected prefix"
    );
    dock.host
        .borrow_mut()
        .fail(&std::io::Error::other("later cleanup observation failed"));
    assert_eq!(dock.failure(), Some(primary.clone()));
    assert_eq!(dock.error(), Some(primary.message));
    visual
        .cx
        .update(|cx| dock.begin_native_shutdown(cx))
        .unwrap();
    visual.run_until_parked();
    visual.cx.update(|cx| dock.pump_native(cx));
    assert!(
        !transport.is_empty(),
        "shutdown must retain the rejected head and affine suffix"
    );
    assert!(!dock.native_is_quiescent());
    assert_eq!(
        ticket.snapshot().outcome(),
        open_gpui::WindowFrameSubmissionOutcome::Submitted,
        "cleanup cannot relabel the actual renderer terminal"
    );
    visual
        .cx
        .update(|cx| foreign.begin_native_shutdown(cx))
        .unwrap();
    visual.run_until_parked();
    visual.cx.update(|cx| foreign.pump_native(cx));
    assert!(foreign.native_is_quiescent());
}

#[open_gpui::test]
fn a_managed_submission_keeps_its_native_fifo_boundary(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let dock = cx.update(workspace);
    let first = cx
        .update(|cx| dock.open_native_root(FIRST, true, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    cx.defer_next_window_frame_requests();
    let second = cx
        .update(|cx| dock.open_native_root(SECOND, false, WindowOptions::default(), cx))
        .unwrap();
    finish(cx, &dock);
    let windows = [first, second];
    let mut source = VisualTestContext::from_window(windows[0], cx);
    let mut target = VisualTestContext::from_window(windows[1], cx);
    frames(&mut source, &dock);
    frames(&mut target, &dock);
    let old_artifact = dock.host.borrow().mounts[&windows[0].window_id()].regions[&FIRST]
        .presented
        .unwrap()
        .1;
    source.set_window_present_outcome(windows[0], PlatformWindowPresentOutcome::Deferred);
    dock.host
        .borrow_mut()
        .mounts
        .get_mut(&windows[0].window_id())
        .unwrap()
        .regions
        .get_mut(&FIRST)
        .unwrap()
        .dirty = true;
    frames(&mut source, &dock);
    let (ticket, recorded, transport, mut bridge) = {
        let mut host = dock.host.borrow_mut();
        let pending = host.mounts[&windows[0].window_id()]
            .pending
            .as_ref()
            .expect("the source must retain its actual unsubmitted output");
        assert!(pending.update.is_none());
        let ticket = pending.ticket.clone();
        let recorded = host.session.native_input_recorded_count();
        let native = host.native.as_mut().unwrap();
        let transport = native.transport.clone();
        assert!(transport.is_empty());
        (ticket, recorded, transport, native.bridge.take().unwrap())
    };
    source.update(|_, cx| transport.device_removed(cx));
    source.set_window_present_outcome(windows[0], PlatformWindowPresentOutcome::Submitted);
    assert!(source.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        ..Default::default()
    }));
    assert_eq!(
        ticket.snapshot().outcome(),
        open_gpui::WindowFrameSubmissionOutcome::Submitted,
        "the renderer really completed its original submission"
    );
    source.update(|_, cx| transport.device_removed(cx));
    {
        let mut host = dock.host.borrow_mut();
        assert!(
            host.session
                .presented_surface(FIRST)
                .is_some_and(|surface| { surface.matches_presentation_artifact(old_artifact) }),
            "a renderer terminal cannot publish ahead of its older native prefix"
        );
        assert_eq!(host.session.native_input_recorded_count(), recorded);
        bridge.collect(&mut host.session).unwrap();
        assert_eq!(
            host.session.native_input_recorded_count(),
            recorded + 1,
            "collection must stop at the terminal, leaving its later suffix queued"
        );
        assert!(!transport.is_empty());
        host.native.as_mut().unwrap().bridge = Some(bridge);
    }
    source.update(|_, cx| dock.pump_native(cx));
    frames(&mut source, &dock);
    frames(&mut target, &dock);
    assert!(transport.is_empty());
    assert!(dock.host.borrow().session.native_input_recorded_count() >= recorded + 2);
    assert!(
        dock.host.borrow().mounts[&windows[0].window_id()].regions[&FIRST]
            .presented
            .is_some()
    );
    cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| dock.pump_native(cx));
    cx.run_until_parked();
    assert!(dock.native_is_quiescent());
}
