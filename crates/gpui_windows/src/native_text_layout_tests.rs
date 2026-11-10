use super::*;
use crate::native_test_observation::NativeWindowTestMessageDisposition;
use open_gpui::{
    DecorationRun, LineLayout, Pixels, Point, ScaledPixels, ShapedLine, SharedString, TextAlign,
    TextRun, TruncateFrom, UnderlineStyle, WindowHandle, WindowInitialPresentationStatus,
    WrappedLine, fill, font, rgb, underline_y_offset,
};
use std::ops::Range;

const NATIVE_WRAPPED_TEXT: &str = concat!(
    "alpha/beta/gamma/delta/epsilon e\u{301} 😀你好、世界 ",
    "👩\u{200d}💻 👍🏽 🇯🇵 か\u{3099} omega/theta/lambda/end",
);
const NATIVE_MIDDLE_TEXT: &str = "e\u{301}😀/alpha/beta/gamma/delta/epsilon/か\u{3099}你好.rs";
const NATIVE_MIXED_RTL_TEXT: &str = "left/אבג/😀/right";

fn native_text_layout_runs(source: &str) -> [TextRun; 2] {
    let split = source
        .find('/')
        .expect("the native fixture must contain a slash")
        + 1;
    [
        TextRun {
            len: split,
            font: font("Segoe UI"),
            color: white(),
            background_color: Some(rgb(0x202830).into()),
            underline: Some(UnderlineStyle {
                thickness: px(1.),
                color: None,
                wavy: false,
            }),
            ..Default::default()
        },
        TextRun {
            len: source.len() - split,
            font: font("Segoe UI"),
            color: rgb(0x80c0ff).into(),
            background_color: Some(rgb(0x183040).into()),
            underline: Some(UnderlineStyle {
                thickness: px(1.25),
                color: Some(rgb(0x80c0ff).into()),
                wavy: true,
            }),
            ..Default::default()
        },
    ]
}

#[derive(Clone, Debug)]
struct NativeTextFragmentPaint {
    range: Range<usize>,
    x_offset: Pixels,
    shaped: ShapedLine,
    result: Result<(), String>,
}

#[derive(Clone, Debug)]
struct NativeUnderlinePaint {
    range: Range<usize>,
    origin: Point<Pixels>,
    width: Pixels,
    style: UnderlineStyle,
    bounds: Bounds<ScaledPixels>,
    exclusions: Vec<Range<ScaledPixels>>,
}

#[derive(Clone, Debug)]
struct NativeUnderlineLinePaint {
    origin: Point<Pixels>,
    result: Result<(), String>,
    underlines: Vec<NativeUnderlinePaint>,
}

#[derive(Clone, Debug)]
struct NativeTextLayoutPaint {
    previous_rendered_generation: u64,
    canvas_bounds: Bounds<Pixels>,
    scale_factor: f32,
    shaped: ShapedLine,
    result: Result<(), String>,
    wrapped: Rc<WrappedLine>,
    wrap_indices: Vec<usize>,
    wrapped_result: Result<(), String>,
    middle: ShapedLine,
    middle_runs: Vec<TextRun>,
    middle_result: Result<(), String>,
    source_fragments: Vec<NativeTextFragmentPaint>,
    middle_fragments: Vec<NativeTextFragmentPaint>,
    mixed_rtl: ShapedLine,
    mixed_rtl_fragments: Vec<NativeTextFragmentPaint>,
    raw_background_result: Result<(), String>,
    raw_foreground_result: Result<(), String>,
    underline_lines: Vec<NativeUnderlineLinePaint>,
}

struct NativeTextLayoutRoot {
    source: SharedString,
    wrap_width: Pixels,
    paints: Rc<RefCell<Vec<NativeTextLayoutPaint>>>,
}

