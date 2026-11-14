//! Suspended custody of the original Window's prepared candidate transaction.
//!
//! The prepared checkpoint holds the private candidate overlay while the Window retains its
//! submitted authority. Resumption returns that overlay to the original commit path.

use super::{
    CandidateFrameTransactionPhase, CandidateFrameTransfers, DrawPhase, FocusClaimOutcome,
    FocusClaimResolution, PendingFocusClaim, PreparedCandidateFrame, ProvisionalFocusClaim, Window,
    WindowRemovalState,
};
use crate::{
    App, AtlasTextureLeaseEpoch, EntityId, FocusId, Pixels, PlatformFrameSubmission,
    PlatformFrameSubmissionStatus, PlatformWindowPhysicalGeometry, PlatformWindowPresentOutcome,
    Size, Subscription, WindowPresentAttemptFacts,
};
use open_gpui_collections::FxHashSet;
use std::{mem, rc::Rc, sync::atomic::Ordering::SeqCst};

#[cfg(test)]
mod tests;

pub(super) struct DeferredCandidateFrame {
    prepared: PreparedCandidateFrame,
    pub(super) submission: PlatformFrameSubmission,
    pub(super) delivered: bool,
    transfers: CandidateFrameTransfers,
    focus_resolutions: Vec<FocusClaimResolution>,
    pending_input_clear: bool,
    pending_input_notification: bool,
    text_notifications: (Option<FocusId>, u8),
    virtual_keyboard_request: Option<bool>,
    keyboard_revision: u64,
    accessibility_focus: Option<FocusId>,
    accessed_entities: FxHashSet<EntityId>,
    observers: Vec<Subscription>,
    update_count: usize,
    basis: DeferredCandidateBasis,
    #[cfg(feature = "profiler")]
    pub(super) draw_timing: Option<(
        crate::profiler::PreparedDrawTiming,
        usize,
        super::invalidator::FrameDirtyAccumulator,
    )>,
}

impl DeferredCandidateFrame {
    fn is_current(&self, window: &Window) -> bool {
        self.submission.status() != PlatformFrameSubmissionStatus::Cancelled
            && self.submission.window_id() == window.handle.window_id()
            && self.submission.frame_generation() == window.next_frame.generation
            && self.submission.frame_generation()
                == window.rendered_frame.generation.saturating_add(1)
            && self
                .prepared
                .accessibility
                .as_ref()
                .is_none_or(|(_, generation)| *generation == window.a11y.activation_generation())
            && self.basis.is_current(window)
    }
}

struct DeferredCandidateBasis {
    physical_geometry: Option<PlatformWindowPhysicalGeometry>,
    viewport_size: Size<Pixels>,
    scale_factor: f32,
    font_generation: usize,
    atlas_epoch: AtlasTextureLeaseEpoch,
    focus: Option<FocusId>,
    focus_generation: u64,
    focus_claim_revision: u64,
    pending_focus_claim: Option<PendingFocusClaim>,
    pending_blur_claim_generation: Option<u64>,
    provisional_focus_claim: Option<ProvisionalFocusClaim>,
    focus_completion_id: Option<u64>,
    interaction_admitted: bool,
    provisional_interaction_admitted: bool,
    active: bool,
    key_event_revision: u64,
}

impl DeferredCandidateBasis {
    fn capture(window: &Window) -> Self {
        Self {
            physical_geometry: window.platform_facts.physical_geometry,
            viewport_size: window.viewport_size,
            scale_factor: window.scale_factor,
            font_generation: window.text_system().font_generation(),
            atlas_epoch: window.sprite_atlas.atlas_texture_lease_epoch(),
            focus: window.focus,
            focus_generation: window.focus_generation,
            focus_claim_revision: window.focus_claim_revision,
            pending_focus_claim: window.pending_focus_claim,
            pending_blur_claim_generation: window.pending_blur_claim_generation,
            provisional_focus_claim: window.provisional_focus_claim,
            focus_completion_id: window
                .pending_focus_completion
                .as_ref()
                .map(|claim| claim.id),
            interaction_admitted: window.interaction_authority.is_admitted(),
            provisional_interaction_admitted: window.provisional_session_accepts_interaction(),
            active: window.active.get(),
            key_event_revision: window.key_event_revision,
        }
    }

