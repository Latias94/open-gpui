use super::*;
use crate::{FontId, GlyphId, Styled, TestAppContext, canvas, hsla};

fn layout(text: &str, glyphs: &[(usize, f32)], width: f32) -> LineLayout {
    LineLayout {
        font_size: px(16.),
        width: px(width),
        ascent: px(12.),
        descent: px(4.),
        len: text.len(),
        runs: vec![ShapedRun {
            font_id: FontId(0),
            glyphs: glyphs
                .iter()
                .map(|&(index, x)| ShapedGlyph {
                    index,
                    id: GlyphId(0),
                    position: point(px(x), px(0.)),
                    is_emoji: false,
                })
                .collect(),
        }],
    }
}

fn decoration(len: u32, color: Hsla) -> DecorationRun {
    DecorationRun {
        len,
        color,
        background_color: Some(color),
        underline: Some(UnderlineStyle {
            thickness: px(1.),
            color: None,
            wavy: false,
        }),
        strikethrough: None,
    }
}

#[open_gpui::test]
fn underline_callback_preserves_utf8_ranges_resolved_style_and_forwarded_geometry(
    cx: &mut TestAppContext,
) {
    let visual = cx.add_empty_window();
    visual.draw(Point::default(), size(px(150.), px(100.)), |_, _| {
        canvas(
            |_, _, _| {},
            |_, _, window, cx| {
                let red = hsla(0., 1., 0.5, 1.);
                let blue = hsla(0.6, 1., 0.5, 1.);
                let line = ShapedLine {
                    layout: Arc::new(layout("éAB", &[(0, 0.), (2, 10.), (3, 20.)], 30.)),
                    text: "éAB".into(),
                    decoration_runs: smallvec::smallvec![
                        decoration(2, red),
                        decoration(1, red),
                        decoration(1, blue)
                    ],
                };
                let origin = point(px(10.), px(20.));
                let mut observations = Vec::new();
                line.paint_with_underline_handler(
                    origin,
                    px(20.),
                    TextAlign::Center,
                    Some(px(50.)),
                    window,
                    cx,
                    |range, origin, width, style, window| {
                        observations.push((range, origin, width, *style));
                        window.paint_underline(origin, width, style);
                    },
                )
                .unwrap();
                assert_eq!(observations.len(), 2);
                assert_eq!(observations[0].0, 0..3);
                assert_eq!(observations[1].0, 3..4);
                assert_eq!(
                    observations[0].1,
                    point(
                        px(20.),
                        px(20.) + underline_y_offset(px(20.), px(12.), px(4.))
                    )
                );
                assert_eq!(observations[0].2, px(20.));
                assert_eq!(observations[0].3.color, Some(red));
                assert_eq!(observations[1].1.x, px(40.));
                assert_eq!(observations[1].2, px(10.));
                assert_eq!(observations[1].3.color, Some(blue));
                let forwarded = window.next_frame.scene.underlines.clone();
                line.paint(
                    origin,
                    px(20.),
                    TextAlign::Center,
                    Some(px(50.)),
                    window,
                    cx,
                )
                .unwrap();
                for (left, right) in forwarded
                    .iter()
                    .zip(&window.next_frame.scene.underlines[forwarded.len()..])
                {
                    assert_eq!(left.bounds, right.bounds);
                    assert_eq!(left.color, right.color);
                    assert_eq!(left.thickness, right.thickness);
                    assert_eq!(left.transform, right.transform);
                }
            },
        )
        .size_full()
    });
}

