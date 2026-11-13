//! Real GPUI capture packets retain their place ahead of later device facts.

use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use dockspace::runtime::{NativePointerId, PointerDeviceKind, PointerEventMetadata};
use open_gpui::{
    Context, Empty, IntoElement, Modifiers, MouseButton, NativeCapturedDragPhase, Render,
    TestAppContext, VisualTestContext, Window, div, point, prelude::*, px,
};

use super::Inbound;

struct DragView;

impl Render for DragView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("ordered-native-drag").size_full().on_drag(
            "original FIFO payload",
            |_, geometry, _, cx| {
                let _consumer = geometry.prepare_native_captured_drag_consumer();
                cx.new(|_| Empty)
            },
        )
    }
}

#[open_gpui::test]
fn captured_motion_keeps_its_fifo_position_while_an_update_lease_is_held(cx: &mut TestAppContext) {
    let (mut session, mut bridge, bindings, windows) =
        cx.update(crate::adapter::native::tests::fixture);
    let transport = bridge.transport();
    let received = Rc::new(RefCell::new(Vec::new()));
    let metadata = PointerEventMetadata::new(PointerDeviceKind::Mouse, Duration::from_millis(17));
    let subscription = cx.update({
        let received = received.clone();
        let transport = transport.clone();
        move |cx| {
            cx.consume_native_captured_drag(
                |_, _, _| Arc::new("original route lock"),
                move |event, cx| {
                    if event.phase() == NativeCapturedDragPhase::Moved {
                        received.borrow_mut().push(event.clone());
                        transport.captured_drag(
                            bindings[0].window(),
                            event,
                            NativePointerId::new(1),
                            metadata,
                            cx,
                        );
                        transport.device_removed(cx);
                    }
                },
            )
        }
    });
    cx.update_window(windows[0], |_, window, cx| {
        window.replace_root(cx, |_, _| DragView);
        let _ = window.activate_window();
        window.draw(cx).clear(cx);
    })
    .unwrap();
    // Root replacement/activation can publish original native observations.
    // Settle those facts before suspending the candidate used by this probe.
    for _ in 0..4 {
        cx.run_until_parked();
        if transport.is_empty() {
            break;
        }
        bridge.collect(&mut session).unwrap();
        let mut frame = session
            .begin_native_host_frame(|_| dockspace::runtime::NativeReceiverAnswer::Unknown)
            .unwrap();
        frame
            .complete_unpainted_surfaces(dockspace::runtime::SurfaceUnavailableReason::Deferred)
            .unwrap();
        let report = frame.commit().unwrap();
        bridge.accept_report(report);
    }
    assert!(transport.is_empty());
    let update = session
        .begin_native_host_frame(|_| dockspace::runtime::NativeReceiverAnswer::Unknown)
        .unwrap()
        .suspend();
    let recorded = session.native_input_recorded_count();
    let mut visual = VisualTestContext::from_window(windows[0], cx);
    visual.simulate_mouse_down(
        point(px(20.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(60.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(80.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    assert_eq!(
        session.native_input_recorded_count(),
        recorded,
        "the original candidate holds semantic publication while transport records arrivals"
    );
    let received = received.borrow();
    assert!(
        !received.is_empty(),
        "the test must receive an actual GPUI native captured move"
    );
    let expected = received.len();
    {
        let queue = transport.0.queue.borrow();
        assert_eq!(queue.len(), expected * 2);
        let mut queued = queue.iter();
        for original in received.iter() {
            let Some(Inbound::CapturedDrag {
                binding,
                event,
                metadata: actual_metadata,
                ..
            }) = queued.next()
            else {
                panic!("later device removal overtook the original native capture packet");
            };
            assert_eq!(*binding, bindings[0].window());
            assert_eq!(event.sequence(), original.sequence());
            assert_eq!(event.generation(), original.generation());
            assert_eq!(event.source_window(), original.source_window());
            assert_eq!(event.physical_frame(), original.physical_frame());
            assert_eq!(event.payload::<&str>(), Some(&"original FIFO payload"));
            assert_eq!(*actual_metadata, metadata);
            assert!(matches!(queued.next(), Some(Inbound::DeviceRemoved)));
        }
        assert!(queued.next().is_none());
    }
    drop(received);
    drop(update);
    assert_eq!(bridge.collect(&mut session).unwrap(), expected * 2);
    assert!(transport.is_empty());
    assert!(session.native_input_recorded_count() > recorded);
    let _ = subscription;
}

#[open_gpui::test]
fn a_queued_release_cannot_cancel_the_next_real_drag_generation(cx: &mut TestAppContext) {
    queued_terminal_preserves_the_next_generation(cx, false);
}

#[open_gpui::test]
fn a_queued_cancellation_cannot_cancel_the_next_real_drag_generation(cx: &mut TestAppContext) {
    queued_terminal_preserves_the_next_generation(cx, true);
}

fn queued_terminal_preserves_the_next_generation(cx: &mut TestAppContext, cancel_first: bool) {
    let (mut session, mut bridge, bindings, windows) =
        cx.update(crate::adapter::native::tests::fixture);
    let transport = bridge.transport();
    let received = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx.update({
        let received = received.clone();
        let transport = transport.clone();
        move |cx| {
            cx.consume_native_captured_drag(
                |_, _, _| Arc::new("original generation route"),
                move |event, cx| {
                    received.borrow_mut().push(event.clone());
                    transport.captured_drag(
                        bindings[0].window(),
                        event,
                        NativePointerId::new(1),
                        PointerEventMetadata::new(
                            PointerDeviceKind::Mouse,
                            Duration::from_millis(19),
                        ),
                        cx,
                    );
                },
            )
        }
    });
    cx.update_window(windows[0], |_, window, cx| {
        window.replace_root(cx, |_, _| DragView);
        let _ = window.activate_window();
        window.draw(cx).clear(cx);
    })
    .unwrap();
    for _ in 0..4 {
        cx.run_until_parked();
        if transport.is_empty() {
            break;
        }
        bridge.collect(&mut session).unwrap();
        let mut frame = session
            .begin_native_host_frame(|_| dockspace::runtime::NativeReceiverAnswer::Unknown)
            .unwrap();
        frame
            .complete_unpainted_surfaces(dockspace::runtime::SurfaceUnavailableReason::Deferred)
            .unwrap();
        let report = frame.commit().unwrap();
        bridge.accept_report(report);
    }
    assert!(transport.is_empty());
    let update = session
        .begin_native_host_frame(|_| dockspace::runtime::NativeReceiverAnswer::Unknown)
        .unwrap()
        .suspend();
    let recorded = session.native_input_recorded_count();
    let mut visual = VisualTestContext::from_window(windows[0], cx);
    visual.simulate_mouse_down(
        point(px(20.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(60.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(80.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    if cancel_first {
        visual.update(|window, cx| {
            window.cancel_pointer_session(open_gpui::PointerCancelReason::CaptureRevoked, cx);
        });
    }
    visual.simulate_mouse_up(
        point(px(80.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_down(
        point(px(100.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(140.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    visual.simulate_mouse_move(
        point(px(160.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    let events = received.borrow();
    let first_terminal = events
        .iter()
        .find(|event| !matches!(event.phase(), NativeCapturedDragPhase::Moved))
        .expect("the first real GPUI generation must terminate");
    if cancel_first {
        assert!(matches!(
            first_terminal.phase(),
            NativeCapturedDragPhase::Cancelled(_)
        ));
    } else {
        assert_eq!(first_terminal.phase(), NativeCapturedDragPhase::Released);
    }
    let second = events.last().expect("a second real GPUI drag must start");
    assert_eq!(second.phase(), NativeCapturedDragPhase::Moved);
    assert_ne!(first_terminal.generation(), second.generation());
    assert!(first_terminal.sequence().ordinal() < second.sequence().ordinal());
    let second_generation = second.generation();
    let second_sequence = second.sequence();
    let expected = events.len();
    drop(events);
    assert_eq!(session.native_input_recorded_count(), recorded);
    drop(update);
    assert_eq!(bridge.collect(&mut session).unwrap(), expected);
    visual.update(|_, cx| {
        assert_eq!(
            cx.active_native_captured_drag_source_window(),
            Some(windows[0].window_id())
        );
        bridge
            .dispatch_effects(
                &mut |_, _, _| panic!("capture cleanup cannot create a window"),
                cx,
            )
            .unwrap();
        assert_eq!(
            cx.active_native_captured_drag_source_window(),
            Some(windows[0].window_id()),
            "the first generation's deferred terminal must not cancel the current generation"
        );
    });
    visual.simulate_mouse_move(
        point(px(180.0), px(20.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    let events = received.borrow();
    let continued = events.last().unwrap();
    assert_eq!(continued.phase(), NativeCapturedDragPhase::Moved);
    assert_eq!(continued.generation(), second_generation);
    assert!(continued.sequence().ordinal() > second_sequence.ordinal());
    assert!(events.iter().all(|event| {
        event.generation() != second_generation
            || matches!(event.phase(), NativeCapturedDragPhase::Moved)
    }));
    drop(events);
    bridge.collect(&mut session).unwrap();
    assert!(bridge.take_reports().iter().all(|report| {
        !matches!(report, crate::adapter::native::NativeReport::CaptureTerminal { barrier, .. }
            if barrier.drag_generation() == second_generation)
    }));
    let _ = subscription;
}
