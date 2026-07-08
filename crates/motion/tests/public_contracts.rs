use open_gpui_motion::{
    MotionClockSample, MotionDuration, MotionEasing, MotionFrameDemand, MotionFrameDriver,
    MotionFrameHostResetReason, MotionFrameReason, MotionIntent, MotionPolicyContext,
    MotionPreference, MotionProgressSequence, MotionProgressSequenceStepState, MotionProjection,
    MotionProjectionClip, MotionRunState, MotionTransition,
    advanced::{
        MotionExecutionPlan, MotionModel, MotionPolicyInput, MotionProgressExecution,
        MotionScalarController, MotionSpec,
    },
    motion_point, motion_px, motion_rect, motion_size,
};
use std::time::Duration;

#[test]
fn scalar_run_samples_by_explicit_elapsed_controller_time() {
    let transition = MotionTransition::duration(
        MotionIntent::CommittedLayout,
        MotionPreference::Animated,
        MotionDuration::Custom(Duration::from_millis(100)),
        MotionEasing::Linear,
    );
    let execution = transition.scalar_run(0.0, 10.0, 0.0, Duration::from_millis(20));

    let start = execution.sample_elapsed(Duration::from_millis(20));
    assert_eq!(start.value(), 0.0);
    assert!(start.frame_demand().needs_frame());
    assert_eq!(
        start.frame_demand().reason(),
        Some(MotionFrameReason::UpdateRender)
    );

    let midpoint = execution.sample_elapsed(Duration::from_millis(70));
    assert_eq!(midpoint.value(), 5.0);
    assert!(!midpoint.complete());

    let complete = execution.sample_elapsed(Duration::from_millis(140));
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
fn progress_execution_is_public_adapter_lifecycle_contract() {
    let progress = MotionTransition::duration(
        MotionIntent::Continuity,
        MotionPreference::Animated,
        MotionDuration::Custom(Duration::from_millis(100)),
        MotionEasing::Linear,
    );
    let progress = progress.progress_run(Duration::ZERO);

    let midpoint = progress.sample_elapsed(Duration::from_millis(50));
    assert_eq!(midpoint.progress(), 0.5);
    assert_eq!(
        midpoint.frame_demand().reason(),
        Some(MotionFrameReason::UpdateRender)
    );

    let complete = progress.sample_elapsed(Duration::from_millis(120));
    assert_eq!(complete.progress(), 1.0);
    assert!(complete.complete());
    assert!(!complete.frame_demand().needs_frame());
}

#[test]
fn advanced_progress_execution_is_elapsed_time_only() {
    let model = MotionModel::timeline(MotionSpec::new(
        MotionPreference::Animated,
        MotionDuration::Custom(Duration::from_millis(100)),
        MotionEasing::Linear,
    ));
    let plan = MotionExecutionPlan::resolve(
        MotionPolicyInput::new(MotionPolicyContext::Continuity, model)
            .with_reduced_motion_final_state(true),
    );
    let execution = MotionProgressExecution::start(plan, Duration::ZERO);

    assert_eq!(execution.started_at(), Duration::ZERO);
    assert_eq!(
        execution.sample_at(Duration::from_millis(50)).progress(),
        0.5
    );
    assert!(execution.sample_at(Duration::from_millis(120)).complete());
}

#[test]
fn frame_demand_and_clock_samples_are_public_adapter_contracts() {
    let active = MotionFrameDemand::NeedsFrame(MotionFrameReason::UpdateRender);
    let combined = MotionFrameDemand::combine_all([MotionFrameDemand::Idle, active]);
    let clock =
        MotionClockSample::from_elapsed(Duration::from_millis(90), Duration::from_millis(30));

    assert_eq!(combined, active);
    assert_eq!(combined.reason(), Some(MotionFrameReason::UpdateRender));
    assert_eq!(clock.elapsed(), Duration::from_millis(90));
    assert_eq!(clock.delta(), Duration::ZERO);
    assert!(clock.clamped());
}

#[test]
fn frame_driver_is_a_public_adapter_contract() {
    let active = MotionFrameDemand::NeedsFrame(MotionFrameReason::UpdateRender);
    let mut frame_driver = MotionFrameDriver::new();

    let first = frame_driver.observe(MotionFrameDemand::Idle);
    let second =
        frame_driver.sample_elapsed(Duration::from_millis(16), |clock| (clock.elapsed(), active));

    assert!(!first.should_request_frame());
    assert!(second.should_request_frame());
    assert_eq!(*second.value(), Duration::from_millis(16));
    assert_eq!(second.frame_demand(), active);
    assert_eq!(second.update().requested_frames(), 1);
    assert_eq!(frame_driver.last_frame_demand(), active);
}

#[test]
fn frame_driver_reset_starts_a_new_adapter_epoch() {
    let active = MotionFrameDemand::NeedsFrame(MotionFrameReason::UpdateRender);
    let mut frame_driver = MotionFrameDriver::new();

    let active_sample =
        frame_driver.sample_elapsed(Duration::from_millis(90), |clock| (clock.elapsed(), active));
    assert!(active_sample.should_request_frame());
    assert_eq!(frame_driver.last_elapsed(), Duration::from_millis(90));
    assert_eq!(frame_driver.requested_frames(), 1);

    frame_driver.reset(MotionFrameHostResetReason::Retarget);

    assert_eq!(frame_driver.last_elapsed(), Duration::ZERO);
    assert_eq!(frame_driver.last_frame_demand(), MotionFrameDemand::Idle);
    assert_eq!(frame_driver.requested_frames(), 0);
    assert_eq!(
        frame_driver.last_reset_reason(),
        Some(MotionFrameHostResetReason::Retarget)
    );

    let new_epoch =
        frame_driver.sample_elapsed(Duration::from_millis(10), |clock| (clock.elapsed(), active));
    assert_eq!(*new_epoch.value(), Duration::from_millis(10));
    assert_eq!(new_epoch.clock().delta(), Duration::from_millis(10));
    assert!(!new_epoch.clock().clamped());
}

#[test]
fn scalar_controller_lifecycle_is_public_and_demand_driven() {
    let model = MotionModel::timeline(MotionSpec::new(
        MotionPreference::Animated,
        MotionDuration::Custom(Duration::from_millis(200)),
        MotionEasing::Linear,
    ));
    let mut controller = MotionScalarController::new();

    controller.start("indicator", model, 0.0, 1.0, 0.0, Duration::ZERO);
    assert!(
        controller
            .sample_clock(MotionClockSample::from_elapsed(
                Duration::ZERO,
                Duration::from_millis(50),
            ))
            .frame_demand()
            .needs_frame()
    );

    controller.finish(&"indicator", Duration::from_millis(80));
    let sample = controller
        .sample_at(Duration::from_millis(90))
        .track(&"indicator")
        .expect("indicator sample")
        .sample();
    assert_eq!(sample.state(), MotionRunState::Completed);
    assert_eq!(sample.value(), 1.0);
    assert_eq!(controller.prune_terminal_at(Duration::from_millis(90)), 1);
    assert!(
        !controller
            .frame_demand_at(Duration::from_millis(90))
            .needs_frame()
    );
}

#[test]
fn sequence_plan_is_public_renderer_neutral_motion_composition() {
    let model = MotionModel::timeline(MotionSpec::new(
        MotionPreference::Animated,
        MotionDuration::Custom(Duration::from_millis(100)),
        MotionEasing::Linear,
    ));
    let mut sequence = MotionProgressSequence::new();

    sequence
        .append("first", model)
        .insert_with_previous("parallel", model)
        .insert_after_previous("after", model, Duration::from_millis(20));

    assert_eq!(sequence.steps()[0].start_at(), Duration::ZERO);
    assert_eq!(sequence.steps()[1].start_at(), Duration::ZERO);
    assert_eq!(sequence.steps()[2].start_at(), Duration::from_millis(120));
    assert_eq!(sequence.duration_hint(), Duration::from_millis(220));

    let active = sequence.sample_at(Duration::from_millis(50));
    assert_eq!(
        active.step(&"first").expect("first step").state(),
        MotionProgressSequenceStepState::Active
    );
    assert_eq!(
        active.step(&"after").expect("after step").state(),
        MotionProgressSequenceStepState::Pending
    );
    assert!(active.frame_demand().needs_frame());

    let complete = sequence.sample_at(Duration::from_millis(240));
    assert!(complete.complete());
    assert!(!complete.frame_demand().needs_frame());
}

#[test]
fn low_level_motion_internals_are_explicit_advanced_imports() {
    let manifest = include_str!("../src/lib.rs");

    for forbidden in [
        "MotionScalarController",
        "MotionScalarExecution",
        "MotionExecutionPlan",
        "MotionFrameHost",
        "MotionSequence,",
        "MotionSpec",
        "MotionModel",
        "MotionPreset",
        "MotionPolicyInput",
        "MotionPreviewTargetPolicy",
        "validate_motion_policy",
    ] {
        assert!(
            !manifest
                .lines()
                .filter(|line| line.trim_start().starts_with("pub use "))
                .any(|line| source_line_contains_identifier(line, forbidden)),
            "root public surface should not re-export low-level {forbidden}; use open_gpui_motion::advanced"
        );
    }
}

#[test]
fn advanced_manifest_does_not_export_instant_lifecycle_types() {
    let manifest = include_str!("../src/advanced.rs");

    for forbidden in ["MotionSpring", "MotionTimeline", "MotionTimelineSample"] {
        assert!(
            !manifest
                .lines()
                .filter(|line| line.trim_start().starts_with("pub use "))
                .any(|line| source_line_contains_identifier(line, forbidden)),
            "advanced public surface should not re-export Instant-owning lifecycle type {forbidden}"
        );
    }
}

fn source_line_contains_identifier(line: &str, token: &str) -> bool {
    line.split(|character: char| character != '_' && !character.is_ascii_alphanumeric())
        .any(|part| part == token)
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
