//! Exact submission authority for one platform delivery attempt.
//!
//! The original Window owns the pending candidate or retained raster snapshot and its
//! previously submitted publications.
//! This module carries only that candidate's immutable identity, shared settlement state and
//! original completion callback. It does not retain a Scene, select a newer candidate or schedule
//! work. Clones share one settlement on the UI thread and cannot be sent between threads.

use super::PlatformWindowPresentOutcome;
use crate::WindowId;
#[cfg(feature = "profiler")]
use open_gpui_scheduler::Instant;
use std::{
    cell::{Cell, RefCell},
    fmt,
    rc::Rc,
};

type FrameValidity = Rc<dyn Fn() -> bool>;

/// When a platform settles an exact frame submission attempt.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlatformFrameDeliveryMode {
    /// The renderer returns its outcome during the original draw call.
    #[default]
    Synchronous,
    /// The platform retains the original token and reports a terminal outcome later.
    ///
    /// Returning [`PlatformWindowPresentOutcome::Deferred`] leaves the token pending. The
    /// original Window retains the candidate or retained scene until the exact attempt settles.
    Deferred,
}

/// A snapshot of an exact platform submission token's local settlement state.
///
/// Terminal outcomes are visible before the original completion callback runs. They describe
/// platform delivery, not GPU completion or publication by the Window. If the callback refuses
/// admission, the status becomes [`Self::Cancelled`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformFrameSubmissionStatus {
    /// The attempt may still receive one terminal outcome or be cancelled by its owner.
    Pending,
    /// The renderer reported actual submission to its presentation path.
    Submitted,
    /// The candidate's renderer resources are invalid and a new frame is required.
    RepaintRequired,
    /// The renderer or native surface rejected this attempt.
    Rejected,
    /// The owner cancelled the attempt or the original completion path refused admission.
    Cancelled,
}

/// Cloneable UI-thread authority for one exact Window, frame generation and delivery attempt.
///
/// The original Window creates the token before handing its candidate or retained raster
/// snapshot to the platform. A
/// synchronous return settles that same token without invoking its completion callback. A
/// deferred platform retains the token for that exact candidate; retries for the same frame
/// generation have distinct attempt identities and settlement state.
///
/// To report [`PlatformWindowPresentOutcome::Submitted`], the caller must first actually submit
/// the candidate to the renderer's presentation path, then call [`Self::complete`] with the
/// original token. Recording or encoding commands alone is insufficient. Completion reports
/// submission, not GPU completion, and does not release GPU resources or publish Window state.
/// The original completion callback returns the exact attempt through its existing ingress path
/// for the Window to validate and admit.
///
/// Every clone shares the same settlement state, and only a pending token can settle. Settlement
/// precedes notification, so reentrant completion and later callbacks cannot settle the token
/// again. Owner cancellation and inline settlement never invoke the callback. Dropping a clone
/// does not cancel the other clones; invalidation and shutdown cancellation remain the original
/// owner's responsibility.
#[derive(Clone)]
pub struct PlatformFrameSubmission {
    window_id: WindowId,
    frame_generation: u64,
    attempt_id: u64,
    state: Rc<Cell<PlatformFrameSubmissionStatus>>,
    completion: Rc<dyn Fn(PlatformFrameSubmission, PlatformWindowPresentOutcome) -> bool>,
    validity: Rc<RefCell<Option<FrameValidity>>>,
    #[cfg(feature = "profiler")]
    settled_at: Rc<Cell<Option<Instant>>>,
}

impl PlatformFrameSubmission {
    /// Creates authority for an attempt allocated by the original Window.
    ///
    /// # Panics
    ///
    /// Panics if the frame generation or attempt ID is zero. Both are nonzero internal
    /// identities allocated before the platform receives the candidate.
    pub(crate) fn new(
        window_id: WindowId,
        frame_generation: u64,
        attempt_id: u64,
        completion: Rc<dyn Fn(PlatformFrameSubmission, PlatformWindowPresentOutcome) -> bool>,
    ) -> Self {
        assert_ne!(frame_generation, 0, "frame generation must be nonzero");
        assert_ne!(attempt_id, 0, "frame submission attempt ID must be nonzero");
        Self {
            window_id,
            frame_generation,
            attempt_id,
            state: Rc::new(Cell::new(PlatformFrameSubmissionStatus::Pending)),
            completion,
            validity: Rc::new(RefCell::new(None)),
            #[cfg(feature = "profiler")]
            settled_at: Rc::new(Cell::new(None)),
        }
    }