fn paint_native_text_fragments(
    line: &ShapedLine,
    origin: Point<Pixels>,
    window: &mut Window,
    cx: &mut open_gpui::App,
) -> Vec<NativeTextFragmentPaint> {
    let mut cursor = line.cursor();
    let mut start = 0;
    std::iter::once(0)
        .chain(
            line.text
                .char_indices()
                .map(|(index, character)| index + character.len_utf8()),
        )
        .chain(std::iter::once(line.len()))
        .map(|end| {
            let x_offset = cursor.x_offset();
            let shaped = cursor.take_until(end);
            let result = shaped
                .paint(
                    origin + point(x_offset, px(0.)),
                    px(32.),
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .map_err(|error| format!("{error:#}"));
            let range = start..end;
            start = end;
            NativeTextFragmentPaint {
                range,
                x_offset,
                shaped,
                result,
            }
        })
        .collect()
}

impl Render for NativeTextLayoutRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let source = self.source.clone();
        let wrap_width = self.wrap_width;
        let paints = self.paints.clone();
        canvas(
            move |_, window, cx| {
                let runs = [TextRun {
                    len: source.len(),
                    font: font("Segoe UI"),
                    color: white(),
                    ..Default::default()
                }];
                let shaped = window
                    .text_system()
                    .shape_line(source, px(16.), &runs, None);
                let mut wrapped = window
                    .text_system()
                    .shape_text(
                        NATIVE_WRAPPED_TEXT.into(),
                        px(16.),
                        &native_text_layout_runs(NATIVE_WRAPPED_TEXT),
                        Some(wrap_width),
                        None,
                    )
                    .expect("the complete native Unicode wrapping source must shape");
                assert_eq!(wrapped.len(), 1);
                let wrapped = wrapped
                    .pop()
                    .expect("the native wrapping paragraph must exist");
                let middle_runs = native_text_layout_runs(NATIVE_MIDDLE_TEXT);
                let mut wrapper = cx.text_system().line_wrapper(font("Segoe UI"), px(16.));
                let (middle_text, middle_runs) = wrapper.truncate_line(
                    NATIVE_MIDDLE_TEXT.into(),
                    wrap_width,
                    "…",
                    &middle_runs,
                    TruncateFrom::Middle,
                );
                let middle =
                    window
                        .text_system()
                        .shape_line(middle_text, px(16.), &middle_runs, None);
                let mixed_rtl = window.text_system().shape_line(
                    NATIVE_MIXED_RTL_TEXT.into(),
                    px(16.),
                    &native_text_layout_runs(NATIVE_MIXED_RTL_TEXT),
                    None,
                );
                (shaped, wrapped, middle, middle_runs.into_owned(), mixed_rtl)
            },
            move |bounds, (shaped, wrapped, middle, middle_runs, mixed_rtl), window, cx| {
                let previous_rendered_generation = window.rendered_frame_revision();
                let result = shaped
                    .paint(
                        bounds.origin + point(px(16.), px(16.)),
                        px(32.),
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .map_err(|error| format!("{error:#}"));
                let middle_result = middle
                    .paint(
                        bounds.origin + point(px(16.), px(56.)),
                        px(32.),
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .map_err(|error| format!("{error:#}"));
                let wrap_indices = wrapped
                    .wrap_boundaries()
                    .iter()
                    .map(|boundary| wrapped.runs()[boundary.run_ix].glyphs[boundary.glyph_ix].index)
                    .collect();
                let wrapped_result = wrapped
                    .paint(
                        bounds.origin + point(px(16.), px(96.)),
                        px(24.),
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .map_err(|error| format!("{error:#}"));
                let fragment_origin =
                    bounds.origin + point(bounds.size.width / 2. + px(8.), px(96.));
                let source_fragments =
                    paint_native_text_fragments(&shaped, fragment_origin, window, cx);
                let middle_fragments = paint_native_text_fragments(
                    &middle,
                    fragment_origin + point(px(0.), px(40.)),
                    window,
                    cx,
                );
                let mixed_rtl_fragments = paint_native_text_fragments(
                    &mixed_rtl,
                    fragment_origin + point(px(0.), px(80.)),
                    window,
                    cx,
                );
                let decorations = middle_runs
                    .iter()
                    .map(|run| DecorationRun {
                        len: u32::try_from(run.len)
                            .expect("the finite native decoration length must fit in u32"),
                        color: run.color,
                        background_color: run.background_color,
                        underline: run.underline,
                        strikethrough: run.strikethrough,
                    })
                    .collect::<Vec<_>>();
                let raw_layout: &LineLayout = &middle;
                let raw_origin = fragment_origin + point(px(0.), px(120.));
                let align_width = wrap_width + px(16.);
                let raw_background_result = raw_layout
                    .paint_background(
                        raw_origin,
                        px(32.),
                        TextAlign::Right,
                        Some(align_width),
                        &decorations,
                        window,
                        cx,
                    )
                    .map_err(|error| format!("{error:#}"));
                window.paint_quad(fill(
                    Bounds::new(raw_origin + point(px(12.), px(2.)), size(px(24.), px(28.))),
                    rgb(0x605020),
                ));
                let raw_foreground_result = raw_layout
                    .paint(
                        raw_origin,
                        px(32.),
                        TextAlign::Right,
                        Some(align_width),
                        &decorations,
                        window,
                        cx,
                    )
                    .map_err(|error| format!("{error:#}"));
                let mut underline_lines = Vec::new();
                for (row, exclude) in [(160., false), (200., true)] {
                    let origin = fragment_origin + point(px(0.), px(row));
                    let mut underlines = Vec::new();
                    let result = middle
                        .paint_with_underline_handler(
                            origin,
                            px(32.),
                            TextAlign::Right,
                            Some(align_width),
                            window,
                            cx,
                            |range, origin, width, style, window| {
                                let bounds = window.underline_bounds(origin, width, style);
                                let exclusions = if exclude {
                                    let third = bounds.size.width * (1. / 3.);
                                    vec![bounds.left() + third..bounds.right() - third]
                                } else {
                                    Vec::new()
                                };
                                if exclude {
                                    window.paint_underline_with_exclusions(
                                        origin,
                                        width,
                                        style,
                                        &exclusions,
                                    );
                                } else {
                                    window.paint_underline(origin, width, style);
                                }
                                underlines.push(NativeUnderlinePaint {
                                    range,
                                    origin,
                                    width,
                                    style: *style,
                                    bounds,
                                    exclusions,
                                });
                            },
                        )
                        .map_err(|error| format!("{error:#}"));
                    underline_lines.push(NativeUnderlineLinePaint {
                        origin,
                        result,
                        underlines,
                    });
                }
                paints.borrow_mut().push(NativeTextLayoutPaint {
                    previous_rendered_generation,
                    canvas_bounds: bounds,
                    scale_factor: window.scale_factor(),
                    shaped,
                    result,
                    wrapped: Rc::new(wrapped),
                    wrap_indices,
                    wrapped_result,
                    middle,
                    middle_runs,
                    middle_result,
                    source_fragments,
                    middle_fragments,
                    mixed_rtl,
                    mixed_rtl_fragments,
                    raw_background_result,
                    raw_foreground_result,
                    underline_lines,
                });
            },
        )
        .size_full()
    }
}

fn wait_for_native_text_layout_submission(
    platform: &WindowsPlatform,
    app: &mut Application,
    window: WindowHandle<NativeTextLayoutRoot>,
    paints: &RefCell<Vec<NativeTextLayoutPaint>>,
    source: &str,
    wrap_width: Pixels,
    previous_generation: u64,
) -> (u64, NativeTextLayoutPaint) {
    let mut submitted = None;
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "native text layout accepted nonempty submission",
        Duration::from_millis(5),
        || {
            let (facts, rendered_generation, diagnostics_generation) = app
                .update_for_test(|cx| {
                    window.update(cx, |_, window, _| {
                        (
                            window.presentation_facts(),
                            window.rendered_frame_revision(),
                            window
                                .submitted_frame_diagnostics_for_test()
                                .map(|diagnostics| diagnostics.generation()),
                        )
                    })
                })
                .expect("the native text window must stay open until its frame is submitted");
            let paints = paints.borrow();
            for paint in paints.iter() {
                assert!(
                    paint.result.is_ok(),
                    "native ShapedLine::paint failed for {:?}: {:?}",
                    paint.shaped.text,
                    paint.result,
                );
                assert!(
                    paint.wrapped_result.is_ok(),
                    "native WrappedLine::paint failed for {:?}: {:?}",
                    paint.wrapped.text,
                    paint.wrapped_result,
                );
                assert!(
                    paint.middle_result.is_ok(),
                    "native Middle ShapedLine::paint failed for {:?}: {:?}",
                    paint.middle.text,
                    paint.middle_result,
                );
                for fragments in [
                    &paint.source_fragments,
                    &paint.middle_fragments,
                    &paint.mixed_rtl_fragments,
                ] {
                    for fragment in fragments {
                        assert!(
                            fragment.result.is_ok(),
                            "native cursor fragment {:?} failed: {:?}",
                            fragment.range,
                            fragment.result,
                        );
                    }
                }
                assert_eq!(paint.raw_background_result, Ok(()));
                assert_eq!(paint.raw_foreground_result, Ok(()));
                for line in &paint.underline_lines {
                    assert_eq!(line.result, Ok(()));
                }
            }
            let Some(attempt) = facts.latest_present_attempt else {
                return false;
            };
            let Some(paint) = paints.last() else {
                return false;
            };
            if paint.shaped.text.as_ref() == source
                && paint.wrapped.text.as_ref() == NATIVE_WRAPPED_TEXT
                && paint.wrapped.wrap_width == Some(wrap_width)
                && !paint.wrap_indices.is_empty()
                && paint.middle.text.contains('…')
                && paint.mixed_rtl.text.as_ref() == NATIVE_MIXED_RTL_TEXT
                && paint.underline_lines.len() == 2
                && paint
                    .underline_lines
                    .iter()
                    .all(|line| !line.underlines.is_empty())
                && paint.previous_rendered_generation.checked_add(1) == Some(attempt.generation)
                && attempt.generation > previous_generation
                && attempt.outcome == PlatformWindowPresentOutcome::Submitted
                && attempt.contained_valid_primitives
                && facts.initial_presentation == WindowInitialPresentationStatus::Completed
                && facts.native_visible
                && facts.frame_accepted_generation == Some(attempt.generation)
                && facts.present_submitted_generation == Some(attempt.generation)
                && facts.non_empty_presented_generation == Some(attempt.generation)
                && rendered_generation == attempt.generation
                && diagnostics_generation == Some(attempt.generation)
            {
                submitted = Some((attempt.generation, paint.clone()));
                true
            } else {
                false
            }
        },
    );
    submitted.expect("the painted text must belong to an accepted nonempty Submitted frame")
}

fn assert_native_text_fragment_layout(
    original: &LineLayout,
    fragment: &LineLayout,
    range: Range<usize>,
    x_offset: Pixels,
) {
    assert!(range.start <= range.end && range.end <= original.len);
    assert_eq!(fragment.len, range.len());
    assert_eq!(fragment.font_size, original.font_size);
    assert_eq!(fragment.ascent, original.ascent);
    assert_eq!(fragment.descent, original.descent);
    assert!(fragment.width.as_f32().is_finite());
    assert!(x_offset.as_f32().is_finite());
    let source_range = &range;
    let expected = original
        .runs
        .iter()
        .flat_map(|run| {
            run.glyphs
                .iter()
                .filter(move |glyph| source_range.contains(&glyph.index))
                .map(move |glyph| (run.font_id, glyph))
        })
        .collect::<Vec<_>>();
    let actual = fragment
        .runs
        .iter()
        .flat_map(|run| run.glyphs.iter().map(move |glyph| (run.font_id, glyph)))
        .collect::<Vec<_>>();
    assert_eq!(
        actual.len(),
        expected.len(),
        "glyph membership for {range:?}"
    );
    for ((expected_font, expected), (actual_font, actual)) in expected.into_iter().zip(actual) {
        assert!(actual.index < range.len());
        let source_index = range
            .start
            .checked_add(actual.index)
            .expect("a fragment glyph source index must not overflow");
        assert!(source_range.contains(&source_index));
        assert_eq!(source_index, expected.index);
        assert_eq!(actual_font, expected_font);
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.is_emoji, expected.is_emoji);
        assert!(actual.position.x.as_f32().is_finite());
        assert!(actual.position.y.as_f32().is_finite());
        assert!((actual.position.x + x_offset - expected.position.x).abs() < px(0.001));
        assert_eq!(actual.position.y, expected.position.y);
    }
}

