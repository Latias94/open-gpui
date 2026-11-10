use super::*;

#[test]
fn long_press_deadline_uses_the_injected_clock() {
    let tuning = GestureTuning::default();
    let mut recognizer = TouchGestureRecognizer::new(tuning);
    let touch = TouchId(1);
    let now = Instant::now() + Duration::from_secs(600);
    let deadline = now + tuning.long_press_duration;

    recognizer.handle_event_at(&touch_event(touch, TouchPhase::Started, 10., 20.), now);
    assert_eq!(
        recognizer.pending_long_press_at(now),
        Some((touch, tuning.long_press_duration))
    );
    let before_deadline = deadline - Duration::from_millis(1);
    assert_eq!(
        recognizer.pending_long_press_at(before_deadline),
        Some((touch, Duration::from_millis(1)))
    );
    assert!(
        recognizer
            .offer_long_press_at(touch, before_deadline)
            .is_none()
    );
    assert_eq!(
        recognizer.pending_long_press_at(deadline),
        Some((touch, Duration::ZERO))
    );
    assert!(
        recognizer
            .offer_long_press_at(TouchId(2), deadline)
            .is_none()
    );
    let Some(RecognizedTouchGesture::LongPress(started)) =
        recognizer.offer_long_press_at(touch, deadline)
    else {
        panic!("expected long press at the injected deadline");
    };
    assert_eq!(started.phase, TouchPhase::Started);
    assert_eq!(started.start_position, point(px(10.), px(20.)));
    assert!(recognizer.pending_long_press_at(deadline).is_none());
    assert!(recognizer.offer_long_press_at(touch, deadline).is_none());
}

#[test]
fn overdue_long_press_has_no_remaining_delay() {
    let tuning = GestureTuning::default();
    let mut recognizer = TouchGestureRecognizer::new(tuning);
    let touch = TouchId(1);
    let now = Instant::now() + Duration::from_secs(600);
    let overdue = now + tuning.long_press_duration + Duration::from_secs(1);

    recognizer.handle_event_at(&touch_event(touch, TouchPhase::Started, 10., 20.), now);
    assert_eq!(
        recognizer.pending_long_press_at(overdue),
        Some((touch, Duration::ZERO))
    );
    assert!(matches!(
        recognizer.offer_long_press_at(touch, overdue),
        Some(RecognizedTouchGesture::LongPress(_))
    ));
}

#[test]
fn cancel_discards_pending_taps_and_resets_multi_tap_history() {
    for leave_pending in [false, true] {
        let mut recognizer = TouchGestureRecognizer::new(GestureTuning::default());
        let now = Instant::now();
        recognizer.handle_event_at(&touch_event(TouchId(1), TouchPhase::Started, 10., 20.), now);
        recognizer.handle_event_at(
            &touch_event(TouchId(1), TouchPhase::Ended, 10., 20.),
            now + Duration::from_millis(10),
        );
        if leave_pending {
            recognizer.handle_event_at(
                &touch_event(TouchId(2), TouchPhase::Started, 10., 20.),
                now + Duration::from_millis(20),
            );
        }

        assert!(recognizer.cancel().is_empty());
        assert!(recognizer.cancel().is_empty());
        assert!(!recognizer.has_momentum());
        assert!(recognizer.pending_long_press_at(now).is_none());
        assert!(
            recognizer
                .offer_long_press_at(TouchId(2), now + Duration::from_secs(1))
                .is_none()
        );
        assert!(
            recognizer
                .handle_event_at(
                    &touch_event(TouchId(2), TouchPhase::Ended, 10., 20.),
                    now + Duration::from_millis(30),
                )
                .is_empty()
        );

        recognizer.handle_event_at(
            &touch_event(TouchId(3), TouchPhase::Started, 10., 20.),
            now + Duration::from_millis(40),
        );
        let recognized = recognizer.handle_event_at(
            &touch_event(TouchId(3), TouchPhase::Ended, 10., 20.),
            now + Duration::from_millis(50),
        );
        let [RecognizedTouchGesture::Tap { down, up }] = recognized.as_slice() else {
            panic!("expected fresh tap after cancellation, got {recognized:?}");
        };
        assert_eq!(down.click_count, 1);
        assert_eq!(up.click_count, 1);
    }
}

