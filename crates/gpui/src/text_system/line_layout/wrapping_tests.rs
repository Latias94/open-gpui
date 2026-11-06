//! Shaped wrapping regressions, including Zed's `f0210f1621` cluster and RTL cases.

use super::*;
use crate::text_system::font_installation_tests::fixture;
use std::borrow::Cow;

fn shaped_layout(text: &str, width: f32, runs: &[&[(usize, f32)]]) -> LineLayout {
    LineLayout {
        font_size: px(16.),
        width: px(width),
        ascent: px(12.),
        descent: px(4.),
        runs: runs
            .iter()
            .enumerate()
            .map(|(run_ix, glyphs)| ShapedRun {
                font_id: FontId(run_ix),
                glyphs: glyphs
                    .iter()
                    .map(|&(index, x)| {
                        assert!(index < text.len() && text.is_char_boundary(index));
                        ShapedGlyph {
                            id: GlyphId(0),
                            position: point(px(x), px(0.)),
                            index,
                            is_emoji: false,
                        }
                    })
                    .collect(),
            })
            .collect(),
        len: text.len(),
    }
}

fn byte_indices(layout: &LineLayout, boundaries: &[WrapBoundary]) -> Vec<usize> {
    boundaries
        .iter()
        .map(|boundary| layout.runs[boundary.run_ix].glyphs[boundary.glyph_ix].index)
        .collect()
}

#[test]
fn test_wrap_preserves_shaped_clusters() {
    let layout = shaped_layout("你好", 20., &[&[(0, 0.), (0, 5.), (3, 10.), (3, 15.)]]);
    assert_eq!(
        layout
            .compute_wrap_boundaries("你好", px(15.), None)
            .as_slice(),
        &[WrapBoundary {
            run_ix: 0,
            glyph_ix: 2,
        }],
    );

    let layout = shaped_layout("a", 20., &[&[(0, 0.), (0, 10.)]]);
    assert!(
        layout
            .compute_wrap_boundaries("a", px(10.), None)
            .is_empty()
    );
}

#[test]
fn test_wrap_right_to_left_text() {
    for (text, indices, expected_boundaries) in [
        ("אבג דהו", vec![11, 9, 7, 6, 4, 2, 0], vec![4]),
        ("a אבג b", vec![0, 1, 6, 4, 2, 8, 9], vec![2, 6]),
    ] {
        let glyphs = indices
            .iter()
            .enumerate()
            .map(|(position, index)| (*index, position as f32 * 10.))
            .collect::<Vec<_>>();
        let layout = shaped_layout(text, indices.len() as f32 * 10., &[&glyphs]);
        assert_eq!(
            layout.compute_wrap_boundaries(text, px(40.), None),
            expected_boundaries
                .into_iter()
                .map(|glyph_ix| WrapBoundary {
                    run_ix: 0,
                    glyph_ix,
                })
                .collect::<SmallVec<[WrapBoundary; 1]>>(),
            "{text:?}",
        );
    }
}

#[test]
fn test_wrap_ignores_breaks_inside_shaped_clusters() {
    let text = "x a-bcde";
    let glyphs = [0, 1, 2, 5, 6, 7].map(|index| (index, index as f32 * 10.));
    for runs in [vec![glyphs.as_slice()], vec![&glyphs[..3], &glyphs[3..]]] {
        let layout = shaped_layout(text, 80., &runs);
        assert_eq!(
            layout
                .compute_wrap_boundaries(text, px(60.), None)
                .as_slice(),
            &[WrapBoundary {
                run_ix: 0,
                glyph_ix: 2,
            }],
        );
    }
}

#[test]
fn wrap_keeps_shaped_clusters_whole_across_font_runs() {
    let text = "你好";
    let layout = shaped_layout(
        text,
        20.,
        &[&[(0, 0.)], &[], &[(0, 5.), (3, 10.)], &[(3, 15.)]],
    );
    for wrap_width in [0., 10., 15.] {
        assert_eq!(
            layout
                .compute_wrap_boundaries(text, px(wrap_width), None)
                .as_slice(),
            &[WrapBoundary {
                run_ix: 2,
                glyph_ix: 1,
            }],
            "wrap width {wrap_width}",
        );
    }

    let layout = shaped_layout("a", 20., &[&[(0, 0.)], &[], &[(0, 10.)]]);
    assert!(
        layout
            .compute_wrap_boundaries("a", px(10.), None)
            .is_empty()
    );
}

#[test]
fn wrap_uses_original_utf8_offsets_across_font_runs() {
    let text = "é中🙂文";
    let glyphs = [(0, 0.), (2, 10.), (5, 20.), (9, 30.)];
    let whole = shaped_layout(text, 40., &[&glyphs]);
    let chunked = shaped_layout(text, 40., &[&glyphs[..2], &[], &glyphs[2..3], &glyphs[3..]]);
    let whole_boundaries = whole.compute_wrap_boundaries(text, px(20.), None);
    let chunked_boundaries = chunked.compute_wrap_boundaries(text, px(20.), None);
    assert_eq!(byte_indices(&whole, &whole_boundaries), vec![5]);
    assert_eq!(
        byte_indices(&chunked, &chunked_boundaries),
        byte_indices(&whole, &whole_boundaries),
    );
    assert_eq!(
        chunked_boundaries.as_slice(),
        &[WrapBoundary {
            run_ix: 2,
            glyph_ix: 0,
        }],
    );
    let index = byte_indices(&chunked, &chunked_boundaries)[0];
    assert_eq!((&text[..index], &text[index..]), ("é中", "🙂文"));
}

