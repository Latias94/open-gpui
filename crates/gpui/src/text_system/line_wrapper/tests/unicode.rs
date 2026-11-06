use super::*;
use unicode_segmentation::UnicodeSegmentation;

fn unit_width_wrapper(text: &str) -> LineWrapper {
    let mut wrapper = build_wrapper();
    wrapper.width_for_char(' ');
    wrapper.char_widths.ascii.fill(Some(px(1.)));
    for character in text.chars().chain(std::iter::once('…')) {
        if !character.is_ascii() {
            wrapper.char_widths.other.insert(character, px(1.));
        }
    }
    wrapper
}

#[track_caller]
fn assert_run_coverage(text: &str, runs: &[TextRun]) {
    let mut offset = 0;
    for run in runs {
        offset += run.len;
        assert!(
            text.is_char_boundary(offset),
            "run ends outside a UTF-8 boundary at {offset} in {text:?}"
        );
    }
    assert_eq!(offset, text.len(), "runs must cover every output byte");
}

#[track_caller]
fn assert_truncation_preserves_source_graphemes(source: &str, result: &str) {
    let (prefix, suffix) = result.split_once('…').expect("text must be truncated");
    assert!(source.starts_with(prefix), "{source:?} -> {result:?}");
    assert!(source.ends_with(suffix), "{source:?} -> {result:?}");
    let suffix_start = source.len() - suffix.len();
    let boundaries = source
        .grapheme_indices(true)
        .map(|(index, _)| index)
        .chain(std::iter::once(source.len()))
        .collect::<Vec<_>>();
    assert!(prefix.len() <= suffix_start);
    assert!(
        boundaries.contains(&prefix.len()),
        "prefix ends inside a source grapheme: {source:?} -> {result:?}"
    );
    assert!(
        boundaries.contains(&suffix_start),
        "suffix starts inside a source grapheme: {source:?} -> {result:?}"
    );
}

#[test]
fn emergency_wrap_preserves_graphemes_across_text_fragments() {
    for cluster in ["e\u{301}", "か\u{3099}", "👍🏽", "🇯🇵", "👩\u{200d}💻"] {
        for text in [cluster.to_owned(), format!("x{cluster}y")] {
            let mut wrapper = unit_width_wrapper(&text);
            let fragments = [LineFragment::text(&text)];
            let split_fragments = text
                .split_inclusive(|_: char| true)
                .map(LineFragment::text)
                .collect::<Vec<_>>();
            let grapheme_boundaries = text
                .grapheme_indices(true)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let cluster_width = cluster.chars().count() as f32;

            for width in [0., 0.9, 1., 1.1, cluster_width - 0.1, cluster_width] {
                let boundaries = wrapper
                    .wrap_line(&fragments, px(width), IndentAdjustment::NoIndent)
                    .collect::<Vec<_>>();
                let split_boundaries = wrapper
                    .wrap_line(&split_fragments, px(width), IndentAdjustment::NoIndent)
                    .collect::<Vec<_>>();
                assert_eq!(
                    boundaries, split_boundaries,
                    "fragmentation changed {text:?} at width {width}"
                );
                let mut previous = 0;
                for boundary in &boundaries {
                    assert!(boundary.ix > previous && boundary.ix < text.len());
                    assert!(
                        grapheme_boundaries.contains(&boundary.ix),
                        "break inside a grapheme in {text:?} at width {width}: {boundaries:?}"
                    );
                    previous = boundary.ix;
                }
                if width <= 1. {
                    let expected = if text == cluster {
                        Vec::new()
                    } else {
                        vec![Boundary::new(1, 0), Boundary::new(1 + cluster.len(), 0)]
                    };
                    assert_eq!(boundaries, expected, "{text:?}, width={width}");
                }
            }

            assert!(
                wrapper
                    .wrap_line(
                        &fragments,
                        px(text.chars().count() as f32),
                        IndentAdjustment::NoIndent,
                    )
                    .next()
                    .is_none()
            );
        }
    }
}