    fn is_current(&self, window: &Window) -> bool {
        let platform = window.platform_window.platform_facts();
        window.presentation_is_allowed()
            && window.removal_state == WindowRemovalState::Open
            && !window.removed
            && !window.native_closed.load(SeqCst)
            && self.physical_geometry == window.platform_facts.physical_geometry
            && self.physical_geometry == platform.physical_geometry
            && self.viewport_size == window.viewport_size
            && self.viewport_size == platform.content_size
            && self.scale_factor == window.scale_factor
            && self.scale_factor == platform.scale_factor
            && self.font_generation == window.text_system().font_generation()
            && self.atlas_epoch == window.sprite_atlas.atlas_texture_lease_epoch()
            && self.focus == window.focus
            && self.focus_generation == window.focus_generation
            && self.focus_claim_revision == window.focus_claim_revision
            && self.pending_focus_claim == window.pending_focus_claim
            && self.pending_blur_claim_generation == window.pending_blur_claim_generation
            && self.provisional_focus_claim == window.provisional_focus_claim
            && self.focus_completion_id
                == window
                    .pending_focus_completion
                    .as_ref()
                    .map(|claim| claim.id)
            && self.interaction_admitted == window.interaction_authority.is_admitted()
            && self.provisional_interaction_admitted
                == window.provisional_session_accepts_interaction()
            && self.active == window.active.get()
            && self.key_event_revision == window.key_event_revision
            && window
                .next_frame
                .atlas_texture_lease_entries
                .iter()
                .all(|entry| entry.as_ref().is_ok_and(|lease| lease.validate().is_ok()))
    }
}

impl Window {
    pub(super) fn deferred_candidate_generation(&self) -> Option<u64> {
        self.deferred_candidate
            .as_ref()
            .map(|candidate| candidate.submission.frame_generation())
    }

    pub(super) fn defer_candidate_frame(
        &mut self,
        mut prepared: PreparedCandidateFrame,
        cx: &mut App,
    ) {
        assert!(self.deferred_candidate.is_none());
        assert!(self.candidate_frame_transaction.is_none());
        assert_eq!(
            prepared.transaction.phase,
            CandidateFrameTransactionPhase::Prepared
        );
        assert_eq!(
            prepared.transaction.frame_generation,
            self.next_frame.generation
        );

        self.next_frame.scene.finish();
        let original_authority = mem::replace(
            &mut prepared.authority_checkpoint,
            self.candidate_frame_authority_checkpoint(),
        );
        let interaction_changed = original_authority.interaction_quiesced
            != prepared.authority_checkpoint.interaction_quiesced;
        let focus_resolutions = self
            .focus_claim_resolutions
            .split_off(original_authority.focus_claim_resolutions_len);
        let pending_input_clear = self.candidate_pending_input_clear;
        let pending_input_notification = self.candidate_pending_input_notification;
        let text_notifications = self.candidate_text_notifications.replace((None, 0));
        let virtual_keyboard_request = self.candidate_virtual_keyboard_request.take();
        let accessibility_focus = self.candidate_accessibility_focus.take();
        let transfers = self.suspend_candidate_frame_transfers();
        self.refreshing = false;
        self.invalidator.set_phase(DrawPhase::None);
        let notify_pending_input_changed =
            self.restore_candidate_frame_authority(original_authority);
        self.mouse_hit_test = self.rendered_frame.hit_test(self.mouse_position);
        if self.user_interaction_is_admitted()
            && let Some(index) = prepared.restored_input_handler_index
            && let Some(handler) = self.rendered_frame.input_handlers[index].value.take()
        {
            self.platform_window.set_input_handler(handler);
        }
        if notify_pending_input_changed {
            self.defer_pending_input_changed_after_interaction_quiescence(cx);
        }

        let accessed_entities = cx.entities.take_accessed();
        let observers = accessed_entities
            .iter()
            .map(|entity_id| {
                let invalidator = self.invalidator.clone();
                cx.new_observer(
                    *entity_id,
                    Box::new(move |_| {
                        invalidator.set_dirty(true);
                        true
                    }),
                )
            })
            .collect();
        let app = cx.to_async();
        let basis = DeferredCandidateBasis::capture(self);
        let text_system = self.text_system.clone();
        let sprite_atlas = self.sprite_atlas.clone();
        let font_generation = basis.font_generation;
        let atlas_epoch = basis.atlas_epoch;
        let submission = PlatformFrameSubmission::new(
            self.handle.window_id(),
            prepared.transaction.frame_generation,
            prepared.transaction.attempt_id.0,
            Rc::new(move |submission, outcome| {
                app.enqueue_frame_submission_completed(submission, outcome)
            }),
        )
        .with_validity(Rc::new(move || {
            text_system.font_generation() == font_generation
                && sprite_atlas.atlas_texture_lease_epoch() == atlas_epoch
        }));
        self.deferred_candidate = Some(DeferredCandidateFrame {
            prepared,
            submission,
            delivered: false,
            transfers,
            focus_resolutions,
            pending_input_clear,
            pending_input_notification,
            text_notifications,
            virtual_keyboard_request,
            keyboard_revision: self.virtual_keyboard_request_revision.get(),
            accessibility_focus,
            accessed_entities,
            observers,
            update_count: self.invalidator.update_count(),
            basis,
            #[cfg(feature = "profiler")]
            draw_timing: None,
        });
        self.needs_present.set(true);
        if interaction_changed {
            self.cancel_deferred_candidate(cx);
        }
    }

