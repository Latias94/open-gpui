//! Shared motion primitives for GPUI elements and UI controls.

mod value;

pub mod advanced;
mod controller;
mod frame_host;
pub mod geometry;
mod motion;
mod policy;
pub mod projection;
mod runtime;
mod spring;
mod spring_solver;
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
pub use projection::{
    MotionProjection, MotionProjectionClip, MotionProjectionError, MotionProjectionScale,
    MotionProjectionTransformSample,
};
pub use runtime::{MotionEdge, MotionRunState, reveal_rect_from_edge};
pub use spring::{MotionScalarSample, MotionSpringOptions, MotionSpringPolicyHints};
pub use spring_solver::{SpringConfig, SpringState, sampled_easing};
pub use transition::{
    MotionFrameDriver, MotionFrameDriverSample, MotionFrameDriverUpdate, MotionFrameResetReason,
    MotionIntent, MotionProgressRun, MotionScalarRun, MotionScalarRunSample, MotionTransition,
};
