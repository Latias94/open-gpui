use super::*;
use crate::{AvailableSpace, TestAppContext, TextStyleRefinement, div, px, size};
use unicode_segmentation::UnicodeSegmentation as _;

const SOURCE: &str = "LEFT e\u{301} 👩‍🔬 alpha beta gamma delta epsilon omega RIGHT";

#[crate::test]
fn untruncated_text_restores_intrinsic_size_after_finite_wrapping(cx: &mut TestAppContext) {
    let visual = cx.add_empty_window();
    visual.draw(Point::default(), size(px(800.), px(400.)), |window, cx| {
        window.with_text_style(
            Some(TextStyleRefinement {
                font_size: Some(px(20.).into()),
                line_height: Some(px(24.).into()),
                white_space: Some(WhiteSpace::Normal),
                ..Default::default()
            }),
            |window| {
                let mut fresh = StyledText::new(SOURCE);
                let expected = measure(&mut fresh, AvailableSpace::MaxContent, window, cx);
                assert_eq!(fresh.layout().text(), SOURCE);
                assert_eq!(expected.height, px(24.));
                let mut reused = StyledText::new(SOURCE);
                for width in [
                    AvailableSpace::Definite(px(60.)),
                    AvailableSpace::MaxContent,
                    AvailableSpace::Definite(px(120.)),
                    AvailableSpace::MaxContent,
                ] {
                    let actual = measure(&mut reused, width, window, cx);
                    assert_eq!(reused.layout().text(), SOURCE);
                    assert_eq!(reused.layout().len(), SOURCE.len());
                    if width == AvailableSpace::MaxContent {
                        assert_eq!(actual, expected);
                        assert_eq!(reused.layout().wrapped_text(), SOURCE);
                    } else {
                        assert!(actual.height > expected.height);
                        assert!(actual.width < expected.width);
                    }
                }
            },
        );
        div()
    });
}

fn measure(
    element: &mut StyledText,
    width: AvailableSpace,
    window: &mut Window,
    cx: &mut App,
) -> Size<Pixels> {
    let (layout_id, ()) = element.request_layout(None, None, window, cx);
    window.compute_layout(layout_id, size(width, AvailableSpace::MaxContent), cx);
    window.layout_bounds(layout_id).size
}

fn assert_truncated(layout: &TextLayout, truncate_from: TruncateFrom) {
    let displayed = layout.text();
    let (prefix, suffix) = displayed.split_once('…').expect("text must be truncated");
    assert!(SOURCE.starts_with(prefix));
    assert!(SOURCE.ends_with(suffix));
    assert!(prefix.len() + suffix.len() < SOURCE.len());
    let boundaries = SOURCE
        .grapheme_indices(true)
        .map(|(index, _)| index)
        .chain([SOURCE.len()])
        .collect::<Vec<_>>();
    assert!(boundaries.contains(&prefix.len()));
    assert!(boundaries.contains(&(SOURCE.len() - suffix.len())));
    match truncate_from {
        TruncateFrom::Start => assert!(prefix.is_empty()),
        TruncateFrom::End => assert!(suffix.is_empty()),
        TruncateFrom::Middle => {}
    }
    assert_eq!(layout.len(), displayed.len());
}

fn remeasure_after_truncation(
    cx: &mut TestAppContext,
    white_space: WhiteSpace,
    line_clamp: Option<usize>,
) {
    let cx = cx.add_empty_window();
    for truncate_from in [TruncateFrom::Start, TruncateFrom::Middle, TruncateFrom::End] {
        for reverse in [false, true] {
            cx.draw(
                Point::default(),
                size(px(800.), px(400.)),
                move |window, cx| {
                    let overflow = match truncate_from {
                        TruncateFrom::Start => TextOverflow::TruncateStart("…".into()),
                        TruncateFrom::Middle => TextOverflow::TruncateMiddle("…".into()),
                        TruncateFrom::End => TextOverflow::Truncate("…".into()),
                    };
                    window.with_text_style(
                        Some(TextStyleRefinement {
                            font_size: Some(px(20.).into()),
                            line_height: Some(px(24.).into()),
                            white_space: Some(white_space),
                            line_clamp,
                            text_overflow: Some(overflow),
                            ..Default::default()
                        }),
                        |window| {
                            let style = window.text_style();
                            let intrinsic_lines = window
                                .text_system()
                                .shape_text(
                                    SOURCE.into(),
                                    px(20.),
                                    &[style.to_run(SOURCE.len())],
                                    None,
                                    None,
                                )
                                .expect("source text must shape");
                            let intrinsic = intrinsic_lines[0].size(px(24.));
                            let intrinsic = size(intrinsic.width.ceil(), intrinsic.height);
                            let mut element = StyledText::new(SOURCE);
                            let mut widths = [
                                AvailableSpace::Definite(px(1.)),
                                AvailableSpace::MaxContent,
                                AvailableSpace::Definite(px(240.)),
                                AvailableSpace::Definite(intrinsic.width),
                            ];
                            if reverse {
                                widths.reverse();
                            }
                            let mut snapshots = Vec::new();
                            for width in widths {
                                let measured = measure(&mut element, width, window, cx);
                                if width == AvailableSpace::MaxContent
                                    || width == AvailableSpace::Definite(intrinsic.width)
                                {
                                    assert_eq!(measured, intrinsic, "{truncate_from:?}, {width:?}");
                                    assert_eq!(element.layout().text(), SOURCE);
                                    assert_eq!(element.layout().len(), SOURCE.len());
                                } else {
                                    assert_truncated(element.layout(), truncate_from);
                                    assert!(measured.width < intrinsic.width);
                                    assert!(measured.height <= px(24.) * line_clamp.unwrap_or(1));
                                    if width == AvailableSpace::Definite(px(240.)) {
                                        assert!(measured.width <= px(240.));
                                        if truncate_from == TruncateFrom::Middle {
                                            assert!(element.layout().text().ends_with("RIGHT"));
                                        }
                                    }
                                }
                                let inner = element.layout().0.borrow();
                                snapshots.push((
                                    inner.as_ref().unwrap().lines.clone(),
                                    element.layout().text(),
                                ));
                            }
                            for (lines, displayed) in snapshots {
                                assert_eq!(
                                    lines.iter().map(|line| line.text.as_str()).join("\n"),
                                    displayed
                                );
                            }
                        },
                    );
                    div()
                },
            );
        }
    }
}

#[crate::test]
fn nowrap_text_remeasures_after_truncated_and_intrinsic_probes(cx: &mut TestAppContext) {
    remeasure_after_truncation(cx, WhiteSpace::Nowrap, None);
}

#[crate::test]
fn clamped_text_remeasures_after_truncated_and_intrinsic_probes(cx: &mut TestAppContext) {
    remeasure_after_truncation(cx, WhiteSpace::Normal, Some(2));
}