#[test]
fn emergency_wrap_retains_grapheme_width_on_indented_rows() {
    let text = "  xe\u{301}yze\u{301}q";
    let mut wrapper = unit_width_wrapper(text);
    for (adjustment, expected) in [
        (
            IndentAdjustment::SameIndent,
            vec![Boundary::new(6, 2), Boundary::new(8, 2)],
        ),
        (
            IndentAdjustment::ExtraColumns(1),
            vec![
                Boundary::new(6, 3),
                Boundary::new(8, 3),
                Boundary::new(11, 3),
            ],
        ),
    ] {
        assert_eq!(
            wrapper
                .wrap_line(&[LineFragment::text(text)], px(5.), adjustment)
                .collect::<Vec<_>>(),
            expected,
            "{adjustment:?}"
        );
    }
}

#[test]
fn emergency_wrap_keeps_inline_elements_atomic_and_counts_their_bytes() {
    let mut wrapper = unit_width_wrapper("e\u{301}");
    for element_length in [1, 3, 32] {
        let fragments = [
            LineFragment::text("x"),
            LineFragment::text("e"),
            LineFragment::text("\u{301}"),
            LineFragment::element(px(4.), element_length),
            LineFragment::text("e"),
            LineFragment::text("\u{301}yz"),
        ];
        assert_eq!(
            wrapper
                .wrap_line(&fragments, px(2.), IndentAdjustment::NoIndent)
                .collect::<Vec<_>>(),
            [
                Boundary::new(1, 0),
                Boundary::new(4, 0),
                Boundary::new(4 + element_length, 0),
                Boundary::new(7 + element_length, 0),
            ],
            "element_length={element_length}"
        );
    }
}

#[test]
fn unicode_breaks_follow_solidus_punctuation_glue_and_cjk_rules() {
    for (text, fitting_prefix, expected_index) in [
        ("aaa required!", "aaa required", 4),
        ("aaa foo/bar", "aaa foo/", 8),
        ("aaa a-bcde", "aaa a-bc", 6),
        ("aaa (hello)", "aaa (hello", 4),
        ("aaa αβγδε", "aaa αβγδ", 4),
        ("aaa a\u{a0}b", "aaa a\u{a0}", 4),
        ("aaa a\u{2060}b", "aaa a\u{2060}", 4),
        ("aaa 你好！", "aaa 你好", 7),
    ] {
        let mut wrapper = unit_width_wrapper(text);
        let width = px(fitting_prefix.chars().count() as f32);
        let fragments = text
            .split_inclusive(|_: char| true)
            .map(LineFragment::text)
            .collect::<Vec<_>>();
        for fragments in [&[LineFragment::text(text)][..], &fragments[..]] {
            assert_eq!(
                wrapper
                    .wrap_line(fragments, width, IndentAdjustment::NoIndent)
                    .collect::<Vec<_>>(),
                [Boundary::new(expected_index, 0)],
                "{text:?}"
            );
        }
    }
}

#[test]
fn indent_policies_apply_to_every_continuation() {
    let text = "  abcdefghijkl";
    let mut wrapper = unit_width_wrapper(text);
    assert_eq!(IndentAdjustment::default(), IndentAdjustment::SameIndent);
    for (adjustment, expected) in [
        (
            IndentAdjustment::NoIndent,
            vec![Boundary::new(6, 0), Boundary::new(12, 0)],
        ),
        (
            IndentAdjustment::SameIndent,
            vec![Boundary::new(6, 2), Boundary::new(10, 2)],
        ),
        (
            IndentAdjustment::ExtraColumns(0),
            vec![Boundary::new(6, 2), Boundary::new(10, 2)],
        ),
        (
            IndentAdjustment::ExtraColumns(1),
            vec![
                Boundary::new(6, 3),
                Boundary::new(9, 3),
                Boundary::new(12, 3),
            ],
        ),
    ] {
        assert_eq!(
            wrapper
                .wrap_line(&[LineFragment::text(text)], px(6.), adjustment)
                .collect::<Vec<_>>(),
            expected,
            "{adjustment:?}"
        );
    }
}

