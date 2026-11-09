use super::*;

const EPSILON: f32 = 1e-4;

#[test]
fn spring_targets_resolve_typed_outputs() {
    assert_eq!(12.0_f32.target(), 12.0);
    assert_eq!(12.0_f32.resolve(14.0), 14.0);
    assert_eq!(Pixels::from(12.0).target(), 12.0);
    assert_eq!(Pixels::from(12.0).resolve(14.0), Pixels::from(14.0));
    assert_eq!(false.target(), 0.0);
    assert_eq!(true.target(), 1.0);
    assert_eq!(true.resolve(1.25), AnimationPhase(1.25));
    assert_eq!(Rems(2.0).target(), 2.0);
    assert_eq!(Rems(2.0).resolve(-0.5), Rems(-0.5));
    assert_eq!(AnimationPhase(2.0).target(), 2.0);
    assert_eq!(AnimationPhase(2.0).resolve(-0.5), AnimationPhase(-0.5));
    assert_eq!(AnimationPhase::from(false), AnimationPhase(0.0));
    assert_eq!(AnimationPhase::from(true), AnimationPhase(1.0));
    assert_eq!(AnimationPhase::from(1.5), AnimationPhase(1.5));
}

#[test]
fn animation_phases_interpolate_over_arbitrary_ranges() {
    let phase = AnimationPhase(1.5);
    assert_eq!(phase.interpolate_between(1.0..=2.0, 10.0, 20.0), 15.0);
    assert_eq!(
        phase.interpolate_between_clamped(2.0..=3.0, 10.0, 20.0),
        10.0
    );
    assert_eq!(
        AnimationPhase(3.5).interpolate_between(2.0..=3.0, 10.0, 20.0),
        25.0
    );
}

#[test]
fn hsla_interpolation_takes_the_shortest_hue_path() {
    let from = Hsla {
        h: 0.9,
        s: 0.5,
        l: 0.5,
        a: 1.0,
    };
    let to = Hsla {
        h: 0.1,
        s: 1.0,
        l: 0.75,
        a: 0.5,
    };
    let result = AnimationPhase(0.5).interpolate(from, to);

    assert!(result.h < EPSILON || (1.0 - result.h) < EPSILON);
    assert!((result.s - 0.75).abs() < EPSILON);
    assert!((result.l - 0.625).abs() < EPSILON);
    assert!((result.a - 0.75).abs() < EPSILON);

    let reverse = AnimationPhase(0.25).interpolate(to, from);
    assert!((reverse.h - 0.05).abs() < EPSILON);
    let extrapolated = AnimationPhase(2.0).interpolate(from, to);
    assert!((extrapolated.h - 0.3).abs() < EPSILON);
    assert!((extrapolated.s - 1.5).abs() < EPSILON);
}

#[test]
fn phase_projection_preserves_overshoot_unless_explicitly_clamped() {
    assert_eq!(AnimationPhase(-0.5).interpolate(10.0, 20.0), 5.0);
    assert_eq!(AnimationPhase(1.5).interpolate(10.0, 20.0), 25.0);
    assert_eq!(AnimationPhase(-0.5).interpolate_clamped(10.0, 20.0), 10.0);
    assert_eq!(AnimationPhase(1.5).interpolate_clamped(10.0, 20.0), 20.0);
    assert_eq!(
        AnimationPhase(1.5).interpolate(Pixels::from(10.0), Pixels::from(20.0)),
        Pixels::from(25.0)
    );
    assert_eq!(
        AnimationPhase(-0.5).interpolate(Rems(2.0), Rems(4.0)),
        Rems(1.0)
    );
}

