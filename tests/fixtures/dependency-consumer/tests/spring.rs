//! Ordinary Spring consumers with no GPUI test-support. Solver, easing, target and parent
//! construction tests execute; the separately named signature closure is never executed.

use open_gpui::{
    Animation, AnimationExt as _, AnimationPhase, App, Div, Hsla, InteractiveElement as _,
    Interpolate, ParentElement, Pixels, Rems, Rgba, SpringAnimation, SpringAnimationElement,
    SpringConfig, SpringPlayback, SpringState, SpringTarget, Stateful, Styled as _, Window, div,
    px, rgb, sampled_easing,
};
use std::{cell::Cell, rc::Rc, time::Duration};

fn close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.0005,
        "expected {expected}, got {actual}",
    );
}

#[test]
fn downstream_solver_advances_and_retargets_in_explicit_elapsed_time() {
    let initial = SpringState {
        position: -2.,
        velocity: 3.,
    };
    for damping in [4., 20., 40.] {
        let config = SpringConfig::new(100., damping, 1.);
        assert_eq!(config.stiffness, 100.);
        assert_eq!(config.damping, damping);
        assert_eq!(config.mass, 1.);
        let (frequency, ratio) = config.canonical();
        close(frequency, 10.);
        close(ratio, damping / 20.);
        let first = config.step(initial, 5., 0.05);
        assert!(first.position > initial.position);
        assert!(first.velocity > 0.);
        let second = config.step(first, 5., 0.025);
        let direct = config.step(initial, 5., 0.075);
        close(second.position, direct.position);
        close(second.velocity, direct.velocity);

        let matrix = config.propagator(0.025);
        close(
            second.position,
            5. + matrix[0][0] * (first.position - 5.) + matrix[0][1] * first.velocity,
        );
        close(
            second.velocity,
            matrix[1][0] * (first.position - 5.) + matrix[1][1] * first.velocity,
        );

        let retargeted = config.step(first, -4., 0.001);
        assert!(retargeted.position > first.position);
        assert!(retargeted.velocity > 0.);
        let duration = config.settle_time(retargeted, -4., 0.001);
        assert!(duration > Duration::ZERO);
        assert_ne!(duration, Duration::MAX);
        for after in [0., 0.25, 1.] {
            let state = config.step(retargeted, -4., duration.as_secs_f32() + after);
            assert!(
                config.is_settled(state, -4., 0.001),
                "damping={damping}, duration={duration:?}, after={after}, state={state:?}"
            );
        }
    }
}

#[test]
fn downstream_ramp_and_settling_preserve_the_official_physical_contract() {
    let config = SpringConfig::new(100., 16., 1.);
    let target_velocity = 3.;
    let initial = SpringState {
        position: -0.48,
        velocity: target_velocity,
    };
    let ramp = config.step_ramp(initial, 0., target_velocity, 0.25);
    close(ramp.position, 0.27);
    close(ramp.velocity, target_velocity);
    let stationary = config.step_ramp(initial, 0., 0., 0.25);
    let fixed = config.step(initial, 0., 0.25);
    close(stationary.position, fixed.position);
    close(stationary.velocity, fixed.velocity);
    assert!(!config.is_settled(
        SpringState {
            position: 0.,
            velocity: 1.
        },
        0.,
        0.001
    ));
    assert!(config.is_settled(SpringState::default(), 0., 0.001));
    assert_eq!(
        config.settle_time(SpringState::default(), 0., 0.001),
        Duration::ZERO
    );
    let undamped = SpringConfig::new(100., 0., 1.);
    assert_eq!(
        undamped.settle_time(SpringState::default(), 1., 0.001),
        Duration::MAX
    );
    for epsilon in [-1., f32::NAN, f32::INFINITY] {
        assert!(!config.is_settled(SpringState::default(), 0., epsilon));
    }
}