#[test]
fn indent_is_capped_after_saturating_adjustment() {
    let mut wrapper = unit_width_wrapper("");
    let text = format!("{}{}", " ".repeat(300), "x".repeat(600));
    for adjustment in [
        IndentAdjustment::SameIndent,
        IndentAdjustment::ExtraColumns(20),
    ] {
        let boundaries = wrapper
            .wrap_line(&[LineFragment::text(&text)], px(512.), adjustment)
            .collect::<Vec<_>>();
        assert_eq!(
            boundaries,
            [
                Boundary::new(512, LineWrapper::MAX_INDENT),
                Boundary::new(768, LineWrapper::MAX_INDENT),
            ],
            "{adjustment:?}"
        );
    }

    assert_eq!(
        wrapper
            .wrap_line(
                &[LineFragment::text("  aaaaaaaa")],
                px(4.),
                IndentAdjustment::ExtraColumns(u32::MAX),
            )
            .collect::<Vec<_>>(),
        [Boundary::new(4, 0), Boundary::new(8, 0)]
    );
}

#[test]
fn extra_indent_falls_back_when_the_row_is_too_narrow() {
    let text = "      ab cd ef gh";
    let mut wrapper = unit_width_wrapper(text);
    for (extra, expected) in [
        (8, vec![Boundary::new(9, 0)]),
        (
            2,
            vec![
                Boundary::new(9, 8),
                Boundary::new(11, 8),
                Boundary::new(12, 8),
                Boundary::new(14, 8),
                Boundary::new(15, 8),
            ],
        ),
    ] {
        assert_eq!(
            wrapper
                .wrap_line(
                    &[LineFragment::text(text)],
                    px(10.),
                    IndentAdjustment::ExtraColumns(extra),
                )
                .collect::<Vec<_>>(),
            expected,
            "extra={extra}"
        );
    }
}

#[test]
fn extra_indent_reserves_carried_word_and_full_width_headroom() {
    let mut wrapper = unit_width_wrapper("🦀");
    wrapper.char_widths.other.insert('🦀', px(2.));
    for (text, extra, expected) in [
        ("a abcdefghi", 1, Boundary::new(2, 1)),
        ("a abcdefghi", 2, Boundary::new(2, 0)),
        ("a abcdefghij", 8, Boundary::new(2, 0)),
        (" 🦀🦀🦀🦀🦀🦀🦀🦀", 8, Boundary::new(17, 0)),
    ] {
        assert_eq!(
            wrapper
                .wrap_line(
                    &[LineFragment::text(text)],
                    px(10.),
                    IndentAdjustment::ExtraColumns(extra),
                )
                .collect::<Vec<_>>(),
            [expected],
            "{text:?}, extra={extra}"
        );
    }
}

#[test]
fn extra_indent_resumes_after_a_wide_inline_element() {
    let mut wrapper = unit_width_wrapper("");
    for element_length in [1, 3, 32] {
        let fragments = [
            LineFragment::text("abcdefghijk "),
            LineFragment::element(px(8.), element_length),
            LineFragment::text("z 12"),
        ];
        assert_eq!(
            wrapper
                .wrap_line(&fragments, px(10.), IndentAdjustment::ExtraColumns(8))
                .collect::<Vec<_>>(),
            [
                Boundary::new(10, 8),
                Boundary::new(12, 0),
                Boundary::new(14 + element_length, 8),
            ],
            "element_length={element_length}"
        );
    }
}