fn assert_native_text_fragments(line: &ShapedLine, fragments: &[NativeTextFragmentPaint]) {
    assert_eq!(line.len(), line.text.len());
    assert!(line.width.as_f32().is_finite() && line.width > px(0.));
    for glyph in line.runs.iter().flat_map(|run| &run.glyphs) {
        assert!(glyph.index < line.len() && line.text.is_char_boundary(glyph.index));
    }
    assert_eq!(fragments.len(), line.text.chars().count() + 2);
    let mut reconstructed = String::new();
    let mut start = 0;
    let mut cursor = line.cursor();
    assert_eq!(cursor.x_offset(), px(0.));
    for fragment in fragments {
        assert_eq!(fragment.result, Ok(()));
        assert_eq!(fragment.range.start, start);
        assert!(line.text.is_char_boundary(start));
        assert!(line.text.is_char_boundary(fragment.range.end));
        assert_eq!(fragment.x_offset, cursor.x_offset());
        let piece = cursor.take_until(fragment.range.end);
        assert_eq!(piece.text, fragment.shaped.text);
        assert_eq!(piece.width, fragment.shaped.width);
        assert_eq!(
            fragment.shaped.text.as_ref(),
            &line.text[fragment.range.clone()],
        );
        assert_native_text_fragment_layout(
            line,
            &fragment.shaped,
            fragment.range.clone(),
            fragment.x_offset,
        );
        assert!(cursor.x_offset().as_f32().is_finite());
        assert!((cursor.x_offset() - fragment.x_offset - fragment.shaped.width).abs() < px(0.001),);
        reconstructed.push_str(&fragment.shaped.text);
        start = fragment.range.end;
    }
    assert_eq!(start, line.len());
    assert_eq!(reconstructed, line.text.as_ref());
    assert!((cursor.x_offset() - line.width).abs() < px(0.001));
    for boundary in std::iter::once(0).chain(
        line.text
            .char_indices()
            .map(|(index, character)| index + character.len_utf8()),
    ) {
        let (left, right) = LineLayout::split_at(line, boundary);
        let x_offset = if boundary == 0 {
            px(0.)
        } else {
            line.x_for_index(boundary)
        };
        assert_eq!(left.width, x_offset);
        assert_native_text_fragment_layout(line, &left, 0..boundary, px(0.));
        assert_native_text_fragment_layout(line, &right, boundary..line.len(), x_offset);
        assert!((left.width + right.width - line.width).abs() < px(0.001));
        let (left, right) = line.split_at(boundary);
        assert_eq!(left.text.as_ref(), &line.text[..boundary]);
        assert_eq!(right.text.as_ref(), &line.text[boundary..]);
        assert_native_text_fragment_layout(line, &left, 0..boundary, px(0.));
        assert_native_text_fragment_layout(line, &right, boundary..line.len(), x_offset);
    }
}

