use super::*;
use crate::{
    AnyWindowHandle, AppContext as _, Context, InputEvent, Keystroke, ScrollDelta,
    SubtreeTransform, SubtreeTransformExt as _, TestAppContext, TouchEvent, TouchId,
};

// Hover and padding regressions from Zed f0d8b0b0cd and b1a7ef0cf6.
struct HoverListenerLayoutTestView {
    target_left: Pixels,
    hover_transitions: Rc<RefCell<Vec<bool>>>,
}

impl Render for HoverListenerLayoutTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let hover_transitions = self.hover_transitions.clone();
        div().relative().size_full().child(
            div()
                .id("hover-target")
                .absolute()
                .left(self.target_left)
                .top_0()
                .size(px(20.))
                .on_click(|_, _, _| {})
                .on_hover(move |is_hovered, _, _| {
                    hover_transitions.borrow_mut().push(*is_hovered);
                }),
        )
    }
}

#[crate::test]
fn default_hover_listener_updates_when_layout_changes_under_stationary_mouse(
    cx: &mut TestAppContext,
) {
    let hover_transitions = Rc::new(RefCell::new(Vec::new()));
    let window = cx.add_window({
        let hover_transitions = hover_transitions.clone();
        move |_, _| HoverListenerLayoutTestView {
            target_left: px(40.),
            hover_transitions,
        }
    });
    let any_window = AnyWindowHandle::from(window);

    cx.update_window(any_window, |_, window, cx| {
        window.draw(cx).clear(cx);
        window.simulate_mouse_move(point(px(10.), px(10.)), cx);
    })
    .unwrap();
    assert!(hover_transitions.borrow().is_empty());

    window
        .update(cx, |view, _, cx| {
            view.target_left = px(0.);
            cx.notify();
        })
        .unwrap();
    cx.update_window(any_window, |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    assert_eq!(*hover_transitions.borrow(), [true]);

    window
        .update(cx, |view, _, cx| {
            view.target_left = px(40.);
            cx.notify();
        })
        .unwrap();
    cx.update_window(any_window, |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    assert_eq!(*hover_transitions.borrow(), [true, false]);
}

#[crate::test]
fn default_hover_listener_ends_after_key_press(cx: &mut TestAppContext) {
    let hover_transitions = Rc::new(RefCell::new(Vec::new()));
    let window = cx.add_window({
        let hover_transitions = hover_transitions.clone();
        move |_, _| HoverListenerLayoutTestView {
            target_left: px(0.),
            hover_transitions,
        }
    });
    let any_window = AnyWindowHandle::from(window);

    cx.update_window(any_window, |_, window, cx| {
        window.draw(cx).clear(cx);
        window.simulate_mouse_move(point(px(10.), px(10.)), cx);
    })
    .unwrap();
    assert_eq!(*hover_transitions.borrow(), [true]);

    key_down(cx, any_window, "a");
    cx.update_window(any_window, |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    assert_eq!(*hover_transitions.borrow(), [true, false]);
}

struct HoverListenerModeLayoutTestView {
    target_left: Pixels,
    hover_transitions: Rc<RefCell<Vec<bool>>>,
}

impl Render for HoverListenerModeLayoutTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let hover_transitions = self.hover_transitions.clone();
        div().relative().size_full().child(
            div()
                .id("hover-target")
                .absolute()
                .left(self.target_left)
                .top_0()
                .size(px(20.))
                .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
                .on_hover(move |is_hovered, _, _| {
                    hover_transitions.borrow_mut().push(*is_hovered);
                }),
        )
    }
}

#[crate::test]
fn input_modality_independent_hover_listener_updates_after_key_press(cx: &mut TestAppContext) {
    let hover_transitions = Rc::new(RefCell::new(Vec::new()));
    let window = cx.add_window({
        let hover_transitions = hover_transitions.clone();
        move |_, _| HoverListenerModeLayoutTestView {
            target_left: px(40.),
            hover_transitions,
        }
    });
    let any_window = AnyWindowHandle::from(window);
    let pointer_position = point(px(10.), px(10.));

    cx.update_window(any_window, |_, window, cx| {
        window.draw(cx).clear(cx);
        window.simulate_mouse_move(pointer_position, cx);
    })
    .unwrap();
    assert!(hover_transitions.borrow().is_empty());

    key_down(cx, any_window, "a");
    window
        .update(cx, |view, _, cx| {
            view.target_left = px(0.);
            cx.notify();
        })
        .unwrap();
    cx.update_window(any_window, |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    assert_eq!(*hover_transitions.borrow(), [true]);

    key_down(cx, any_window, "b");
    cx.update_window(any_window, |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    assert_eq!(*hover_transitions.borrow(), [true]);

    cx.update_window(any_window, |_, window, cx| {
        window.simulate_mouse_move(point(px(30.), px(10.)), cx);
    })
    .unwrap();
    assert_eq!(*hover_transitions.borrow(), [true, false]);

    cx.update_window(any_window, |_, window, cx| {
        window.simulate_mouse_move(pointer_position, cx);
        window.dispatch_event(
            MouseExitEvent {
                position: pointer_position,
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
    })
    .unwrap();
    assert_eq!(*hover_transitions.borrow(), [true, false, true, false]);
}

#[crate::test]
fn default_hover_listener_remains_hovered_during_stationary_mouse_press(cx: &mut TestAppContext) {
    let hover_transitions = Rc::new(RefCell::new(Vec::new()));
    let window = cx.add_window({
        let hover_transitions = hover_transitions.clone();
        move |_, _| HoverListenerLayoutTestView {
            target_left: px(0.),
            hover_transitions,
        }
    });
    let any_window = AnyWindowHandle::from(window);
    let mouse_position = point(px(10.), px(10.));

    cx.update_window(any_window, |_, window, cx| {
        window.draw(cx).clear(cx);
        window.simulate_mouse_move(mouse_position, cx);
    })
    .unwrap();
    assert_eq!(*hover_transitions.borrow(), [true]);

    cx.update_window(any_window, |_, window, cx| {
        window.dispatch_event(
            MouseDownEvent {
                position: mouse_position,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }
            .to_platform_input(),
            cx,
        );
        window.draw(cx).clear(cx);
    })
    .unwrap();
    assert_eq!(*hover_transitions.borrow(), [true]);

    cx.update_window(any_window, |_, window, cx| {
        window.dispatch_event(
            MouseUpEvent {
                position: mouse_position,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
            }
            .to_platform_input(),
            cx,
        );
        window.draw(cx).clear(cx);
    })
    .unwrap();
    assert_eq!(*hover_transitions.borrow(), [true]);
}

fn key_down(cx: &mut TestAppContext, window: AnyWindowHandle, key: &str) {
    let os = cx.read(|cx| cx.platform_info().os);
    let keystroke = Keystroke::parse_with_os(key, os).unwrap();
    cx.update_window(window, |_, window, cx| {
        window.dispatch_event(
            KeyDownEvent {
                keystroke,
                is_held: false,
                prefer_character_input: false,
            }
            .to_platform_input(),
            cx,
        );
    })
    .unwrap();
}

struct MouseDownOutOwner {
    mouse_down_out_count: Rc<RefCell<usize>>,
}

impl Render for MouseDownOutOwner {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mouse_down_out_count = self.mouse_down_out_count.clone();
        div()
            .size_full()
            .child(
                div()
                    .id("target")
                    .w(px(50.))
                    .h(px(50.))
                    .on_mouse_down_out(move |_, _, _| {
                        *mouse_down_out_count.borrow_mut() += 1;
                    }),
            )
    }
}

// Official touch dispatch regressions from 635026fabc.
#[test]
fn touch_tap_out_fires_once_after_release() {
    let mut test_app = TestAppContext::single();
    let mouse_down_out_count = Rc::new(RefCell::new(0));
    let window = test_app.add_window({
        let mouse_down_out_count = mouse_down_out_count.clone();
        move |_, _| MouseDownOutOwner {
            mouse_down_out_count,
        }
    });
    let any_window: AnyWindowHandle = window.into();

    test_app
        .update_window(any_window, |_, window, cx| {
            window.draw(cx).clear(cx);
            let started = TouchEvent {
                id: TouchId(1),
                phase: TouchPhase::Started,
                position: point(px(75.), px(75.)),
                force: None,
                predicted_position: None,
            };
            window.dispatch_event(started.clone().to_platform_input(), cx);
            assert_eq!(*mouse_down_out_count.borrow(), 0);
            window.dispatch_event(
                TouchEvent {
                    phase: TouchPhase::Ended,
                    ..started
                }
                .to_platform_input(),
                cx,
            );
        })
        .unwrap();

    assert_eq!(
        *mouse_down_out_count.borrow(),
        1,
        "a touch tap outside the element should use mouse-down-out listeners"
    );
}

#[crate::test]
fn touch_pan_and_cancel_do_not_fire_mouse_down_out(cx: &mut TestAppContext) {
    let mouse_down_out_count = Rc::new(RefCell::new(0));
    let window = cx.add_window({
        let mouse_down_out_count = mouse_down_out_count.clone();
        move |_, _| MouseDownOutOwner {
            mouse_down_out_count,
        }
    });
    cx.update_window(window.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        for (id, samples) in [
            (
                TouchId(1),
                [
                    (TouchPhase::Started, 100.),
                    (TouchPhase::Moved, 75.),
                    (TouchPhase::Ended, 75.),
                ],
            ),
            (
                TouchId(2),
                [
                    (TouchPhase::Started, 100.),
                    (TouchPhase::Moved, 100.),
                    (TouchPhase::Cancelled, 100.),
                ],
            ),
        ] {
            for (phase, vertical_position) in samples {
                window.dispatch_event(
                    TouchEvent {
                        id,
                        phase,
                        position: point(px(75.), px(vertical_position)),
                        predicted_position: None,
                        force: None,
                    }
                    .to_platform_input(),
                    cx,
                );
            }
        }
    })
    .unwrap();
    assert_eq!(*mouse_down_out_count.borrow(), 0);
}

#[crate::test]
fn test_fractional_padding_does_not_make_a_fitting_container_scrollable(cx: &mut TestAppContext) {
    struct PaddedContainer {
        scroll_handle: ScrollHandle,
    }

    impl Render for PaddedContainer {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            // 4.25px of padding snaps to 4px in layout, so a 42px child fits the 50px box exactly.
            div().size_full().child(
                div()
                    .id("container")
                    .h(px(50.))
                    .w(px(100.))
                    .py(px(4.25))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .child(div().w_full().h(px(42.))),
            )
        }
    }

    let scroll_handle = ScrollHandle::new();
    let window: AnyWindowHandle = cx
        .add_window({
            let scroll_handle = scroll_handle.clone();
            move |_, _| PaddedContainer { scroll_handle }
        })
        .into();
    cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();

    assert_eq!(scroll_handle.max_offset().y, px(0.));
}

// The fork inverse-projects pixel input before choosing a scroll axis and records the source
// in its single tracked viewport owner.
struct TransformedAxisLockView {
    handle: ScrollHandle,
}

impl Render for TransformedAxisLockView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            div()
                .id("axis-lock")
                .size(px(100.))
                .overflow_scroll()
                .restrict_scroll_to_axis()
                .track_scroll(&self.handle)
                .child(div().flex_none().size(px(300.)))
                .with_subtree_transform(SubtreeTransform::try_scale(size(2., 0.5)).unwrap()),
        )
    }
}

