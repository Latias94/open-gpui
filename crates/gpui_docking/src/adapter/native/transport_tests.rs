//! Real GPUI capture packets retain their place ahead of later device facts.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use dockspace::{
    model::{
        DockspaceLayout, DockspaceNode, DockspaceRootLayout, DockspaceSurfaceLayout, ItemId,
        RootId, SurfaceId,
    },
    runtime::{
        DockspaceSession, NativePointerId, NativeReceiverAnswer, NativeSurfaceBinding,
        PointerDeviceKind, PointerEventMetadata, SurfaceUnavailableReason,
    },
};
use open_gpui::{
    AnyWindowHandle, Context, Empty, IntoElement, Modifiers, MouseButton, NativeCapturedDragEvent,
    NativeCapturedDragGeneration, NativeCapturedDragPhase, NativeCapturedDragReleaseTerminal,
    PlatformPointerCaptureReleaseOutcome, Render, Subscription, TestAppContext, VisualTestContext,
    Window, div, point, prelude::*, px,
};

use super::Inbound;
use crate::adapter::native::{NativeBridge, NativeBridgeError, NativeCallbacks, NativeReport};

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

struct ContinuedCapture {
    session: DockspaceSession,
    bridge: NativeBridge,
    before: NativeSurfaceBinding,
    after: NativeSurfaceBinding,
    source: AnyWindowHandle,
    callbacks: NativeCallbacks,
    events: Rc<RefCell<Vec<NativeCapturedDragEvent>>>,
    generation: NativeCapturedDragGeneration,
    release_allowed: Rc<Cell<bool>>,
    _subscription: Subscription,
}

fn commit_capture_transport(session: &mut DockspaceSession, bridge: &mut NativeBridge) {
    bridge.collect(session).unwrap();
    let mut frame = session
        .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
        .unwrap();
    frame
        .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
        .unwrap();
    let report = frame.commit().unwrap();
    bridge.accept_report(report);
}