fn assert_native_text_split_layers_and_underlines(
    paint: &NativeTextLayoutPaint,
    wrap_width: Pixels,
) {
    assert_native_text_fragments(&paint.shaped, &paint.source_fragments);
    assert_native_text_fragments(&paint.middle, &paint.middle_fragments);
    let unwrapped = &paint.wrapped.unwrapped_layout;
    for boundary in std::iter::once(0)
        .chain(paint.wrap_indices.iter().copied())
        .chain(std::iter::once(unwrapped.len))
    {
        let (left, right) = unwrapped.split_at(boundary);
        let x_offset = if boundary == 0 {
            px(0.)
        } else {
            unwrapped.x_for_index(boundary)
        };
        assert_native_text_fragment_layout(unwrapped, &left, 0..boundary, px(0.));
        assert_native_text_fragment_layout(unwrapped, &right, boundary..unwrapped.len, x_offset);
    }
    assert_eq!(paint.mixed_rtl.text.as_ref(), NATIVE_MIXED_RTL_TEXT);
    assert_native_text_fragments(&paint.mixed_rtl, &paint.mixed_rtl_fragments);
    for character in ['א', 'ב', 'ג', '😀'] {
        let index = NATIVE_MIXED_RTL_TEXT
            .find(character)
            .expect("the native mixed-direction fixture glyph must exist");
        assert!(
            paint
                .mixed_rtl
                .runs
                .iter()
                .flat_map(|run| &run.glyphs)
                .any(|glyph| glyph.index == index && glyph.id.0 != 0),
        );
    }
    assert_eq!(paint.raw_background_result, Ok(()));
    assert_eq!(paint.raw_foreground_result, Ok(()));
    assert!(paint.middle_runs.iter().all(|run| {
        run.background_color.is_some_and(|color| color.a == 1.) && run.underline.is_some()
    }));
    assert!(paint.scale_factor.is_finite() && paint.scale_factor > 0.);
    assert_eq!(paint.underline_lines.len(), 2);
    let mut expected_underlines: Vec<(Range<usize>, UnderlineStyle)> = Vec::new();
    let mut offset = 0;
    for run in &paint.middle_runs {
        let end = offset + run.len;
        let style = run
            .underline
            .expect("the native Middle run must retain its underline");
        let style = UnderlineStyle {
            color: Some(style.color.unwrap_or(run.color)),
            ..style
        };
        if let Some((range, previous_style)) = expected_underlines.last_mut()
            && *previous_style == style
        {
            range.end = end;
        } else {
            expected_underlines.push((offset..end, style));
        }
        offset = end;
    }
    assert_eq!(offset, paint.middle.len());
    assert_eq!(expected_underlines.len(), 2);
    let expected_y = underline_y_offset(px(32.), paint.middle.ascent, paint.middle.descent);
    assert!(expected_y.as_f32().is_finite());
    assert!(
        (expected_y
            - ((px(32.) - paint.middle.ascent - paint.middle.descent) / 2.
                + paint.middle.ascent
                + paint.middle.descent * 0.618))
            .abs()
            < px(0.001),
    );
    for (line_index, line) in paint.underline_lines.iter().enumerate() {
        assert_eq!(line.result, Ok(()));
        assert_eq!(line.underlines.len(), expected_underlines.len());
        let aligned_x = line.origin.x + wrap_width + px(16.) - paint.middle.width;
        let mut start = 0;
        for (underline_index, (underline, (range, style))) in
            line.underlines.iter().zip(&expected_underlines).enumerate()
        {
            let end = range.end;
            assert_eq!(range.start, start);
            assert_eq!(underline.range, *range);
            assert!(paint.middle.text.is_char_boundary(start));
            assert!(paint.middle.text.is_char_boundary(end));
            assert_eq!(underline.style, *style);
            assert!(underline.width.as_f32().is_finite() && underline.width > px(0.));
            assert!(
                (underline.origin.x - aligned_x - paint.middle.x_for_index(start)).abs()
                    < px(0.001),
            );
            assert!((underline.origin.y - line.origin.y - expected_y).abs() < px(0.001));
            assert!(
                (underline.width
                    - (paint.middle.x_for_index(end) - paint.middle.x_for_index(start)))
                .abs()
                    < px(0.001),
            );
            assert!(!underline.bounds.is_empty());
            for edge in [
                underline.bounds.left(),
                underline.bounds.right(),
                underline.bounds.top(),
                underline.bounds.bottom(),
            ] {
                assert!(edge.0.is_finite());
                assert!(edge.0.fract().abs() < 0.001);
            }
            assert!(
                (underline.bounds.left().0 - underline.origin.x.as_f32() * paint.scale_factor)
                    .abs()
                    <= 1.,
            );
            assert!(
                (underline.bounds.top().0 - underline.origin.y.as_f32() * paint.scale_factor).abs()
                    <= 1.,
            );
            assert!(
                (underline.bounds.size.width.0 - underline.width.as_f32() * paint.scale_factor)
                    .abs()
                    <= 2.,
            );
            let canvas = paint.canvas_bounds.scale(paint.scale_factor);
            assert!(canvas.left() <= underline.bounds.left());
            assert!(canvas.right() >= underline.bounds.right());
            assert!(canvas.top() <= underline.bounds.top());
            assert!(canvas.bottom() >= underline.bounds.bottom());
            if line_index == 0 {
                assert!(underline.exclusions.is_empty());
            } else {
                let [exclusion] = underline.exclusions.as_slice() else {
                    panic!("the native underline must exclude one central device span");
                };
                assert!(underline.bounds.left() < exclusion.start);
                assert!(exclusion.start < exclusion.end);
                assert!(exclusion.end < underline.bounds.right());
                let forwarded = &paint.underline_lines[0].underlines[underline_index];
                assert_eq!(underline.range, forwarded.range);
                assert_eq!(underline.width, forwarded.width);
                assert_eq!(underline.style, forwarded.style);
                assert_eq!(underline.origin.x, forwarded.origin.x);
            }
            start = end;
        }
        assert_eq!(start, paint.middle.len());
    }
}

