use std::{any::Any, panic::AssertUnwindSafe};

use anyhow::Context as _;
use open_gpui_core_util::ResultExt;

use crate::{
    App, LongPressEvent, RecognizedTouchGesture, ScrollEvent, ScrollSource, TouchDragEvent,
    TouchEvent, TouchPhase, Window,
};

use super::InputDispatchGuard;

pub(super) fn is_touch_cancellation(event: &dyn Any) -> bool {
    event
        .downcast_ref::<LongPressEvent>()
        .is_some_and(|event| event.phase == TouchPhase::Cancelled)
        || event
            .downcast_ref::<TouchDragEvent>()
            .is_some_and(|event| event.phase == TouchPhase::Cancelled)
        || event.downcast_ref::<ScrollEvent>().is_some_and(|event| {
            event.source == ScrollSource::Touch
                && matches!(event.touch_phase, TouchPhase::Ended | TouchPhase::Cancelled)
        })
}

impl Window {
    /// Returns whether recognized touch pans may use platform-predicted positions.
    /// Defaults to true.
    pub fn touch_prediction_enabled(&self) -> bool {
        self.touch_prediction_enabled
    }

    /// Enables platform-predicted touch positions for recognized pans.
    ///
    /// Predictions affect emitted movement, not gesture classification or velocity estimation.
    pub fn set_touch_prediction_enabled(&mut self, enabled: bool) {
        self.touch_prediction_enabled = enabled;
    }

    pub(super) fn dispatch_touch_event(&mut self, event: &TouchEvent, cx: &mut App) {
        let mut event = event.clone();
        if !self.touch_prediction_enabled {
            event.predicted_position = None;
        }
        let now = cx.background_executor().now();
        let generation = self.touch_gesture_generation;
        let recognized = self.touch_gestures.handle_event_at(&event, now);
        if event.phase == TouchPhase::Started
            && let Some(drag) = self.touch_gestures.offer_touch_drag(event.id)
        {
            self.dispatch_recognized_touch_gesture(drag, cx);
        }
        if event.phase == TouchPhase::Started
            && self.touch_gestures.pending_long_press_at(now).is_some()
        {
            self.long_press_capture = None;
        }
        let mut tapped = false;
        for gesture in recognized {
            if generation != self.touch_gesture_generation || !self.user_interaction_is_admitted() {
                break;
            }
            tapped |= matches!(gesture, RecognizedTouchGesture::Tap { .. });
            self.dispatch_recognized_touch_gesture(gesture, cx);
        }
        if generation != self.touch_gesture_generation || !self.user_interaction_is_admitted() {
            return;
        }
        if event.phase == TouchPhase::Started {
            self.schedule_long_press_timer(cx);
        } else if self.touch_gestures.pending_long_press_at(now).is_none() {
            self.long_press_timer.take();
        }
        // The platform's touch-release handler may inspect the input handler
        // as soon as this dispatch returns (the web platform decides virtual
        // keyboard visibility there, inside the user gesture). Input handlers
        // are registered during draw, so draw now to make them reflect any
        // focus change the tap just caused.
        if tapped && self.invalidator.is_dirty() {
            self.draw(cx).clear(cx);
        }
        if self.touch_gestures.has_momentum() {
            self.schedule_touch_momentum_tick();
        }
    }