#[test]
fn downstream_sampled_easing_drives_the_existing_duration_animation_builder() {
    let config = SpringConfig::new(100., 2., 1.);
    let (duration, easing) = sampled_easing(config, 0.001);
    assert!(duration > Duration::ZERO);
    assert_ne!(duration, Duration::MAX);
    let animation = Animation::new(duration).with_easing(easing);
    assert_eq!(animation.duration, duration);
    assert!(animation.oneshot);
    assert_eq!((animation.easing)(-0.2), 0.);
    assert_eq!((animation.easing)(0.), 0.);
    assert_eq!((animation.easing)(1.), 1.);
    assert_eq!((animation.easing)(1.2), 1.);
    for progress in [0.01, 0.1, 0.25, 0.5, 0.75] {
        let expected = config.step(
            SpringState::default(),
            1.,
            duration.as_secs_f32() * progress,
        );
        close((animation.easing)(progress), expected.position);
    }
    assert!(
        (1..200).any(|index| (animation.easing)(index as f32 / 200.) > 1.01),
        "the public duration adapter must preserve Spring overshoot",
    );
}

#[derive(Clone, Copy)]
struct PanelTarget(bool);

#[derive(Debug, PartialEq)]
struct PanelPose {
    width: Pixels,
    color: Rgba,
    phase: AnimationPhase,
}

impl SpringTarget for PanelTarget {
    type Output = PanelPose;

    fn target(&self) -> f32 {
        if self.0 { 1. } else { 0. }
    }

    fn resolve(&self, value: f32) -> Self::Output {
        let phase = AnimationPhase(value);
        PanelPose {
            width: phase.interpolate(px(20.), px(100.)),
            color: phase.interpolate_clamped(rgb(0x224466), rgb(0x6688aa)),
            phase,
        }
    }
}

#[test]
fn downstream_typed_targets_and_color_interpolation_execute_without_a_window() {
    assert_eq!(12_f32.target(), 12.);
    assert_eq!(12_f32.resolve(14.), 14.);
    assert_eq!(px(12.).target(), 12.);
    assert_eq!(px(12.).resolve(14.), px(14.));
    assert_eq!(Rems(2.).resolve(3.), Rems(3.));
    assert_eq!(false.target(), 0.);
    assert_eq!(true.target(), 1.);
    assert_eq!(true.resolve(1.25), AnimationPhase(1.25));
    assert_eq!(AnimationPhase(2.).target(), 2.);
    assert_eq!(AnimationPhase(2.).resolve(-0.5), AnimationPhase(-0.5));
    let target = PanelTarget(true);
    assert_eq!(target.target(), 1.);
    let overshoot = target.resolve(1.25);
    assert_eq!(overshoot.width, px(120.));
    assert_eq!(overshoot.phase, AnimationPhase(1.25));
    let expected_color = rgb(0x6688aa);
    close(overshoot.color.r, expected_color.r);
    close(overshoot.color.g, expected_color.g);
    close(overshoot.color.b, expected_color.b);
    close(overshoot.color.a, expected_color.a);
    assert_eq!(AnimationPhase(1.5).clamp(2.0..=1.0), AnimationPhase(1.5));
    assert_eq!(
        AnimationPhase(1.5).interpolate_between(1.0..=2.0, px(20.), px(100.)),
        px(60.)
    );
    assert_eq!(
        AnimationPhase(0.5).interpolate_between(1.0..=2.0, 20_f32, 100.),
        -20.
    );
    assert_eq!(
        AnimationPhase(0.5).interpolate_between_clamped(1.0..=2.0, 20_f32, 100.),
        20.
    );
    assert_eq!(
        AnimationPhase(0.5).interpolate_between(1.0..=0.0, 20_f32, 100.),
        60.
    );
    assert_eq!(
        AnimationPhase(0.5).interpolate_between(1.0..=1.0, 20_f32, 100.),
        20.
    );
    assert_eq!(
        AnimationPhase(1.).interpolate_between(1.0..=1.0, 20_f32, 100.),
        100.
    );
    assert_eq!(f32::interpolate(2., 6., 1.25), 7.);

    let rgba = AnimationPhase(0.5).interpolate(
        Rgba {
            r: 0.2,
            g: 0.4,
            b: 0.6,
            a: 0.2,
        },
        Rgba {
            r: 0.6,
            g: 0.8,
            b: 1.,
            a: 0.8,
        },
    );
    close(rgba.r, 0.4);
    close(rgba.g, 0.6);
    close(rgba.b, 0.8);
    close(rgba.a, 0.5);
    let hsla = AnimationPhase(0.5).interpolate(
        Hsla {
            h: 0.9,
            s: 0.4,
            l: 0.2,
            a: 0.2,
        },
        Hsla {
            h: 0.1,
            s: 0.8,
            l: 0.6,
            a: 0.8,
        },
    );
    assert!(hsla.h < 0.0005 || hsla.h > 0.9995);
    close(hsla.s, 0.6);
    close(hsla.l, 0.4);
    close(hsla.a, 0.5);
}