fn assert_native_text_layout(paint: &NativeTextLayoutPaint, source: &str) {
    let shaped = &paint.shaped;
    assert_eq!(paint.result, Ok(()));
    assert_eq!(shaped.text.as_ref(), source);
    assert_eq!(shaped.len(), source.len());
    assert!(shaped.width.as_f32().is_finite() && shaped.width > px(0.));
    assert!(shaped.width + px(32.) < paint.canvas_bounds.size.width);
    assert!(paint.canvas_bounds.size.height > px(48.));
    assert_eq!(shaped.x_for_index(source.len()), shaped.width);
    assert!(source.contains('\t') && source.contains('\u{2028}') && source.contains('\u{2029}'));
    assert!(source.chars().any(|character| character.len_utf8() == 4));
    let glyphs = shaped
        .runs
        .iter()
        .flat_map(|run| &run.glyphs)
        .collect::<Vec<_>>();
    assert!(!glyphs.is_empty());
    for glyph in &glyphs {
        assert!(glyph.index < source.len() && source.is_char_boundary(glyph.index));
        assert!(glyph.position.x.as_f32().is_finite());
        assert!(glyph.position.y.as_f32().is_finite());
    }
    let mut remainder = shaped.clone();
    let mut reconstructed = String::new();
    for (index, character) in source.char_indices() {
        let (piece, rest) = remainder.split_at(character.len_utf8());
        remainder = rest;
        assert_eq!(piece.text.as_ref(), character.to_string());
        assert_eq!(piece.len(), character.len_utf8());
        reconstructed.push_str(&piece.text);
        match character {
            '\t' => {
                assert!(piece.width > px(0.), "tab at byte {index} must advance");
                assert!(glyphs.iter().any(|glyph| glyph.index == index));
                assert_eq!(shaped.index_for_x(shaped.x_for_index(index)), Some(index));
            }
            '\u{2028}' | '\u{2029}' => {
                assert!(piece.width.abs() < px(0.001));
            }
            _ if character.len_utf8() == 4 => {
                assert!(glyphs.iter().any(|glyph| glyph.index == index));
                assert!(piece.width > px(0.));
            }
            _ => {}
        }
    }
    assert_eq!(reconstructed, source);
    assert!(remainder.text.is_empty());
    assert_eq!(remainder.len(), 0);
}

