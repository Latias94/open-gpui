//! Ordinary public gesture consumers, without test-support or an application runtime.
//!
//! The tests call pure event conversions, scroll filtering and platform policy APIs, and build
//! actual listener/style interfaces. Registered callbacks and the Window/Platform closures are
//! not invoked. TouchGestureRecognizer and its outputs are crate-private at official target
//! 635026fabc38440fce5e1767d125026a17b12012; their recognition/timer matrix belongs to Core.

use open_gpui::{
    App, BackGestureEvent, BackGesturePhase, DispatchPhase, Entity, GestureKinds, GestureTuning,
    InputEvent, InteractiveElement as _, LongPressEvent, NullPlatformGestures, OngoingScroll,
    Platform, PlatformGestures, PlatformInput, ScrollDelta, ScrollEvent, ScrollPhysics,
    ScrollSource, ScrollWheelIntent, Styled as _, Subscription, TargetedEvent, TouchDragEvent,
    TouchEvent, TouchId, TouchPhase, Window, WindowMouseEvent, div, point, px,
};
use std::{rc::Rc, time::Duration};

#[test]
fn ordinary_scroll_events_preserve_pointer_defaults_and_touch_payloads() {
    let pointer = ScrollEvent::default();
    assert_eq!(pointer.source, ScrollSource::Pointer);
    assert!(!pointer.delta.precise());
    assert_eq!(pointer.to_platform_input().kind_name(), "scroll");

    let position = point(px(20.), px(30.));
    let delta = point(px(1.), px(-24.));
    let input = ScrollEvent {
        position,
        delta: ScrollDelta::Pixels(delta),
        modifiers: Default::default(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Touch,
    }
    .to_platform_input();
    assert!(input.touch_event().is_none());
    let PlatformInput::Scroll(scroll) = input else {
        panic!("a public ScrollEvent must convert to PlatformInput::Scroll");
    };
    assert_eq!(scroll.source, ScrollSource::Touch);
    assert_eq!(scroll.position, position);
    assert_eq!(scroll.touch_phase, TouchPhase::Moved);
    assert!(scroll.delta.precise());
    assert_eq!(scroll.delta.pixel_delta(px(16.)), delta);
}

#[test]
fn ordinary_raw_touch_conversion_retains_identity_prediction_and_cancellation() {
    let position = point(px(40.), px(50.));
    let prediction = point(px(42.), px(54.));
    let input = TouchEvent {
        id: TouchId(17),
        phase: TouchPhase::Moved,
        position,
        predicted_position: Some(prediction),
        force: Some(0.5),
    }
    .to_platform_input();
    assert_eq!(input.kind_name(), "touch");
    let touch = input.touch_event().expect("raw touches must be accessible");
    assert_eq!(touch.id, TouchId(17));
    assert_eq!(touch.position, position);
    assert_eq!(touch.predicted_position, Some(prediction));
    assert_eq!(touch.force, Some(0.5));

    let cancelled = TouchEvent {
        phase: TouchPhase::Cancelled,
        predicted_position: None,
        ..touch.clone()
    }
    .to_platform_input();
    let touch = cancelled
        .touch_event()
        .expect("cancellation is still raw touch input");
    assert_eq!(touch.id, TouchId(17));
    assert_eq!(touch.phase, TouchPhase::Cancelled);
    assert_eq!(touch.predicted_position, None);
}

#[test]
fn ordinary_semantic_gesture_conversions_preserve_terminal_phases() {
    let start_position = point(px(10.), px(12.));
    let position = point(px(18.), px(20.));
    let long_press = LongPressEvent {
        phase: TouchPhase::Cancelled,
        start_position,
        position,
    }
    .to_platform_input();
    assert_eq!(long_press.kind_name(), "long_press");
    let PlatformInput::LongPress(long_press) = long_press else {
        panic!("a public long press must keep its platform variant");
    };
    assert_eq!(long_press.phase, TouchPhase::Cancelled);
    assert_eq!(long_press.start_position, start_position);
    assert_eq!(long_press.position, position);

    let drag = TouchDragEvent {
        phase: TouchPhase::Ended,
        start_position,
        position,
    }
    .to_platform_input();
    assert_eq!(drag.kind_name(), "touch_drag");
    let PlatformInput::TouchDrag(drag) = drag else {
        panic!("a public touch drag must keep its platform variant");
    };
    assert_eq!(drag.phase, TouchPhase::Ended);
    assert_eq!(drag.start_position, start_position);
    assert_eq!(drag.position, position);

    let back = BackGestureEvent {
        phase: BackGesturePhase::Committed,
        start_position,
        progress: 1.,
    }
    .to_platform_input();
    assert_eq!(back.kind_name(), "back_gesture");
    let PlatformInput::BackGesture(back) = back else {
        panic!("a public back gesture must keep its platform variant");
    };
    assert_eq!(back.phase, BackGesturePhase::Committed);
    assert_eq!(back.start_position, start_position);
    assert_eq!(back.progress, 1.);
}

#[test]
fn ordinary_axis_filter_consumes_public_deltas_without_a_window() {
    let mut scroll = OngoingScroll::default();
    let mut delta = point(px(2.), px(-12.));
    scroll.filter(&mut delta, TouchPhase::Started);
    assert_eq!(delta, point(px(0.), px(-12.)));

    let mut cancellation = point(px(0.), px(0.));
    scroll.filter(&mut cancellation, TouchPhase::Cancelled);
    let mut next_gesture = point(px(14.), px(1.));
    scroll.filter(&mut next_gesture, TouchPhase::Started);
    assert_eq!(next_gesture, point(px(14.), px(0.)));
}

#[test]
fn ordinary_platform_policy_selects_public_gesture_tuning() {
    assert_eq!(
        NullPlatformGestures.native_recognizers(),
        GestureKinds::NONE
    );
    assert_eq!(NullPlatformGestures.tuning(), GestureTuning::default());

    struct NativePan;

    impl PlatformGestures for NativePan {
        fn tuning(&self) -> GestureTuning {
            GestureTuning {
                long_press_duration: Duration::from_millis(600),
                scroll_physics: ScrollPhysics::android(),
                ..GestureTuning::default()
            }
        }

        fn native_recognizers(&self) -> GestureKinds {
            GestureKinds {
                pan: true,
                ..GestureKinds::NONE
            }
        }
    }

    assert_eq!(
        NativePan.tuning().long_press_duration,
        Duration::from_millis(600)
    );
    assert_eq!(NativePan.tuning().scroll_physics, ScrollPhysics::android());
    assert!(NativePan.native_recognizers().pan);
    assert!(!NativePan.native_recognizers().tap);
}

#[test]
fn ordinary_elements_build_scroll_back_listeners_and_axis_style() {
    let mut surface = div()
        .overflow_y_scroll()
        .restrict_scroll_to_axis()
        .on_scroll(
            |event: &TargetedEvent<ScrollEvent>, _: &mut Window, _: &mut App| {
                let _: ScrollSource = event.window_event().source;
                let _ = event.target_local_delta();
                ScrollWheelIntent::handled().stop_propagation()
            },
        )
        .capture_scroll(|_: &TargetedEvent<ScrollEvent>, _, _| ScrollWheelIntent::allow_default())
        .on_raw_scroll(|_: &ScrollEvent, _, _| {})
        .capture_raw_scroll(|_: &ScrollEvent, _, _| {})
        .on_back_gesture(|event: &TargetedEvent<BackGestureEvent>, _, _| {
            let _: BackGesturePhase = event.window_event().phase;
        });
    assert_eq!(surface.style().restrict_scroll_to_axis, Some(true));

    // A real Window registers these listeners only while painting. This closure is type-only.
    let _paint_listeners_type_only = |window: &mut Window| {
        window.on_mouse_event(|event: &LongPressEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble && event.phase == TouchPhase::Started {
                window.prevent_default();
                cx.stop_propagation();
            }
        });
        window
            .on_mouse_event(|_: &TouchDragEvent, _: DispatchPhase, _: &mut Window, _: &mut App| {});
    };

    // No Platform or Window is created, and neither closure is invoked.
    let _platform_services_type_only: fn(&dyn Platform) -> Option<Rc<dyn PlatformGestures>> =
        |platform| platform.gestures();
    let _persistent_controls_type_only =
        |window: &mut Window, owner: &Entity<()>| -> Subscription {
            window.set_touch_prediction_enabled(true);
            let _: bool = window.last_input_was_touch();
            let owner = owner.clone();
            window.intercept_window_mouse_events(move |event, window, _| match event {
                WindowMouseEvent::LongPress(event) => {
                    if event.phase == TouchPhase::Started {
                        window.capture_long_press(&owner);
                        window.prevent_default();
                    }
                    let _: bool = window.has_long_press_capture(&owner);
                }
                WindowMouseEvent::TouchDrag(event) => {
                    let _: TouchPhase = event.phase;
                }
                WindowMouseEvent::BackGesture(event) => {
                    let _: BackGesturePhase = event.phase;
                }
                // WindowMouseEvent is non_exhaustive to ordinary dependencies.
                _ => {}
            })
        };
}