#[test]
fn oversized_carried_suffix_rejects_an_earlier_word_boundary() {
    let mut wrapper = unit_width_wrapper("");
    let fragments = [
        LineFragment::text("aaaaaaaaaaaaaaaaaaa a"),
        LineFragment::element(px(10.), 3),
    ];
    assert_eq!(
        wrapper
            .wrap_line(&fragments, px(10.), IndentAdjustment::ExtraColumns(2))
            .collect::<Vec<_>>(),
        [
            Boundary::new(10, 2),
            Boundary::new(18, 2),
            Boundary::new(21, 0),
        ]
    );
}

#[test]
fn whitespace_rows_wait_for_content_before_applying_extra_indent() {
    let mut wrapper = unit_width_wrapper("");
    for adjustment in [
        IndentAdjustment::NoIndent,
        IndentAdjustment::SameIndent,
        IndentAdjustment::ExtraColumns(8),
    ] {
        assert_eq!(
            wrapper
                .wrap_line(
                    &[LineFragment::text("                    ")],
                    px(10.),
                    adjustment,
                )
                .collect::<Vec<_>>(),
            [Boundary::new(10, 0)],
            "{adjustment:?}"
        );
    }

    for (adjustment, expected) in [
        (
            IndentAdjustment::NoIndent,
            vec![Boundary::new(10, 0), Boundary::new(20, 0)],
        ),
        (
            IndentAdjustment::SameIndent,
            vec![Boundary::new(10, 0), Boundary::new(20, 4)],
        ),
        (
            IndentAdjustment::ExtraColumns(2),
            vec![
                Boundary::new(10, 0),
                Boundary::new(20, 6),
                Boundary::new(23, 6),
            ],
        ),
    ] {
        assert_eq!(
            wrapper
                .wrap_line(
                    &[LineFragment::text("              ab cd ef gh")],
                    px(10.),
                    adjustment,
                )
                .collect::<Vec<_>>(),
            expected,
            "{adjustment:?}"
        );
    }
}

#[test]
fn middle_truncation_keeps_the_affix_with_tiny_budgets() {
    let text = "abcdef";
    let mut wrapper = unit_width_wrapper(text);
    let runs = generate_test_runs(&[2, 2, 2]);
    for (width, affix) in [(0., "…"), (0.5, "…"), (1., "…"), (1., "……")] {
        let (result, result_runs) =
            wrapper.truncate_line(text.into(), px(width), affix, &runs, TruncateFrom::Middle);
        assert_eq!(result.as_ref(), affix, "width={width}, affix={affix:?}");
        assert_eq!(result_runs.len(), 1);
        assert_run_coverage(&result, &result_runs);
    }
}

#[test]
fn middle_truncation_borrows_unchanged_runs_when_text_fits() {
    for text in ["", "é🦀中"] {
        let mut wrapper = unit_width_wrapper(text);
        let runs = generate_test_runs(&[text.len()]);
        let (result, result_runs) = wrapper.truncate_line(
            text.into(),
            px(text.chars().count() as f32),
            "…",
            &runs,
            TruncateFrom::Middle,
        );
        assert_eq!(result.as_ref(), text);
        assert!(matches!(&result_runs, Cow::Borrowed(_)));
        assert_eq!(result_runs.as_ref(), runs.as_slice());
        assert_run_coverage(&result, &result_runs);
    }
}

#[test]
fn middle_truncation_preserves_partial_and_exact_style_run_boundaries() {
    let text = "abcdefghijklmnopqr";
    let mut wrapper = unit_width_wrapper(text);
    let mut runs = generate_test_runs(&[4, 8, 6]);
    runs[0].font.weight = FontWeight::LIGHT;
    runs[1].font.weight = FontWeight::BOLD;
    runs[2].font.style = FontStyle::Italic;

    for (width, expected_text, expected_parts) in [
        (4., "ab…r", vec![(0, 5), (2, 1)]),
        (7., "abcd…qr", vec![(0, 4), (1, 3), (2, 2)]),
        (10., "abcdef…pqr", vec![(0, 4), (1, 5), (2, 3)]),
        (13., "abcdefgh…opqr", vec![(0, 4), (1, 7), (2, 4)]),
    ] {
        let (result, result_runs) =
            wrapper.truncate_line(text.into(), px(width), "…", &runs, TruncateFrom::Middle);
        assert_eq!(result.as_ref(), expected_text, "width={width}");
        let expected_runs = expected_parts
            .into_iter()
            .map(|(index, len)| TextRun {
                len,
                ..runs[index].clone()
            })
            .collect::<Vec<_>>();
        assert_eq!(result_runs.as_ref(), expected_runs.as_slice());
        assert_run_coverage(&result, &result_runs);
    }
}

