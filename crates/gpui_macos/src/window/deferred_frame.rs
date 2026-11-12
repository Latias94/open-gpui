//! Retains the original candidate until a native callback owns a drawable.

use super::*;
use open_gpui::{PlatformFrameSubmissionStatus, Scene};

pub(super) struct MacDeferredFrame {
    scene: Scene,
    submission: PlatformFrameSubmission,
    geometry: MacRendererGeometry,
}

impl MacDeferredFrame {
    fn matches(&self, submission: &PlatformFrameSubmission) -> bool {
        self.submission.window_id() == submission.window_id()
            && self.submission.frame_generation() == submission.frame_generation()
            && self.submission.attempt_id() == submission.attempt_id()
    }
}

impl MacWindowState {
    pub(super) fn draw_scene(&mut self, scene: &Scene) -> PlatformWindowPresentOutcome {
        if self.is_closed() || self.presentation_shutdown_authority.lock().ticket.is_some() {
            return PlatformWindowPresentOutcome::Rejected;
        }
        let visible = self
            .native_window()
            .occlusionState()
            .contains(NSWindowOcclusionState::Visible);
        // Avoid blocking nextDrawable while occluded, except for the initial attempt.
        if should_defer_occluded_draw(self.attempted_window_draw, visible) {
            return PlatformWindowPresentOutcome::Deferred;
        }
        self.attempted_window_draw = true;
        self.renderer.draw(scene)
    }
}

pub(super) fn draw(
    window_state: &Arc<Mutex<MacWindowState>>,
    scene: &Scene,
    submission: &PlatformFrameSubmission,
) -> PlatformWindowPresentOutcome {
    // Validation can complete an invalid token through ingress, so it must run without the lock.
    if !submission.is_pending() {
        return PlatformWindowPresentOutcome::RepaintRequired;
    }
    let frames = {
        let mut state = window_state.lock();
        if state.is_closed()
            || state.handle.window_id() != submission.window_id()
            || state
                .presentation_shutdown_authority
                .lock()
                .ticket
                .is_some()
        {
            return PlatformWindowPresentOutcome::Rejected;
        }
        if let Some(pending) = state.deferred_frame.as_ref()
            && pending.submission.status() == PlatformFrameSubmissionStatus::Pending
        {
            return if pending.matches(submission) {
                PlatformWindowPresentOutcome::Deferred
            } else {
                PlatformWindowPresentOutcome::Rejected
            };
        }
        state.deferred_frame = None;
        let frames = match &state.frame_source {
            FrameSource::Metal(frames) => Some(frames.clone()),
            FrameSource::DisplayLink(_) => None,
        };
        if !frames.as_ref().is_some_and(|frames| frames.is_attached()) {
            let outcome = state.draw_scene(scene);
            if outcome != PlatformWindowPresentOutcome::Deferred {
                return outcome;
            }
        }
        state.deferred_frame = Some(MacDeferredFrame {
            scene: scene.clone(),
            submission: submission.clone(),
            geometry: state.renderer_geometry,
        });
        frames
    };
    if let Some(frames) = frames {
        // A prior unused native update may have parked the link before this queued draw arrived.
        frames.wake();
    }
    PlatformWindowPresentOutcome::Deferred
}

pub(super) fn present(window_state: &Arc<Mutex<MacWindowState>>) {
    let Some(pending) = window_state.lock().deferred_frame.take() else {
        return;
    };
    if !pending.submission.is_pending() {
        return;
    }
    let outcome = {
        let mut state = window_state.lock();
        let outcome = if state.is_closed()
            || state
                .presentation_shutdown_authority
                .lock()
                .ticket
                .is_some()
        {
            PlatformWindowPresentOutcome::Rejected
        } else if pending.geometry != state.renderer_geometry {
            PlatformWindowPresentOutcome::RepaintRequired
        } else {
            state.draw_scene(&pending.scene)
        };
        if outcome == PlatformWindowPresentOutcome::Deferred {
            state.deferred_frame = Some(pending);
            return;
        }
        outcome
    };
    pending.submission.complete(outcome);
}

pub(super) fn take_stale(state: &mut MacWindowState) -> Option<MacDeferredFrame> {
    if state
        .deferred_frame
        .as_ref()
        .is_some_and(|pending| pending.geometry != state.renderer_geometry)
    {
        state.deferred_frame.take()
    } else {
        None
    }
}

pub(super) fn complete(pending: Option<MacDeferredFrame>, outcome: PlatformWindowPresentOutcome) {
    if let Some(pending) = pending {
        pending.submission.complete(outcome);
    }
}
