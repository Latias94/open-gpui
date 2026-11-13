use super::*;

#[derive(Default)]
pub(super) struct MacWindowActiveStatus {
    generation: u64,
}

impl MacWindowActiveStatus {
    pub(super) fn record_native_event(&mut self) -> u64 {
        self.generation = self
            .generation
            .checked_add(1)
            .expect("macOS native activation generation overflowed");
        self.generation
    }

    fn is_current(&self, generation: Option<u64>) -> bool {
        generation == Some(self.generation)
    }
}

pub(super) fn enqueue_observation(
    window_state: &Arc<Mutex<MacWindowState>>,
    event: MacWindowActiveEvent,
    generation: Option<u64>,
    source: Option<PlatformWindowActiveStatusSource>,
    exact_candidate: bool,
) {
    // AppKit can notify DidBecomeKey before NSApplication.keyWindow is updated. Like upstream,
    // publish application callbacks after the native notification returns. Synchronous frame
    // requests and the spurious-popup workaround remain in the original event drain.
    let weak_window_state = Arc::downgrade(window_state);
    DispatchQueue::main().exec_async(move || {
        let Some(window_state) = weak_window_state.upgrade() else {
            return;
        };
        let mut panic_boundary = MacWindowCallbackPanicBoundary::default();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            deliver_observation(
                &window_state,
                event,
                generation,
                source,
                exact_candidate,
                &mut panic_boundary,
            );
        }));
        if let Err(panic) = result {
            panic_boundary.retain(panic);
        }
        panic_boundary.isolate_at_native_boundary("a deferred macOS active-state observation");
    });
}

fn exact_native_positive(
    state: &MacWindowState,
    event: MacWindowActiveEvent,
    generation: Option<u64>,
    exact_candidate: bool,
) -> bool {
    event.is_active()
        && exact_candidate
        && state.active_status.is_current(generation)
        && state.committed_native_observation().is_active
        && crate::main_thread_marker().is_some_and(|marker| unsafe {
            let app = NSApplication::sharedApplication(marker);
            let key_window: *mut AnyObject = msg_send![&*app, keyWindow];
            state.native_window().isKeyWindow() && key_window == state.native_window
        })
}

fn deliver_observation(
    window_state: &Arc<Mutex<MacWindowState>>,
    event: MacWindowActiveEvent,
    generation: Option<u64>,
    source: Option<PlatformWindowActiveStatusSource>,
    exact_candidate: bool,
    panic_boundary: &mut MacWindowCallbackPanicBoundary,
) {
    let should_reorder = {
        let state = window_state.lock();
        if state.is_closed() || state.interaction_is_quiesced() {
            return;
        }
        exact_native_positive(&state, event, generation, exact_candidate)
    };
    if should_reorder && !reorder_mac_transient_after_native_activation(window_state) {
        log::error!("failed to reorder an exact active macOS transient window");
    }

    let (callback, exact_positive) = {
        let mut state = window_state.lock();
        if state.is_closed() || state.interaction_is_quiesced() {
            return;
        }
        if event.is_active() {
            state.move_traffic_light();
        }
        let Some(callback) = state.activate_callback.clone() else {
            return;
        };
        // Reordering may reenter native callbacks. Recheck the original event identity and live
        // key authority before granting a positive result to an application activation request.
        let exact_positive = exact_native_positive(&state, event, generation, exact_candidate);
        (callback, exact_positive)
    };
    panic_boundary.deliver(|| {
        let mut observation =
            PlatformWindowActiveStatusObservation::new(event.is_active(), exact_positive);
        if let Some(source) = source {
            observation = observation.with_source(source);
        }
        callback(observation);
    });
    panic_boundary.deliver(|| drop(callback));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_replaced_native_edge_cannot_confirm_reactivation_of_the_same_window() {
        let mut status = MacWindowActiveStatus::default();
        let first_became_key = status.record_native_event();
        assert!(status.is_current(Some(first_became_key)));
        let resigned_key = status.record_native_event();
        let second_became_key = status.record_native_event();
        assert!(!status.is_current(Some(first_became_key)));
        assert!(!status.is_current(Some(resigned_key)));
        assert!(status.is_current(Some(second_became_key)));
        assert!(!status.is_current(None));
    }

    #[test]
    fn observation_replay_retains_each_original_activation_generation() {
        let mut status = MacWindowActiveStatus::default();
        let mut events = MacWindowPendingObservationEvents::default();
        for (event, observed) in [
            (MacWindowActiveEvent::BecameKey, true),
            (MacWindowActiveEvent::ResignedKey, false),
            (MacWindowActiveEvent::BecameKey, true),
        ] {
            let mut pending = MacWindowPendingObservationEvent::observed(
                MacWindowObservationEvent::Active(event),
                observed,
            );
            pending.active_generation = Some(status.record_native_event());
            events.record(pending);
        }
        let batches = events.into_effect_batches(true);
        assert_eq!(batches.len(), 3);
        for (index, batch) in batches.into_iter().enumerate() {
            let event = batch.event.unwrap();
            assert_eq!(event.active_generation, Some(index as u64 + 1));
            assert_eq!(event.is_latest_active, index == 2);
            assert_eq!(status.is_current(event.active_generation), index == 2);
        }
    }
}