#[test]
fn middle_truncation_preserves_utf8_and_exact_style_byte_coverage() {
    let text = "aé🦀中ßΩyzабвг";
    let mut wrapper = unit_width_wrapper(text);
    let mut runs = generate_test_runs(&["aé".len(), "🦀中ßΩyz".len(), "абвг".len()]);
    runs[0].font.weight = FontWeight::LIGHT;
    runs[1].font.weight = FontWeight::BOLD;
    runs[2].font.style = FontStyle::Italic;
    let (result, result_runs) =
        wrapper.truncate_line(text.into(), px(7.), "…", &runs, TruncateFrom::Middle);
    assert_eq!(result.as_ref(), "aé🦀中…вг");
    assert_eq!(
        result_runs.as_ref(),
        &[
            TextRun {
                len: "aé".len(),
                ..runs[0].clone()
            },
            TextRun {
                len: "🦀中…".len(),
                ..runs[1].clone()
            },
            TextRun {
                len: "вг".len(),
                ..runs[2].clone()
            },
        ]
    );
    assert_run_coverage(&result, &result_runs);
}

#[test]
fn middle_truncation_supports_an_empty_affix() {
    let text = "abcdefghijkl";
    let mut wrapper = unit_width_wrapper(text);
    let runs = generate_test_runs(&[4, 4, 4]);
    for (width, expected) in [(0., ""), (6., "abcdkl")] {
        let (result, result_runs) =
            wrapper.truncate_line(text.into(), px(width), "", &runs, TruncateFrom::Middle);
        assert_eq!(result.as_ref(), expected);
        assert_run_coverage(&result, &result_runs);
    }
}

#[test]
fn wrapped_middle_truncation_uses_a_single_row_budget() {
    let text = "abcdefghijklmnopqr";
    let mut wrapper = unit_width_wrapper(text);
    let runs = generate_test_runs(&[4, 8, 6]);
    let (expected_text, expected_runs) =
        wrapper.truncate_line(text.into(), px(10.), "…", &runs, TruncateFrom::Middle);
    let (result, result_runs) =
        wrapper.truncate_wrapped_line(text.into(), px(10.), 3, "…", &runs, TruncateFrom::Middle);
    assert_eq!(result, expected_text);
    assert_eq!(result_runs, expected_runs);
    assert_run_coverage(&result, &result_runs);
}

#[test]
fn wrapped_truncation_consumes_breaks_on_empty_lines() {
    let mut wrapper = unit_width_wrapper("");
    for prefix in ["", "\n", "\n\n", "hello\n\n"] {
        let text = format!("{prefix}aaa required! more words follow");
        let runs = generate_test_runs(&[text.len()]);
        let max_lines = prefix.bytes().filter(|byte| *byte == b'\n').count() + 2;
        let (result, result_runs) = wrapper.truncate_wrapped_line(
            text.into(),
            px(7.5),
            max_lines,
            "…",
            &runs,
            TruncateFrom::End,
        );
        assert_eq!(result.as_ref(), format!("{prefix}aaa requir…"));
        assert_run_coverage(&result, &result_runs);
    }
}

