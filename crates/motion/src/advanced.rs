//! Explicit low-level motion contracts for adapter and framework authors.
//!
//! Most application and component code should start with root-level facade types such as
//! [`crate::MotionTransition`], [`crate::MotionProgressRun`], and [`crate::MotionFrameDriver`].
//! This module keeps deterministic scalar tracks, execution plans, frame-host internals, raw
//! timeline/spring specs, and scalar progress sequences available without making them part of the
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
pub use crate::runtime::{MotionTimeline, MotionTimelineSample};
pub use crate::sequence::{
    MotionSequence, MotionSequenceSample, MotionSequenceStep, MotionSequenceStepSample,
    MotionSequenceStepState,
};
pub use crate::spring::{
    MotionModel, MotionPreset, MotionSpring, MotionSpringPreset, MotionSpringSpec,
};
