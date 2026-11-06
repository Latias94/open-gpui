use super::*;
use crate::{
    GpuClipShape, Scene, SubtreeClipExt, SubtreeTransformExt, SubtreeTransformOrigin,
    TestAppContext, canvas, div, red,
};

fn assert_same_geometry(actual: &Underline, expected: &Underline) {
    assert_eq!(actual.bounds, expected.bounds);
    assert_eq!(actual.transform, expected.transform);
    assert_eq!(actual.thickness, expected.thickness);
    assert_eq!(actual.wavy, expected.wavy);
    assert_eq!(actual.color, expected.color);
}

fn clips<'a>(scene: &'a Scene, underline: &Underline) -> &'a [GpuClipShape] {
    let start = underline.clip.first_clip as usize;
    let end = start + underline.clip.clip_count as usize;
    &scene.clip_shapes()[start..end]
}

fn assert_device_bounds(actual: Bounds<ScaledPixels>, expected: Bounds<ScaledPixels>) {
    for (actual, expected) in [
        (actual.origin.x, expected.origin.x),
        (actual.origin.y, expected.origin.y),
        (actual.size.width, expected.size.width),
        (actual.size.height, expected.size.height),
    ] {
        assert!(
            (actual.0 - expected.0).abs() < 0.0001,
            "device coordinate {actual:?} differs from {expected:?}"
        );
    }
}