#[test]
fn wrapped_end_retains_the_fitting_suffix_carried_into_the_last_row() {
    for (text, expected) in [
        ("a-abcdefghijklm", "a-abcdefghi…"),
        ("  a-abcdefghijklm", "  a-abcdefg…"),
    ] {
        let mut wrapper = unit_width_wrapper(text);
        let runs = generate_test_runs(&[text.len()]);
        let (result, adjusted) =
            wrapper.truncate_wrapped_line(text.into(), px(10.), 2, "…", &runs, TruncateFrom::End);
        assert_eq!(result.as_ref(), expected);
        assert_run_coverage(&result, &adjusted);
        assert!(
            wrapper
                .wrap_line(
                    &[LineFragment::text(&result)],
                    px(10.),
                    IndentAdjustment::SameIndent
                )
                .count()
                < 2
        );
    }
}

#[test]
fn wrapped_start_uses_the_actual_row_budget_instead_of_total_width() {
    let text = "aaaa-bbbb-cccc";
    let mut wrapper = unit_width_wrapper(text);
    let runs = generate_test_runs(&[5, 5, 4]);
    let (result, adjusted) =
        wrapper.truncate_wrapped_line(text.into(), px(7.), 2, "…", &runs, TruncateFrom::Start);
    assert!(result.starts_with('…'));
    assert!(result.ends_with("cccc"));
    assert_ne!(result.as_ref(), text);
    assert_run_coverage(&result, &adjusted);
    assert!(
        wrapper
            .wrap_line(
                &[LineFragment::text(&result)],
                px(7.),
                IndentAdjustment::SameIndent
            )
            .count()
            < 2,
        "{result:?}"
    );
}

#[test]
fn middle_truncation_does_not_detach_combining_marks_at_either_cut() {
    for (text, width, expected) in [("e\u{301}abcd", 3., "…"), ("abcde\u{301}", 4., "ab…")] {
        let mut wrapper = unit_width_wrapper(text);
        let runs = generate_test_runs(&[text.len()]);
        let (result, result_runs) =
            wrapper.truncate_line(text.into(), px(width), "…", &runs, TruncateFrom::Middle);
        assert_eq!(result.as_ref(), expected, "{text:?}, width={width}");
        assert_truncation_preserves_source_graphemes(text, &result);
        assert_run_coverage(&result, &result_runs);
    }
}

#[test]
fn middle_truncation_preserves_complete_graphemes_on_both_sides() {
    for cluster in ["e\u{301}", "か\u{3099}", "👍🏽", "🇯🇵", "👩\u{200d}💻"] {
        let text = format!("{cluster}abcdefgh{cluster}");
        let mut wrapper = unit_width_wrapper(&text);
        let runs = generate_test_runs(&[cluster.len(), 8, cluster.len()]);
        let cluster_columns = cluster.chars().count();
        let fitting_width = cluster_columns as f32 * 3. + 1.;
        for width in [2., 3., 4., 5., fitting_width] {
            let (result, result_runs) = wrapper.truncate_line(
                text.clone().into(),
                px(width),
                "…",
                &runs,
                TruncateFrom::Middle,
            );
            if width == fitting_width {
                assert_eq!(
                    result.as_ref(),
                    format!("{cluster}{}…{cluster}", &"abcdefgh"[..cluster_columns])
                );
            }
            assert_truncation_preserves_source_graphemes(&text, &result);
            assert_run_coverage(&result, &result_runs);
            assert!((result.chars().count() as f32) <= width);
        }
    }
}

