use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PresentationState {
    Unpresented,
    Presented,
    RetryBeforeFirstPresent,
    RetryAfterPresent,
}

impl PresentationState {
    pub(super) fn requires_presentation(self) -> bool {
        matches!(
            self,
            Self::RetryBeforeFirstPresent | Self::RetryAfterPresent
        )
    }

    pub(super) fn has_presented(self) -> bool {
        matches!(self, Self::Presented | Self::RetryAfterPresent)
    }

    pub(super) fn failed(self) -> Self {
        match self {
            Self::Unpresented | Self::RetryBeforeFirstPresent => Self::RetryBeforeFirstPresent,
            Self::Presented | Self::RetryAfterPresent => Self::RetryAfterPresent,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrameLoop {
    Unconfigured,
    Ticking,
    RescheduleRequested,
    PresentationFailed,
    AwaitingCallback,
    Scheduled,
    RetryScheduled,
    Parked,
    Stopped,
}

impl FrameLoop {
    pub(super) fn schedule(self) -> Self {
        match self {
            Self::Parked => Self::Scheduled,
            Self::Ticking => Self::RescheduleRequested,
            _ => self,
        }
    }

    pub(super) fn presentation_failed(self, presentation: PresentationState) -> Self {
        match self {
            Self::Stopped => self,
            Self::Scheduled | Self::RetryScheduled if !presentation.has_presented() => self,
            _ => Self::PresentationFailed,
        }
    }

    pub(super) fn accepts_retry(self, expected: Option<Instant>, deadline: Instant) -> bool {
        self == Self::RetryScheduled && expected == Some(deadline)
    }

    pub(super) fn complete(self, presentation: PresentationState, redraw_requested: bool) -> Self {
        if matches!(
            self,
            Self::Unconfigured
                | Self::AwaitingCallback
                | Self::Scheduled
                | Self::RetryScheduled
                | Self::Stopped
        ) {
            return self;
        }

        if presentation.requires_presentation() {
            if self == Self::PresentationFailed
                && presentation == PresentationState::RetryAfterPresent
            {
                Self::AwaitingCallback
            } else {
                Self::RetryScheduled
            }
        } else if self == Self::RescheduleRequested || redraw_requested {
            Self::RetryScheduled
        } else {
            Self::Parked
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{FrameLoop, PresentationState};

    #[test]
    fn failure_tracks_whether_the_surface_has_presented() {
        for (before, after) in [
            (
                PresentationState::Unpresented,
                PresentationState::RetryBeforeFirstPresent,
            ),
            (
                PresentationState::RetryBeforeFirstPresent,
                PresentationState::RetryBeforeFirstPresent,
            ),
            (
                PresentationState::Presented,
                PresentationState::RetryAfterPresent,
            ),
            (
                PresentationState::RetryAfterPresent,
                PresentationState::RetryAfterPresent,
            ),
        ] {
            assert_eq!(before.failed(), after);
            assert_eq!(before.has_presented(), after.has_presented());
        }
    }

    #[test]
    fn only_retry_states_require_presentation() {
        assert!(!PresentationState::Unpresented.requires_presentation());
        assert!(!PresentationState::Presented.requires_presentation());
        assert!(PresentationState::RetryBeforeFirstPresent.requires_presentation());
        assert!(PresentationState::RetryAfterPresent.requires_presentation());
    }

    #[test]
    fn demand_wakes_a_parked_loop_once_and_preserves_armed_wakeups() {
        assert_eq!(FrameLoop::Parked.schedule(), FrameLoop::Scheduled);
        for armed in [
            FrameLoop::Unconfigured,
            FrameLoop::Scheduled,
            FrameLoop::RetryScheduled,
            FrameLoop::AwaitingCallback,
            FrameLoop::PresentationFailed,
            FrameLoop::RescheduleRequested,
            FrameLoop::Stopped,
        ] {
            assert_eq!(armed.schedule(), armed);
        }
    }

    #[test]
    fn demand_during_a_tick_survives_a_throttled_frame() {
        let rescheduled = FrameLoop::Ticking.schedule();
        assert_eq!(rescheduled, FrameLoop::RescheduleRequested);
        assert_eq!(rescheduled.schedule(), rescheduled);
        assert_eq!(
            rescheduled.complete(PresentationState::Presented, false),
            FrameLoop::RetryScheduled
        );
    }

    #[test]
    fn first_present_failure_retries_without_a_compositor_callback() {
        let failed = PresentationState::Unpresented.failed();
        assert!(!failed.has_presented());
        assert_eq!(
            FrameLoop::PresentationFailed.complete(failed, false),
            FrameLoop::RetryScheduled
        );
        assert_eq!(
            FrameLoop::Ticking.complete(failed, false),
            FrameLoop::RetryScheduled
        );
    }

    #[test]
    fn a_deferred_draw_preserves_the_original_wakeup_and_retry_deadline() {
        for armed in [FrameLoop::Scheduled, FrameLoop::RetryScheduled] {
            let failed = armed.presentation_failed(PresentationState::Unpresented.failed());
            assert_eq!(failed, armed);
            assert_eq!(
                failed.complete(PresentationState::Unpresented.failed(), false),
                armed
            );
            assert_eq!(
                armed
                    .presentation_failed(PresentationState::Presented.failed())
                    .complete(PresentationState::Presented.failed(), false),
                FrameLoop::AwaitingCallback
            );
        }
        assert_eq!(
            FrameLoop::Parked
                .presentation_failed(PresentationState::Unpresented.failed())
                .complete(PresentationState::Unpresented.failed(), false),
            FrameLoop::RetryScheduled
        );
        assert_eq!(
            FrameLoop::Parked
                .presentation_failed(PresentationState::Presented.failed())
                .complete(PresentationState::Presented.failed(), false),
            FrameLoop::AwaitingCallback
        );
    }

    #[test]
    fn stale_retry_deadlines_cannot_consume_a_new_retry() {
        let old_deadline = Instant::now();
        let current_deadline = old_deadline + Duration::from_micros(16_667);
        assert!(!FrameLoop::RetryScheduled.accepts_retry(Some(current_deadline), old_deadline));
        assert!(FrameLoop::RetryScheduled.accepts_retry(Some(current_deadline), current_deadline));
        assert!(!FrameLoop::RetryScheduled.accepts_retry(None, current_deadline));
        for frame_loop in [
            FrameLoop::Unconfigured,
            FrameLoop::Ticking,
            FrameLoop::RescheduleRequested,
            FrameLoop::PresentationFailed,
            FrameLoop::AwaitingCallback,
            FrameLoop::Scheduled,
            FrameLoop::Parked,
            FrameLoop::Stopped,
        ] {
            assert!(!frame_loop.accepts_retry(Some(current_deadline), current_deadline));
        }
    }

    #[test]
    fn failed_present_on_a_mapped_surface_waits_for_the_compositor() {
        let failed = PresentationState::Presented.failed();
        assert!(failed.has_presented());
        assert_eq!(
            FrameLoop::PresentationFailed.complete(failed, true),
            FrameLoop::AwaitingCallback
        );
        assert_eq!(
            FrameLoop::Ticking.complete(failed, false),
            FrameLoop::RetryScheduled
        );
    }

    #[test]
    fn idle_completion_parks_but_redraw_survives_throttling() {
        for presentation in [PresentationState::Unpresented, PresentationState::Presented] {
            assert_eq!(
                FrameLoop::Ticking.complete(presentation, false),
                FrameLoop::Parked
            );
            assert_eq!(
                FrameLoop::Ticking.complete(presentation, true),
                FrameLoop::RetryScheduled
            );
        }
    }

    #[test]
    fn unconfigured_armed_and_stopped_loops_cannot_arm_another_retry() {
        for presentation in [
            PresentationState::Unpresented,
            PresentationState::Presented,
            PresentationState::RetryBeforeFirstPresent,
            PresentationState::RetryAfterPresent,
        ] {
            for redraw_requested in [false, true] {
                for frame_loop in [
                    FrameLoop::Unconfigured,
                    FrameLoop::AwaitingCallback,
                    FrameLoop::Scheduled,
                    FrameLoop::RetryScheduled,
                    FrameLoop::Stopped,
                ] {
                    assert_eq!(
                        frame_loop.complete(presentation, redraw_requested),
                        frame_loop
                    );
                }
            }
        }
    }
}