#[test]
fn cancel_closes_an_active_pan_without_fling_or_tap() {
    let mut recognizer = TouchGestureRecognizer::new(GestureTuning::default());
    let now = Instant::now();
    let touch = TouchId(1);
    recognizer.handle_event_at(&touch_event(touch, TouchPhase::Started, 10., 20.), now);
    recognizer.handle_event_at(
        &touch_event(touch, TouchPhase::Moved, 10., 60.),
        now + Duration::from_millis(16),
    );

    let recognized = recognizer.cancel();
    let [RecognizedTouchGesture::Scroll(scroll)] = recognized.as_slice() else {
        panic!("expected ended pan, got {recognized:?}");
    };
    assert_eq!(scroll.touch_phase, TouchPhase::Ended);
    assert_eq!(scroll.position, point(px(10.), px(20.)));
    assert_eq!(scroll.source, ScrollSource::Touch);
    assert!(scroll.delta.pixel_delta(px(16.)).is_zero());
    assert!(!recognizer.has_momentum());
    assert!(recognizer.cancel().is_empty());
    assert!(
        recognizer
            .handle_event_at(
                &touch_event(touch, TouchPhase::Ended, 10., 60.),
                now + Duration::from_millis(32),
            )
            .is_empty()
    );
}

#[test]
fn cancel_closes_momentum_or_the_touch_that_caught_it() {
    for catch_fling in [false, true] {
        let mut recognizer = TouchGestureRecognizer::new(GestureTuning::default());
        let now = Instant::now();
        recognizer.handle_event_at(
            &touch_event(TouchId(1), TouchPhase::Started, 100., 500.),
            now,
        );
        for step in 1..=3 {
            recognizer.handle_event_at(
                &touch_event(
                    TouchId(1),
                    TouchPhase::Moved,
                    100.,
                    500. - step as f32 * 40.,
                ),
                now + Duration::from_millis(step * 16),
            );
        }
        recognizer.handle_event_at(
            &touch_event(TouchId(1), TouchPhase::Ended, 100., 380.),
            now + Duration::from_millis(64),
        );
        assert!(recognizer.has_momentum());

        let position = if catch_fling {
            recognizer.handle_event_at(
                &touch_event(TouchId(2), TouchPhase::Started, 200., 300.),
                now + Duration::from_millis(80),
            );
            point(px(200.), px(300.))
        } else {
            point(px(100.), px(500.))
        };
        let recognized = recognizer.cancel();
        let [RecognizedTouchGesture::Scroll(scroll)] = recognized.as_slice() else {
            panic!("expected ended scroll, got {recognized:?}");
        };
        assert_eq!(scroll.touch_phase, TouchPhase::Ended);
        assert_eq!(scroll.position, position);
        assert!(scroll.delta.pixel_delta(px(16.)).is_zero());
        assert!(!recognizer.has_momentum());
        assert!(
            recognizer
                .tick_momentum_at(now + Duration::from_secs(1))
                .is_none()
        );
        assert!(recognizer.cancel().is_empty());
    }
}

#[test]
fn cancel_closes_claimed_gestures_at_the_latest_position() {
    for is_drag in [false, true] {
        let tuning = GestureTuning::default();
        let mut recognizer = TouchGestureRecognizer::new(tuning);
        let now = Instant::now();
        let touch = TouchId(1);
        recognizer.handle_event_at(&touch_event(touch, TouchPhase::Started, 10., 20.), now);
        if is_drag {
            assert!(recognizer.offer_touch_drag(touch).is_some());
            recognizer.resolve_touch_drag(true);
        } else {
            assert!(
                recognizer
                    .offer_long_press_at(touch, now + tuning.long_press_duration)
                    .is_some()
            );
            recognizer.resolve_long_press(true);
        }
        recognizer.handle_event_at(
            &touch_event(touch, TouchPhase::Moved, 12., 21.),
            now + tuning.long_press_duration + Duration::from_millis(16),
        );

        let recognized = recognizer.cancel();
        let (phase, start_position, position) = match recognized.as_slice() {
            [RecognizedTouchGesture::TouchDrag(event)] if is_drag => {
                (event.phase, event.start_position, event.position)
            }
            [RecognizedTouchGesture::LongPress(event)] if !is_drag => {
                (event.phase, event.start_position, event.position)
            }
            _ => panic!("expected cancelled claimed gesture, got {recognized:?}"),
        };
        assert_eq!(phase, TouchPhase::Cancelled);
        assert_eq!(start_position, point(px(10.), px(20.)));
        assert_eq!(position, point(px(12.), px(21.)));
        assert!(!recognizer.has_momentum());
        assert!(recognizer.pending_long_press_at(now).is_none());
        assert!(recognizer.cancel().is_empty());
    }
}

fn touch_event(id: TouchId, phase: TouchPhase, x: f32, y: f32) -> TouchEvent {
    TouchEvent {
        id,
        phase,
        position: point(px(x), px(y)),
        predicted_position: None,
        force: None,
    }
}