    fn dispatch_recognized_touch_gesture(&mut self, gesture: RecognizedTouchGesture, cx: &mut App) {
        match gesture {
            RecognizedTouchGesture::Scroll(scroll) => {
                self.mouse_position = scroll.position;
                self.mouse_in_window = true;
                cx.propagate_event = true;
                self.default_prevented = false;
                self.dispatch_mouse_event(&scroll, cx);
            }
            RecognizedTouchGesture::Tap { down, up } => {
                self.mouse_position = up.position;
                self.mouse_in_window = true;
                cx.propagate_event = true;
                self.default_prevented = false;
                self.dispatch_mouse_event(&down, cx);
                if self.user_interaction_is_admitted() {
                    cx.propagate_event = true;
                    self.default_prevented = false;
                    self.dispatch_mouse_event(&up, cx);
                }
            }
            RecognizedTouchGesture::TouchDrag(drag) => {
                self.mouse_position = drag.start_position;
                self.mouse_in_window = true;
                cx.propagate_event = true;
                self.default_prevented = false;
                let started = drag.phase == TouchPhase::Started;
                if started {
                    self.offered_touch_cancellation =
                        Some(RecognizedTouchGesture::TouchDrag(TouchDragEvent {
                            phase: TouchPhase::Cancelled,
                            ..drag.clone()
                        }));
                }
                let dispatch = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    self.dispatch_mouse_event(&drag, cx);
                }));
                self.offered_touch_cancellation.take();
                if let Err(payload) = dispatch {
                    std::panic::resume_unwind(payload);
                }
                if started && self.user_interaction_is_admitted() {
                    self.touch_gestures
                        .resolve_touch_drag(self.default_prevented);
                }
            }
            RecognizedTouchGesture::LongPress(press) => {
                self.mouse_position = if press.phase == TouchPhase::Started {
                    press.start_position
                } else {
                    press.position
                };
                self.mouse_in_window = true;
                cx.propagate_event = true;
                self.default_prevented = false;
                let started = press.phase == TouchPhase::Started;
                if started {
                    self.offered_touch_cancellation =
                        Some(RecognizedTouchGesture::LongPress(LongPressEvent {
                            phase: TouchPhase::Cancelled,
                            ..press.clone()
                        }));
                }
                let dispatch = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    self.dispatch_mouse_event(&press, cx);
                }));
                self.offered_touch_cancellation.take();
                if let Err(payload) = dispatch {
                    std::panic::resume_unwind(payload);
                }
                if started && self.user_interaction_is_admitted() {
                    let claimed = self.default_prevented;
                    self.touch_gestures.resolve_long_press(claimed);
                    if !claimed {
                        self.long_press_capture = None;
                    }
                }
            }
        }
    }

    fn schedule_long_press_timer(&mut self, cx: &mut App) {
        self.long_press_timer.take();
        let Some((touch_id, duration)) = self
            .touch_gestures
            .pending_long_press_at(cx.background_executor().now())
        else {
            return;
        };
        let generation = self.touch_gesture_generation;
        self.long_press_timer = Some(self.spawn(cx, async move |cx| {
            cx.background_executor().timer(duration).await;
            cx.update(move |window, cx| {
                if generation != window.touch_gesture_generation
                    || !window.user_interaction_is_admitted()
                {
                    return;
                }
                window.long_press_timer.take();
                window.with_input_transaction(cx, |window, cx| {
                    let Some(_guard) =
                        InputDispatchGuard::try_enter(window.input_dispatch_active.clone())
                    else {
                        window.schedule_long_press_timer(cx);
                        return;
                    };
                    if let Some(gesture) = window
                        .touch_gestures
                        .offer_long_press_at(touch_id, cx.background_executor().now())
                    {
                        window.dispatch_recognized_touch_gesture(gesture, cx);
                    }
                });
            })
            .with_context(|| {
                format!(
                    "handling long-press timer for window {:?}",
                    cx.window_handle().window_id()
                )
            })
            .log_err();
        }));
    }

    fn schedule_touch_momentum_tick(&mut self) {
        if self.touch_momentum_tick_scheduled {
            return;
        }
        self.touch_momentum_tick_scheduled = true;
        let generation = self.touch_gesture_generation;
        self.on_next_frame(move |window, cx| {
            if generation != window.touch_gesture_generation {
                return;
            }
            window.touch_momentum_tick_scheduled = false;
            if !window.user_interaction_is_admitted() {
                window.cancel_touch_gestures(cx);
                return;
            }
            window.with_input_transaction(cx, |window, cx| {
                let Some(_guard) =
                    InputDispatchGuard::try_enter(window.input_dispatch_active.clone())
                else {
                    window.schedule_touch_momentum_tick();
                    return;
                };
                if let Some(gesture) = window
                    .touch_gestures
                    .tick_momentum_at(cx.background_executor().now())
                {
                    window.dispatch_recognized_touch_gesture(gesture, cx);
                }
                if generation == window.touch_gesture_generation
                    && window.touch_gestures.has_momentum()
                {
                    window.schedule_touch_momentum_tick();
                }
            });
        });
    }

    pub(super) fn finish_touch_gesture_event(&mut self, event: &dyn Any) {
        if event
            .downcast_ref::<LongPressEvent>()
            .is_some_and(|event| matches!(event.phase, TouchPhase::Ended | TouchPhase::Cancelled))
        {
            self.long_press_capture = None;
        }
    }

    pub(super) fn cancel_touch_gestures(&mut self, cx: &mut App) {
        self.touch_gesture_generation = self
            .touch_gesture_generation
            .checked_add(1)
            .expect("touch gesture generation space exhausted");
        self.long_press_timer.take();
        self.touch_momentum_tick_scheduled = false;
        self.pending_touch_cancellations
            .extend(self.touch_gestures.cancel());
        self.pending_touch_cancellations
            .extend(self.offered_touch_cancellation.take());
        if !self.input_dispatch_active.get() {
            self.flush_touch_cancellations(cx);
            self.long_press_capture = None;
        }
    }

    pub(crate) fn dispatch_reserved_native_touch_cancel(
        &mut self,
        event: TouchEvent,
        cx: &mut App,
    ) {
        if event.phase == TouchPhase::Cancelled && !self.user_interaction_is_admitted() {
            self.cancel_touch_gestures(cx);
        }
    }

    pub(super) fn flush_touch_cancellations(&mut self, cx: &mut App) {
        if self.pending_touch_cancellations.is_empty() {
            return;
        }
        if self.input_dispatch_active.get() {
            self.flush_touch_cancellations_inner(cx);
        } else {
            self.with_input_transaction(cx, |window, cx| {
                let _guard = InputDispatchGuard::try_enter(window.input_dispatch_active.clone())
                    .expect("terminal touch dispatch requires exclusive input custody");
                window.flush_touch_cancellations_inner(cx);
            });
        }
    }

    fn flush_touch_cancellations_inner(&mut self, cx: &mut App) {
        let pending = std::mem::take(&mut self.pending_touch_cancellations);
        let propagate = cx.propagate_event;
        let prevented = self.default_prevented;
        let mut first_panic = None;
        for gesture in pending {
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                self.dispatch_recognized_touch_gesture(gesture, cx);
            }));
            if let Err(payload) = result
                && first_panic.is_none()
            {
                first_panic = Some(payload);
            }
        }
        self.long_press_capture = None;
        cx.propagate_event &= propagate;
        self.default_prevented |= prevented;
        if let Some(payload) = first_panic {
            std::panic::resume_unwind(payload);
        }
    }
}

#[cfg(test)]
mod tests;
