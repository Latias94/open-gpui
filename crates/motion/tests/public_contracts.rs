use open_gpui_motion::{
    MotionDuration, MotionEasing, MotionExecutionPlan, MotionFrameReason, MotionModel,
    MotionPolicyContext, MotionPolicyInput, MotionPreference, MotionProjection,
    MotionProjectionClip, MotionScalarExecution, MotionSpec, motion_point, motion_px, motion_rect,
    motion_size,
};
use std::time::Duration;

#[test]
fn scalar_execution_samples_by_explicit_elapsed_controller_time() {
    let spec = MotionSpec::new(
        MotionPreference::Animated,
        MotionDuration::Custom(Duration::from_millis(100)),
        MotionEasing::Linear,
    );
    let plan = MotionExecutionPlan::resolve(
        MotionPolicyInput::new(
            MotionPolicyContext::CommittedLayout,
            MotionModel::timeline(spec),
        )
        .with_spatial_motion(true)
        .with_reduced_motion_final_state(true),
    );
    let execution = MotionScalarExecution::start(plan, 0.0, 10.0, 0.0, Duration::from_millis(20));

    let start = execution.sample_at(Duration::from_millis(20));
    assert_eq!(start.value(), 0.0);
    assert!(start.frame_demand().needs_frame());
    assert_eq!(
        start.frame_demand().reason(),
        Some(MotionFrameReason::UpdateRender)
    );

    let midpoint = execution.sample_at(Duration::from_millis(70));
    assert_eq!(midpoint.value(), 5.0);
    assert!(!midpoint.complete());

    let complete = execution.sample_at(Duration::from_millis(140));
    assert_eq!(complete.value(), 10.0);
    assert!(complete.complete());
    assert!(!complete.frame_demand().needs_frame());
}

#[test]
fn reduced_projection_clip_publishes_final_semantic_bounds_immediately() {
    let source = motion_rect(
        motion_point(motion_px(0.0), motion_px(0.0)),
        motion_size(motion_px(100.0), motion_px(80.0)),
    );
    let target = motion_rect(
        motion_point(motion_px(40.0), motion_px(20.0)),
        motion_size(motion_px(160.0), motion_px(120.0)),
    );

    let clip = MotionProjectionClip::from_projection_with_preference(
        MotionProjection::between(source, target),
        0.0,
        MotionPreference::Reduced,
    );

    assert_eq!(clip.content_bounds(), target);
    assert_eq!(clip.visible_bounds(), target);
    assert_eq!(clip.occlusion_bounds(), target);
    assert_eq!(clip.progress(), 1.0);
}

#[test]
fn motion_manifest_has_no_ui_domain_or_platform_dependencies() {
    let manifest = include_str!("../Cargo.toml");

    for forbidden in [
        "open_gpui",
        "open_gpui_ui_core",
        "open_gpui_ui_components",
        "open_gpui_docking",
        "open_gpui_platform",
        "open_gpui_web",
        "open_gpui_wgpu",
        "open_gpui_linux",
        "open_gpui_macos",
        "open_gpui_windows",
        "open-gpui-ui-core",
        "open-gpui-ui-components",
        "open-gpui-docking",
        "open-gpui-platform",
        "open-gpui-web",
        "open-gpui-wgpu",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "motion crate manifest must not depend on {forbidden}"
        );
    }
}
