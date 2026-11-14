use open_gpui::{
    Animation, AnimationExt as _, Easing, IntoElement as _, ParentElement as _, SubtreeTransform,
    SubtreeTransformExt as _, Svg, TimedTransformation, Transformation, div, heartbeat, percentage,
    point, px, size, svg,
};
use std::time::Duration;

#[test]
fn ordinary_svg_consumers_combine_raster_poses_animation_and_interactive_geometry() {
    let period = Duration::from_secs(2);
    let icon: Svg = svg().with_timed_transformation(
        TimedTransformation::rotation(period, Transformation::alpha(0.8))
            .with_max_fps(24.)
            .with_rotation_easing(Easing::Linear),
    );
    let _ = div()
        .child(icon)
        .with_subtree_transform(
            SubtreeTransform::try_new(
                size(1.25, 0.9),
                point(px(8.), px(-4.)),
                open_gpui::SubtreeTransformOrigin::CENTER,
            )
            .unwrap(),
        )
        .into_any_element();

    let _ = svg()
        .with_timed_transformation(
            TimedTransformation::new(period)
                .from(Transformation::default())
                .to(Transformation::scale(size(1.1, 1.2))
                    .with_translation(point(px(2.), px(3.)))
                    .with_alpha(0.5))
                .with_scale_easing(Easing::Heartbeat)
                .with_translation_easing(Easing::EaseInOut)
                .with_alpha_easing(Easing::Quadratic)
                .alternate(),
        )
        .with_animation(
            "ordinary-svg",
            Animation::new(period).repeat_synced(),
            |icon, phase| icon.with_transformation(Transformation::rotate(percentage(phase))),
        )
        .into_any_element();
}

#[test]
fn public_heartbeat_has_two_distinct_beats_and_a_rest() {
    assert!(heartbeat(0.) < 0.001);
    assert!((heartbeat(0.2) - 1.).abs() < 0.001);
    assert!((heartbeat(0.4) - 0.58).abs() < 0.001);
    assert!(heartbeat(0.8) < 0.001);
    assert!(heartbeat(1.) < 0.001);
    let _: Easing = Easing::EaseOutQuint;
}