    pub(super) fn cancel_deferred_submission(&self) -> bool {
        let retained = self.cancel_deferred_scene_submission();
        self.deferred_candidate
            .as_ref()
            .is_some_and(|candidate| candidate.submission.cancel())
            || retained
    }

    pub(super) fn cancel_deferred_candidate_if_stale(&mut self, cx: &mut App) -> bool {
        if self.deferred_candidate.is_some() {
            self.a11y.sync_active_flag();
        }
        let stale = self
            .deferred_candidate
            .as_ref()
            .is_some_and(|candidate| !candidate.is_current(self));
        stale && self.cancel_deferred_candidate(cx)
    }

    pub(super) fn present_deferred_candidate(
        &mut self,
        cx: &mut App,
    ) -> PlatformWindowPresentOutcome {
        if self.cancel_deferred_candidate_if_stale(cx) {
            return PlatformWindowPresentOutcome::RepaintRequired;
        }
        let Some(candidate) = self.deferred_candidate.as_mut() else {
            return PlatformWindowPresentOutcome::Rejected;
        };
        let submission = candidate.submission.clone();
        if !candidate.delivered {
            candidate.delivered = true;
            let elapsed = cx.background_executor().now() - cx.synced_animation_epoch;
            self.next_frame.scene.advance_animations(elapsed);
            let outcome = self
                .platform_window
                .draw_with_submission(&self.next_frame.scene, &submission);
            submission.finish_without_callback(outcome);
            #[cfg(feature = "profiler")]
            if outcome == PlatformWindowPresentOutcome::Deferred
                && submission.status() == PlatformFrameSubmissionStatus::Pending
            {
                let at = open_gpui_scheduler::Instant::now();
                self.window_profiler.record_present_outcome(
                    outcome,
                    at,
                    at,
                    self.active.get(),
                    !self.next_frame_callbacks.borrow().is_empty(),
                );
            }
        }
        self.needs_present.set(false);
        let outcome = match submission.status() {
            PlatformFrameSubmissionStatus::Pending => {
                return PlatformWindowPresentOutcome::Deferred;
            }
            PlatformFrameSubmissionStatus::Submitted => PlatformWindowPresentOutcome::Submitted,
            PlatformFrameSubmissionStatus::RepaintRequired => {
                PlatformWindowPresentOutcome::RepaintRequired
            }
            PlatformFrameSubmissionStatus::Rejected => PlatformWindowPresentOutcome::Rejected,
            PlatformFrameSubmissionStatus::Cancelled => {
                self.cancel_deferred_candidate(cx);
                return PlatformWindowPresentOutcome::RepaintRequired;
            }
        };
        if self.complete_deferred_frame_submission(&submission, outcome, cx) {
            outcome
        } else {
            PlatformWindowPresentOutcome::RepaintRequired
        }
    }

