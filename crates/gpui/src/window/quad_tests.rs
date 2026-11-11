use super::*;
use crate::{
    ClipEnvelope, GpuClipShape, PrimitiveBatch, Scene, StyleRefinement, SubtreeClipExt,
    SubtreeTransformExt, SubtreeTransformOrigin, TestAppContext, blue, canvas, div, green, red,
};

fn logical_bounds(left: f32, top: f32, right: f32, bottom: f32) -> Bounds<Pixels> {
    Bounds::from_corners(point(px(left), px(top)), point(px(right), px(bottom)))
}

fn clips(scene: &Scene, clip: ClipEnvelope) -> &[GpuClipShape] {
    let start = clip.first_clip as usize;
    &scene.clip_shapes()[start..start + clip.clip_count as usize]
}

fn raster_coverage(quad: &Quad) -> Bounds<ScaledPixels> {
    quad.transform
        .try_project_bounds(quad.bounds)
        .unwrap()
        .intersect(&quad.clip.conservative_bounds)
}

fn assert_same_raster_quad(actual: &Quad, expected: &Quad) {
    assert_eq!(actual.bounds, expected.bounds);
    assert_eq!(actual.transform, expected.transform);
    assert_eq!(actual.corner_radii, expected.corner_radii);
    assert_eq!(actual.border_widths, expected.border_widths);
    assert_eq!(actual.border_color, expected.border_color);
    assert_eq!(actual.border_style, expected.border_style);
}

#[open_gpui::test]
fn border_only_quad_partitions_snapped_device_coverage(cx: &mut TestAppContext) {
    let visual = cx.add_empty_window();
    visual.update(|window, _| window.scale_factor = 1.0);
    visual.draw(Point::default(), size(px(180.0), px(180.0)), |_, _| {
        canvas(
            |_, _, _| (),
            |_, _, window, _| {
                let bounds = logical_bounds(10.25, 20.25, 100.25, 160.25);
                let radii = Corners {
                    top_left: px(8.5),
                    top_right: px(3.0),
                    bottom_right: px(6.25),
                    bottom_left: px(1.5),
                };
                let widths = Edges {
                    top: px(2.0),
                    right: px(4.0),
                    bottom: px(1.0),
                    left: px(3.0),
                };
                let outline = quad(
                    bounds,
                    radii,
                    transparent_black(),
                    widths,
                    red().opacity(0.6),
                    BorderStyle::Dashed,
                );
                window.paint_quad(outline.clone().background(red().opacity(0.25)));
                window.paint_quad(outline);

                let scene = &window.next_frame.scene;
                assert_eq!(scene.quads.len(), 5);
                assert_eq!(scene.clip_shapes().len(), 1);
                let baseline = &scene.quads[0];
                let strips = &scene.quads[1..];
                let expected = [
                    logical_bounds(10.0, 20.0, 100.0, 30.0),
                    logical_bounds(10.0, 152.0, 100.0, 160.0),
                    logical_bounds(10.0, 30.0, 14.0, 152.0),
                    logical_bounds(95.0, 30.0, 100.0, 152.0),
                ];
                for (index, (strip, expected)) in strips.iter().zip(expected).enumerate() {
                    assert_same_raster_quad(strip, baseline);
                    assert!(strip.background.is_transparent());
                    assert_eq!(strip.clip.first_clip, baseline.clip.first_clip);
                    assert_eq!(strip.clip.clip_count, baseline.clip.clip_count);
                    let coverage = raster_coverage(strip);
                    assert_eq!(coverage, expected.scale(1.0));
                    assert!(
                        strips[..index]
                            .iter()
                            .all(|other| { !coverage.intersects(&raster_coverage(other)) })
                    );
                }
            },
        )
        .size_full()
    });
}