#[open_gpui::test]
fn underline_bounds_and_segments_share_projected_geometry(cx: &mut TestAppContext) {
    for (scale, identity_bounds, nested_bounds) in [
        (1.0, [7.0, 10.0, 47.0, 1.0], [22.0, 10.0, 109.0, 1.0]),
        (1.25, [9.0, 12.0, 59.0, 2.0], [27.0, 13.0, 136.0, 1.0]),
        (1.5, [11.0, 15.0, 71.0, 2.0], [33.0, 16.0, 164.0, 1.0]),
        (2.0, [15.0, 19.0, 95.0, 3.0], [44.0, 21.0, 218.0, 2.0]),
        (3.0, [22.0, 29.0, 142.0, 4.0], [66.0, 31.0, 327.0, 3.0]),
    ] {
        for nested in [false, true] {
            let (inner, outer) = if nested {
                (
                    SubtreeTransform::try_new(
                        size(1.15, 1.4),
                        point(px(1.1), px(3.2)),
                        SubtreeTransformOrigin::TOP_LEFT,
                    )
                    .unwrap(),
                    SubtreeTransform::try_new(
                        size(2.0, 0.5),
                        point(px(3.0), px(2.0)),
                        SubtreeTransformOrigin::TOP_LEFT,
                    )
                    .unwrap(),
                )
            } else {
                (SubtreeTransform::IDENTITY, SubtreeTransform::IDENTITY)
            };
            for wavy in [false, true] {
                let origin = point(px(7.3), px(9.7));
                let width = px(47.4);
                let style = UnderlineStyle {
                    thickness: px(1.35),
                    color: Some(red().opacity(0.8)),
                    wavy,
                };
                let expected = if nested {
                    nested_bounds
                } else {
                    identity_bounds
                };
                let visual = cx.add_empty_window();
                visual.update(|window, _| window.scale_factor = scale);
                visual.draw(Point::default(), size(px(200.0), px(100.0)), move |_, _| {
                    div().size_full().opacity(0.5).child(
                        canvas(
                            move |_, window, _| window.underline_bounds(origin, width, &style),
                            move |_, prepaint_bounds, window, _| {
                                let bounds = window.underline_bounds(origin, width, &style);
                                assert_eq!(bounds, prepaint_bounds);
                                assert_device_bounds(
                                    bounds,
                                    Bounds::new(
                                        point(ScaledPixels(expected[0]), ScaledPixels(expected[1])),
                                        size(
                                            ScaledPixels(expected[2]),
                                            ScaledPixels(
                                                expected[3] * if wavy { 3.0 } else { 1.0 },
                                            ),
                                        ),
                                    ),
                                );
                                for opacity in [1.0, 0.4] {
                                    window.with_element_opacity(Some(opacity), |window| {
                                        let first = window.next_frame.scene.underlines.len();
                                        window.paint_underline(origin, width, &style);
                                        window.paint_underline_with_exclusions(
                                            origin,
                                            width,
                                            &style,
                                            &[],
                                        );
                                        let scene = &window.next_frame.scene;
                                        assert_eq!(scene.underlines.len(), first + 2);
                                        let baseline = scene.underlines[first];
                                        let empty = &scene.underlines[first + 1];
                                        assert_same_geometry(empty, &baseline);
                                        assert_eq!(empty.clip, baseline.clip);
                                        assert_eq!(
                                            baseline
                                                .transform
                                                .try_project_bounds(baseline.bounds)
                                                .unwrap(),
                                            bounds
                                        );
                                        assert_eq!(
                                            baseline.color,
                                            style.color.unwrap().opacity(0.5 * opacity)
                                        );
                                        assert_eq!(
                                            baseline.bounds,
                                            Bounds::new(
                                                origin.scale(scale),
                                                size(
                                                    width.scale(scale),
                                                    ScaledPixels(
                                                        style.thickness.0
                                                            * scale
                                                            * if wavy { 3.0 } else { 1.0 },
                                                    ),
                                                ),
                                            )
                                        );

                                        let left = bounds.left().0;
                                        let exclusions = [
                                            ScaledPixels(left + 7.25)..ScaledPixels(left + 13.5),
                                            ScaledPixels(left + 22.75)..ScaledPixels(left + 28.25),
                                        ];
                                        window.paint_underline_with_exclusions(
                                            origin,
                                            width,
                                            &style,
                                            &exclusions,
                                        );
                                        let scene = &window.next_frame.scene;
                                        let segments = &scene.underlines[first + 2..];
                                        assert_eq!(segments.len(), 3);
                                        for (segment, (start, end)) in segments.iter().zip([
                                            (bounds.left(), exclusions[0].start),
                                            (exclusions[0].end, exclusions[1].start),
                                            (exclusions[1].end, bounds.right()),
                                        ]) {
                                            assert_same_geometry(segment, &baseline);
                                            assert_eq!(
                                                segment
                                                    .transform
                                                    .try_project_bounds(segment.bounds)
                                                    .unwrap(),
                                                bounds
                                            );
                                            let segment_clips = clips(scene, segment);
                                            assert_eq!(
                                                &segment_clips[..segment_clips.len() - 1],
                                                clips(scene, &baseline)
                                            );
                                            assert_device_bounds(
                                                segment_clips.last().unwrap().bounds,
                                                Bounds::from_corners(
                                                    point(start, bounds.top()),
                                                    point(end, bounds.bottom()),
                                                ),
                                            );
                                        }
                                        window.paint_underline(origin, width, &style);
                                        assert_eq!(
                                            window.next_frame.scene.underlines.last().unwrap().clip,
                                            baseline.clip
                                        );
                                    });
                                }
                            },
                        )
                        .size_full()
                        .with_subtree_transform(inner)
                        .with_subtree_transform(outer),
                    )
                });
                visual.update(|window, _| {
                    assert_eq!(window.rendered_frame.scene.underlines.len(), 12)
                });
            }
        }
    }
}

