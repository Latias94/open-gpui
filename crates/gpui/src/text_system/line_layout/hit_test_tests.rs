use super::*;

fn unwrapped_layout(text: &str, width: f32, runs: &[&[(usize, f32)]]) -> Arc<LineLayout> {
    Arc::new(LineLayout {
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
    })
}

fn multibyte_layout() -> WrappedLineLayout {
    WrappedLineLayout {
        unwrapped_layout: unwrapped_layout(
            "aé中🙂z",
            50.,
            &[&[(0, 0.), (1, 10.)], &[(3, 20.), (6, 30.)], &[(10, 40.)]],
        ),
        wrap_boundaries: SmallVec::from_slice(&[
            WrapBoundary {
                run_ix: 1,
                glyph_ix: 0,
            },
            WrapBoundary {
                run_ix: 2,
                glyph_ix: 0,
            },
        ]),
        wrap_width: Some(px(20.)),
    }
}

fn overshooting_layout() -> WrappedLineLayout {
    WrappedLineLayout {
        unwrapped_layout: unwrapped_layout(
            "é中\u{200b}",
            20.,
            &[&[(0, 0.), (2, 10.)], &[(5, 20_f32.next_up().next_up())]],
        ),
        wrap_boundaries: SmallVec::from_slice(&[WrapBoundary {
            run_ix: 1,
            glyph_ix: 0,
        }]),
        wrap_width: Some(px(20.)),
    }
}

#[test]
fn first_wrapped_row_past_unwrapped_width_returns_row_end() {
    let layout = overshooting_layout();
    let line_height = px(20.);
    let row_end = &layout.runs()[1].glyphs[0];
    assert_ne!(row_end.index, layout.len());

    for x in [20., 20_f32.next_up()] {
        assert!(px(x) < row_end.position.x);
        assert_eq!(layout.unwrapped_layout.index_for_x(px(x)), None);
        let position = point(px(x), px(1.));
        assert_eq!(
            layout.closest_index_for_position(position, line_height),
            Ok(row_end.index),
        );
        assert_eq!(
            layout.index_for_position(position, line_height),
            Err(row_end.index),
        );
    }
}

#[test]
fn offset_wrapped_row_past_unwrapped_width_returns_its_multibyte_end() {
    let mut layout = overshooting_layout();
    layout.wrap_boundaries.insert(
        0,
        WrapBoundary {
            run_ix: 0,
            glyph_ix: 1,
        },
    );
    layout.wrap_width = Some(px(10.));
    let line_height = px(20.);
    let row_start_x = layout.runs()[0].glyphs[1].position.x;
    let row_end = &layout.runs()[1].glyphs[0];
    assert_eq!(row_end.index, "é中".len());

    for unwrapped_x in [20., 20_f32.next_up()] {
        let position = point(px(unwrapped_x) - row_start_x, px(21.));
        assert_eq!(position.x + row_start_x, px(unwrapped_x));
        assert!(position.x + row_start_x < row_end.position.x);
        assert_eq!(layout.unwrapped_layout.index_for_x(px(unwrapped_x)), None,);
        assert_eq!(
            layout.closest_index_for_position(position, line_height),
            Ok(row_end.index),
        );
        assert_eq!(
            layout.index_for_position(position, line_height),
            Err(row_end.index),
        );
    }
}

#[test]
fn wrapped_exact_and_closest_hit_tests_preserve_utf8_and_row_edges() {
    let layout = multibyte_layout();
    let line_height = px(20.);

    for (x, y, exact, closest) in [
        (-1., 1., Err(0), Err(0)),
        (0., 0., Ok(0), Ok(0)),
        (5., 1., Ok(0), Ok(0)),
        (6., 1., Ok(0), Ok(1)),
        (10., 1., Ok(1), Ok(1)),
        (19., 1., Ok(1), Ok(3)),
        (20., 1., Err(3), Err(3)),
        (-1., 21., Err(3), Err(3)),
        (0., 20., Ok(3), Ok(3)),
        (6., 21., Ok(3), Ok(6)),
        (19., 21., Ok(6), Ok(10)),
        (20., 21., Err(10), Err(10)),
        (-1., 41., Err(10), Err(10)),
        (0., 40., Ok(10), Ok(10)),
        (9., 41., Ok(10), Ok(11)),
        (10., 41., Err(11), Err(11)),
    ] {
        let position = point(px(x), px(y));
        assert_eq!(
            layout.index_for_position(position, line_height),
            exact,
            "exact hit at {position:?}",
        );
        assert_eq!(
            layout.closest_index_for_position(position, line_height),
            closest,
            "closest hit at {position:?}",
        );
    }
}

#[test]
fn wrapped_caret_positions_preserve_preceding_row_affinity() {
    let layout = multibyte_layout();
    let line_height = px(20.);

    for (index, x, y, hit) in [
        (0, 0., 0., Ok(0)),
        (1, 10., 0., Ok(1)),
        (3, 20., 0., Err(3)),
        (6, 10., 20., Ok(6)),
        (10, 20., 20., Err(10)),
        (11, 10., 40., Err(11)),
    ] {
        let position = point(px(x), px(y));
        assert_eq!(
            layout.position_for_index(index, line_height),
            Some(position),
            "caret for byte {index}",
        );
        assert_eq!(layout.index_for_position(position, line_height), hit);
        assert_eq!(
            layout.closest_index_for_position(position, line_height),
            hit
        );
    }
    assert_eq!(
        layout.position_for_index(layout.len() + 1, line_height),
        None
    );
}

#[test]
fn closest_index_in_the_final_glyph_uses_its_original_cluster_and_terminal_width() {
    for (text, runs, last_index, last_x) in [
        ("ab", vec![vec![(0, 0.)], vec![(1, 10.)]], 1, 10.),
        ("aé", vec![vec![(0, 0.), (1, 10.)]], 1, 10.),
        ("a🙂", vec![vec![(0, 0.)], vec![(1, 10.)]], 1, 10.),
        ("fi", vec![vec![(0, 0.)]], 0, 0.),
        ("é", vec![vec![(0, 0.)]], 0, 0.),
        ("a", vec![vec![(0, 0.)]], 0, 0.),
    ] {
        let runs = runs.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let layout = unwrapped_layout(text, 20., &runs);
        let middle = (last_x + 20.) / 2.;
        for (x, expected) in [
            (last_x, last_index),
            ((last_x + middle) / 2., last_index),
            (middle, last_index),
            ((middle + 20.) / 2., text.len()),
            (20., text.len()),
            (21., text.len()),
        ] {
            assert_eq!(
                layout.closest_index_for_x(px(x)),
                expected,
                "{text:?} at {x}"
            );
        }
    }
    assert_eq!(unwrapped_layout("", 0., &[]).closest_index_for_x(px(0.)), 0);
    assert_eq!(
        unwrapped_layout("unshaped", 0., &[]).closest_index_for_x(px(0.)),
        "unshaped".len()
    );
}
