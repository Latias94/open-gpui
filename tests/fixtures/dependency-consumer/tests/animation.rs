//! Ordinary public animation consumers without GPUI test-support or a profiler requirement.
//! Builder/value/easing checks and element construction execute. Explicitly unexecuted
//! App/Window closures only check public types and do not qualify scheduling or rendering.

use open_gpui::{
    Animation, AnimationElement, AnimationExt as _, AnyElement, App, Div, InteractiveElement as _,
    ParentElement, Stateful, Styled as _, Window, bounce, div, ease_in_out, ease_out_quint, linear,
    pulsating_between, px, quadratic,
};
use std::time::Duration;

#[test]
fn downstream_animation_builders_preserve_official_public_values_and_finite_overshoot() {
    let duration = Duration::from_millis(750);
    let default = Animation::new(duration);
    assert_eq!(default.duration, duration);
    assert!(default.oneshot);
    assert!(!default.synced);
    assert_eq!(default.max_fps, None);
    assert_eq!((default.easing)(0.25), 0.25);
    let ordinary = default.clone().repeat();
    assert!(!ordinary.oneshot);
    assert!(!ordinary.synced);
    let synced = default
        .repeat_synced()
        .with_max_fps(24.)
        .with_easing(|value| value * 1.25);
    assert!(!synced.oneshot);
    assert!(synced.synced);
    assert_eq!(synced.duration, duration);
    assert_eq!(synced.max_fps, Some(24.));
    assert_eq!((synced.easing)(1.), 1.25);
    let cloned = synced.clone();
    assert_eq!((cloned.easing)(0.5), 0.625);
    for cap in [0., -1., f32::INFINITY, f32::NEG_INFINITY, f32::NAN] {
        let animation = Animation::new(duration).with_max_fps(cap);
        assert_eq!(animation.max_fps.unwrap().to_bits(), cap.to_bits());
    }
}

#[test]
fn downstream_public_easing_functions_execute_without_an_app() {
    assert_eq!(linear(0.25), 0.25);
    assert_eq!(quadratic(0.5), 0.25);
    assert_eq!(ease_in_out(0.), 0.);
    assert_eq!(ease_in_out(0.5), 0.5);
    assert_eq!(ease_in_out(1.), 1.);
    assert_eq!(ease_out_quint()(0.5), 0.96875);
    assert_eq!(bounce(linear)(0.), 0.);
    assert_eq!(bounce(linear)(0.5), 1.);
    assert_eq!(bounce(linear)(1.), 0.);
    let pulse = pulsating_between(0.2, 0.8);
    for phase in [0., 0.125, 0.25, 0.5, 0.75, 1.] {
        let value = pulse(phase);
        assert!((0.19999..=0.80001).contains(&value));
    }
}

fn accepts_parent(element: impl ParentElement + open_gpui::IntoElement) -> AnyElement {
    element.into_any_element()
}

#[test]
fn downstream_animation_parents_construct_children_and_chains_without_a_window() {
    let animated: AnimationElement<Stateful<Div>> = div()
        .id("downstream-parent")
        .with_animation(
            "downstream-animation",
            Animation::new(Duration::from_secs(1))
                .repeat_synced()
                .with_max_fps(30.),
            |element, value| element.w(px(value * 100.)),
        )
        .child(div().size(px(4.)))
        .children([div(), div()])
        .map_element(|element| element.h(px(24.)));
    let _: AnyElement = accepts_parent(animated);
    let chained: AnimationElement<Div> = div()
        .with_animations(
            "downstream-chain",
            vec![
                Animation::new(Duration::from_millis(100)),
                Animation::new(Duration::from_millis(200)).repeat(),
            ],
            |element, index, value| element.w(px(index as f32 + value)),
        )
        .child(div());
    let _: AnyElement = accepts_parent(chained);
}

#[test]
fn downstream_app_and_animation_signatures_type_check_in_explicitly_unexecuted_closures() {
    let _: fn(Duration) -> Animation = Animation::new;
    let _: fn(Animation) -> Animation = Animation::repeat;
    let _: fn(Animation) -> Animation = Animation::repeat_synced;
    let _: fn(Animation, f32) -> Animation = Animation::with_max_fps;
    let _: fn(&App) -> bool = App::reduce_motion;
    let _: fn(&mut App, bool) = App::set_reduce_motion;
    let _unexecuted_app_window_consumer: fn(&mut App, &mut Window) -> (bool, bool) =
        |app, window| {
            let before: bool = app.reduce_motion();
            app.set_reduce_motion(!before);
            window.request_animation_frame();
            (before, app.reduce_motion())
        };
}