fn accepts_parent<E: ParentElement + open_gpui::IntoElement>(element: E) -> E {
    element
}

struct ConsumedCapture(Rc<Cell<usize>>);

impl Drop for ConsumedCapture {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn downstream_spring_builders_and_parents_construct_with_move_only_animators() {
    let config = SpringConfig::new(100., 10., 1.);
    assert_eq!(SpringPlayback::default(), SpringPlayback::Running);
    for playback in [
        SpringPlayback::Running,
        SpringPlayback::Paused,
        SpringPlayback::Stopped,
        SpringPlayback::Completed,
        SpringPlayback::Cancelled,
    ] {
        let animation = SpringAnimation::new(config)
            .with_epsilon(0.005)
            .playback(playback)
            .to(PanelTarget(true))
            .from(PanelTarget(false));
        let cloned = animation.clone();
        let drops = Rc::new(Cell::new(0));
        let capture = ConsumedCapture(Rc::clone(&drops));
        let mapped = Rc::new(Cell::new(0));
        let child: SpringAnimationElement<Stateful<Div>> = div()
            .id("public-spring-parent")
            .with_spring("public-spring", cloned, move |element, pose| {
                drop(capture);
                element.w(pose.width).bg(pose.color)
            })
            .child(div().size(px(4.)))
            .children([div(), div()])
            .map_element({
                let mapped = Rc::clone(&mapped);
                move |element| {
                    mapped.set(mapped.get() + 1);
                    element.h(px(24.))
                }
            });
        assert_eq!(mapped.get(), 1);
        assert_eq!(drops.get(), 0);
        let element = accepts_parent(child);
        drop(element);
        assert_eq!(drops.get(), 1);
        let typed: SpringAnimationElement<Div> = div().with_spring(
            "typed-pixels",
            SpringAnimation::new(config).to(px(80.)).from(px(20.)),
            |element, value: Pixels| element.w(value),
        );
        drop(accepts_parent(typed));
    }
}

#[test]
fn downstream_spring_signatures_type_check_in_an_explicitly_unexecuted_window_closure() {
    let _: fn(f32, f32, f32) -> SpringConfig = SpringConfig::new;
    let _: fn(&SpringConfig) -> (f32, f32) = SpringConfig::canonical;
    let _: fn(&SpringConfig, f32) -> [[f32; 2]; 2] = SpringConfig::propagator;
    let _: fn(&SpringConfig, SpringState, f32, f32) -> SpringState = SpringConfig::step;
    let _: fn(&SpringConfig, SpringState, f32, f32, f32) -> SpringState = SpringConfig::step_ramp;
    let _: fn(&SpringConfig, SpringState, f32, f32) -> bool = SpringConfig::is_settled;
    let _: fn(&SpringConfig, SpringState, f32, f32) -> Duration = SpringConfig::settle_time;
    let _: fn(SpringConfig) -> SpringAnimation<()> = SpringAnimation::new;
    let _: fn(SpringAnimation<()>, Pixels) -> SpringAnimation<Pixels> = SpringAnimation::to;
    let _: fn(SpringAnimation<Pixels>, Pixels) -> SpringAnimation<Pixels> = SpringAnimation::from;
    let _: fn(SpringAnimation<Pixels>, f32) -> SpringAnimation<Pixels> =
        SpringAnimation::with_epsilon;
    let _: fn(SpringAnimation<Pixels>, SpringPlayback) -> SpringAnimation<Pixels> =
        SpringAnimation::playback;
    let _unexecuted_app_window_consumer: fn(&mut App, &mut Window) -> bool = |app, window| {
        app.set_reduce_motion(true);
        window.request_animation_frame();
        app.reduce_motion()
    };
}
