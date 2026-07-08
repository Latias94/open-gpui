//! Explicit low-level motion contracts for adapter and framework authors.
//!
//! Most application and component code should start with root-level facade types such as
//! [`crate::MotionTransition`], [`crate::MotionProgressRun`], and [`crate::MotionFrameDriver`].
//! This module keeps deterministic scalar tracks, execution plans, frame-host internals, raw
//! timeline/spring models, and scalar progress sequences available without making them part of the
//! default import surface.

pub use crate::controller::{
    MotionExecutionPlan, MotionExecutionState, MotionProgressExecution, MotionScalarController,
    MotionScalarControllerSample, MotionScalarExecution, MotionScalarExecutionSample,
    MotionScalarTrack, MotionScalarTrackSample,
};
pub use crate::frame_host::{
    MotionFrameHost, MotionFrameHostResetReason, MotionFrameHostSample, MotionFrameHostUpdate,
};
pub use crate::motion::MotionSpec;
pub use crate::policy::{
    MOTION_POLICY_MAX_UI_DURATION, MotionPolicyContext, MotionPolicyInput, MotionPolicyIssue,
    MotionPolicyReport, MotionPreviewTargetPolicy, validate_motion_policy,
};
pub use crate::sequence::{
    MotionSequence, MotionSequenceSample, MotionSequenceStep, MotionSequenceStepSample,
    MotionSequenceStepState,
};
pub use crate::spring::{MotionModel, MotionPreset, MotionSpringPreset, MotionSpringSpec};