fn assert_native_text_fixture_boundary(source: &str, index: usize) {
    assert!(source.is_char_boundary(index));
    for cluster in ["e\u{301}", "👩\u{200d}💻", "👍🏽", "🇯🇵", "か\u{3099}"] {
        for (start, _) in source.match_indices(cluster) {
            assert!(
                index <= start || index >= start + cluster.len(),
                "the native fixture cluster {cluster:?} must not split at byte {index}",
            );
        }
    }
}

fn assert_native_text_wrapping_and_middle(paint: &NativeTextLayoutPaint, wrap_width: Pixels) {
    let wrapped = &paint.wrapped;
    assert_eq!(paint.wrapped_result, Ok(()));
    assert_eq!(wrapped.text.as_ref(), NATIVE_WRAPPED_TEXT);
    assert_eq!(wrapped.len(), NATIVE_WRAPPED_TEXT.len());
    assert_eq!(wrapped.wrap_width, Some(wrap_width));
    assert!(wrap_width.as_f32().is_finite() && wrap_width > px(0.));
    assert!(wrapped.unwrapped_layout.width.as_f32().is_finite());
    assert!(wrapped.unwrapped_layout.width > wrap_width);
    assert!(wrap_width + px(32.) < paint.canvas_bounds.size.width);
    assert!(px(96.) + wrapped.size(px(24.)).height + px(16.) <= paint.canvas_bounds.size.height);
    assert!(!paint.wrap_indices.is_empty());
    assert_eq!(paint.wrap_indices.len(), wrapped.wrap_boundaries().len());
    assert!(paint.wrap_indices.windows(2).all(|pair| pair[0] < pair[1]));
    for &index in &paint.wrap_indices {
        assert!(index > 0 && index < NATIVE_WRAPPED_TEXT.len());
        assert_native_text_fixture_boundary(NATIVE_WRAPPED_TEXT, index);
    }
    let reconstructed = std::iter::once(0)
        .chain(paint.wrap_indices.iter().copied())
        .zip(
            paint
                .wrap_indices
                .iter()
                .copied()
                .chain(std::iter::once(NATIVE_WRAPPED_TEXT.len())),
        )
        .map(|(start, end)| &wrapped.text[start..end])
        .collect::<String>();
    assert_eq!(reconstructed, NATIVE_WRAPPED_TEXT);
    for character in ['😀', '你', '世'] {
        let index = NATIVE_WRAPPED_TEXT
            .find(character)
            .expect("the native glyph fixture must exist");
        assert!(
            wrapped
                .runs()
                .iter()
                .flat_map(|run| &run.glyphs)
                .any(|glyph| glyph.index == index && glyph.id.0 != 0),
            "the native wrapped source must contain a real {character:?} glyph",
        );
    }

    let middle = &paint.middle;
    assert_eq!(paint.middle_result, Ok(()));
    assert_eq!(middle.len(), middle.text.len());
    assert!(middle.width.as_f32().is_finite() && middle.width > px(0.));
    assert!(middle.width <= wrap_width + px(0.01));
    assert!(middle.width + px(32.) < paint.canvas_bounds.size.width);
    let (front, back) = middle
        .text
        .split_once('…')
        .expect("the native Middle affix must exist");
    assert!(!front.is_empty() && !back.is_empty());
    assert!(!back.contains('…'));
    assert!(NATIVE_MIDDLE_TEXT.starts_with(front));
    assert!(NATIVE_MIDDLE_TEXT.ends_with(back));
    assert!(front.len() + back.len() < NATIVE_MIDDLE_TEXT.len());
    assert_native_text_fixture_boundary(NATIVE_MIDDLE_TEXT, front.len());
    assert_native_text_fixture_boundary(NATIVE_MIDDLE_TEXT, NATIVE_MIDDLE_TEXT.len() - back.len());
    assert_eq!(
        paint.middle_runs.iter().map(|run| run.len).sum::<usize>(),
        middle.text.len(),
    );
    let original_runs = native_text_layout_runs(NATIVE_MIDDLE_TEXT);
    assert_eq!(
        paint
            .middle_runs
            .first()
            .expect("the native Middle prefix run must exist")
            .color,
        original_runs[0].color,
    );
    assert_eq!(
        paint
            .middle_runs
            .last()
            .expect("the native Middle suffix run must exist")
            .color,
        original_runs[1].color,
    );
    for (text, runs) in [
        (wrapped.text.as_ref(), wrapped.runs()),
        (middle.text.as_ref(), middle.runs.as_slice()),
    ] {
        assert!(runs.iter().any(|run| !run.glyphs.is_empty()));
        for glyph in runs.iter().flat_map(|run| &run.glyphs) {
            assert!(glyph.index < text.len() && text.is_char_boundary(glyph.index));
            assert!(glyph.position.x.as_f32().is_finite());
            assert!(glyph.position.y.as_f32().is_finite());
        }
    }
}