fn continued_capture(cx: &mut TestAppContext) -> ContinuedCapture {
    let (mut session, mut bridge, bindings, windows) =
        cx.update(crate::adapter::native::tests::fixture);
    let before = bindings[1];
    let source = windows[1];
    let callbacks = bridge.callbacks(before.window()).unwrap();
    let transport = bridge.transport();
    let events = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx.update({
        let events = events.clone();
        let callbacks = callbacks.clone();
        let transport = transport.clone();
        move |cx| {
            cx.consume_native_captured_drag(
                |_, _, _| Arc::new("continued physical capture"),
                move |event, cx| {
                    events.borrow_mut().push(event.clone());
                    transport.captured_drag(
                        callbacks.binding(),
                        event,
                        NativePointerId::new(1),
                        PointerEventMetadata::new(
                            PointerDeviceKind::Mouse,
                            Duration::from_millis(23),
                        ),
                        cx,
                    );
                },
            )
        }
    });
    cx.update_window(source, |_, window, cx| {
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
        commit_capture_transport(&mut session, &mut bridge);
    }
    assert!(transport.is_empty());
    let mut visual = VisualTestContext::from_window(source, cx);
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
    let generation = events
        .borrow()
        .last()
        .expect("a real GPUI capture must start before continuation")
        .generation();
    commit_capture_transport(&mut session, &mut bridge);
    let release_allowed = Rc::new(Cell::new(false));
    let attempts = Rc::new(Cell::new(0));
    cx.set_pointer_capture_release_callback(source, {
        let release_allowed = release_allowed.clone();
        let attempts = attempts.clone();
        move |_| {
            attempts.set(attempts.get() + 1);
            if release_allowed.get() {
                PlatformPointerCaptureReleaseOutcome::Released
            } else {
                PlatformPointerCaptureReleaseOutcome::Rejected
            }
        }
    });
    cx.update(|cx| callbacks.cancel_capture(cx));
    cx.run_until_parked();
    assert_eq!(attempts.get(), 1);
    commit_capture_transport(&mut session, &mut bridge);
    assert!(bridge.take_reports().iter().all(|report| {
        !matches!(report, NativeReport::CaptureTerminal { barrier, .. }
            if barrier.drag_generation() == generation)
    }));
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
    let reset = session.prepare_layout_reset(layout).unwrap();
    let mut frame = session
        .begin_native_layout_reset_frame(&reset, |_| NativeReceiverAnswer::Unknown)
        .unwrap();
    frame
        .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
        .unwrap();
    let report = frame.commit().unwrap();
    let after = report
        .native_root_continuations()
        .iter()
        .find(|pair| pair.predecessor() == before)
        .expect("the original Root must continue on the same physical window")
        .successor();
    bridge.accept_report(report);
    bridge.take_reports();
    assert_eq!(callbacks.binding(), after.window());
    assert_eq!(
        bridge.binding_for_window(source.window_id()),
        Some(after.window())
    );
    assert!(transport.is_empty());
    assert!(!session.has_pending_native_input());
    ContinuedCapture {
        session,
        bridge,
        before,
        after,
        source,
        callbacks,
        events,
        generation,
        release_allowed,
        _subscription: subscription,
    }
}

#[open_gpui::test]
fn continued_root_waits_for_the_original_capture_release_before_quiescence(
    cx: &mut TestAppContext,
) {
    let mut capture = continued_capture(cx);
    assert!(matches!(
        capture
            .bridge
            .quiesce_binding(&mut capture.session, capture.before),
        Err(NativeBridgeError::NotQuiescent)
    ));
    assert!(capture.session.recognizes_native_binding(capture.before));
    capture.release_allowed.set(true);
    cx.background_executor
        .advance_clock(Duration::from_millis(8));
    cx.run_until_parked();
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    let reports = capture.bridge.take_reports();
    assert_eq!(
        reports
            .iter()
            .filter(|report| matches!(report,
                NativeReport::CaptureTerminal { binding, barrier, terminal }
                    if *binding == capture.before.window()
                        && barrier.source_window() == capture.source.window_id()
                        && barrier.drag_generation() == capture.generation
                        && *terminal == NativeCapturedDragReleaseTerminal::Released
            ))
            .count(),
        1,
        "{reports:?}"
    );
    capture
        .bridge
        .quiesce_binding(&mut capture.session, capture.before)
        .unwrap();
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    assert!(!capture.session.recognizes_native_binding(capture.before));
    assert!(capture.session.is_current_native_binding(capture.after));
}

#[open_gpui::test]
fn continued_root_capture_release_preserves_the_next_real_drag(cx: &mut TestAppContext) {
    let mut capture = continued_capture(cx);
    let mut visual = VisualTestContext::from_window(capture.source, cx);
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
    let second_generation = capture.events.borrow().last().unwrap().generation();
    assert_ne!(capture.generation, second_generation);
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    let mut reports = capture.bridge.take_reports();
    capture.release_allowed.set(true);
    cx.background_executor
        .advance_clock(Duration::from_millis(8));
    cx.run_until_parked();
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    reports.extend(capture.bridge.take_reports());
    assert_eq!(
        reports
            .iter()
            .filter(|report| matches!(report,
                NativeReport::CaptureTerminal { binding, barrier, terminal }
                    if *binding == capture.before.window()
                        && barrier.drag_generation() == capture.generation
                        && *terminal != NativeCapturedDragReleaseTerminal::Failed
            ))
            .count(),
        1,
        "{reports:?}"
    );
    cx.update(|cx| {
        assert_eq!(
            cx.active_native_captured_drag_source_window(),
            Some(capture.source.window_id())
        );
        capture.callbacks.cancel_capture(cx);
        assert_eq!(cx.active_native_captured_drag_source_window(), None);
    });
    cx.run_until_parked();
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    assert!(
        capture
            .bridge
            .take_reports()
            .iter()
            .any(|report| matches!(report,
                NativeReport::CaptureTerminal { binding, barrier, terminal }
                    if *binding == capture.after.window()
                        && barrier.drag_generation() == second_generation
                        && *terminal != NativeCapturedDragReleaseTerminal::Failed
            ))
    );
}

#[open_gpui::test]
fn a_late_capture_release_cannot_settle_a_replaced_callback_owner(cx: &mut TestAppContext) {
    let mut capture = continued_capture(cx);
    capture.release_allowed.set(true);
    cx.background_executor
        .advance_clock(Duration::from_millis(8));
    cx.run_until_parked();
    assert!(
        capture
            .bridge
            .transport()
            .0
            .queue
            .borrow()
            .iter()
            .any(|record| matches!(record, Inbound::CaptureTerminal(..)))
    );
    let original = capture
        .bridge
        .windows
        .remove(capture.after.window())
        .unwrap();
    let replacement = cx
        .update(|cx| {
            capture
                .bridge
                .bind_window(capture.after.window(), capture.source, false, cx)
        })
        .unwrap();
    commit_capture_transport(&mut capture.session, &mut capture.bridge);
    let reports = capture.bridge.take_reports();
    assert!(reports.iter().any(|report| matches!(report,
        NativeReport::StaleCallback(binding) if *binding == capture.before.window()
    )));
    assert!(reports.iter().all(|report| !matches!(report,
        NativeReport::CaptureTerminal { barrier, .. }
            if barrier.drag_generation() == capture.generation
    )));
    assert_eq!(replacement.binding(), capture.after.window());
    assert_eq!(original.window_id(), Some(capture.source.window_id()));
}
