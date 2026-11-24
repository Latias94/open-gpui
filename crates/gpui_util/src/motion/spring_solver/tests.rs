use super::*;

const EPSILON: f32 = 1e-4;

#[test]
fn propagators_compose_and_have_expected_determinant() {
    for damping_ratio in [0.4, 1.0, 1.5] {
        let natural_frequency = 12.0;
        let config = SpringConfig::new(
            natural_frequency * natural_frequency,
            2.0 * damping_ratio * natural_frequency,
            1.0,
        );
        let first = config.propagator(0.013);
        let second = config.propagator(0.021);
        let combined = multiply(second, first);
        let direct = config.propagator(0.034);

        for row in 0..2 {
            for column in 0..2 {
                assert!(
                    (combined[row][column] - direct[row][column]).abs() < 2e-4,
                    "{damping_ratio}: {combined:?} != {direct:?}"
                );
            }
        }

        let determinant = direct[0][0] * direct[1][1] - direct[0][1] * direct[1][0];
        let expected = (-2.0 * damping_ratio * natural_frequency * 0.034).exp();
        assert!((determinant - expected).abs() < 2e-4);
    }
}

#[test]
fn step_preserves_semigroup_for_every_damping_regime() {
    let state = SpringState {
        position: -3.0,
        velocity: 5.0,
    };
    for damping in [4.0, 20.0, 40.0] {
        let config = SpringConfig::new(100.0, damping, 1.0);
        let stepped = config.step(config.step(state, 7.0, 0.013), 7.0, 0.021);
        let direct = config.step(state, 7.0, 0.034);

        assert!((stepped.position - direct.position).abs() < 2e-4);
        assert!((stepped.velocity - direct.velocity).abs() < 2e-4);
    }
}

#[test]
fn ramp_tracks_steady_state_lag() {
    let natural_frequency = 10.0;
    let damping_ratio = 0.8;
    let target_velocity = 3.0;
    let config = SpringConfig::new(
        natural_frequency * natural_frequency,
        2.0 * damping_ratio * natural_frequency,
        1.0,
    );
    let lag = -2.0 * damping_ratio * target_velocity / natural_frequency;
    let state = SpringState {
        position: lag,
        velocity: target_velocity,
    };
    let next = config.step_ramp(state, 0.0, target_velocity, 0.25);

    assert!((next.position - (target_velocity * 0.25 + lag)).abs() < EPSILON);
    assert!((next.velocity - target_velocity).abs() < EPSILON);
}

#[test]
fn settling_requires_low_velocity() {
    let config = SpringConfig::new(100.0, 10.0, 1.0);
    assert!(!config.is_settled(
        SpringState {
            position: 1.0,
            velocity: 1.0,
        },
        1.0,
        0.01,
    ));
    assert!(config.is_settled(
        SpringState {
            position: 1.005,
            velocity: 0.05,
        },
        1.0,
        0.01,
    ));
}

#[test]
fn settle_time_is_conservative_for_every_damping_regime() {
    let initial_state = SpringState {
        position: -2.0,
        velocity: 4.0,
    };
    for damping in [4.0, 20.0, 40.0] {
        let config = SpringConfig::new(100.0, damping, 1.0);
        let duration = config.settle_time(initial_state, 3.0, 0.001);
        assert_ne!(duration, Duration::MAX);

        for additional_time in [0.0, 0.1, 1.0] {
            let state = config.step(initial_state, 3.0, duration.as_secs_f32() + additional_time);
            assert!(
                config.is_settled(state, 3.0, 0.001),
                "{damping}: {duration:?} produced {state:?}"
            );
        }
    }
}

#[test]
fn settle_time_accounts_for_motion_outside_an_instantaneous_tolerance() {
    let config = SpringConfig::new(100.0, 2.0, 1.0);
    let state = SpringState {
        position: 1.125,
        velocity: 1.25,
    };
    assert!(config.is_settled(state, 1.0, 0.125));

    let duration = config.settle_time(state, 1.0, 0.125);
    assert!(duration > Duration::ZERO);
    assert!(config.is_settled(config.step(state, 1.0, duration.as_secs_f32()), 1.0, 0.125,));
}

#[test]
fn settle_time_respects_translated_float_position_boundaries() {
    for damping in [4.0, 20.0, 40.0] {
        let config = SpringConfig::new(100.0, damping, 1.0);
        let initial = SpringState {
            position: -2.0,
            velocity: 3.0,
        };
        let moving = config.step(initial, 5.0, 0.05);
        let retargeted = config.step(moving, -4.0, 0.001);
        for target in [-4.0, 4.0, 100_000.0] {
            let state = SpringState {
                position: retargeted.position + target + 4.0,
                velocity: retargeted.velocity,
            };
            let duration = config.settle_time(state, target, 0.001);
            assert_ne!(duration, Duration::MAX);
            for after in [0.0, 0.0001, 0.001, 0.01, 0.25, 1.0] {
                let sampled = config.step(state, target, duration.as_secs_f32() + after);
                assert!(
                    config.is_settled(sampled, target, 0.001),
                    "{damping}, {target}, {duration:?}, +{after}: {sampled:?}"
                );
            }
        }
    }
}

#[test]
fn settle_time_respects_cancelling_position_contributions() {
    let config = SpringConfig::new(100.0, 40.0, 1.0);
    for sign in [-1.0, 1.0] {
        let target = 4.0 * sign;
        let initial = SpringState {
            position: 5.0 * sign,
            velocity: -40.0 * sign,
        };
        let duration = config.settle_time(initial, target, 0.001);
        assert_ne!(duration, Duration::MAX);
        for after in [0.0, 0.0001, 0.001, 0.01, 0.25, 1.0] {
            let sampled = config.step(initial, target, duration.as_secs_f32() + after);
            assert!(
                config.is_settled(sampled, target, 0.001),
                "{sign}, {duration:?}, +{after}: {sampled:?}"
            );
        }
    }
}

