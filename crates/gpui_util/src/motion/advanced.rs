//! Explicit low-level motion contracts for adapter and framework authors.
//!
//! Most application and component code should start with root-level facade types such as
//! [`crate::motion::MotionTransition`], [`crate::motion::MotionProgressRun`], and [`crate::motion::MotionFrameDriver`].
//! This module exposes scalar tracks, execution plans, and timeline and spring models.

pub use crate::motion::controller::{
    MotionExecutionPlan, MotionExecutionState, MotionProgressExecution, MotionScalarController,
    MotionScalarControllerSample, MotionScalarExecution, MotionScalarExecutionSample,
    MotionScalarTrack, MotionScalarTrackSample,
};
pub use crate::motion::motion::MotionSpec;
pub use crate::motion::policy::{
    MOTION_POLICY_MAX_UI_DURATION, MotionPolicyContext, MotionPolicyInput, MotionPolicyIssue,
    MotionPolicyReport, MotionPreviewTargetPolicy, validate_motion_policy,
};
pub use crate::motion::spring::{MotionModel, MotionPreset, MotionSpringPreset, MotionSpringSpec};
