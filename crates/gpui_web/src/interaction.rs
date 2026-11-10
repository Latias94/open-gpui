//! Immediate browser input dispatch and sequenced terminal cleanup.

use std::cell::{Cell, RefCell};

use open_gpui::{
    DispatchEventResult, NativePointerCancelReservation, PlatformInput, PlatformInputCallbackSlot,
    TouchEvent, TouchPhase,
};

#[derive(Default)]
pub(crate) struct WebInteractionGate {
    quiesced: Cell<bool>,
    dispatching: Cell<bool>,
    pending_terminal_inputs: RefCell<Vec<PlatformInput>>,
}

struct WebInputDispatchGuard<'a>(&'a Cell<bool>);

impl Drop for WebInputDispatchGuard<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

impl WebInteractionGate {
    pub(crate) fn quiesce(&self) {
        self.quiesced.set(true);
    }

    pub(crate) fn is_quiesced(&self) -> bool {
        self.quiesced.get()
    }

    pub(crate) fn dispatch_callback(
        &self,
        input_callback: &PlatformInputCallbackSlot,
        input: PlatformInput,
    ) -> DispatchEventResult {
        let terminal_cleanup = matches!(
            &input,
            PlatformInput::PointerCanceled(_)
                | PlatformInput::Touch(TouchEvent {
                    phase: TouchPhase::Cancelled,
                    ..
                })
        );
        if self.is_quiesced() && !terminal_cleanup {
            return DispatchEventResult::default();
        }

        if self.dispatching.get() && terminal_cleanup {
            let reservation = match &input {
                PlatformInput::PointerCanceled(event) => {
                    input_callback.reserve_reentrant_pointer_cancel(event.reason)
                }
                PlatformInput::Touch(event) => {
                    input_callback.reserve_reentrant_touch_cancel(event.clone())
                }
                _ => unreachable!(),
            };
            match reservation {
                NativePointerCancelReservation::Reserved
                | NativePointerCancelReservation::ApplicationGone
                | NativePointerCancelReservation::IngressClosed
                | NativePointerCancelReservation::RetiredSlot => {}
                NativePointerCancelReservation::UnleasedTestFallback => {
                    self.pending_terminal_inputs.borrow_mut().push(input);
                }
                outcome => panic!("failed to reserve terminal web input: {outcome:?}"),
            }
            return DispatchEventResult::default();
        }

        assert!(
            !self.dispatching.replace(true),
            "reentrant web input dispatch"
        );
        let dispatch_guard = WebInputDispatchGuard(&self.dispatching);
        let result = input_callback.dispatch(input);
        drop(dispatch_guard);
        let pending = std::mem::take(&mut *self.pending_terminal_inputs.borrow_mut());
        for input in pending {
            self.dispatch_callback(input_callback, input);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use open_gpui::{PlatformInputCallback, Point, TouchId};

    use super::*;

    #[test]
    fn touch_cleanup_reentered_from_input_is_delivered_after_callback_returns() {
        let callback = PlatformInputCallbackSlot::default();
        let interaction_gate = Rc::new(WebInteractionGate::default());
        let observed = Rc::new(RefCell::new(Vec::new()));
        callback.set(PlatformInputCallback::new_unleased_for_test(Box::new({
            let callback = callback.clone();
            let interaction_gate = interaction_gate.clone();
            let observed = observed.clone();
            move |input| {
                match input {
                    PlatformInput::Touch(event) if matches!(event.phase, TouchPhase::Started) => {
                        observed.borrow_mut().push("started");
                        interaction_gate.quiesce();
                        interaction_gate.dispatch_callback(
                            &callback,
                            PlatformInput::Touch(TouchEvent {
                                phase: TouchPhase::Cancelled,
                                ..event
                            }),
                        );
                        observed.borrow_mut().push("finished-started");
                    }
                    PlatformInput::Touch(event) if matches!(event.phase, TouchPhase::Cancelled) => {
                        observed.borrow_mut().push("canceled");
                    }
                    _ => panic!("unexpected input"),
                }
                DispatchEventResult {
                    propagate: false,
                    default_prevented: true,
                }
            }
        })));

        let result = interaction_gate.dispatch_callback(
            &callback,
            PlatformInput::Touch(TouchEvent {
                id: TouchId(1),
                phase: TouchPhase::Started,
                position: Point::default(),
                predicted_position: None,
                force: None,
            }),
        );

        assert!(!result.propagate);
        assert!(result.default_prevented);
        assert_eq!(
            observed.borrow().as_slice(),
            &["started", "finished-started", "canceled"]
        );
        callback.terminate();
    }
}