#[test]
fn undamped_spring_never_settles() {
    let config = SpringConfig::new(100.0, 0.0, 1.0);
    assert_eq!(
        config.settle_time(
            SpringState {
                position: 0.0,
                velocity: 0.0,
            },
            1.0,
            0.001,
        ),
        Duration::MAX
    );
}

#[test]
fn sampled_easing_has_exact_endpoints_and_can_overshoot() {
    let config = SpringConfig::new(100.0, 6.0, 1.0);
    let (duration, easing) = sampled_easing(config, 0.001);

    assert_ne!(duration, Duration::MAX);
    assert_eq!(easing(0.0), 0.0);
    assert_eq!(easing(1.0), 1.0);
    assert!((1..100).any(|step| easing(step as f32 / 100.0) > 1.0));
}

fn multiply(left: [[f32; 2]; 2], right: [[f32; 2]; 2]) -> [[f32; 2]; 2] {
    [
        [
            left[0][0] * right[0][0] + left[0][1] * right[1][0],
            left[0][0] * right[0][1] + left[0][1] * right[1][1],
        ],
        [
            left[1][0] * right[0][0] + left[1][1] * right[1][0],
            left[1][0] * right[0][1] + left[1][1] * right[1][1],
        ],
    ]
}

#[test]
fn scaled_physical_configs_preserve_finite_canonical_parameters() {
    for damping in [4.0, 20.0, 40.0] {
        let reference = SpringConfig::new(100.0, damping, 1.0).canonical();
        assert_eq!(reference, (10.0, damping / 20.0));

        for scale in [1.0e-30, 1.0, 1.0e30] {
            let config = SpringConfig::new(100.0 * scale, damping * scale, scale);
            let (frequency, ratio) = config.canonical();

            assert!(frequency.is_finite(), "{damping}, {scale}: {frequency}");
            assert!(ratio.is_finite(), "{damping}, {scale}: {ratio}");
            assert!(
                (frequency - reference.0).abs() < EPSILON,
                "{damping}, {scale}: {frequency} != {}",
                reference.0
            );
            assert!(
                (ratio - reference.1).abs() < EPSILON,
                "{damping}, {scale}: {ratio} != {}",
                reference.1
            );
        }
    }
}

#[test]
fn scaled_physical_configs_preserve_finite_propagators_and_states() {
    let initial = SpringState {
        position: -3.0,
        velocity: 5.0,
    };
    let target = 7.0;

    for damping in [4.0, 20.0, 40.0] {
        let reference = SpringConfig::new(100.0, damping, 1.0);
        for scale in [1.0e-30, 1.0, 1.0e30] {
            let config = SpringConfig::new(100.0 * scale, damping * scale, scale);
            for delta_time in [0.0, 0.013, 0.034, 0.25, 2.0] {
                let expected_matrix = reference.propagator(delta_time);
                let matrix = config.propagator(delta_time);
                for row in 0..2 {
                    for column in 0..2 {
                        let actual = matrix[row][column];
                        let expected = expected_matrix[row][column];
                        assert!(
                            actual.is_finite() && (actual - expected).abs() < EPSILON,
                            "{damping}, {scale}, {delta_time}: {matrix:?} != {expected_matrix:?}"
                        );
                    }
                }

                for (actual, expected) in [
                    (
                        config.step(initial, target, delta_time),
                        reference.step(initial, target, delta_time),
                    ),
                    (
                        config.step_ramp(initial, target, 3.0, delta_time),
                        reference.step_ramp(initial, target, 3.0, delta_time),
                    ),
                ] {
                    assert!(
                        actual.position.is_finite()
                            && (actual.position - expected.position).abs() < EPSILON,
                        "{damping}, {scale}, {delta_time}: {actual:?} != {expected:?}"
                    );
                    assert!(
                        actual.velocity.is_finite()
                            && (actual.velocity - expected.velocity).abs() < EPSILON,
                        "{damping}, {scale}, {delta_time}: {actual:?} != {expected:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn scaled_physical_configs_preserve_finite_conservative_settle_times() {
    let initial = SpringState {
        position: -2.0,
        velocity: 4.0,
    };
    let target = 3.0;

    for damping in [4.0, 20.0, 40.0] {
        let reference = SpringConfig::new(100.0, damping, 1.0);
        let expected = reference.settle_time(initial, target, 0.001);
        assert_ne!(expected, Duration::MAX);

        for scale in [1.0e-30, 1.0, 1.0e30] {
            let config = SpringConfig::new(100.0 * scale, damping * scale, scale);
            let actual = config.settle_time(initial, target, 0.001);

            assert_ne!(actual, Duration::MAX, "{damping}, {scale}");
            assert!(
                actual.abs_diff(expected).as_secs_f32() < EPSILON,
                "{damping}, {scale}: {actual:?} != {expected:?}"
            );
            for additional_time in [0.0, 0.1, 1.0] {
                let state = config.step(initial, target, actual.as_secs_f32() + additional_time);
                assert!(state.position.is_finite() && state.velocity.is_finite());
                assert!(
                    config.is_settled(state, target, 0.001),
                    "{damping}, {scale}: {actual:?} + {additional_time} produced {state:?}"
                );
            }
        }
    }
}
