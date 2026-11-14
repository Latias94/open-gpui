//! Exact asynchronous re-presentation of an already accepted animated Scene.

use super::*;
use crate::{PlatformFrameSubmission, PlatformFrameSubmissionStatus, Scene};

pub(super) struct DeferredScenePresentation {
    submission: PlatformFrameSubmission,
    scene: Scene,
    lease: Rc<AtlasTextureLease>,
    physical_geometry: Option<crate::PlatformWindowPhysicalGeometry>,
    viewport_size: Size<Pixels>,
    scale_factor: f32,
    font_generation: usize,
    validities: Vec<SubtreeGeometryValidity>,
}

impl DeferredScenePresentation {
    fn is_current(&self, window: &Window) -> bool {
        let platform = window.platform_window.platform_facts();
        window.presentation_is_allowed()
            && window.removal_state == WindowRemovalState::Open
            && !window.removed
            && !window.native_closed.load(SeqCst)
            && self.submission.status() != PlatformFrameSubmissionStatus::Cancelled
            && self.submission.frame_generation() == window.rendered_frame.generation
            && self.physical_geometry == window.platform_facts.physical_geometry
            && self.physical_geometry == platform.physical_geometry
            && self.viewport_size == window.viewport_size
            && self.viewport_size == platform.content_size
            && self.scale_factor == window.scale_factor
            && self.scale_factor == platform.scale_factor
            && self.font_generation == window.text_system().font_generation()
            && self.lease.validate().is_ok()
            && self
                .validities
                .iter()
                .all(SubtreeGeometryValidity::is_valid)
    }
}

impl Window {
    pub(super) fn cancel_deferred_scene(&mut self) -> bool {
        let Some(presentation) = self.deferred_scene.take() else {
            return false;
        };
        presentation.submission.cancel();
        true
    }

    pub(super) fn cancel_deferred_scene_if_stale(&mut self) {
        if self
            .deferred_scene
            .as_ref()
            .is_some_and(|frame| !frame.is_current(self))
        {
            self.cancel_deferred_scene();
            self.refresh();
        }
    }

    pub(super) fn cancel_deferred_scene_submission(&self) -> bool {
        self.deferred_scene
            .as_ref()
            .is_some_and(|frame| frame.submission.cancel())
    }

    pub(super) fn present_animated_deferred_scene(
        &mut self,
        cx: &mut App,
    ) -> PlatformWindowPresentOutcome {
        self.cancel_deferred_scene_if_stale();
        if let Some(pending) = self.deferred_scene.as_ref() {
            let submission = pending.submission.clone();
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
                    return PlatformWindowPresentOutcome::RepaintRequired;
                }
            };
            self.complete_deferred_scene_submission(&submission, outcome, cx);
            return outcome;
        }
        if !self.presentation_is_allowed() || self.rendered_frame.generation == 0 {
            return PlatformWindowPresentOutcome::Rejected;
        }
        let mut scene = self.rendered_frame.scene.clone();
        scene.finish();
        let elapsed = cx.background_executor().now() - cx.synced_animation_epoch;
        scene.advance_animations(elapsed);
        let lease = match scene.retain_atlas_textures(self.sprite_atlas.clone()) {
            Ok(lease) => Rc::new(lease),
            Err(_) => {
                return self.record_deferred_present_outcome(
                    PlatformWindowPresentOutcome::RepaintRequired,
                    cx,
                );
            }
        };
        let validities = scene.valid_geometry_scopes().collect::<Vec<_>>();
        let submission_validities = validities.clone();
        let text_system = self.text_system.clone();
        let font_generation = text_system.font_generation();
        let validity_lease = lease.clone();
        let app = cx.to_async();
        self.next_candidate_frame_attempt_id = self
            .next_candidate_frame_attempt_id
            .checked_add(1)
            .expect("frame submission attempt space exhausted");
        let submission = PlatformFrameSubmission::new(
            self.handle.window_id(),
            self.rendered_frame.generation,
            self.next_candidate_frame_attempt_id,
            Rc::new(move |submission, outcome| {
                app.enqueue_frame_submission_completed(submission, outcome)
            }),
        )
        .with_validity(Rc::new(move || {
            text_system.font_generation() == font_generation
                && validity_lease.validate().is_ok()
                && submission_validities
                    .iter()
                    .all(SubtreeGeometryValidity::is_valid)
        }));
        self.deferred_scene = Some(DeferredScenePresentation {
            submission: submission.clone(),
            scene,
            lease,
            physical_geometry: self.platform_facts.physical_geometry,
            viewport_size: self.viewport_size,
            scale_factor: self.scale_factor,
            font_generation,
            validities,
        });
        let outcome = self
            .platform_window
            .draw_with_submission(&self.deferred_scene.as_ref().unwrap().scene, &submission);
        submission.finish_without_callback(outcome);
        self.needs_present.set(false);
        if outcome != PlatformWindowPresentOutcome::Deferred {
            self.complete_deferred_scene_submission(&submission, outcome, cx);
        }
        outcome
    }

    pub(super) fn complete_deferred_scene_submission(
        &mut self,
        submission: &PlatformFrameSubmission,
        outcome: PlatformWindowPresentOutcome,
        cx: &mut App,
    ) -> bool {
        let expected = match outcome {
            PlatformWindowPresentOutcome::Submitted => PlatformFrameSubmissionStatus::Submitted,
            PlatformWindowPresentOutcome::RepaintRequired => {
                PlatformFrameSubmissionStatus::RepaintRequired
            }
            PlatformWindowPresentOutcome::Rejected => PlatformFrameSubmissionStatus::Rejected,
            PlatformWindowPresentOutcome::Deferred => return false,
        };
        let Some(pending) = self.deferred_scene.as_ref() else {
            return false;
        };
        if !pending.submission.same_authority(submission) || submission.status() != expected {
            return false;
        }
        if !pending.is_current(self) {
            self.cancel_deferred_scene();
            self.refresh();
            self.schedule_pending_frame();
            return false;
        }
        let pending = self.deferred_scene.take().unwrap();
        if outcome == PlatformWindowPresentOutcome::Submitted {
            // This updates only the exact accepted scene's raster sample, not semantic output.
            self.rendered_frame.scene = pending.scene;
            self.scene_animation_interval = self.rendered_frame.scene.animation_frame_interval();
        }
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
        if outcome == PlatformWindowPresentOutcome::Rejected {
            self.refresh();
        }
        self.advance_initial_presentation(cx);
        self.schedule_pending_frame();
        true
    }
}