#[open_gpui::test]
fn aligned_wrapped_backgrounds_and_decorations_cover_each_actual_row(cx: &mut TestAppContext) {
    for align in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
        for align_width in [20., 80.] {
            let visual = cx.add_empty_window();
            visual.draw(Point::default(), size(px(200.), px(100.)), move |_, _| {
                canvas(
                    |_, _, _| {},
                    move |_, _, window, cx| {
                        let color = hsla(0., 1., 0.5, 1.);
                        let origin = point(px(40.), px(10.));
                        let line = WrappedLine {
                            layout: Arc::new(WrappedLineLayout {
                                unwrapped_layout: Arc::new(layout(
                                    "abcdef",
                                    &[(0, 0.), (1, 10.), (2, 20.), (3, 30.), (4, 40.), (5, 50.)],
                                    60.,
                                )),
                                wrap_boundaries: smallvec::smallvec![WrapBoundary {
                                    run_ix: 0,
                                    glyph_ix: 4
                                }],
                                wrap_width: Some(px(align_width)),
                            }),
                            text: "abcdef".into(),
                            decoration_runs: vec![decoration(6, color)],
                        };
                        let bounds = Some(Bounds::new(origin, size(px(align_width), px(40.))));
                        line.paint_background(origin, px(20.), align, bounds, window, cx)
                            .unwrap();
                        line.paint(origin, px(20.), align, bounds, window, cx)
                            .unwrap();
                        let row_x = |width| {
                            origin.x
                                + match align {
                                    TextAlign::Left => px(0.),
                                    TextAlign::Center => px((align_width - width) / 2.),
                                    TextAlign::Right => px(align_width - width),
                                }
                        };
                        let scene = &window.next_frame.scene;
                        assert_eq!(scene.quads.len(), 2);
                        assert_eq!(scene.underlines.len(), 2);
                        let footprint =
                            scene.quads.iter().fold(Bounds::default(), |bounds, quad| {
                                let projected =
                                    quad.transform.try_project_bounds(quad.bounds).unwrap();
                                bounds.join(&projected.intersect(&quad.clip.conservative_bounds))
                            });
                        let mut expected_footprint = Bounds::default();
                        for (row, width) in [(0, 40.), (1, 20.)] {
                            let expected = Bounds::new(
                                point(row_x(width), origin.y + px(row as f32 * 20.)),
                                size(px(width), px(20.)),
                            );
                            let quad = &scene.quads[row];
                            let expected = expected.scale(window.scale_factor());
                            expected_footprint = expected_footprint.join(&expected);
                            assert_eq!(quad.bounds, expected);
                            assert!(footprint.contains(&expected.origin));
                            assert!(footprint.right() >= expected.right());
                            assert!(footprint.bottom() >= expected.bottom());
                            assert_eq!(scene.underlines[row].bounds.origin.x, expected.origin.x);
                            assert_eq!(
                                scene.underlines[row].bounds.size.width,
                                expected.size.width
                            );
                        }
                        assert_eq!(footprint, expected_footprint);
                        let line_order = scene.underlines[0].order;
                        window.paint_quad(fill(
                            Bounds::new(point(row_x(40.), origin.y), size(px(1.), px(20.))),
                            color,
                        ));
                        assert!(window.next_frame.scene.quads.last().unwrap().order > line_order);
                    },
                )
                .size_full()
            });
        }
    }
}

#[open_gpui::test]
fn zero_advance_underline_and_background_receive_the_same_half_cell_adjustment(
    cx: &mut TestAppContext,
) {
    let visual = cx.add_empty_window();
    visual.draw(Point::default(), size(px(100.), px(80.)), |_, _| {
        canvas(
            |_, _, _| {},
            |_, _, window, cx| {
                let line = ShapedLine {
                    layout: Arc::new(layout("\u{0301}A", &[(0, 0.), (2, 0.)], 10.)),
                    text: "\u{0301}A".into(),
                    decoration_runs: smallvec::smallvec![
                        decoration(2, black()),
                        DecorationRun {
                            underline: None,
                            background_color: None,
                            ..decoration(1, black())
                        }
                    ],
                };
                let origin = point(px(20.), px(10.));
                let half_width = cx
                    .text_system()
                    .bounding_box(FontId(0), px(16.))
                    .size
                    .width
                    .half();
                assert!(half_width > px(0.));
                line.paint_background(origin, px(20.), TextAlign::Left, None, window, cx)
                    .unwrap();
                let background = window.next_frame.scene.quads[0];
                window.paint_quad(fill(
                    Bounds::new(
                        point(origin.x - half_width, origin.y),
                        size(half_width / 2., px(20.)),
                    ),
                    black(),
                ));
                assert_eq!(
                    background.order,
                    window.next_frame.scene.quads.last().unwrap().order
                );
                assert_eq!(window.next_frame.scene.quads[0].bounds, background.bounds);
                line.paint_with_underline_handler(
                    origin,
                    px(20.),
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                    |range, underline_origin, width, style, window| {
                        assert_eq!(range, 0..2);
                        assert_eq!(underline_origin.x, origin.x - half_width);
                        assert_eq!(width, half_width);
                        window.paint_underline(underline_origin, width, style);
                    },
                )
                .unwrap();
                assert_eq!(window.next_frame.scene.quads.len(), 2);
                assert_eq!(
                    window.next_frame.scene.quads[0].bounds.size.width,
                    half_width.scale(window.scale_factor())
                );
            },
        )
        .size_full()
    });
}