#[open_gpui::test]
fn border_only_quad_preserves_anisotropic_antialias_coverage(cx: &mut TestAppContext) {
    let visual = cx.add_empty_window();
    visual.update(|window, _| window.scale_factor = 1.25);
    let transform = SubtreeTransform::try_new(
        size(4.0, 0.5),
        point(px(3.2), px(9.6)),
        SubtreeTransformOrigin::TOP_LEFT,
    )
    .unwrap();
    visual.draw(Point::default(), size(px(340.0), px(120.0)), move |_, _| {
        canvas(
            |_, _, _| (),
            |_, _, window, _| {
                window.paint_quad(
                    outline(
                        logical_bounds(12.0, 16.0, 76.0, 76.0),
                        red(),
                        BorderStyle::Dashed,
                    )
                    .border_widths(Edges {
                        top: px(2.3),
                        right: px(1.4),
                        bottom: px(1.0),
                        left: px(1.0),
                    }),
                );

                let strips = &window.next_frame.scene.quads;
                assert_eq!(strips.len(), 4);
                assert_eq!(strips[0].border_widths.left, ScaledPixels(1.25));
                assert!(strips[0].transform.scale().width > 3.0);
                assert!(strips[0].transform.scale().height < 1.0);
                let coverage_count = |x, y| {
                    let sample = point(ScaledPixels(x), ScaledPixels(y));
                    strips
                        .iter()
                        .filter(|strip| raster_coverage(strip).contains(&sample))
                        .count()
                };
                assert_eq!(coverage_count(70.5, 40.5), 1);
                assert_eq!(coverage_count(377.5, 40.5), 1);
                assert_eq!(coverage_count(150.5, 40.5), 0);
                for y in [23.5, 24.5, 56.5, 57.5] {
                    assert_eq!(coverage_count(65.5, y), 1);
                }
            },
        )
        .size_full()
        .with_subtree_transform(transform)
    });
}

#[open_gpui::test]
fn border_only_quad_keeps_original_vertices_when_large_local_origin_cancels(
    cx: &mut TestAppContext,
) {
    let visual = cx.add_empty_window();
    visual.update(|window, _| window.scale_factor = 1.0);
    let transform = SubtreeTransform::try_new(
        size(1.0 / 10001.0, 1.0 / 8001.0),
        point(px(8.0), px(-4185.78)),
        SubtreeTransformOrigin::TOP_LEFT,
    )
    .unwrap();
    visual.draw(Point::default(), size(px(128.0), px(128.0)), move |_, _| {
        canvas(
            |_, _, _| (),
            |_, (), window, _| {
                window.paint_quad(
                    outline(
                        Bounds::new(
                            point(px(0.0), px(33_554_432.0)),
                            size(px(200_020.0), px(160_020.0)),
                        ),
                        red(),
                        BorderStyle::Dashed,
                    )
                    .border_widths(Edges {
                        top: px(8001.0),
                        right: px(10001.0),
                        bottom: px(8001.0),
                        left: px(10001.0),
                    }),
                );
                let [quad] = window.next_frame.scene.quads.as_slice() else {
                    panic!("visible large-local-coordinate geometry must remain a single quad");
                };
                assert_eq!(
                    raster_coverage(quad),
                    logical_bounds(8.0, 8.0, 28.0, 28.0).scale(1.0)
                );
                assert_eq!(quad.clip.clip_count, 1);
            },
        )
        .size_full()
        .with_subtree_transform(transform)
    });
}

#[open_gpui::test]
fn border_only_quad_handles_oversized_corners_and_full_quad_fallbacks(cx: &mut TestAppContext) {
    let visual = cx.add_empty_window();
    visual.update(|window, _| window.scale_factor = 1.3);
    visual.draw(Point::default(), size(px(200.0), px(100.0)), |_, _| {
        canvas(
            |_, _, _| (),
            |_, _, window, _| {
                let wide = outline(
                    logical_bounds(10.0, 10.0, 170.0, 34.0),
                    red(),
                    BorderStyle::Solid,
                );
                window.paint_quad(wide.clone().corner_radii(Corners {
                    top_left: px(32.0),
                    ..Default::default()
                }));
                let scene = &window.next_frame.scene;
                assert_eq!(scene.quads.len(), 4);
                assert!(scene.quads.iter().all(|strip| {
                    strip.corner_radii.top_left == px(32.0).scale(1.3) && strip.clip.clip_count == 1
                }));
                let sample = point(ScaledPixels(25.5), ScaledPixels(28.5));
                assert_eq!(
                    scene
                        .quads
                        .iter()
                        .filter(|strip| raster_coverage(strip).contains(&sample))
                        .count(),
                    1
                );

                for fallback in [
                    wide.clone().corner_radii(200.0),
                    wide.clone().border_widths(20.0),
                    wide.clone().border_widths(0.0),
                    wide.clone().corner_radii(-2.0),
                    wide.clone().background(red().opacity(0.01)),
                    wide.border_color(transparent_black()),
                ] {
                    let first = window.next_frame.scene.quads.len();
                    window.paint_quad(fallback);
                    let scene = &window.next_frame.scene;
                    assert_eq!(scene.quads.len(), first + 1);
                    assert_eq!(scene.quads[first].clip.clip_count, 1);
                }
            },
        )
        .size_full()
    });
}

