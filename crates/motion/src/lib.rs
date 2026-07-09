#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

//! Renderer-neutral motion primitives for Open GPUI.
//!
//! This crate owns deterministic motion specifications, sampling, policy, projection helpers, and
//! frame-demand contracts without depending on GPUI windows, component state, docking state, or
//! platform renderers.

mod value;

pub mod advanced;
mod controller;
mod frame_host;
pub mod geometry;
mod motion;
mod policy;
pub mod projection;
mod runtime;
mod sequence;
mod spring;
pub mod transition;

pub use controller::{
    MotionClockSample, MotionFrameDemand, MotionFrameReason, MotionProgressSample,
};
pub use geometry::{
    MotionEdges, MotionPoint, MotionPx, MotionRect, MotionSize, motion_edges, motion_point,
    motion_px, motion_rect, motion_size,
};
pub use motion::{MotionDuration, MotionEasing, MotionPreference};
pub use policy::{
    MOTION_POLICY_MAX_UI_DURATION, MotionPolicyContext, MotionPolicyIssue, MotionPolicyReport,
};
pub use projection::{MotionProjection, MotionProjectionClip};
pub use runtime::{
    MotionEdge, MotionRetargetItem, MotionRetargetSet, MotionRunState, MotionSnapshot, lerp_rect,
    motion_source_rect, preferred_motion_edge, retarget_motion_snapshots, reveal_rect_from_edge,
};
pub use sequence::{
    MotionSequence as MotionProgressSequence, MotionSequenceSample as MotionProgressSequenceSample,
    MotionSequenceStep as MotionProgressSequenceStep,
    MotionSequenceStepSample as MotionProgressSequenceStepSample,
    MotionSequenceStepState as MotionProgressSequenceStepState,
};
pub use spring::{MotionScalarSample, MotionSpringPhysics};
pub use transition::{
    MotionFrameDriver, MotionFrameDriverSample, MotionFrameDriverUpdate, MotionFrameResetReason,
    MotionIntent, MotionProgressRun, MotionScalarRun, MotionScalarRunSample, MotionTransition,
};
