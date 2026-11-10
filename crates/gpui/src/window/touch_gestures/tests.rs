use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use crate::{
    App, Context, DispatchPhase, InteractiveElement, IntoElement, KeyDownEvent, Keystroke,
    LongPressEvent, MouseButton, MouseDownEvent, ParentElement, PlatformInput,
    PointerCaptureHandle, Render, ScrollEvent, ScrollHandle, ScrollSource,
    StatefulInteractiveElement, Styled, TestAppContext, TouchDragEvent, TouchEvent, TouchId,
    TouchPhase, Window, WindowMouseEvent, canvas, div, point, px,
};

#[derive(Clone, Copy, Default, PartialEq)]
enum Claim {
    #[default]
    None,
    LongPress,
    Drag,
}

#[derive(Clone, Copy, Default)]
enum Terminate {
    #[default]
    None,
    Quiesce,
    Close,
}

#[derive(Default)]
struct Observed {
    claim: Cell<Claim>,
    terminate: Cell<Terminate>,
    clicks: Cell<usize>,
    long_press: RefCell<Vec<TouchPhase>>,
    drag: RefCell<Vec<TouchPhase>>,
    scroll: RefCell<Vec<ScrollEvent>>,
    handle: ScrollHandle,
    capture: Cell<Option<PointerCaptureHandle>>,
    reenter_on_cancel: Cell<bool>,
    nested_rejected: Cell<Option<bool>>,
    panic_on_cancel: Cell<bool>,
}

struct TouchView(Rc<Observed>);

impl Render for TouchView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let observed = self.0.clone();
        let capture = observed.capture.get().unwrap_or_else(|| {
            let handle = window.new_pointer_capture_handle();
            observed.capture.set(Some(handle));
            handle
        });
        let clicks = observed.clone();
        let entity = cx.entity();
        div()
            .id("touch-scroll")
            .size(px(200.))
            .overflow_y_scroll()
            .track_scroll(&observed.handle)
            .track_pointer_capture(&capture)
            .on_click(move |_, _, _| clicks.clicks.set(clicks.clicks.get() + 1))
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, (), window, _| {
                        let press = observed.clone();
                        let entity = entity.clone();
                        window.on_mouse_event(move |event: &LongPressEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble
                                || press.claim.get() != Claim::LongPress
                            {
                                return;
                            }
                            press.long_press.borrow_mut().push(event.phase);
                            if event.phase == TouchPhase::Cancelled {
                                if press.reenter_on_cancel.get() {
                                    let nested = window.dispatch_event(
                                        PlatformInput::MouseDown(MouseDownEvent {
                                            button: MouseButton::Left,
                                            position: point(px(50.), px(50.)),
                                            ..Default::default()
                                        }),
                                        cx,
                                    );
                                    press
                                        .nested_rejected
                                        .set(Some(nested.default_prevented && !nested.propagate));
                                }
                                assert!(!press.panic_on_cancel.get(), "touch cancel panic");
                            }
                            if event.phase == TouchPhase::Started {
                                window.capture_long_press(&entity);
                                window.prevent_default();
                                terminate(press.terminate.get(), window, cx);
                            }
                        });
                        let drag = observed.clone();
                        window.on_mouse_event(move |event: &TouchDragEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble || drag.claim.get() != Claim::Drag {
                                return;
                            }
                            drag.drag.borrow_mut().push(event.phase);
                            if event.phase == TouchPhase::Started {
                                window.prevent_default();
                                terminate(drag.terminate.get(), window, cx);
                            }
                        });
                        let scroll = observed.clone();
                        window.on_mouse_event(move |event: &ScrollEvent, phase, _, _| {
                            if phase == DispatchPhase::Bubble {
                                scroll.scroll.borrow_mut().push(event.clone());
                            }
                        });
                    },
                )
                .w_full()
                .h(px(1000.)),
            )
    }
}

fn terminate(action: Terminate, window: &mut Window, cx: &mut App) {
    match action {
        Terminate::None => {}
        Terminate::Quiesce => {
            window.quiesce_interaction(cx);
        }
        Terminate::Close => window.remove_window(cx),
    }
}