#[open_gpui::test]
fn border_only_quad_is_culled_by_an_interior_clip(cx: &mut TestAppContext) {
    let visual = cx.add_empty_window();
    let clip = SubtreeClip::try_rect(logical_bounds(30.5, 30.5, 40.5, 40.5)).unwrap();
    visual.draw(Point::default(), size(px(120.0), px(120.0)), move |_, _| {
        canvas(
            |_, _, _| (),
            |_, _, window, _| {
                let inherited = window.clip_stack();
                let bounds = logical_bounds(10.0, 10.0, 110.0, 110.0);
                window.paint_quad(outline(bounds, red(), BorderStyle::Dashed));
                assert!(window.next_frame.scene.quads.is_empty());
                assert_eq!(window.clip_stack(), inherited);
                window.paint_quad(fill(bounds, red()));
                let scene = &window.next_frame.scene;
                assert_eq!(scene.quads.len(), 1);
                assert_eq!(scene.quads[0].clip.clip_count, 2);
            },
        )
        .size_full()
        .with_subtree_clip(clip)
    });
}

struct CachedBorderOnlyQuad {
    paints: Rc<Cell<usize>>,
}

impl Render for CachedBorderOnlyQuad {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let paints = self.paints.clone();
        canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                paints.set(paints.get() + 1);
                let origin = point(px(25.0), px(10.0));
                let width = px(35.0);
                let before = UnderlineStyle {
                    thickness: px(2.0),
                    color: Some(blue()),
                    wavy: false,
                };
                window.paint_underline(origin, width, &before);
                window.paint_quad(outline(
                    logical_bounds(10.0, 10.0, 100.0, 100.0),
                    red().opacity(0.6),
                    BorderStyle::Dashed,
                ));
                window.paint_underline(
                    origin,
                    width,
                    &UnderlineStyle {
                        color: Some(green()),
                        ..before
                    },
                );
            },
        )
        .size_full()
    }
}

struct CachedBorderOnlyQuadRoot {
    child: Entity<CachedBorderOnlyQuad>,
    paint_prefix: bool,
}

impl Render for CachedBorderOnlyQuadRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let paint_prefix = self.paint_prefix;
        let outer_clip = SubtreeClip::try_rounded_rect(
            logical_bounds(10.25, 10.25, 90.0, 90.0),
            Corners::all(px(6.0)).map(|width| size(*width, px(8.0))),
        )
        .unwrap();
        let inner_clip = SubtreeClip::try_rounded_rect(
            logical_bounds(9.75, 9.75, 85.25, 84.75),
            Corners::all(px(4.0)).map(|width| size(*width, px(5.0))),
        )
        .unwrap();
        div()
            .size_full()
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        if paint_prefix {
                            window.paint_quad(fill(logical_bounds(0.0, 0.0, 5.0, 5.0), red()));
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .child(
                AnyView::from(self.child.clone())
                    .cached(StyleRefinement::default().size(px(120.0)))
                    .with_subtree_clip(inner_clip)
                    .with_subtree_clip(outer_clip),
            )
    }
}