#[open_gpui::test]
fn underline_exclusions_normalize_actual_emitted_spans(cx: &mut TestAppContext) {
    const CASES: &[(&str, &[(f32, f32)], &[(f32, f32)])] = &[
        ("empty", &[], &[(10.0, 50.0)]),
        ("covering", &[(0.0, 100.0)], &[]),
        ("exactly covering", &[(10.0, 50.0)], &[]),
        (
            "overlapping",
            &[(20.0, 35.0), (15.0, 25.0)],
            &[(10.0, 15.0), (35.0, 50.0)],
        ),
        (
            "unsorted",
            &[(40.0, 45.0), (20.0, 25.0)],
            &[(10.0, 20.0), (25.0, 40.0), (45.0, 50.0)],
        ),
        (
            "adjacent",
            &[(20.0, 25.0), (25.0, 30.0)],
            &[(10.0, 20.0), (30.0, 50.0)],
        ),
        (
            "duplicate",
            &[(20.0, 30.0), (20.0, 30.0)],
            &[(10.0, 20.0), (30.0, 50.0)],
        ),
        ("outside", &[(-100.0, 0.0), (60.0, 100.0)], &[(10.0, 50.0)]),
        (
            "touching edges",
            &[(0.0, 10.0), (50.0, 60.0)],
            &[(10.0, 50.0)],
        ),
        (
            "partial edges",
            &[(-5.0, 15.0), (45.0, 60.0)],
            &[(15.0, 45.0)],
        ),
        ("reversed", &[(35.0, 20.0)], &[(10.0, 50.0)]),
        ("zero length", &[(25.0, 25.0)], &[(10.0, 50.0)]),
        (
            "non-finite",
            &[
                (f32::NAN, 25.0),
                (25.0, f32::NAN),
                (f32::NEG_INFINITY, 20.0),
                (35.0, f32::INFINITY),
            ],
            &[(10.0, 50.0)],
        ),
        (
            "mixed invalid",
            &[(f32::NAN, 25.0), (40.0, 30.0), (20.0, 30.0)],
            &[(10.0, 20.0), (30.0, 50.0)],
        ),
        (
            "covering union",
            &[(30.0, 80.0), (-1.0, 25.0), (20.0, 40.0)],
            &[],
        ),
        (
            "fractional gap",
            &[(10.0, 20.0), (20.25, 50.0)],
            &[(20.0, 20.25)],
        ),
    ];
    for wavy in [false, true] {
        let visual = cx.add_empty_window();
        visual.update(|window, _| window.scale_factor = 1.0);
        visual.draw(Point::default(), size(px(100.0), px(100.0)), move |_, _| {
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    let origin = point(px(10.0), px(10.0));
                    let style = UnderlineStyle {
                        thickness: px(2.0),
                        color: None,
                        wavy,
                    };
                    window.paint_underline(origin, px(40.0), &style);
                    let baseline = *window.next_frame.scene.underlines.last().unwrap();
                    for &(name, exclusions, visible) in CASES {
                        let exclusions = exclusions
                            .iter()
                            .map(|&(start, end)| ScaledPixels(start)..ScaledPixels(end))
                            .collect::<Vec<_>>();
                        let first = window.next_frame.scene.underlines.len();
                        window.paint_underline_with_exclusions(
                            origin,
                            px(40.0),
                            &style,
                            &exclusions,
                        );
                        let scene = &window.next_frame.scene;
                        let emitted = &scene.underlines[first..];
                        assert_eq!(emitted.len(), visible.len(), "{name}");
                        for (segment, &(start, end)) in emitted.iter().zip(visible) {
                            assert_same_geometry(segment, &baseline);
                            let expected_bounds = Bounds::new(
                                point(ScaledPixels(start), ScaledPixels(10.0)),
                                size(
                                    ScaledPixels(end - start),
                                    ScaledPixels(if wavy { 6.0 } else { 2.0 }),
                                ),
                            );
                            let visual_bounds = segment
                                .transform
                                .try_project_bounds(segment.bounds)
                                .unwrap()
                                .intersect(&segment.clip.conservative_bounds);
                            assert_device_bounds(visual_bounds, expected_bounds);
                            if visible == [(10.0, 50.0)] {
                                assert_eq!(segment.clip, baseline.clip, "{name}");
                            } else {
                                let segment_clips = clips(scene, segment);
                                let baseline_clips = clips(scene, &baseline);
                                assert_eq!(segment_clips.len(), baseline_clips.len() + 1);
                                assert_eq!(&segment_clips[..baseline_clips.len()], baseline_clips);
                                assert_device_bounds(
                                    segment_clips.last().unwrap().bounds,
                                    expected_bounds,
                                );
                            }
                        }
                    }
                },
            )
            .size_full()
        });
    }
}