    pub(crate) fn complete_deferred_frame_submission(
        &mut self,
        submission: &PlatformFrameSubmission,
        outcome: PlatformWindowPresentOutcome,
        cx: &mut App,
    ) -> bool {
        if self.deferred_scene.is_some() {
            return self.complete_deferred_scene_submission(submission, outcome, cx);
        }
        let status = match outcome {
            PlatformWindowPresentOutcome::Submitted => PlatformFrameSubmissionStatus::Submitted,
            PlatformWindowPresentOutcome::RepaintRequired => {
                PlatformFrameSubmissionStatus::RepaintRequired
            }
            PlatformWindowPresentOutcome::Rejected => PlatformFrameSubmissionStatus::Rejected,
            PlatformWindowPresentOutcome::Deferred => return false,
        };
        let Some(candidate) = self.deferred_candidate.as_ref() else {
            return false;
        };
        if !candidate.delivered
            || !candidate.submission.same_authority(submission)
            || submission.status() != status
        {
            return false;
        }
        if outcome != PlatformWindowPresentOutcome::Submitted {
            #[cfg(feature = "profiler")]
            if let Some(at) = submission.settled_at() {
                self.window_profiler.record_present_outcome(
                    outcome,
                    at,
                    at,
                    self.active.get(),
                    !self.next_frame_callbacks.borrow().is_empty(),
                );
            }
            let candidate = self.deferred_candidate.take().unwrap();
            let generation = candidate.submission.frame_generation();
            let contained_valid_primitives = self.next_frame.scene.has_primitives();
            self.discard_deferred_candidate(candidate, cx);
            self.presentation_state.latest_present_attempt = Some(WindowPresentAttemptFacts {
                generation,
                outcome,
                contained_valid_primitives,
            });
            self.record_exact_frame_submission_present_outcome(generation, outcome, cx);
            if self.fresh_initial_presentation_is_pending() {
                if outcome == PlatformWindowPresentOutcome::Rejected
                    || self.fresh_initial_presentation_attempts_exhausted()
                {
                    self.fail_fresh_initial_presentation(cx);
                } else {
                    self.request_fresh_initial_presentation_frame();
                }
            }
            return true;
        }
        self.a11y.sync_active_flag();
        if !self.deferred_candidate.as_ref().unwrap().is_current(self) {
            let candidate = self.deferred_candidate.take().unwrap();
            self.discard_deferred_candidate(candidate, cx);
            return false;
        }
        let mut candidate = self.deferred_candidate.take().unwrap();
        #[cfg(feature = "profiler")]
        if let Some((timing, update_count, _)) = candidate.draw_timing.take() {
            let duration = self
                .window_profiler
                .accept_prepared_draw(timing, update_count);
            self.debug_frame_overlay.record_frame(duration);
        }
        let _access_frame = cx.entities.frame_access_scope();
        cx.entities.extend_accessed(&candidate.accessed_entities);
        let dirty = self.invalidator.is_dirty()
            || self.invalidator.update_count() != candidate.update_count;
        if let Some(index) = candidate.prepared.restored_input_handler_index
            && let Some(handler) = self.platform_window.take_input_handler_for_frame()
        {
            let slot = &mut self.rendered_frame.input_handlers[index];
            assert!(slot.value.is_none());
            slot.value = Some(handler);
        }
        self.resume_candidate_frame_transfers(candidate.transfers);
        let mut authority = mem::replace(
            &mut candidate.prepared.authority_checkpoint,
            self.candidate_frame_authority_checkpoint(),
        );
        authority.focus_claim_resolutions_len = self.focus_claim_resolutions.len();
        if dirty {
            authority.requested_autoscroll = self.requested_autoscroll.clone();
            authority.tooltip_bounds = self.tooltip_bounds.clone();
        }
        authority.focus_followup_requested |= self.focus_followup_requested;
        self.restore_candidate_frame_authority(authority);
        self.append_focus_claim_resolutions(candidate.focus_resolutions);
        self.candidate_pending_input_clear = candidate.pending_input_clear;
        self.candidate_pending_input_notification = candidate.pending_input_notification;
        self.candidate_text_notifications
            .set(candidate.text_notifications);
        self.candidate_virtual_keyboard_request.set(
            (self.virtual_keyboard_request_revision.get() == candidate.keyboard_revision)
                .then_some(candidate.virtual_keyboard_request)
                .flatten(),
        );
        self.candidate_accessibility_focus = candidate.accessibility_focus;
        candidate.prepared.preserve_dirty_views = dirty;
        let generation = submission.frame_generation();
        let mut completion_panic = None;
        super::retain_first_window_cleanup_panic(
            &mut completion_panic,
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.commit_candidate_frame(candidate.prepared, cx);
            })),
            "submitted deferred frame publications",
        );
        super::retain_first_window_cleanup_panic(
            &mut completion_panic,
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if self.rendered_frame.generation == generation {
                    #[cfg(feature = "profiler")]
                    if let Some(at) = submission.settled_at() {
                        self.window_profiler.record_present_outcome(
                            outcome,
                            at,
                            at,
                            self.active.get(),
                            !self.next_frame_callbacks.borrow().is_empty(),
                        );
                    }
                    self.record_deferred_present_outcome(outcome, cx);
                    self.advance_initial_presentation(cx);
                }
                if dirty
                    || self.invalidator.is_dirty()
                    || self.invalidator.update_count() != candidate.update_count
                {
                    self.refresh();
                }
                self.schedule_pending_frame();
            })),
            "submitted deferred frame presentation completion",
        );
        drop(candidate.observers);
        if let Some(panic) = completion_panic {
            std::panic::resume_unwind(panic);
        }
        true
    }

    pub(crate) fn cancel_deferred_candidate(&mut self, cx: &mut App) -> bool {
        let retained = self.cancel_deferred_scene();
        let Some(candidate) = self.deferred_candidate.take() else {
            return retained;
        };
        self.discard_deferred_candidate(candidate, cx);
        true
    }

    fn discard_deferred_candidate(&mut self, mut candidate: DeferredCandidateFrame, cx: &mut App) {
        candidate.submission.cancel();
        #[cfg(feature = "profiler")]
        if let Some((_, _, dirty)) = candidate.draw_timing.take() {
            self.invalidator.restore_frame_dirty(dirty);
        }
        let transaction = &candidate.prepared.transaction;
        candidate.focus_resolutions.retain_mut(|resolution| {
            if !transaction.owns_focus_completion(resolution.id) {
                return false;
            }
            if resolution.outcome == FocusClaimOutcome::Committed {
                resolution.outcome = FocusClaimOutcome::Rejected;
            }
            true
        });
        if let Some(completion) = candidate
            .prepared
            .authority_checkpoint
            .pending_focus_completion
            .take()
            && transaction.owns_focus_completion(completion.id)
        {
            candidate.focus_resolutions.push(FocusClaimResolution {
                id: completion.id,
                outcome: FocusClaimOutcome::Rejected,
                callback: completion.callback,
            });
        }
        self.invalidator.set_phase(DrawPhase::None);
        if let Some(layout_engine) = self.layout_engine.as_mut() {
            layout_engine.clear();
        }
        self.text_system().abort_frame();
        self.a11y.discard_candidate_frame();
        self.next_frame.clear();
        self.rendered_frame
            .input_handlers
            .truncate(candidate.prepared.rendered_input_handlers_len);
        self.append_focus_claim_resolutions(candidate.focus_resolutions);
        self.schedule_focus_claim_resolution_dispatch(cx);
        self.mouse_hit_test = self.rendered_frame.hit_test(self.mouse_position);
        self.needs_present.set(false);
        self.refresh();
        drop(candidate.observers);
        self.schedule_pending_frame();
    }
}