#[crate::test]
fn scroll_axis_lock_uses_target_local_delta_and_preserves_touch_source(cx: &mut TestAppContext) {
    let handle = ScrollHandle::new();
    let (_, cx) = cx.add_window_view({
        let handle = handle.clone();
        move |_, _| TransformedAxisLockView { handle }
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_event(MouseMoveEvent {
        position: point(px(50.), px(50.)),
        ..Default::default()
    });
    cx.simulate_event(ScrollEvent {
        position: point(px(50.), px(50.)),
        delta: ScrollDelta::Pixels(point(px(-8.), px(-3.))),
        touch_phase: TouchPhase::Started,
        source: ScrollSource::Touch,
        ..Default::default()
    });
    // The local delta is (-4, -6), so the vertical axis wins despite the window-space X dominance.
    assert_eq!(handle.offset(), point(px(0.), px(-6.)));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(
        handle.committed_viewport_snapshot().unwrap().source(),
        ScrollViewportChangeSource::Touch,
    );
    cx.simulate_event(ScrollEvent {
        position: point(px(50.), px(50.)),
        delta: ScrollDelta::Pixels(point(px(-10.), px(-2.))),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Touch,
        ..Default::default()
    });
    // Below the unlock threshold, the next local delta (-5, -4) retains the vertical lock.
    assert_eq!(handle.offset(), point(px(0.), px(-10.)));
}
