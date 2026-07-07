#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

//! Renderer-neutral motion primitives for Open GPUI.
//!
//! This crate owns deterministic motion specifications, sampling, policy, projection helpers, and
//! frame-demand contracts without depending on GPUI windows, component state, docking state, or
//! platform renderers.

mod value;

pub mod controller;
pub mod frame_host;
pub mod geometry;
pub mod motion;
pub mod policy;
pub mod projection;
pub mod runtime;
pub mod sequence;
pub mod spring;

pub use controller::{
    MotionClockSample, MotionExecutionPlan, MotionExecutionState, MotionFrameDemand,
    MotionFrameReason, MotionProgressExecution, MotionProgressSample, MotionScalarController,
    MotionScalarControllerSample, MotionScalarExecution, MotionScalarExecutionSample,
    MotionScalarTrack, MotionScalarTrackSample,
};
pub use frame_host::{
    MotionFrameHost, MotionFrameHostResetReason, MotionFrameHostSample, MotionFrameHostUpdate,
};
pub use geometry::{
    MotionEdges, MotionPoint, MotionPx, MotionRect, MotionSize, motion_edges, motion_point,
    motion_px, motion_rect, motion_size,
};
pub use motion::{MotionDuration, MotionEasing, MotionPreference, MotionSpec};
pub use policy::{
    MOTION_POLICY_MAX_UI_DURATION, MotionPolicyContext, MotionPolicyInput, MotionPolicyIssue,
    MotionPolicyReport, MotionPreviewTargetPolicy, validate_motion_policy,
};
pub use projection::{MotionProjection, MotionProjectionClip};
pub use runtime::{
    MotionEdge, MotionRetargetItem, MotionRetargetSet, MotionRunState, MotionSnapshot,
    MotionTimeline, MotionTimelineSample, lerp_rect, motion_source_rect, preferred_motion_edge,
    retarget_motion_snapshots, reveal_rect_from_edge,
};
pub use sequence::{
    MotionSequence, MotionSequenceSample, MotionSequenceStep, MotionSequenceStepSample,
    MotionSequenceStepState,
};
pub use spring::{
    MotionModel, MotionPreset, MotionScalarSample, MotionSpring, MotionSpringPhysics,
    MotionSpringPreset, MotionSpringSpec,
};
