use super::*;
use open_gpui::{TextRun, TextSystem, WindowTextSystem, font, px};

const IBM_PLEX: &[u8] = include_bytes!("../../../../tests/fixtures/fonts/IBMPlexSans-Regular.ttf");
const LILEX: &[u8] = include_bytes!("../../../../tests/fixtures/fonts/Lilex-Regular.ttf");
const HEBREW: &[u8] = include_bytes!("../../../../tests/fixtures/fonts/NotoSansHebrew-Regular.ttf");
const SEPARATORS: &[char] = &[
    '\n', '\r', '\u{001c}', '\u{001d}', '\u{001e}', '\u{0085}', '\u{2029}',
];

fn platform() -> Result<CosmicTextSystem> {
    let platform = CosmicTextSystem::new_without_system_fonts("IBM Plex Sans");
    platform.add_fonts(vec![Cow::Borrowed(IBM_PLEX), Cow::Borrowed(LILEX)])?;
    Ok(platform)
}

fn layout(platform: &CosmicTextSystem, text: &str) -> Result<LineLayout> {
    let font_id = platform.font_id(&font("IBM Plex Sans"))?;
    Ok(platform.layout_line(
        text,
        px(16.),
        &[FontRun {
            len: text.len(),
            font_id,
        }],
    ))
}

#[test]
fn combining_marks_are_positioned_over_their_base() -> Result<()> {
    let platform = platform()?;
    let layout = layout(&platform, "v\u{0304}v")?;
    let glyphs = layout
        .runs
        .iter()
        .flat_map(|run| run.glyphs.iter())
        .collect::<Vec<_>>();
    assert_eq!(glyphs.len(), 3);
    let base = glyphs[0].position;
    let mark = glyphs[1].position;
    let next = glyphs[2].position;
    assert!(mark.x > base.x);
    assert!(
        mark.x < next.x,
        "combining mark positioned after its base: mark.x={:?}, next.x={:?}",
        mark.x,
        next.x
    );
    Ok(())
}

#[test]
fn public_shape_text_preserves_newlines_and_mixed_direction_paragraphs() -> Result<()> {
    let text_system = WindowTextSystem::new(Arc::new(TextSystem::new(Arc::new(platform()?))));
    let source: SharedString = "first line\nא\u{001c}A".into();
    let runs = [TextRun {
        len: source.len(),
        font: font("IBM Plex Sans"),
        ..Default::default()
    }];
    let lines = text_system.shape_text(source, px(16.), &runs, None, None)?;
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].len(), "first line".len());
    assert_eq!(lines[1].len(), "א\u{001c}A".len());
    assert!(lines.iter().all(|line| line.width() > Pixels::ZERO));
    Ok(())
}

