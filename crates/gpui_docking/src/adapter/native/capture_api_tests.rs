use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use dockspace::runtime::{
    DockspaceSession, NativeDesktopPointerLocation, NativeDesktopPosition, NativePointerButton,
    NativePointerEvent, NativePointerHover, NativePointerId, NativePointerInput,
    NativePointerOwner, NativeReceiverAnswer, NativeSurfaceBinding, NativeWindowBinding,
    PointerDeviceKind, PointerEventMetadata,
};
use open_gpui::{
    AnyWindowHandle, Context, Empty, IntoElement, Modifiers, MouseButton, NativeCapturedDragEvent,
    NativeCapturedDragPhase, NativeCapturedDragReleaseTerminal,
    PlatformPointerCaptureReleaseOutcome, Render, Subscription, TestAppContext, VisualTestContext,
    Window, div, point, prelude::*, px,
};

use crate::native::{NativeBridge, NativeReport, NativeTransport};

struct CaptureView {
    transport: NativeTransport,
    binding: NativeWindowBinding,
}

impl Render for CaptureView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let transport = self.transport.clone();
        let binding = self.binding;
        div()
            .id("public-native-capture")
            .size_full()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                transport.pointer(
                    NativePointerInput::new(
                        NativePointerId::new(1),
                        NativePointerEvent::ButtonPressed(NativePointerButton::Primary),
                        NativeDesktopPointerLocation::new(
                            NativeDesktopPosition::Unknown,
                            NativePointerHover::Unknown,
                            None,
                        ),
                        NativePointerOwner::Native(binding),
                        NativePointerOwner::Unknown,
                    )
                    .with_metadata(PointerEventMetadata::new(
                        PointerDeviceKind::Mouse,
                        Duration::from_millis(37),
                    )),
                    cx,
                );
            })
            .on_drag("public native capture", |_, geometry, _, cx| {
                let _consumer = geometry.prepare_native_captured_drag_consumer();
                cx.new(|_| Empty)
            })
    }
}

struct CaptureFixture {
    session: DockspaceSession,
    bridge: Rc<RefCell<NativeBridge>>,
    bindings: [NativeSurfaceBinding; 2],
    windows: [AnyWindowHandle; 2],
    events: Rc<RefCell<Vec<NativeCapturedDragEvent>>>,
    release_generations: Rc<RefCell<Vec<u64>>>,
    _subscription: Subscription,
}

impl CaptureFixture {
    fn new(cx: &mut TestAppContext) -> Self {
        let (session, bridge, bindings, windows) = cx.update(super::fixture);
        let callbacks = bridge.callbacks(bindings[1].window()).unwrap();
        let transport = bridge.transport();
        let bridge = Rc::new(RefCell::new(bridge));
        let events = Rc::new(RefCell::new(Vec::new()));
        let routed_bridge = bridge.clone();
        let routed_events = events.clone();
        let subscription = cx.update(|cx| {
            cx.consume_native_captured_drag(
                move |event, _, cx| {
                    callbacks.observe_capture_release(event, cx);
                    Arc::new(())
                },
                move |event, cx| {
                    routed_bridge
                        .borrow_mut()
                        .captured_drag(
                            &event,
                            NativePointerId::new(1),
                            PointerEventMetadata::new(
                                PointerDeviceKind::Mouse,
                                Duration::from_millis(37),
                            ),
                            cx,
                        )
                        .unwrap();
                    routed_events.borrow_mut().push(event);
                },
            )
        });
        let release_generations = Rc::new(RefCell::new(Vec::new()));
        cx.set_pointer_capture_release_callback(windows[1], {
            let release_generations = release_generations.clone();
            move |generation| {
                release_generations.borrow_mut().push(generation);
                PlatformPointerCaptureReleaseOutcome::Released
            }
        });
        cx.update_window(windows[1], |_, window, cx| {
            window.replace_root(cx, |_, _| CaptureView {
                transport,
                binding: bindings[1].window(),
            });
            let _ = window.activate_window();
            window.draw(cx).clear(cx);
        })
        .unwrap();
        cx.run_until_parked();
        let mut fixture = Self {
            session,
            bridge,
            bindings,
            windows,
            events,
            release_generations,
            _subscription: subscription,
        };
        fixture.collect();
        fixture
    }

    fn collect(&mut self) -> Vec<NativeReport> {
        let mut reports = Vec::new();
        for _ in 0..4 {
            let mut bridge = self.bridge.borrow_mut();
            if let Err(error) = bridge.collect(&mut self.session) {
                assert!(self.session.has_pending_native_input(), "{error}");
            }
            let report = self
                .session
                .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
                .unwrap()
                .commit()
                .unwrap();
            bridge.accept_report(report);
            reports.extend(bridge.take_reports());
            if bridge.transport().is_empty() {
                return reports;
            }
        }
        panic!("the original native FIFO did not drain");
    }