fn assert_clipped_border_order(scene: &Scene, scale: f32) -> Vec<Quad> {
    let strips = scene
        .quads
        .iter()
        .filter(|quad| quad.background.is_transparent())
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(strips.len(), 2);
    assert_eq!(scene.underlines.len(), 2);
    let inherited = clips(scene, scene.underlines[0].clip);
    assert_eq!(inherited.len(), 3);
    assert_eq!(inherited[1].radii_x.top_left, px(6.0).scale(scale));
    assert_eq!(inherited[2].radii_y.top_left, px(5.0).scale(scale));
    for strip in &strips {
        assert_eq!(strip.clip.first_clip, scene.underlines[0].clip.first_clip);
        assert_eq!(clips(scene, strip.clip), inherited);
    }

    let sample = point(px(35.0), px(10.75)).scale(scale);
    let mut colors = Vec::new();
    for batch in scene.batches() {
        match batch {
            PrimitiveBatch::Quads(range) => {
                colors.extend(scene.quads[range].iter().filter_map(|quad| {
                    raster_coverage(quad)
                        .contains(&sample)
                        .then_some(quad.border_color)
                }));
            }
            PrimitiveBatch::Underlines(range) => {
                colors.extend(scene.underlines[range].iter().map(|line| line.color));
            }
            _ => unreachable!(),
        }
    }
    assert_eq!(colors, [blue(), red().opacity(0.6), green()]);
    strips
}

#[open_gpui::test]
fn border_only_quad_replays_clips_and_overlapping_primitive_order(cx: &mut TestAppContext) {
    let paints = Rc::new(Cell::new(0));
    let child = cx.new(|_| CachedBorderOnlyQuad {
        paints: paints.clone(),
    });
    let window = cx.add_window(|_, _| CachedBorderOnlyQuadRoot {
        child,
        paint_prefix: false,
    });
    let initial_paints = paints.get();
    assert!(initial_paints > 0);
    let (initial, scale) = cx
        .update_window(window.into(), |_, window, _| {
            (window.rendered_frame.scene.clone(), window.scale_factor())
        })
        .unwrap();
    assert_eq!(initial.clip_shapes().len(), 3);
    let initial_strips = assert_clipped_border_order(&initial, scale);

    window
        .update(cx, |root, _, cx| {
            root.paint_prefix = true;
            cx.notify();
        })
        .unwrap();
    assert_eq!(paints.get(), initial_paints);
    cx.update_window(window.into(), |_, window, _| {
        let scene = &window.rendered_frame.scene;
        assert_eq!(scene.quads.len(), 3);
        assert_eq!(scene.clip_shapes().len(), 4);
        let replayed = assert_clipped_border_order(scene, scale);
        for (actual, expected) in replayed.iter().zip(&initial_strips) {
            assert_same_raster_quad(actual, expected);
            assert_eq!(actual.background, expected.background);
            assert_ne!(actual.clip.first_clip, expected.clip.first_clip);
            assert_eq!(
                actual.clip.conservative_bounds,
                expected.clip.conservative_bounds
            );
            assert_eq!(clips(scene, actual.clip), clips(&initial, expected.clip));
        }
    })
    .unwrap();
}

#[open_gpui::test]
fn border_only_quad_strips_share_late_geometry_invalidation(cx: &mut TestAppContext) {
    let visual = cx.add_empty_window();
    visual.draw(Point::default(), size(px(160.0), px(160.0)), |_, _| {
        div()
            .size_full()
            .child(
                canvas(
                    |_, _, _| (),
                    |_, _, window, _| {
                        window.paint_quad(outline(
                            logical_bounds(10.0, 10.0, 110.0, 110.0),
                            red(),
                            BorderStyle::Solid,
                        ));
                        assert_eq!(window.next_frame.scene.quads.len(), 4);
                        window.paint_quad(fill(
                            Bounds::new(
                                point(px(f32::INFINITY), px(0.0)),
                                size(px(20.0), px(20.0)),
                            ),
                            red(),
                        ));
                    },
                )
                .size_full()
                .with_subtree_transform(SubtreeTransform::IDENTITY),
            )
            .child(
                canvas(
                    |_, _, _| (),
                    |_, _, window, _| {
                        window.paint_quad(fill(logical_bounds(20.0, 20.0, 80.0, 80.0), green()));
                    },
                )
                .size_full(),
            )
    });
    visual.update(|window, _| {
        let scene = &window.rendered_frame.scene;
        assert_eq!(scene.quads.len(), 1);
        assert_eq!(scene.quads[0].background, Background::from(green()));
        assert_eq!(scene.len(), 1);
    });
}
