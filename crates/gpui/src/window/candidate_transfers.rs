use super::Window;
use crate::GlobalElementId;
use std::{any::TypeId, mem};

#[derive(Default)]
pub(super) struct CandidateFrameTransfers {
    pub(super) element_states: Vec<(GlobalElementId, TypeId)>,
    pub(super) mouse_listeners: Vec<(usize, usize)>,
    pub(super) input_handlers: Vec<(usize, usize)>,
}

impl Window {
    pub(super) fn rollback_candidate_frame_transfers(&mut self, unwound: bool) {
        for key in self.candidate_frame_transfers.element_states.drain(..) {
            let Some(state) = self.next_frame.element_states.remove(&key) else {
                assert!(
                    unwound,
                    "candidate element state transfer must remain available for rollback"
                );
                continue;
            };
            assert!(
                self.rendered_frame
                    .element_states
                    .insert(key, state)
                    .is_none(),
                "candidate element state rollback must restore one vacant committed slot"
            );
        }

        for (rendered_index, candidate_index) in
            self.candidate_frame_transfers.mouse_listeners.drain(..)
        {
            let listener = self.next_frame.mouse_listeners[candidate_index]
                .value
                .take();
            assert!(
                self.rendered_frame.mouse_listeners[rendered_index]
                    .value
                    .is_none(),
                "candidate mouse-listener rollback must restore one vacant committed slot"
            );
            self.rendered_frame.mouse_listeners[rendered_index].value = listener;
        }

        for (rendered_index, candidate_index) in
            self.candidate_frame_transfers.input_handlers.drain(..)
        {
            let mut handler = self.next_frame.input_handlers[candidate_index].value.take();
            if let Some(handler) = handler.as_mut() {
                handler.set_validity(
                    self.rendered_frame.input_handlers[rendered_index]
                        .validity
                        .clone(),
                );
            }
            assert!(
                self.rendered_frame.input_handlers[rendered_index]
                    .value
                    .is_none(),
                "candidate input-handler rollback must restore one vacant committed slot"
            );
            self.rendered_frame.input_handlers[rendered_index].value = handler;
        }
    }

    /// Returns transferred values to their rendered slots and takes the transfer log.
    ///
    /// Candidate drawing must have completed successfully. Every recorded element state
    /// must still be in `next_frame`, and the recorded rendered destinations must be vacant.
    /// Neither frame may be cleared, finished, swapped, or have its recorded slots removed
    /// before the returned log is resumed or discarded together with the candidate.
    ///
    /// Both frames retain their slot indices and validity metadata. Transferred input
    /// handlers regain their rendered validity while suspended. If the caller checks one
    /// out for the platform, it must return that handler to its exact rendered slot before
    /// calling `resume_candidate_frame_transfers`.
    pub(super) fn suspend_candidate_frame_transfers(&mut self) -> CandidateFrameTransfers {
        for key in &self.candidate_frame_transfers.element_states {
            assert!(
                !self.rendered_frame.element_states.contains_key(key),
                "candidate element state suspension must restore one vacant committed slot"
            );
            let (key, state) =
                self.next_frame.element_states.remove_entry(key).expect(
                    "candidate element state transfer must remain available for suspension",
                );
            self.rendered_frame.element_states.insert(key, state);
        }

        for &(rendered_index, candidate_index) in &self.candidate_frame_transfers.mouse_listeners {
            let candidate = &mut self.next_frame.mouse_listeners[candidate_index];
            if candidate.value.is_some() {
                let rendered = &mut self.rendered_frame.mouse_listeners[rendered_index];
                assert!(
                    rendered.value.is_none(),
                    "candidate mouse-listener suspension must restore one vacant committed slot"
                );
                rendered.value = candidate.value.take();
            }
        }

        for &(rendered_index, candidate_index) in &self.candidate_frame_transfers.input_handlers {
            let candidate = &mut self.next_frame.input_handlers[candidate_index];
            if let Some(handler) = candidate.value.as_mut() {
                let rendered = &mut self.rendered_frame.input_handlers[rendered_index];
                assert!(
                    rendered.value.is_none(),
                    "candidate input-handler suspension must restore one vacant committed slot"
                );
                handler.set_validity(rendered.validity.clone());
                rendered.value = candidate.value.take();
            }
        }

        mem::take(&mut self.candidate_frame_transfers)
    }

    /// Returns suspended values to their candidate slots and reinstates the transfer log.
    ///
    /// `transfers` must come from `suspend_candidate_frame_transfers` for these same frames.
    /// Their recorded indices, keys, and validity metadata must be unchanged, the candidate
    /// destinations must be vacant, and the active transfer log must be empty. Every
    /// recorded element state must still be in `rendered_frame`.
    ///
    /// The caller must return any transferred value checked out while suspended to its
    /// original rendered slot first. In particular, a transferred input handler installed
    /// on the platform must be taken back and reinserted there before this call. Empty
    /// optional source slots are left untouched; values still held outside the frames
    /// cannot be recovered here. Input handlers regain their candidate slot's validity.
    pub(super) fn resume_candidate_frame_transfers(&mut self, transfers: CandidateFrameTransfers) {
        assert!(
            self.candidate_frame_transfers.element_states.is_empty()
                && self.candidate_frame_transfers.mouse_listeners.is_empty()
                && self.candidate_frame_transfers.input_handlers.is_empty(),
            "resuming candidate frame transfers requires an empty active transfer log"
        );

        for key in &transfers.element_states {
            assert!(
                !self.next_frame.element_states.contains_key(key),
                "candidate element state resumption must restore one vacant candidate slot"
            );
            let (key, state) =
                self.rendered_frame.element_states.remove_entry(key).expect(
                    "suspended element state transfer must remain available for resumption",
                );
            self.next_frame.element_states.insert(key, state);
        }

        for &(rendered_index, candidate_index) in &transfers.mouse_listeners {
            let rendered = &mut self.rendered_frame.mouse_listeners[rendered_index];
            if rendered.value.is_some() {
                let candidate = &mut self.next_frame.mouse_listeners[candidate_index];
                assert!(
                    candidate.value.is_none(),
                    "candidate mouse-listener resumption must restore one vacant candidate slot"
                );
                candidate.value = rendered.value.take();
            }
        }

        for &(rendered_index, candidate_index) in &transfers.input_handlers {
            let rendered = &mut self.rendered_frame.input_handlers[rendered_index];
            if let Some(handler) = rendered.value.as_mut() {
                let candidate = &mut self.next_frame.input_handlers[candidate_index];
                assert!(
                    candidate.value.is_none(),
                    "candidate input-handler resumption must restore one vacant candidate slot"
                );
                handler.set_validity(candidate.validity.clone());
                candidate.value = rendered.value.take();
            }
        }

        self.candidate_frame_transfers = transfers;
    }
}