    fn finish(&mut self, cx: &mut TestAppContext) {
        cx.update(|cx| self.bridge.borrow_mut().begin_shutdown(cx))
            .unwrap();
        for _ in 0..4 {
            cx.update(|cx| self.bridge.borrow_mut().drive_shutdown(cx))
                .unwrap();
            cx.run_until_parked();
            self.collect();
        }
        cx.update(|cx| {
            assert!(
                self.windows
                    .iter()
                    .all(|window| !cx.windows().contains(window))
            );
        });
        for binding in self.bindings {
            let mut bridge = self.bridge.borrow_mut();
            bridge.callbacks(binding.window()).unwrap().seal().unwrap();
            bridge.quiesce_binding(&mut self.session, binding).unwrap();
            drop(bridge);
            self.collect();
        }
        assert!(self.bridge.borrow().is_quiescent());
    }
}

#[open_gpui::test]
fn public_release_locker_keeps_natural_release_until_delayed_collection(cx: &mut TestAppContext) {
    let mut fixture = CaptureFixture::new(cx);
    let source = fixture.windows[1];
    let recorded = fixture.session.native_input_recorded_count();
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
    visual.simulate_mouse_up(
        point(px(80.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(cx.active_native_captured_drag_source_window(), None));
    assert_eq!(fixture.release_generations.borrow().len(), 1);
    assert_eq!(fixture.session.native_input_recorded_count(), recorded);
    let released = fixture
        .events
        .borrow()
        .iter()
        .find(|event| event.phase() == NativeCapturedDragPhase::Released)
        .unwrap()
        .clone();

    let reports = fixture.collect();
    let terminals: Vec<_> = reports
        .iter()
        .filter_map(|report| match report {
            NativeReport::CaptureTerminal {
                binding,
                barrier,
                terminal,
            } => Some((*binding, *barrier, *terminal)),
            _ => None,
        })
        .collect();
    assert_eq!(terminals.len(), 1, "{reports:?}");
    let (binding, barrier, terminal) = terminals[0];
    assert_eq!(binding, fixture.bindings[1].window());
    assert_eq!(barrier.source_window(), source.window_id());
    assert_eq!(barrier.drag_generation(), released.generation());
    assert_eq!(
        barrier.release_generation(),
        fixture.release_generations.borrow()[0]
    );
    assert_eq!(terminal, NativeCapturedDragReleaseTerminal::Released);
    assert!(reports.iter().any(|report| matches!(report,
        NativeReport::CapturedRelease { capture, .. }
            if capture.generation() == released.generation()
    )));
    fixture.finish(cx);
    assert_eq!(fixture.release_generations.borrow().len(), 1);
}

#[open_gpui::test]
fn public_delayed_old_release_cannot_cancel_a_new_drag_generation(cx: &mut TestAppContext) {
    let mut fixture = CaptureFixture::new(cx);
    let source = fixture.windows[1];
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
    visual.simulate_mouse_up(
        point(px(60.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let released = fixture
        .events
        .borrow()
        .iter()
        .find(|event| event.phase() == NativeCapturedDragPhase::Released)
        .unwrap()
        .clone();

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
    cx.run_until_parked();
    let current = fixture.events.borrow().last().unwrap().generation();
    assert_ne!(released.generation(), current);
    assert_eq!(fixture.release_generations.borrow().len(), 1);
    let reports = fixture.collect();
    assert!(
        reports.iter().any(|report| matches!(report,
            NativeReport::CaptureTerminal { barrier, terminal, .. }
                if barrier.drag_generation() == released.generation()
                    && *terminal == NativeCapturedDragReleaseTerminal::Released
        )),
        "{reports:?}"
    );
    assert!(
        !reports.iter().any(|report| matches!(report,
            NativeReport::CaptureTerminal { barrier, .. } if barrier.drag_generation() == current
        )),
        "{reports:?}"
    );
    cx.update(|cx| {
        fixture
            .bridge
            .borrow_mut()
            .dispatch_effects(&mut |_, _, _| panic!("capture cannot create a window"), cx)
            .unwrap();
    });
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            cx.active_native_captured_drag_source_window(),
            Some(source.window_id())
        );
    });
    assert_eq!(fixture.release_generations.borrow().len(), 1);
    assert!(fixture.events.borrow().iter().all(|event| {
        event.generation() != current || event.phase() == NativeCapturedDragPhase::Moved
    }));

    visual.simulate_mouse_up(
        point(px(160.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let reports = fixture.collect();
    assert_eq!(
        reports
            .iter()
            .filter(|report| matches!(report,
                NativeReport::CaptureTerminal { barrier, terminal, .. }
                    if barrier.drag_generation() == current
                        && barrier.release_generation() == fixture.release_generations.borrow()[1]
                        && *terminal == NativeCapturedDragReleaseTerminal::Released
            ))
            .count(),
        1,
        "{reports:?}"
    );
    fixture.finish(cx);
    assert_eq!(fixture.release_generations.borrow().len(), 2);
}