fn touch(phase: TouchPhase, y: f32) -> TouchEvent {
    TouchEvent {
        id: TouchId(1),
        phase,
        position: point(px(50.), px(y)),
        ..Default::default()
    }
}

#[crate::test]
fn raw_touch_tap_uses_committed_click_and_persistent_interception(cx: &mut TestAppContext) {
    let observed = Rc::new(Observed::default());
    let (_view, cx) = cx.add_window_view({
        let observed = observed.clone();
        move |_, _| TouchView(observed)
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let intercepted = Rc::new(RefCell::new(Vec::new()));
    let _subscription = cx.update(|window, _| {
        let intercepted = intercepted.clone();
        window.intercept_window_mouse_events(move |event, _, _| match event {
            WindowMouseEvent::Down(_) => intercepted.borrow_mut().push("down"),
            WindowMouseEvent::Up(_) => intercepted.borrow_mut().push("up"),
            _ => {}
        })
    });
    cx.simulate_event(touch(TouchPhase::Started, 50.));
    cx.simulate_event(touch(TouchPhase::Ended, 50.));
    assert_eq!(observed.clicks.get(), 1);
    assert_eq!(intercepted.borrow().as_slice(), &["down", "up"]);
    cx.update(|window, _| assert!(window.last_input_was_touch()));
}

#[crate::test]
fn long_press_uses_executor_clock_and_deactivation_cancels_the_claim(cx: &mut TestAppContext) {
    let observed = Rc::new(Observed::default());
    observed.claim.set(Claim::LongPress);
    let (view, cx) = cx.add_window_view({
        let observed = observed.clone();
        move |_, _| TouchView(observed)
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_event(touch(TouchPhase::Started, 50.));
    cx.executor().advance_clock(Duration::from_millis(499));
    cx.run_until_parked();
    assert!(observed.long_press.borrow().is_empty());
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    assert_eq!(
        observed.long_press.borrow().as_slice(),
        &[TouchPhase::Started]
    );
    cx.update(|window, _| assert!(window.has_long_press_capture(&view)));
    cx.update(|window, cx| window.native_active_status_changed(false, cx));
    assert_eq!(
        observed.long_press.borrow().as_slice(),
        &[TouchPhase::Started, TouchPhase::Cancelled]
    );
    cx.update(|window, _| assert!(!window.has_long_press_capture(&view)));
    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    assert_eq!(observed.long_press.borrow().len(), 2);
    assert_eq!(observed.clicks.get(), 0);
}

#[crate::test]
fn claimed_drag_started_quiescence_returns_one_terminal_after_listener_checkout(
    cx: &mut TestAppContext,
) {
    let observed = Rc::new(Observed::default());
    observed.claim.set(Claim::Drag);
    observed.terminate.set(Terminate::Quiesce);
    let (_view, cx) = cx.add_window_view({
        let observed = observed.clone();
        move |_, _| TouchView(observed)
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_event(touch(TouchPhase::Started, 50.));
    assert_eq!(
        observed.drag.borrow().as_slice(),
        &[TouchPhase::Started, TouchPhase::Cancelled]
    );
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    cx.simulate_event(touch(TouchPhase::Ended, 50.));
    assert_eq!(observed.drag.borrow().len(), 2);
    assert!(observed.long_press.borrow().is_empty());
    assert_eq!(observed.clicks.get(), 0);
}

#[crate::test]
fn claimed_long_press_started_close_delivers_cancel_before_terminal_removal(
    cx: &mut TestAppContext,
) {
    let observed = Rc::new(Observed::default());
    observed.claim.set(Claim::LongPress);
    observed.terminate.set(Terminate::Close);
    let (_view, cx) = cx.add_window_view({
        let observed = observed.clone();
        move |_, _| TouchView(observed)
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_event(touch(TouchPhase::Started, 50.));
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    assert_eq!(
        observed.long_press.borrow().as_slice(),
        &[TouchPhase::Started, TouchPhase::Cancelled]
    );
    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    assert_eq!(observed.long_press.borrow().len(), 2);
    assert_eq!(observed.clicks.get(), 0);
}

#[crate::test]
fn raw_pan_scrolls_the_existing_container_and_quiescence_retires_momentum(cx: &mut TestAppContext) {
    let observed = Rc::new(Observed::default());
    let (_view, cx) = cx.add_window_view({
        let observed = observed.clone();
        move |_, _| TouchView(observed)
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_event(touch(TouchPhase::Started, 100.));
    cx.executor().advance_clock(Duration::from_millis(16));
    cx.simulate_event(touch(TouchPhase::Moved, 60.));
    cx.executor().advance_clock(Duration::from_millis(16));
    cx.simulate_event(touch(TouchPhase::Ended, 20.));
    assert!(observed.handle.offset().y < px(0.));
    assert!(
        observed
            .scroll
            .borrow()
            .iter()
            .all(|event| event.source == ScrollSource::Touch)
    );
    cx.update(|window, cx| window.quiesce_interaction(cx));
    assert_eq!(
        observed.scroll.borrow().last().unwrap().touch_phase,
        TouchPhase::Ended
    );
    let count = observed.scroll.borrow().len();
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(observed.scroll.borrow().len(), count);
    assert_eq!(observed.clicks.get(), 0);
}

#[crate::test]
fn keyboard_quiescence_settles_the_existing_claimed_touch(cx: &mut TestAppContext) {
    let observed = Rc::new(Observed::default());
    observed.claim.set(Claim::LongPress);
    let (view, cx) = cx.add_window_view({
        let observed = observed.clone();
        move |_, _| TouchView(observed)
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_event(touch(TouchPhase::Started, 50.));
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    assert_eq!(
        observed.long_press.borrow().as_slice(),
        &[TouchPhase::Started]
    );
    let _subscription = cx.update(|window, _| {
        window.intercept_window_key_down(|_, window, cx| {
            window.quiesce_interaction(cx);
        })
    });
    cx.simulate_event(KeyDownEvent {
        keystroke: Keystroke::parse("escape").unwrap(),
        is_held: false,
        prefer_character_input: false,
    });
    assert_eq!(
        observed.long_press.borrow().as_slice(),
        &[TouchPhase::Started, TouchPhase::Cancelled]
    );
    cx.update(|window, _| assert!(!window.has_long_press_capture(&view)));
}

#[crate::test]
fn native_deactivation_terminal_listener_cannot_reenter_user_input(cx: &mut TestAppContext) {
    let observed = Rc::new(Observed::default());
    observed.claim.set(Claim::LongPress);
    observed.reenter_on_cancel.set(true);
    let (_view, cx) = cx.add_window_view({
        let observed = observed.clone();
        move |_, _| TouchView(observed)
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_event(touch(TouchPhase::Started, 50.));
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    cx.update(|window, cx| window.native_active_status_changed(false, cx));
    assert_eq!(observed.nested_rejected.get(), Some(true));
    cx.update(|window, app| assert!(!window.has_active_pointer_session(app)));
}

#[crate::test]
fn touch_cancel_panic_keeps_the_existing_pointer_retirement_complete(cx: &mut TestAppContext) {
    let observed = Rc::new(Observed::default());
    observed.claim.set(Claim::LongPress);
    observed.panic_on_cancel.set(true);
    let (_view, cx) = cx.add_window_view({
        let observed = observed.clone();
        move |_, _| TouchView(observed)
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, _| {
        let _ = window.activate_window();
    });
    cx.run_until_parked();
    cx.simulate_event(touch(TouchPhase::Started, 50.));
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    cx.update(|window, app| {
        assert!(window.is_window_active());
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(50.), px(50.)),
                ..Default::default()
            }),
            app,
        );
        window
            .capture_pointer(&observed.capture.get().unwrap(), MouseButton::Left)
            .unwrap();
        assert!(window.captured_pointer().is_some());
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            window.native_active_status_changed(false, app);
        }))
        .expect_err("the original touch observer panic must propagate");
        assert!(
            panic
                .downcast_ref::<String>()
                .is_some_and(|text| text.contains("touch cancel panic"))
                || panic
                    .downcast_ref::<&str>()
                    .is_some_and(|text| text.contains("touch cancel panic"))
        );
        assert!(!window.has_active_pointer_session(app));
        assert!(window.captured_pointer().is_none());
    });
}