#[open_gpui::test]
fn underline_segments_retain_rounded_ancestor_clips_and_scene_replay(cx: &mut TestAppContext) {
    for scale in [1.0, 1.25, 1.5, 2.0, 3.0] {
        let parent = SubtreeTransform::try_new(
            size(1.5, 1.25),
            point(px(4.25), px(6.5)),
            SubtreeTransformOrigin::TOP_LEFT,
        )
        .unwrap();
        let child = SubtreeTransform::try_new(
            size(1.1, 0.8),
            point(px(1.2), px(0.4)),
            SubtreeTransformOrigin::TOP_LEFT,
        )
        .unwrap();
        let radius = size(px(20.0), px(20.0));
        let rounded = SubtreeClip::try_own_rounded_border_box(Corners {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        })
        .unwrap();
        let visual = cx.add_empty_window();
        visual.update(|window, _| window.scale_factor = scale);
        visual.draw(Point::default(), size(px(100.0), px(40.0)), move |_, _| {
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    let inherited = window.clip_stack();
                    assert!(!inherited.contains(point(px(4.35), px(6.6))));
                    let origin = point(px(-10.0), px(0.25));
                    let style = UnderlineStyle {
                        thickness: px(4.0),
                        color: Some(red()),
                        wavy: true,
                    };
                    let bounds = window.underline_bounds(origin, px(120.0), &style);
                    let clip_bounds = inherited.conservative_bounds().scale(scale);
                    assert!(bounds.left() < clip_bounds.left());
                    assert!(bounds.right() > clip_bounds.right());
                    window.paint_underline(origin, px(120.0), &style);
                    let baseline = *window.next_frame.scene.underlines.last().unwrap();
                    let baseline_clips = clips(&window.next_frame.scene, &baseline).to_vec();
                    assert_eq!(baseline_clips.len(), 2);
                    let rounded = baseline_clips.last().unwrap();
                    assert_device_bounds(
                        rounded.bounds,
                        Bounds::new(
                            point(ScaledPixels(4.25 * scale), ScaledPixels(6.5 * scale)),
                            size(ScaledPixels(150.0 * scale), ScaledPixels(50.0 * scale)),
                        ),
                    );
                    assert_eq!(rounded.radii_x.top_left, ScaledPixels(30.0 * scale));
                    assert_eq!(rounded.radii_y.top_left, ScaledPixels(25.0 * scale));
                    let visible = bounds.intersect(&clip_bounds);
                    let exclusions = [
                        ScaledPixels(visible.left().0 + 25.25)
                            ..ScaledPixels(visible.left().0 + 30.5),
                        ScaledPixels(visible.right().0 - 20.75)
                            ..ScaledPixels(visible.right().0 - 10.5),
                    ];
                    window.paint_underline_with_exclusions(origin, px(120.0), &style, &exclusions);
                    let scene = &window.next_frame.scene;
                    assert_eq!(scene.underlines.len(), 4);
                    for (segment, (start, end)) in scene.underlines[1..].iter().zip([
                        (visible.left(), exclusions[0].start),
                        (exclusions[0].end, exclusions[1].start),
                        (exclusions[1].end, visible.right()),
                    ]) {
                        assert_same_geometry(segment, &baseline);
                        let segment_clips = clips(scene, segment);
                        assert_eq!(&segment_clips[..baseline_clips.len()], baseline_clips);
                        assert_eq!(segment_clips.len(), baseline_clips.len() + 1);
                        let scissor = segment_clips.last().unwrap();
                        assert_eq!(scissor.radii_x, Corners::default());
                        assert_eq!(scissor.radii_y, Corners::default());
                        assert_device_bounds(
                            scissor.bounds,
                            Bounds::from_corners(
                                point(start, visible.top()),
                                point(end, visible.bottom()),
                            ),
                        );
                    }
                    assert_eq!(window.clip_stack(), inherited);
                    window.paint_underline_with_exclusions(
                        origin,
                        px(120.0),
                        &style,
                        &[clip_bounds.left()..clip_bounds.right()],
                    );
                    assert_eq!(window.next_frame.scene.underlines.len(), 4);
                    assert_eq!(window.clip_stack(), inherited);
                    window.paint_underline(origin, px(120.0), &style);
                    assert_eq!(
                        window.next_frame.scene.underlines.last().unwrap().clip,
                        baseline.clip
                    );
                },
            )
            .size_full()
            .with_subtree_transform(child)
            .with_subtree_clip(rounded)
            .with_subtree_transform(parent)
        });
        visual.update(|window, _| {
            let scene = &window.rendered_frame.scene;
            assert_eq!(scene.underlines.len(), 5);
            let mut replayed = Scene::default();
            replayed
                .insert_primitive_scoped(
                    scene.underlines[0],
                    &ClipStackSnapshot::root(Bounds::new(
                        Point::default(),
                        size(px(500.0), px(500.0)),
                    )),
                    scale,
                    None,
                )
                .unwrap();
            replayed
                .replay(0..scene.journal_len(), scene, None)
                .unwrap();
            replayed.finish();
            assert_eq!(replayed.underlines.len(), scene.underlines.len() + 1);
            for (actual, expected) in replayed.underlines[1..].iter().zip(&scene.underlines) {
                assert_same_geometry(actual, expected);
                assert_eq!(clips(&replayed, actual), clips(scene, expected));
            }
        });
    }
}

