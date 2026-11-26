mod motion;
mod policy;
mod runtime;
mod spring;
mod spring_solver;
mod transition;

pub use motion::{MotionDuration, MotionEasing, MotionPreference, MotionSpec};
pub use policy::{
    MotionPolicyContext, MotionPolicyInput, MotionPolicyIssue, MotionPolicyReport,
    validate_motion_policy,
};
pub use runtime::MotionRunState;
pub use spring::{
    MotionModel, MotionScalarSample, MotionSpringOptions, MotionSpringPolicyHints, MotionSpringSpec,
};
pub use spring_solver::{SpringConfig, SpringState, sampled_easing};
pub(crate) use transition::MotionProgressRun;
pub use transition::{MotionIntent, MotionTransition};