#[test]
fn native_text_layout_submits_complete_unicode_source_and_notified_redraw() {
    const INITIAL_TEXT: &str = "\té\t𐐀\u{2028}中\u{2029}z\t";
    const CHANGED_TEXT: &str = "updated\t𐐀\u{2029}é\t中\u{2028}zz\t";

    discard_stale_quit_messages();
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let observation_cursor = observation.cursor();
    let platform = Rc::new(
        WindowsPlatform::new_windowed_for_test()
            .expect("the native DirectWrite/DirectX platform must initialize"),
    );
    let platform_hwnd = platform.handle;
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let platform_text_system = platform.text_system();
    let weak_platform_text_system = Arc::downgrade(&platform_text_system);
    drop(platform_text_system);
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let text_system = app.text_system();
    let weak_text_system = Arc::downgrade(&text_system);
    drop(text_system);
    platform
        .begin_vsync_thread()
        .expect("native vsync provider should start");

    let paints = Rc::new(RefCell::new(Vec::new()));
    let window = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(640.), px(480.)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| {
                    cx.new(|_| NativeTextLayoutRoot {
                        source: INITIAL_TEXT.into(),
                        wrap_width: px(240.),
                        paints: paints.clone(),
                    })
                },
            )
        })
        .expect("the native text layout window must open");
    let registration = platform
        .raw_window_handles
        .read()
        .iter()
        .copied()
        .find(|registration| registration.window_id() == window.window_id())
        .expect("the native text layout window must own its exact HWND registration");
    let hwnd = registration.as_raw();
    let native_window = platform
        .window_from_hwnd(hwnd)
        .expect("the native text layout HWND must remain registered");
    assert_eq!(native_window.registration, registration);
    let frame_requests = observe_frame_requests(&native_window);
    let initial_paint_cursor = observation.cursor();
    assert!(unsafe { RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE).as_bool() });
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "native text layout initialization WM_PAINT",
        Duration::from_millis(5),
        || {
            observation.any_after(initial_paint_cursor, |event| {
                event.window().window_id() == registration.window_id()
                    && event.window().native_generation() == registration.generation()
                    && matches!(
                        event.kind(),
                        NativeWindowTestEventKind::WindowMessage {
                            message: NativeWindowTestMessage::Other(WM_PAINT),
                            disposition: NativeWindowTestMessageDisposition::Returned(0),
                            ..
                        }
                    )
            })
        },
    );
    let (initial_generation, initial_paint) = wait_for_native_text_layout_submission(
        &platform,
        &mut app,
        window,
        &paints,
        INITIAL_TEXT,
        px(240.),
        0,
    );
    assert_native_text_layout(&initial_paint, INITIAL_TEXT);
    assert_native_text_wrapping_and_middle(&initial_paint, px(240.));
    assert_native_text_split_layers_and_underlines(&initial_paint, px(240.));

    let paints_before = paints.borrow().len();
    let frame_requests_before = frame_requests.get();
    app.update_for_test(|cx| {
        window.update(cx, |root, _, cx| {
            root.source = CHANGED_TEXT.into();
            root.wrap_width = px(112.);
            cx.notify();
        })
    })
    .expect("changing the native text source must notify its existing view");
    let (changed_generation, changed_paint) = wait_for_native_text_layout_submission(
        &platform,
        &mut app,
        window,
        &paints,
        CHANGED_TEXT,
        px(112.),
        initial_generation,
    );
    assert!(changed_generation > initial_generation);
    assert!(paints.borrow().len() > paints_before);
    assert!(frame_requests.get() > frame_requests_before);
    assert_native_text_layout(&changed_paint, CHANGED_TEXT);
    assert_ne!(initial_paint.shaped.text, changed_paint.shaped.text);
    assert_native_text_wrapping_and_middle(&changed_paint, px(112.));
    assert_native_text_split_layers_and_underlines(&changed_paint, px(112.));
    assert_eq!(initial_paint.wrapped.text, changed_paint.wrapped.text);
    assert!(changed_paint.wrap_indices.len() > initial_paint.wrap_indices.len());
    assert_ne!(initial_paint.middle.text, changed_paint.middle.text);
    assert!(changed_paint.middle.text.len() < initial_paint.middle.text.len());

    assert!(platform.raw_window_handles.read().contains(&registration));
    app.update_for_test(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("the exact native text layout window must close");
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "exact native text layout registration retirement",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(hwnd)).as_bool() }
                && !is_registered(&platform, hwnd)
                && platform.inner.native_retirement_is_settled()
        },
    );
    let lifecycle = observation
        .events_after(observation_cursor)
        .into_iter()
        .filter(|event| {
            event.window().window_id() == registration.window_id()
                && event.window().native_generation() == registration.generation()
        })
        .filter_map(|event| match event.kind() {
            NativeWindowTestEventKind::WindowMessage { disposition, .. } => {
                assert_ne!(
                    disposition,
                    NativeWindowTestMessageDisposition::Panicked,
                    "the native text boundary must not swallow a panic",
                );
                None
            }
            lifecycle => Some(lifecycle),
        })
        .collect::<Vec<_>>();
    let [
        NativeWindowTestEventKind::PresentationQuiesced {
            ticket_generation: quiesced,
        },
        NativeWindowTestEventKind::DestroyEntered {
            ticket_generation: destroyed,
        },
        NativeWindowTestEventKind::NativeTerminal {
            ticket_generation: terminal,
        },
    ] = lifecycle.as_slice()
    else {
        panic!("unexpected native text layout retirement sequence: {lifecycle:?}");
    };
    assert_eq!(quiesced, destroyed);
    assert_eq!(destroyed, terminal);
    assert!(app.update_for_test(|cx| cx.windows().is_empty()));
    assert!(platform.raw_window_handles.read().is_empty());
    drop(native_window);

    app.update_for_test(|cx| cx.shutdown());
    drop(app);
    let pump_platform = Rc::downgrade(&platform.inner);
    drop(platform);
    pump_messages_until_with_delay(
        &pump_platform,
        "native text layout platform message window retirement",
        Duration::from_millis(5),
        || unsafe { !IsWindow(Some(platform_hwnd)).as_bool() },
    );
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
    assert!(weak_text_system.upgrade().is_none());
    assert!(weak_platform_text_system.upgrade().is_none());
}