#[test]
fn start_and_end_truncation_cut_only_at_source_grapheme_boundaries() {
    for cluster in ["e\u{301}", "か\u{3099}", "👍🏽", "🇯🇵", "👩\u{200d}💻"] {
        let fitting_width = cluster.chars().count() as f32 + 2.;
        for (direction, text, expected_fitting) in [
            (
                TruncateFrom::End,
                format!("{cluster}abcd"),
                format!("{cluster}…"),
            ),
            (
                TruncateFrom::Start,
                format!("abcd{cluster}"),
                format!("…{cluster}"),
            ),
        ] {
            let mut wrapper = unit_width_wrapper(&text);
            let runs = generate_test_runs(&[text.len()]);
            for width in [3., fitting_width] {
                let (result, result_runs) =
                    wrapper.truncate_line(text.clone().into(), px(width), "…", &runs, direction);
                if width == fitting_width {
                    assert_eq!(
                        result.as_ref(),
                        expected_fitting.as_str(),
                        "{direction:?}, {text:?}"
                    );
                }
                assert_truncation_preserves_source_graphemes(&text, &result);
                assert_run_coverage(&result, &result_runs);
                assert!((result.chars().count() as f32) <= width);
            }
        }

        let text = cluster.repeat(2);
        let mut wrapper = unit_width_wrapper(&text);
        let runs = generate_test_runs(&[cluster.len(), cluster.len()]);
        let width = cluster.chars().count() as f32 + 1.;
        for direction in [TruncateFrom::Start, TruncateFrom::End] {
            let (result, result_runs) =
                wrapper.truncate_line(text.clone().into(), px(width), "…", &runs, direction);
            assert_truncation_preserves_source_graphemes(&text, &result);
            assert_run_coverage(&result, &result_runs);
            assert!((result.chars().count() as f32) <= width);
        }
    }
}

#[test]
fn start_truncation_with_no_content_budget_keeps_only_the_affix() {
    let text = "abcdefe\u{301}";
    let mut wrapper = unit_width_wrapper(text);
    let runs = generate_test_runs(&[3, text.len() - 3]);
    for width in [0., 0.5, 1., 1.5] {
        let (result, adjusted) =
            wrapper.truncate_line(text.into(), px(width), "…", &runs, TruncateFrom::Start);
        assert_eq!(result.as_ref(), "…", "width={width}");
        assert_run_coverage(&result, &adjusted);
    }
}

fn shaped_rows(wrapper: &LineWrapper, text: &str, runs: &[TextRun], width: Pixels) -> usize {
    crate::WindowTextSystem::new(wrapper.text_system.clone())
        .shape_text(
            text.to_owned().into(),
            wrapper.font_size,
            runs,
            Some(width),
            None,
        )
        .unwrap()
        .iter()
        .map(|line| line.wrap_boundaries().len() + 1)
        .sum()
}

#[test]
fn wrapped_start_deletes_source_before_prepending_the_affix() {
    let source = "     aaaaaaa";
    let mut wrapper = unit_width_wrapper(source);
    let runs = generate_test_runs(&[source.len()]);
    let (result, adjusted) =
        wrapper.truncate_wrapped_line(source.into(), px(7.), 2, "…", &runs, TruncateFrom::Start);
    assert_run_coverage(&result, &adjusted);
    let retained = result.strip_prefix('…').unwrap();
    assert!(source.ends_with(retained) && retained.len() < source.len());
}

#[test]
fn wrapped_single_row_truncation_accounts_for_explicit_paragraphs() {
    let source = "a\nb";
    let mut wrapper = build_wrapper();
    let runs = generate_test_runs(&[source.len()]);
    for (direction, expected) in [
        (TruncateFrom::Start, "…b"),
        (TruncateFrom::End, "a…"),
        (TruncateFrom::Middle, "a…b"),
    ] {
        let (result, adjusted) =
            wrapper.truncate_wrapped_line(source.into(), px(100.), 1, "…", &runs, direction);
        assert_eq!(result.as_ref(), expected);
        assert_run_coverage(&result, &adjusted);
        assert_eq!(shaped_rows(&wrapper, &result, &adjusted, px(100.)), 1);
    }
}

