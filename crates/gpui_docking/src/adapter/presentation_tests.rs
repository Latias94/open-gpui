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