#[test]
fn phase_ranges_support_reverse_and_degenerate_coordinates() {
    let reversed = RangeInclusive::new(2.0, 1.0);
    assert_eq!(
        AnimationPhase(1.75).interpolate_between(reversed.clone(), 10.0, 20.0),
        12.5
    );
    assert_eq!(
        AnimationPhase(3.0).interpolate_between_clamped(reversed.clone(), 10.0, 20.0),
        10.0
    );
    assert_eq!(
        AnimationPhase(0.0).interpolate_between_clamped(reversed, 10.0, 20.0),
        20.0
    );
    for phase in [0.5, 1.0, 1.5] {
        let expected = if phase < 1.0 { 10.0 } else { 20.0 };
        assert_eq!(
            AnimationPhase(phase).interpolate_between(1.0..=1.0, 10.0, 20.0),
            expected
        );
        assert_eq!(
            AnimationPhase(phase).interpolate_between_clamped(1.0..=1.0, 10.0, 20.0),
            expected
        );
    }
    assert_eq!(
        AnimationPhase(-1.0).clamp(RangeInclusive::new(2.0, 1.0)),
        AnimationPhase(1.0)
    );
    assert_eq!(AnimationPhase(3.0).clamp(1.0..=2.0), AnimationPhase(2.0));
}

#[test]
fn rgba_interpolation_preserves_all_channels_and_extrapolation() {
    let from = Rgba {
        r: 0.0,
        g: 0.25,
        b: 0.5,
        a: 1.0,
    };
    let to = Rgba {
        r: 1.0,
        g: 0.75,
        b: 0.25,
        a: 0.5,
    };
    assert_eq!(
        AnimationPhase(0.5).interpolate(from, to),
        Rgba {
            r: 0.5,
            g: 0.5,
            b: 0.375,
            a: 0.75,
        }
    );
    assert_eq!(
        AnimationPhase(2.0).interpolate(from, to),
        Rgba {
            r: 2.0,
            g: 1.25,
            b: 0.0,
            a: 0.0,
        }
    );
    assert_eq!(AnimationPhase(2.0).interpolate_clamped(from, to), to);
}

#[derive(Debug)]
struct ProjectedTarget {
    coordinate: f32,
    label: String,
}

impl SpringTarget for ProjectedTarget {
    type Output = (String, f32);

    fn target(&self) -> f32 {
        self.coordinate
    }

    fn resolve(&self, value: f32) -> Self::Output {
        (self.label.clone(), value * 2.0)
    }
}

#[test]
fn builder_preserves_settings_and_custom_noncopy_target_projection() {
    let config = SpringConfig::new(144.0, 18.0, 2.0);
    let animation = SpringAnimation::new(config)
        .with_epsilon(0.025)
        .playback(SpringPlayback::Paused)
        .to(ProjectedTarget {
            coordinate: 4.0,
            label: "target".into(),
        })
        .from(ProjectedTarget {
            coordinate: -2.0,
            label: "initial".into(),
        });

    assert_eq!(animation.config, config);
    assert_eq!(animation.epsilon, 0.025);
    assert_eq!(animation.playback, SpringPlayback::Paused);
    assert_eq!(animation.initial, Some(-2.0));
    assert_eq!(animation.target.target(), 4.0);
    assert_eq!(animation.target.resolve(5.5), ("target".into(), 11.0));

    let default = SpringAnimation::new(config).to(true);
    assert_eq!(default.initial, None);
    assert_eq!(default.playback, SpringPlayback::Running);
    assert_eq!(default.epsilon, DEFAULT_SPRING_EPSILON);
    assert_eq!(default.target.resolve(1.5), AnimationPhase(1.5));
}

#[test]
fn solver_reexports_are_the_motion_types_and_function() {
    let config: open_gpui_motion::SpringConfig = SpringConfig::new(100.0, 10.0, 1.0);
    let state: SpringState = open_gpui_motion::SpringState {
        position: 0.0,
        velocity: 2.0,
    };
    let accepts_motion_state = |state: open_gpui_motion::SpringState| state;
    assert_eq!(accepts_motion_state(state), state);
    let (duration, easing) = sampled_easing(config, 0.001);
    let (motion_duration, motion_easing) = open_gpui_motion::sampled_easing(config, 0.001);
    assert_eq!(duration, motion_duration);
    assert_eq!(easing(0.25), motion_easing(0.25));
}