#[test]
fn wrapped_crlf_budget_matches_the_public_shaped_paragraphs() {
    let source = "abc\r\nx";
    let mut wrapper = build_wrapper();
    let runs = generate_test_runs(&[source.len()]);
    assert_eq!(shaped_rows(&wrapper, source, &runs, px(30.)), 3);
    for direction in [TruncateFrom::Start, TruncateFrom::End, TruncateFrom::Middle] {
        let (result, adjusted) =
            wrapper.truncate_wrapped_line(source.into(), px(30.), 2, "…", &runs, direction);
        assert_ne!(result.as_ref(), source);
        assert_truncation_preserves_source_graphemes(source, &result);
        assert_run_coverage(&result, &adjusted);
        assert!(
            shaped_rows(&wrapper, &result, &adjusted, px(30.)) <= 2,
            "{direction:?}, {result:?}"
        );
    }
}

#[test]
fn wrapped_middle_keeps_complete_ends_in_one_paragraph() {
    for (source, expected) in [
        ("first\nsecond\nlast", "first…last"),
        ("e\u{301}\r\n👍🏽", "e\u{301}…👍🏽"),
        ("\n", "…"),
    ] {
        let mut wrapper = build_wrapper();
        let runs = generate_test_runs(&[source.len()]);
        let (result, adjusted) = wrapper.truncate_wrapped_line(
            source.into(),
            px(300.),
            3,
            "…",
            &runs,
            TruncateFrom::Middle,
        );
        assert_eq!(result.as_ref(), expected);
        assert_truncation_preserves_source_graphemes(source, &result);
        assert_run_coverage(&result, &adjusted);
        assert_eq!(shaped_rows(&wrapper, &result, &adjusted, px(300.)), 1);
    }
}

#[test]
fn end_trimming_preserves_prepend_graphemes_with_ascii_punctuation() {
    let source = "\u{0600}.abcdefghijklmnopqrstuvwxyz";
    let mut wrapper = unit_width_wrapper(source);
    let runs = generate_test_runs(&[2, 1, source.len() - 3]);
    let (flat, adjusted) =
        wrapper.truncate_line(source.into(), px(4.), "…", &runs, TruncateFrom::End);
    assert_eq!(flat.as_ref(), "\u{0600}.…");
    assert_truncation_preserves_source_graphemes(source, &flat);
    assert_run_coverage(&flat, &adjusted);
    for width in 1..=6 {
        let (result, adjusted) = wrapper.truncate_wrapped_line(
            source.into(),
            px(width as f32),
            2,
            "…",
            &runs,
            TruncateFrom::End,
        );
        assert_truncation_preserves_source_graphemes(source, &result);
        assert_run_coverage(&result, &adjusted);
    }
}

#[test]
fn wrapped_end_truncation_preserves_graphemes_at_wrap_and_affix_cuts() {
    for cluster in ["e\u{301}", "か\u{3099}", "👍🏽", "🇯🇵", "👩\u{200d}💻"] {
        let text = format!("x{cluster}yz");
        let mut wrapper = unit_width_wrapper(&text);
        let runs = generate_test_runs(&[1, cluster.len(), 2]);
        let (result, result_runs) = wrapper.truncate_wrapped_line(
            text.clone().into(),
            px(2.),
            2,
            "…",
            &runs,
            TruncateFrom::End,
        );
        assert_truncation_preserves_source_graphemes(&text, &result);
        assert_run_coverage(&result, &result_runs);
        assert!(
            wrapper
                .wrap_line(
                    &[LineFragment::text(&result)],
                    px(2.),
                    IndentAdjustment::NoIndent,
                )
                .count()
                < 2,
            "truncated text exceeds two rows: {result:?}"
        );

        let text = format!("x\n{cluster}yzabcd");
        let runs = generate_test_runs(&[2, cluster.len(), 6]);
        let (result, result_runs) = wrapper.truncate_wrapped_line(
            text.clone().into(),
            px(cluster.chars().count() as f32 + 2.),
            2,
            "…",
            &runs,
            TruncateFrom::End,
        );
        assert_eq!(result.as_ref(), format!("x\n{cluster}y…"));
        assert_truncation_preserves_source_graphemes(&text, &result);
        assert_run_coverage(&result, &result_runs);
    }
}