#[test]
fn every_bidi_paragraph_separator_accepts_both_direction_orders() -> Result<()> {
    let platform = platform()?;
    for separator in SEPARATORS {
        for source in [format!("א{separator}A"), format!("A{separator}א")] {
            let shaped = layout(&platform, &source)?;
            assert_eq!(shaped.len, source.len(), "{source:?}");
            assert!(shaped.width > Pixels::ZERO, "{source:?}");
            assert!(
                shaped.runs.iter().any(|run| !run.glyphs.is_empty()),
                "{source:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn leading_trailing_and_repeated_paragraph_separators_keep_complete_source() -> Result<()> {
    let platform = platform()?;
    for source in [
        "\u{001c}",
        "\u{001c}\u{001c}",
        "\u{001c}א",
        "א\u{001c}",
        "א\u{001c}\u{001c}A",
        "\u{001c}א\u{001c}A\u{001c}",
        "א\r\nA",
    ] {
        let shaped = layout(&platform, source)?;
        assert_eq!(shaped.len, source.len(), "{source:?}");
        assert!(f32::from(shaped.width).is_finite() && shaped.width >= Pixels::ZERO);
        for glyph in shaped.runs.iter().flat_map(|run| &run.glyphs) {
            assert!(glyph.index < source.len());
            assert!(source.is_char_boundary(glyph.index));
            assert!(f32::from(glyph.position.x).is_finite());
        }
    }
    Ok(())
}

#[test]
fn segments_preserve_font_runs_source_indices_and_total_width() -> Result<()> {
    let platform = platform()?;
    let proportional = platform.font_id(&font("IBM Plex Sans"))?;
    let monospace = platform.font_id(&font("Lilex"))?;
    let source = "ab\u{0085}cd\u{2029}éx";
    let runs = [
        FontRun {
            len: "ab".len(),
            font_id: proportional,
        },
        FontRun {
            len: "\u{0085}cd\u{2029}é".len(),
            font_id: monospace,
        },
        FontRun {
            len: "x".len(),
            font_id: proportional,
        },
    ];
    let shaped = platform.layout_line(source, px(16.), &runs);
    assert_eq!(shaped.len, source.len());
    let mut last_x = Pixels::ZERO;
    for (index, character) in source
        .char_indices()
        .filter(|(_, character)| character.is_alphabetic())
    {
        let (font_id, glyph) = shaped
            .runs
            .iter()
            .find_map(|run| {
                run.glyphs
                    .iter()
                    .find(|glyph| glyph.index == index)
                    .map(|glyph| (run.font_id, glyph))
            })
            .with_context(|| format!("missing original source byte {index} in {source:?}"))?;
        let expected = if index < 2 || character == 'x' {
            proportional
        } else {
            monospace
        };
        assert_eq!(font_id, expected);
        assert_eq!(Some(glyph.id), platform.glyph_for_char(expected, character));
        assert!(glyph.position.x >= last_x);
        last_x = glyph.position.x;
    }
    let pieces = [
        ("ab", proportional),
        ("\u{0085}", monospace),
        ("cd", monospace),
        ("\u{2029}", monospace),
        ("é", monospace),
        ("x", proportional),
    ];
    let expected_width = pieces
        .into_iter()
        .map(|(text, font_id)| {
            platform
                .layout_line(
                    text,
                    px(16.),
                    &[FontRun {
                        len: text.len(),
                        font_id,
                    }],
                )
                .width
        })
        .sum::<Pixels>();
    assert!((shaped.width - expected_width).abs() < px(0.001));
    Ok(())
}

#[test]
fn separator_detection_matches_all_bidi_paragraph_controls() {
    for separator in SEPARATORS {
        assert!(is_paragraph_separator(*separator));
        assert!(contains_paragraph_separator(&format!("a{separator}b")));
    }
    for source in ["", "plain ascii", "א", "a\tb", "😀", "a\u{2028}b"] {
        assert!(!contains_paragraph_separator(source), "{source:?}");
    }
}

#[test]
fn no_separator_path_preserves_the_original_shape_exactly() -> Result<()> {
    let platform = platform()?;
    let font_id = platform.font_id(&font("IBM Plex Sans"))?;
    for source in ["", "office é", "אבג", "mixed אב", "a\tb", "a\u{2028}b"] {
        let runs = [FontRun {
            len: source.len(),
            font_id,
        }];
        let mut state = platform.0.write();
        let expected =
            state.layout_line_without_separators(source, px(16.), &runs, &mut Vec::new());
        let actual = state.layout_line(source, px(16.), &runs);
        assert_layout_matches(&actual, &expected);
    }
    Ok(())
}

fn assert_layout_matches(actual: &LineLayout, expected: &LineLayout) {
    assert_eq!(actual.len, expected.len);
    assert_eq!(actual.font_size, expected.font_size);
    assert_eq!(actual.width, expected.width);
    assert_eq!(actual.ascent, expected.ascent);
    assert_eq!(actual.descent, expected.descent);
    assert_eq!(actual.runs.len(), expected.runs.len());
    for (actual, expected) in actual.runs.iter().zip(&expected.runs) {
        assert_eq!(actual.font_id, expected.font_id);
        assert_eq!(actual.glyphs.len(), expected.glyphs.len());
        for (actual, expected) in actual.glyphs.iter().zip(&expected.glyphs) {
            assert_eq!(actual.id, expected.id);
            assert_eq!(actual.index, expected.index);
            assert_eq!(actual.position, expected.position);
            assert_eq!(actual.is_emoji, expected.is_emoji);
        }
    }
}

#[test]
fn sequential_font_slices_preserve_zero_length_runs_and_split_multibyte_separators() {
    let runs = [
        FontRun {
            len: 0,
            font_id: FontId(0),
        },
        FontRun {
            len: 5,
            font_id: FontId(1),
        },
        FontRun {
            len: 0,
            font_id: FontId(2),
        },
        FontRun {
            len: 4,
            font_id: FontId(3),
        },
    ];
    let mut cursor = FontRunCursor::new(&runs);
    assert_eq!(
        cursor.take(2).as_slice(),
        &[FontRun {
            len: 2,
            font_id: FontId(1)
        }]
    );
    assert_eq!(
        cursor.take(3).as_slice(),
        &[FontRun {
            len: 3,
            font_id: FontId(1)
        }]
    );
    assert_eq!(cursor.take(0).len(), 0);
    assert_eq!(cursor.remaining.len(), 1);
    assert_eq!(
        cursor.take(4).as_slice(),
        &[FontRun {
            len: 4,
            font_id: FontId(3)
        }]
    );
    assert!(cursor.remaining.is_empty());
    assert_eq!(cursor.offset, 0);
}

#[test]
fn actual_hebrew_segments_match_independent_shaping_and_raster_after_rebasing() -> Result<()> {
    let platform = platform()?;
    platform.add_fonts(vec![Cow::Borrowed(HEBREW)])?;
    let mut requested = font("IBM Plex Sans");
    requested.fallbacks = Some(FontFallbacks::from_fonts(vec!["Noto Sans Hebrew".into()]));
    let primary = platform.font_id(&requested)?;
    let mono = platform.font_id(&font("Lilex"))?;
    let source = "é\u{001c}א\u{05b7}ב\u{2029}office";
    let pieces = ["é", "\u{001c}", "א\u{05b7}ב", "\u{2029}", "office"];
    let runs = [
        FontRun {
            len: "é\u{001c}א\u{05b7}ב\u{2029}".len(),
            font_id: primary,
        },
        FontRun {
            len: "office".len(),
            font_id: mono,
        },
    ];
    let actual = platform.layout_line(source, px(20.), &runs);
    let mut expected = LineLayout {
        len: source.len(),
        font_size: px(20.),
        ..Default::default()
    };
    let mut source_offset = 0;
    for (index, piece) in pieces.into_iter().enumerate() {
        let id = if index == 4 { mono } else { primary };
        let piece_layout = platform.layout_line(
            piece,
            px(20.),
            &[FontRun {
                len: piece.len(),
                font_id: id,
            }],
        );
        for mut run in piece_layout.runs {
            for glyph in &mut run.glyphs {
                glyph.index += source_offset;
                glyph.position.x += expected.width;
            }
            if let Some(last) = expected
                .runs
                .last_mut()
                .filter(|last| last.font_id == run.font_id)
            {
                last.glyphs.append(&mut run.glyphs);
            } else {
                expected.runs.push(run);
            }
        }
        expected.width += piece_layout.width;
        expected.ascent = expected.ascent.max(piece_layout.ascent);
        expected.descent = expected.descent.max(piece_layout.descent);
        source_offset += piece.len();
    }
    assert_layout_matches(&actual, &expected);
    let mut hebrew_glyphs = 0;
    for run in &actual.runs {
        let state = platform.0.read();
        let loaded = state.loaded_font(run.font_id);
        let family = &state
            .font_system
            .db()
            .face(loaded.font.id())
            .context("the glyph face must stay loaded")?
            .families[0]
            .0;
        if family != "Noto Sans Hebrew" {
            continue;
        }
        drop(state);
        for glyph in &run.glyphs {
            assert_ne!(glyph.id.0, 0);
            assert!(source.is_char_boundary(glyph.index));
            hebrew_glyphs += 1;
            for scale_factor in [1., 1.5] {
                let params = RenderGlyphParams {
                    font_id: run.font_id,
                    glyph_id: glyph.id,
                    font_size: px(20.),
                    subpixel_variant: point(0, 0),
                    scale_factor,
                    is_emoji: glyph.is_emoji,
                    subpixel_rendering: false,
                    dilation: 0,
                };
                let bounds = platform.glyph_raster_bounds(&params)?;
                let raster = platform.rasterize_glyph(&params, bounds)?;
                assert_eq!(raster.0, bounds.size);
                assert!(raster.1.iter().any(|byte| *byte != 0));
                assert_eq!(platform.rasterize_glyph(&params, bounds)?, raster);
            }
        }
    }
    assert!(
        hebrew_glyphs >= 3,
        "Hebrew bases and zero-advance marks must all retain actual glyphs"
    );
    Ok(())
}

#[test]
fn paragraph_missing_reports_retain_complete_original_graphemes_and_source_classes() -> Result<()> {
    #[derive(Default)]
    struct Reports {
        missing: parking_lot::Mutex<Vec<MissingGlyph>>,
        calls: std::sync::atomic::AtomicUsize,
    }
    impl MissingGlyphSink for Reports {
        fn report(&self, missing: Vec<MissingGlyph>) {
            self.calls
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.missing.lock().extend(missing);
        }
    }
    let platform = platform()?;
    let proportional = platform.font_id(&font("IBM Plex Sans"))?;
    let monospace = platform.font_id(&font("Lilex"))?;
    let reports = Arc::new(Reports::default());
    platform.set_missing_glyph_sink(Some(reports.clone()));
    let source = "界\u{0301}\r\n👩‍💻\u{2029}界\u{0301}";
    let runs = [
        FontRun {
            len: "界\u{0301}\r\n".len(),
            font_id: proportional,
        },
        FontRun {
            len: "👩‍💻\u{2029}界\u{0301}".len(),
            font_id: monospace,
        },
    ];
    let shaped = platform.layout_line(source, px(16.), &runs);
    assert_eq!(shaped.len, source.len());
    assert_eq!(reports.calls.load(std::sync::atomic::Ordering::Relaxed), 1);
    let reports = reports.missing.lock();
    for report in reports.iter() {
        assert!(
            source
                .graphemes(true)
                .any(|grapheme| grapheme == report.grapheme()),
            "fragmented report {report:?}"
        );
    }
    for expected in [
        MissingGlyph::new("\r\n".into(), FallbackFontClass::Proportional),
        MissingGlyph::new("界\u{0301}".into(), FallbackFontClass::Proportional),
        MissingGlyph::new("👩‍💻".into(), FallbackFontClass::Monospace),
        MissingGlyph::new("界\u{0301}".into(), FallbackFontClass::Monospace),
    ] {
        assert!(
            reports.contains(&expected),
            "missing complete source report {expected:?}: {reports:?}"
        );
    }
    Ok(())
}