#[test]
fn wrap_preserves_zero_advance_combining_glyphs_and_emoji_clusters() {
    for (text, next_cluster) in [("a\u{301}b", 3), ("👍🏽z", 8), ("👩‍💻z", 11)] {
        let layout = shaped_layout(text, 20., &[&[(0, 0.), (0, 0.)], &[(next_cluster, 10.)]]);
        let boundaries = layout.compute_wrap_boundaries(text, px(5.), None);
        assert_eq!(byte_indices(&layout, &boundaries), vec![next_cluster]);
    }
}

#[test]
fn emergency_wrap_keeps_source_graphemes_whole_across_distinct_shaped_indices() {
    for (text, runs, width) in [
        (
            "e\u{301}x",
            vec![vec![(0, 0.)], vec![(1, 10.), (3, 20.)]],
            30.,
        ),
        ("👍🏽x", vec![vec![(0, 0.)], vec![(4, 10.), (8, 20.)]], 30.),
        (
            "👩\u{200d}💻x",
            vec![vec![(0, 0.)], vec![(4, 10.), (7, 10.), (11, 20.)]],
            30.,
        ),
    ] {
        let run_slices = runs.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let layout = shaped_layout(text, width, &run_slices);
        let boundaries = layout.compute_wrap_boundaries(text, px(10.), None);
        assert_eq!(
            byte_indices(&layout, &boundaries),
            [text.len() - 1],
            "{text:?}"
        );
    }
}

#[test]
fn rtl_wrap_moves_the_first_visual_mark_with_its_source_grapheme() {
    let text = "אב\u{05b7}ג";
    let layout = shaped_layout(text, 30., &[&[(6, 0.)], &[(4, 10.)], &[(2, 10.), (0, 20.)]]);
    let boundaries = layout.compute_wrap_boundaries(text, px(10.), None);
    assert_eq!(
        boundaries.as_slice(),
        &[
            WrapBoundary {
                run_ix: 1,
                glyph_ix: 0
            },
            WrapBoundary {
                run_ix: 2,
                glyph_ix: 1
            }
        ]
    );
    let row = |position: WrapBoundary| {
        boundaries
            .iter()
            .filter(|boundary| **boundary <= position)
            .count()
    };
    assert_eq!(
        row(WrapBoundary {
            run_ix: 1,
            glyph_ix: 0
        }),
        row(WrapBoundary {
            run_ix: 2,
            glyph_ix: 0
        })
    );
}

#[test]
fn wrap_clamps_complete_clusters_without_adding_an_empty_first_row() {
    let empty = shaped_layout("", 0., &[]);
    assert!(empty.compute_wrap_boundaries("", px(0.), None).is_empty());

    let text = "你好世界";
    let layout = shaped_layout(text, 40., &[&[(0, 0.), (3, 10.), (6, 20.), (9, 30.)]]);
    let boundaries = layout.compute_wrap_boundaries(text, px(10.), None);
    assert_eq!(byte_indices(&layout, &boundaries), vec![3, 6, 9]);
    assert!(
        layout
            .compute_wrap_boundaries(text, px(40.), None)
            .is_empty()
    );
    for max_lines in 0usize..=4 {
        let clamped = layout.compute_wrap_boundaries(text, px(10.), Some(max_lines));
        assert_eq!(
            clamped.as_slice(),
            &boundaries[..max_lines.saturating_sub(1)],
            "maximum lines {max_lines}",
        );
    }
}

#[test]
fn rejected_wrapped_candidates_retain_unicode_boundaries_and_font_epoch_on_retry() {
    let source = "é a-bcde";
    let runs = [FontRun {
        len: source.len(),
        font_id: FontId(1),
    }];
    for install in [false, true] {
        let (_, text_system, window) = fixture();
        let cache = &window.line_layout_cache;
        let committed = cache.layout_wrapped_line(source, px(10.), &runs, Some(px(36.)), None, 0);
        assert_eq!(
            byte_indices(&committed.unwrapped_layout, committed.wrap_boundaries()),
            vec![5],
        );
        cache.finish_frame();
        if install {
            text_system.add_fonts(vec![Cow::Borrowed(b"font")]).unwrap();
        }
        let generation = text_system.font_generation();
        let candidate =
            cache.layout_wrapped_line(source, px(10.), &runs, Some(px(36.)), None, generation);
        assert_eq!(Arc::ptr_eq(&committed, &candidate), !install);
        let expected = if install { vec![3, 5, 8] } else { vec![5] };
        assert_eq!(
            byte_indices(&candidate.unwrapped_layout, candidate.wrap_boundaries()),
            expected,
        );
        cache.abort_frame();
        {
            let state = cache.state();
            assert_eq!(state.font_generation, generation);
            assert!(state.current_frame.wrapped_lines.is_empty());
            assert!(state.current_frame.used_wrapped_lines.is_empty());
            assert_eq!(state.previous_frame.wrapped_lines.len(), 1);
        }
        let retried =
            cache.layout_wrapped_line(source, px(10.), &runs, Some(px(36.)), None, generation);
        assert!(Arc::ptr_eq(&candidate, &retried));
        assert_eq!(
            byte_indices(&retried.unwrapped_layout, retried.wrap_boundaries()),
            expected,
        );
        cache.finish_frame();
        let admitted =
            cache.layout_wrapped_line(source, px(10.), &runs, Some(px(36.)), None, generation);
        assert!(Arc::ptr_eq(&retried, &admitted));
    }
}
