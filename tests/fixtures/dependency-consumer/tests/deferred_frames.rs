//! Feature-free consumption of the deferred delivery interfaces.
//!
//! Typed closures remain unexecuted: this checks downstream availability without constructing
//! private submission authority, simulating a host, or claiming GPU/native qualification.

use open_gpui::{
    PlatformFrameDeliveryMode, PlatformFrameSubmission, PlatformFrameSubmissionStatus,
    PlatformWindow, PlatformWindowPresentOutcome, Scene, WindowId,
};

#[test]
fn deferred_delivery_uses_the_original_public_submission_identity() {
    let _: fn(&PlatformFrameSubmission) -> WindowId = PlatformFrameSubmission::window_id;
    let _: fn(&PlatformFrameSubmission) -> u64 = PlatformFrameSubmission::frame_generation;
    let _: fn(&PlatformFrameSubmission) -> u64 = PlatformFrameSubmission::attempt_id;
    let _: fn(&PlatformFrameSubmission) -> bool = PlatformFrameSubmission::is_pending;
    let _: fn(&PlatformFrameSubmission) -> PlatformFrameSubmissionStatus =
        PlatformFrameSubmission::status;
    let _: fn(&PlatformFrameSubmission, PlatformWindowPresentOutcome) -> bool =
        PlatformFrameSubmission::complete;

    let _deliver =
        |window: &dyn PlatformWindow, scene: &Scene, submission: &PlatformFrameSubmission| {
            assert_eq!(
                window.frame_delivery_mode(),
                PlatformFrameDeliveryMode::Deferred
            );
            window.draw_with_submission(scene, submission)
        };
    let _clone = |submission: &PlatformFrameSubmission| submission.clone();
    assert_eq!(
        PlatformFrameDeliveryMode::default(),
        PlatformFrameDeliveryMode::Synchronous
    );
}