#[open_gpui::test]
fn invalid_underline_projection_suppresses_its_scope_and_preserves_siblings(
    cx: &mut TestAppContext,
) {
    for during_prepaint in [false, true] {
        let visual = cx.add_empty_window();
        visual.draw(Point::default(), size(px(100.0), px(100.0)), move |_, _| {
            let style = UnderlineStyle {
                thickness: px(1.0),
                color: None,
                wavy: true,
            };
            div()
                .size_full()
                .child(
                    canvas(
                        move |_, window, _| {
                            if during_prepaint {
                                assert!(
                                    window
                                        .underline_bounds(
                                            point(px(f32::INFINITY), px(10.0)),
                                            px(40.0),
                                            &style,
                                        )
                                        .is_empty()
                                );
                            }
                        },
                        move |_, _, window, _| {
                            window.paint_underline_with_exclusions(
                                point(px(10.0), px(10.0)),
                                px(40.0),
                                &style,
                                &[ScaledPixels(30.0)..ScaledPixels(40.0)],
                            );
                            window.paint_underline_with_exclusions(
                                point(px(f32::NAN), px(10.0)),
                                px(40.0),
                                &style,
                                &[],
                            );
                        },
                    )
                    .size_full()
                    .with_subtree_transform(SubtreeTransform::IDENTITY),
                )
                .child(
                    canvas(
                        |_, _, _| {},
                        move |_, _, window, _| {
                            window.paint_underline(point(px(10.0), px(40.0)), px(40.0), &style);
                        },
                    )
                    .size_full(),
                )
        });
        visual.update(|window, _| {
            let underlines = &window.rendered_frame.scene.underlines;
            assert_eq!(underlines.len(), 1);
            assert_eq!(underlines[0].bounds.origin.y, ScaledPixels(80.0));
            assert_eq!(underlines[0].clip.clip_count, 1);
        });
    }
}

#[open_gpui::test]
fn empty_underline_geometry_emits_no_primitives(cx: &mut TestAppContext) {
    let visual = cx.add_empty_window();
    visual.draw(Point::default(), size(px(100.0), px(100.0)), |_, _| {
        canvas(
            |_, _, _| {},
            |_, _, window, _| {
                for wavy in [false, true] {
                    for (width, thickness) in [(0.0, 1.0), (-1.0, 1.0), (40.0, 0.0)] {
                        let style = UnderlineStyle {
                            thickness: px(thickness),
                            color: None,
                            wavy,
                        };
                        let origin = point(px(10.3), px(20.7));
                        assert!(
                            window
                                .underline_bounds(origin, px(width), &style)
                                .is_empty()
                        );
                        window.paint_underline(origin, px(width), &style);
                        window.paint_underline_with_exclusions(
                            origin,
                            px(width),
                            &style,
                            &[ScaledPixels(15.0)..ScaledPixels(25.0)],
                        );
                    }
                }
                assert!(window.next_frame.scene.underlines.is_empty());
            },
        )
        .size_full()
    });
    visual.update(|window, _| assert!(window.rendered_frame.scene.underlines.is_empty()));
}