    pub(crate) fn with_validity(self, validity: FrameValidity) -> Self {
        self.validity.replace(Some(validity));
        self
    }

    #[cfg(feature = "profiler")]
    pub(crate) fn settled_at(&self) -> Option<Instant> {
        self.settled_at.get()
    }

    /// Returns the original Window's full identity, including its slot generation.
    pub fn window_id(&self) -> WindowId {
        self.window_id
    }

    /// Returns the exact candidate frame generation carried by this attempt.
    pub fn frame_generation(&self) -> u64 {
        self.frame_generation
    }

    /// Returns the exact delivery attempt ID, including for retries of the same generation.
    pub fn attempt_id(&self) -> u64 {
        self.attempt_id
    }

    /// Returns whether the original attempt remains pending and its resources are current.
    ///
    /// Call this immediately before recording or submitting retained work. Invalid framework
    /// resources settle this original attempt as `RepaintRequired` through its completion path.
    /// Native adapters must additionally invalidate the token before target or binding changes.
    pub fn is_pending(&self) -> bool {
        if self.status() != PlatformFrameSubmissionStatus::Pending {
            return false;
        }
        let validity = self.validity.borrow().clone();
        if validity.as_ref().is_some_and(|valid| !valid()) {
            self.complete(PlatformWindowPresentOutcome::RepaintRequired);
            return false;
        }
        true
    }

    /// Returns the current settlement state shared by every clone.
    pub fn status(&self) -> PlatformFrameSubmissionStatus {
        self.state.get()
    }

    /// Settles this exact deferred attempt and invokes its original completion callback once.
    ///
    /// For [`PlatformWindowPresentOutcome::Submitted`], actual renderer submission must precede
    /// this call. The token first records the terminal outcome, then invokes the original callback
    /// with a clone of this same authority and that outcome. The callback's return value reports
    /// admission to the original completion path. Refused admission terminally cancels every
    /// clone and returns `false`; callers cannot retry completion with another outcome.
    ///
    /// Returns `false` without side effects for [`PlatformWindowPresentOutcome::Deferred`] or
    /// an already settled token. Reentrant calls therefore return `false` without notifying again.
    /// If the callback unwinds, the token remains terminal and cannot notify again.
    pub fn complete(&self, outcome: PlatformWindowPresentOutcome) -> bool {
        if !self.finish_without_callback(outcome) {
            return false;
        }
        if (self.completion)(self.clone(), outcome) {
            true
        } else {
            self.state.set(PlatformFrameSubmissionStatus::Cancelled);
            false
        }
    }

    /// Cancels a pending attempt across all clones without invoking its callback.
    ///
    /// Returns `true` only for the transition from pending to cancelled. Cancellation cannot
    /// rewrite a terminal outcome, including while its original callback is running.
    pub(crate) fn cancel(&self) -> bool {
        if self.status() != PlatformFrameSubmissionStatus::Pending {
            return false;
        }
        self.state.set(PlatformFrameSubmissionStatus::Cancelled);
        #[cfg(feature = "profiler")]
        self.settled_at.set(Some(Instant::now()));
        let validity = self.validity.borrow_mut().take();
        drop(validity);
        true
    }

    /// Returns whether both tokens carry the same shared authority and all exact identities.
    pub(crate) fn same_authority(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
            && self.window_id == other.window_id
            && self.frame_generation == other.frame_generation
            && self.attempt_id == other.attempt_id
    }

    /// Settles a synchronous renderer return on its original token without notification.
    ///
    /// The Window consumes this outcome inline through its existing candidate lifecycle, so this
    /// method does not enqueue completion work. A submitted outcome requires actual renderer
    /// submission before this call. Returns `false` without side effects for a deferred outcome
    /// or an already settled token.
    pub(crate) fn finish_without_callback(&self, outcome: PlatformWindowPresentOutcome) -> bool {
        if self.status() != PlatformFrameSubmissionStatus::Pending {
            return false;
        }
        let status = match outcome {
            PlatformWindowPresentOutcome::Submitted => PlatformFrameSubmissionStatus::Submitted,
            PlatformWindowPresentOutcome::Deferred => return false,
            PlatformWindowPresentOutcome::RepaintRequired => {
                PlatformFrameSubmissionStatus::RepaintRequired
            }
            PlatformWindowPresentOutcome::Rejected => PlatformFrameSubmissionStatus::Rejected,
        };
        self.state.set(status);
        #[cfg(feature = "profiler")]
        self.settled_at.set(Some(Instant::now()));
        let validity = self.validity.borrow_mut().take();
        drop(validity);
        true
    }
}

impl fmt::Debug for PlatformFrameSubmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlatformFrameSubmission")
            .field("window_id", &self.window_id)
            .field("frame_generation", &self.frame_generation)
            .field("attempt_id", &self.attempt_id)
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::SlotMap;
    use std::cell::RefCell;

    fn window_id() -> WindowId {
        SlotMap::<WindowId, ()>::with_key().insert(())
    }

    #[test]
    fn stale_original_resources_refuse_submission_and_notify_repaint_once() {
        let valid = Rc::new(Cell::new(true));
        let completions = Rc::new(Cell::new(0));
        let submission = PlatformFrameSubmission::new(window_id(), 7, 11, {
            let completions = completions.clone();
            Rc::new(move |_, outcome| {
                assert_eq!(outcome, PlatformWindowPresentOutcome::RepaintRequired);
                completions.set(completions.get() + 1);
                true
            })
        })
        .with_validity({
            let valid = valid.clone();
            Rc::new(move || valid.get())
        });
        let clone = submission.clone();
        assert!(submission.is_pending());
        valid.set(false);
        assert!(!clone.is_pending());
        assert_eq!(
            submission.status(),
            PlatformFrameSubmissionStatus::RepaintRequired
        );
        assert!(!submission.complete(PlatformWindowPresentOutcome::Submitted));
        assert!(!submission.is_pending());
        assert_eq!(completions.get(), 1);
    }

    #[test]
    fn terminal_clones_release_their_shared_resource_validation_owner() {
        for outcome in [None, Some(PlatformWindowPresentOutcome::Submitted)] {
            let owner = Rc::new(());
            let weak = Rc::downgrade(&owner);
            let submission = PlatformFrameSubmission::new(window_id(), 7, 11, Rc::new(|_, _| true))
                .with_validity(Rc::new(move || Rc::strong_count(&owner) > 0));
            let clone = submission.clone();
            assert!(weak.upgrade().is_some());
            match outcome {
                Some(outcome) => assert!(submission.complete(outcome)),
                None => assert!(submission.cancel()),
            }
            assert!(weak.upgrade().is_none());
            assert!(!clone.is_pending());
        }
    }

    const TERMINAL_OUTCOMES: [(PlatformWindowPresentOutcome, PlatformFrameSubmissionStatus); 3] = [
        (
            PlatformWindowPresentOutcome::Submitted,
            PlatformFrameSubmissionStatus::Submitted,
        ),
        (
            PlatformWindowPresentOutcome::RepaintRequired,
            PlatformFrameSubmissionStatus::RepaintRequired,
        ),
        (
            PlatformWindowPresentOutcome::Rejected,
            PlatformFrameSubmissionStatus::Rejected,
        ),
    ];

    #[test]
    fn authority_preserves_full_window_frame_and_attempt_identity() {
        let mut windows = SlotMap::<WindowId, ()>::with_key();
        let original_window = windows.insert(());
        windows.remove(original_window);
        let replacement_window = windows.insert(());
        assert_ne!(original_window, replacement_window);

        let completion: Rc<dyn Fn(PlatformFrameSubmission, PlatformWindowPresentOutcome) -> bool> =
            Rc::new(|_, _| true);
        let original = PlatformFrameSubmission::new(original_window, 7, 11, completion.clone());
        let clone = original.clone();
        assert_eq!(clone.window_id(), original_window);
        assert_eq!(clone.frame_generation(), 7);
        assert_eq!(clone.attempt_id(), 11);
        assert!(clone.is_pending());
        assert!(original.same_authority(&clone));

        for (window_id, frame_generation, attempt_id) in [
            (original_window, 7, 11),
            (replacement_window, 7, 11),
            (original_window, 8, 11),
            (original_window, 7, 12),
        ] {
            let independent = PlatformFrameSubmission::new(
                window_id,
                frame_generation,
                attempt_id,
                completion.clone(),
            );
            assert!(!original.same_authority(&independent));
            assert!(!independent.same_authority(&original));
        }
    }

    #[test]
    fn completion_notifies_original_authority_once_after_terminal_settlement() {
        let window_id = window_id();
        for (outcome, status) in TERMINAL_OUTCOMES {
            let calls = Rc::new(Cell::new(0));
            let notified_state = Rc::new(RefCell::new(None));
            let submission = PlatformFrameSubmission::new(window_id, 7, 11, {
                let calls = calls.clone();
                let notified_state = notified_state.clone();
                Rc::new(move |token, actual_outcome| {
                    calls.set(calls.get() + 1);
                    assert_eq!(actual_outcome, outcome);
                    assert_eq!(token.window_id(), window_id);
                    assert_eq!(token.frame_generation(), 7);
                    assert_eq!(token.attempt_id(), 11);
                    assert_eq!(token.status(), status);
                    assert!(!token.is_pending());
                    assert!(!token.complete(actual_outcome));
                    assert!(!token.cancel());
                    assert!(!token.finish_without_callback(actual_outcome));
                    *notified_state.borrow_mut() = Some(token.state.clone());
                    true
                })
            });
            let clone = submission.clone();

            assert!(clone.complete(outcome));
            let state = notified_state.borrow_mut().take().unwrap();
            assert!(Rc::ptr_eq(&state, &submission.state));
            assert_eq!(submission.status(), status);
            assert!(submission.same_authority(&clone));
            for (other_outcome, _) in TERMINAL_OUTCOMES {
                assert!(!submission.complete(other_outcome));
                assert!(!clone.finish_without_callback(other_outcome));
            }
            assert!(!submission.cancel());
            assert_eq!(calls.get(), 1);
            assert_eq!(clone.status(), status);
        }
    }

    #[test]
    fn refused_completion_admission_terminally_cancels_every_clone() {
        let window_id = window_id();
        for (outcome, status) in TERMINAL_OUTCOMES {
            let calls = Rc::new(Cell::new(0));
            let submission = PlatformFrameSubmission::new(window_id, 7, 11, {
                let calls = calls.clone();
                Rc::new(move |token, actual_outcome| {
                    calls.set(calls.get() + 1);
                    assert_eq!(actual_outcome, outcome);
                    assert_eq!(token.status(), status);
                    false
                })
            });
            let clone = submission.clone();

            assert!(!submission.complete(outcome));
            assert_eq!(
                submission.status(),
                PlatformFrameSubmissionStatus::Cancelled
            );
            assert_eq!(clone.status(), PlatformFrameSubmissionStatus::Cancelled);
            assert!(!clone.is_pending());
            assert!(!clone.cancel());
            for (other_outcome, _) in TERMINAL_OUTCOMES {
                assert!(!clone.complete(other_outcome));
                assert!(!submission.finish_without_callback(other_outcome));
            }
            assert_eq!(calls.get(), 1);
        }
    }

    #[test]
    fn same_generation_retry_cannot_admit_another_attempt() {
        let window_id = window_id();
        let current = PlatformFrameSubmission::new(window_id, 7, 12, Rc::new(|_, _| true));
        let calls = Rc::new(Cell::new(0));
        let previous = PlatformFrameSubmission::new(window_id, 7, 11, {
            let expected = current.clone();
            let calls = calls.clone();
            Rc::new(move |token, _| {
                calls.set(calls.get() + 1);
                assert_eq!(token.window_id(), expected.window_id());
                assert_eq!(token.frame_generation(), expected.frame_generation());
                assert_ne!(token.attempt_id(), expected.attempt_id());
                token.same_authority(&expected)
            })
        });

        assert!(!previous.complete(PlatformWindowPresentOutcome::Submitted));
        assert_eq!(previous.status(), PlatformFrameSubmissionStatus::Cancelled);
        assert!(current.is_pending());
        assert!(current.complete(PlatformWindowPresentOutcome::Submitted));
        assert!(!previous.complete(PlatformWindowPresentOutcome::Submitted));
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn cancellation_is_shared_silent_and_limited_to_its_attempt() {
        let window_id = window_id();
        let calls = Rc::new(Cell::new(0));
        let completion: Rc<dyn Fn(PlatformFrameSubmission, PlatformWindowPresentOutcome) -> bool> =
            Rc::new({
                let calls = calls.clone();
                move |_, _| {
                    calls.set(calls.get() + 1);
                    true
                }
            });
        let previous = PlatformFrameSubmission::new(window_id, 7, 11, completion.clone());
        let current = PlatformFrameSubmission::new(window_id, 7, 12, completion);
        let cancelled = current.clone();

        assert!(cancelled.cancel());
        assert_eq!(current.status(), PlatformFrameSubmissionStatus::Cancelled);
        assert!(!current.cancel());
        for (outcome, _) in TERMINAL_OUTCOMES {
            assert!(!current.complete(outcome));
            assert!(!cancelled.finish_without_callback(outcome));
        }
        assert_eq!(calls.get(), 0);
        assert!(previous.is_pending());
        assert!(previous.complete(PlatformWindowPresentOutcome::Submitted));
        assert_eq!(calls.get(), 1);
        assert_eq!(current.status(), PlatformFrameSubmissionStatus::Cancelled);
    }

    #[test]
    fn deferred_outcomes_leave_state_and_completion_untouched() {
        let calls = Rc::new(Cell::new(0));
        let submission = PlatformFrameSubmission::new(window_id(), 7, 11, {
            let calls = calls.clone();
            Rc::new(move |_, _| {
                calls.set(calls.get() + 1);
                true
            })
        });
        let clone = submission.clone();

        assert!(!submission.complete(PlatformWindowPresentOutcome::Deferred));
        assert!(!clone.finish_without_callback(PlatformWindowPresentOutcome::Deferred));
        assert!(submission.is_pending());
        assert_eq!(clone.status(), PlatformFrameSubmissionStatus::Pending);
        assert_eq!(calls.get(), 0);
        assert!(clone.complete(PlatformWindowPresentOutcome::Submitted));
        assert_eq!(calls.get(), 1);
        assert!(!submission.complete(PlatformWindowPresentOutcome::Deferred));
        assert!(!clone.finish_without_callback(PlatformWindowPresentOutcome::Deferred));
        assert_eq!(
            submission.status(),
            PlatformFrameSubmissionStatus::Submitted
        );
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn synchronous_settlement_never_invokes_completion() {
        let window_id = window_id();
        for (outcome, status) in TERMINAL_OUTCOMES {
            let calls = Rc::new(Cell::new(0));
            let submission = PlatformFrameSubmission::new(window_id, 7, 11, {
                let calls = calls.clone();
                Rc::new(move |_, _| {
                    calls.set(calls.get() + 1);
                    true
                })
            });
            let clone = submission.clone();

            assert!(submission.finish_without_callback(outcome));
            assert_eq!(clone.status(), status);
            assert!(!clone.is_pending());
            for (other_outcome, _) in TERMINAL_OUTCOMES {
                assert!(!clone.complete(other_outcome));
                assert!(!submission.finish_without_callback(other_outcome));
            }
            assert!(!clone.cancel());
            assert_eq!(submission.status(), status);
            assert_eq!(calls.get(), 0);
        }
    }

    #[test]
    #[should_panic(expected = "frame generation must be nonzero")]
    fn zero_frame_generation_violates_the_internal_identity_invariant() {
        PlatformFrameSubmission::new(window_id(), 0, 11, Rc::new(|_, _| true));
    }

    #[test]
    #[should_panic(expected = "frame submission attempt ID must be nonzero")]
    fn zero_attempt_id_violates_the_internal_identity_invariant() {
        PlatformFrameSubmission::new(window_id(), 7, 0, Rc::new(|_, _| true));
    }
}
