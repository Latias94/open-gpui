use super::{
    DirectWriteTextSystem, apply_font_features, make_direct_write_tag, slice_from_nullable,
};
use crate::directx_devices::DirectXDevices;
use anyhow::Result;
use open_gpui::{
    Font, FontFeatures, FontWeight, PlatformTextSystem, TextRun, TextSystem, WindowTextSystem, px,
};
use std::sync::Arc;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_HIT_TEST_METRICS, DWRITE_TEXT_METRICS, DWRITE_TEXT_RANGE,
};

#[test]
fn tabs_preserve_native_advances_and_cursor() -> Result<()> {
    let devices = DirectXDevices::new()?;
    let platform = Arc::new(DirectWriteTextSystem::new(&devices)?);
    let text_system = WindowTextSystem::new(Arc::new(TextSystem::new(platform.clone())));
    for family in ["Consolas", "Segoe UI"] {
        let font = Font {
            family: family.into(),
            ..Default::default()
        };
        let font_id = platform.font_id(&font)?;
        for font_size in [px(12.), px(16.)] {
            let format = {
                let state = platform.state.read();
                let font = &state.fonts[font_id.0];
                unsafe {
                    platform.components.factory.CreateTextFormat(
                        &font.font_family_h,
                        &font.font_collection,
                        font.font_face.GetWeight(),
                        font.font_face.GetStyle(),
                        DWRITE_FONT_STRETCH_NORMAL,
                        font_size.as_f32(),
                        &platform.components.locale,
                    )?
                }
            };
            let tab_interval = px(unsafe { format.GetIncrementalTabStop() });
            for text in [
                "ab",
                "\t",
                "\t\t",
                "\t\tab",
                "a\tb",
                "abcd\te",
                "ab\t",
                "ab\t\t",
                "a\tb\tc",
                "é\t𐐀\tz",
                "a \tb  ",
                "אב",
                "aאבb",
                "a\tאב\tb",
            ] {
                let mut native_metrics = DWRITE_TEXT_METRICS::default();
                unsafe {
                    let layout = platform.components.factory.CreateTextLayout(
                        &text.encode_utf16().collect::<Vec<_>>(),
                        &format,
                        f32::INFINITY,
                        f32::INFINITY,
                    )?;
                    layout.GetMetrics(&mut native_metrics)?;
                }
                for split_runs in [false, true] {
                    let runs = if split_runs {
                        text.chars()
                            .map(|character| TextRun {
                                len: character.len_utf8(),
                                font: font.clone(),
                                ..Default::default()
                            })
                            .collect::<Vec<_>>()
                    } else {
                        vec![TextRun {
                            len: text.len(),
                            font: font.clone(),
                            ..Default::default()
                        }]
                    };
                    let shaped = text_system.shape_line(text.into(), font_size, &runs, None);
                    let mut remainder = shaped.clone();
                    let mut expected_x = px(0.);
                    for (index, character) in text.char_indices() {
                        let end = index + character.len_utf8();
                        let expected_end = if character == '\t' {
                            tab_interval * ((expected_x / tab_interval).floor() + 1.)
                        } else {
                            let run = TextRun {
                                len: character.len_utf8(),
                                font: font.clone(),
                                ..Default::default()
                            };
                            expected_x
                                + text_system
                                    .shape_line(
                                        character.to_string().into(),
                                        font_size,
                                        &[run],
                                        None,
                                    )
                                    .width
                        };
                        assert!(
                            (shaped.x_for_index(index) - expected_x).abs() < px(0.001),
                            "{family}, {font_size:?}, {text:?}, split={split_runs}, index={index}: expected {expected_x:?}, got {:?}",
                            shaped.x_for_index(index),
                        );
                        assert_eq!(
                            shaped.index_for_x(expected_x + px(0.01)),
                            Some(index),
                            "{family}, {text:?}, index={index}",
                        );
                        let (piece, rest) = remainder.split_at(end - index);
                        remainder = rest;
                        assert_eq!(piece.text.as_ref(), character.to_string());
                        assert!(
                            (piece.width - (expected_end - expected_x)).abs() < px(0.001),
                            "{family}, {text:?}, index={index}",
                        );
                        expected_x = expected_end;
                    }
                    assert!((shaped.width - expected_x).abs() < px(0.001));
                    assert!(
                        (shaped.width - px(native_metrics.widthIncludingTrailingWhitespace)).abs()
                            < px(0.001),
                    );
                    assert_eq!(shaped.x_for_index(text.len()), shaped.width);
                }
            }
        }
    }
    Ok(())
}

#[test]
fn tabs_preserve_distinct_font_runs_and_native_hit_positions() -> Result<()> {
    let devices = DirectXDevices::new()?;
    let platform = Arc::new(DirectWriteTextSystem::new(&devices)?);
    let fonts = [
        Font {
            family: "Consolas".into(),
            ..Default::default()
        },
        Font {
            family: "Segoe UI".into(),
            weight: FontWeight::BOLD,
            ..Default::default()
        },
    ];
    let font_ids = [platform.font_id(&fonts[0])?, platform.font_id(&fonts[1])?];
    assert_ne!(font_ids[0], font_ids[1]);
    let text = "a\tb\tc";
    let font_size = px(16.);
    let native = {
        let state = platform.state.read();
        let first = &state.fonts[font_ids[0].0];
        unsafe {
            let format = platform.components.factory.CreateTextFormat(
                &first.font_family_h,
                &first.font_collection,
                first.font_face.GetWeight(),
                first.font_face.GetStyle(),
                DWRITE_FONT_STRETCH_NORMAL,
                font_size.as_f32(),
                &platform.components.locale,
            )?;
            let layout = platform.components.factory.CreateTextLayout(
                &text.encode_utf16().collect::<Vec<_>>(),
                &format,
                f32::INFINITY,
                f32::INFINITY,
            )?;
            for (start, len, font_id) in [
                (0, 2, font_ids[0]),
                (2, 2, font_ids[1]),
                (4, 1, font_ids[0]),
            ] {
                let info = &state.fonts[font_id.0];
                let range = DWRITE_TEXT_RANGE {
                    startPosition: start,
                    length: len,
                };
                layout.SetFontCollection(&info.font_collection, range)?;
                layout.SetFontFamilyName(&info.font_family_h, range)?;
                layout.SetFontWeight(info.font_face.GetWeight(), range)?;
                layout.SetFontStyle(info.font_face.GetStyle(), range)?;
                layout.SetFontSize(
                    if start == 2 {
                        font_size.as_f32().next_up()
                    } else {
                        font_size.as_f32()
                    },
                    range,
                )?;
                layout.SetTypography(&info.features, range)?;
            }
            layout
        }
    };
    let text_system = WindowTextSystem::new(Arc::new(TextSystem::new(platform)));
    let runs = [
        TextRun {
            len: 2,
            font: fonts[0].clone(),
            ..Default::default()
        },
        TextRun {
            len: 2,
            font: fonts[1].clone(),
            ..Default::default()
        },
        TextRun {
            len: 1,
            font: fonts[0].clone(),
            ..Default::default()
        },
    ];
    let shaped = text_system.shape_line(text.into(), font_size, &runs, None);
    for (index, expected_font) in [(0, font_ids[0]), (2, font_ids[1]), (4, font_ids[0])] {
        let actual_font = shaped
            .runs
            .iter()
            .find_map(|run| {
                run.glyphs
                    .iter()
                    .any(|glyph| glyph.index == index)
                    .then_some(run.font_id)
            })
            .expect("each letter must retain its native font run");
        assert_eq!(actual_font, expected_font, "byte {index}");
    }
    let mut remainder = shaped.clone();
    for (index, character) in text.char_indices() {
        let mut x = 0.;
        let mut y = 0.;
        let mut metrics = DWRITE_HIT_TEST_METRICS::default();
        unsafe {
            native.HitTestTextPosition(index as u32, false, &mut x, &mut y, &mut metrics)?;
        }
        assert!(
            (shaped.x_for_index(index) - px(x)).abs() < px(0.001),
            "byte {index}"
        );
        assert_eq!(shaped.index_for_x(px(x + 0.01)), Some(index));
        let (piece, rest) = remainder.split_at(character.len_utf8());
        remainder = rest;
        assert_eq!(piece.text.as_ref(), character.to_string());
        assert!(
            (piece.width - px(metrics.width)).abs() < px(0.001),
            "byte {index}"
        );
    }
    let mut metrics = DWRITE_TEXT_METRICS::default();
    unsafe {
        native.GetMetrics(&mut metrics)?;
    }
    assert!((shaped.width - px(metrics.widthIncludingTrailingWhitespace)).abs() < px(0.001));
    assert_eq!(shaped.x_for_index(text.len()), shaped.width);
    Ok(())
}

#[test]
fn unicode_line_separators_preserve_shaped_line_cursor() -> Result<()> {
    check_unicode_separator_cursor('\u{2028}')
}

#[test]
fn unicode_paragraph_separators_preserve_shaped_line_cursor() -> Result<()> {
    check_unicode_separator_cursor('\u{2029}')
}

fn check_unicode_separator_cursor(separator: char) -> Result<()> {
    let devices = DirectXDevices::new()?;
    let platform = Arc::new(DirectWriteTextSystem::new(&devices)?);
    let font = Font {
        family: "Segoe UI".into(),
        ..Default::default()
    };
    platform.font_id(&font)?;
    let text_system = WindowTextSystem::new(Arc::new(TextSystem::new(platform)));
    for text in [
        ["a", "b", "c", "d", "e", "f"].join(&separator.to_string()),
        format!("{separator}ab{separator}{separator}cd{separator}"),
        format!("{separator}{separator}{separator}{separator}{separator}"),
        format!("é{separator}𐐀{separator}z"),
    ] {
        for multiple_fonts in [false, true] {
            let mut runs = vec![TextRun {
                len: text.len(),
                font: font.clone(),
                ..Default::default()
            }];
            if multiple_fonts {
                let first_len = text.chars().next().map_or(0, char::len_utf8);
                runs[0].len = first_len;
                let mut bold = font.clone();
                bold.weight = FontWeight::BOLD;
                runs.push(TextRun {
                    len: text.len() - first_len,
                    font: bold,
                    ..Default::default()
                });
            }
            let shaped = text_system.shape_line(text.clone().into(), px(14.), &runs, None);
            let mut reference_runs = runs.clone();
            let mut offset = 0;
            for run in &mut reference_runs {
                let end = offset + run.len;
                run.len = text[offset..end]
                    .chars()
                    .filter(|character| *character != separator)
                    .map(char::len_utf8)
                    .sum();
                offset = end;
            }
            let reference_text: String = text.chars().filter(|ch| *ch != separator).collect();
            let reference =
                text_system.shape_line(reference_text.into(), px(14.), &reference_runs, None);
            assert!(
                (shaped.width - reference.width).abs() < px(0.001),
                "{text:?}, multiple_fonts={multiple_fonts}",
            );
            let visible_glyphs = shaped
                .runs
                .iter()
                .flat_map(|run| run.glyphs.iter().map(|glyph| (run.font_id, glyph)))
                .filter(|(_, glyph)| !text[glyph.index..].starts_with(separator))
                .collect::<Vec<_>>();
            let reference_glyphs = reference
                .runs
                .iter()
                .flat_map(|run| run.glyphs.iter().map(|glyph| (run.font_id, glyph)))
                .collect::<Vec<_>>();
            assert_eq!(visible_glyphs.len(), reference_glyphs.len(), "{text:?}");
            for ((actual_font, actual), (expected_font, expected)) in
                visible_glyphs.into_iter().zip(reference_glyphs)
            {
                assert_eq!(actual_font, expected_font, "{text:?}");
                assert_eq!(actual.id, expected.id, "{text:?}");
                assert!((actual.position.x - expected.position.x).abs() < px(0.001));
                assert!((actual.position.y - expected.position.y).abs() < px(0.001));
                let reference_index: usize = text[..actual.index]
                    .chars()
                    .filter(|ch| *ch != separator)
                    .map(char::len_utf8)
                    .sum();
                assert_eq!(reference_index, expected.index, "{text:?}");
            }
            let mut remainder = shaped.clone();
            for character in text.chars() {
                let (piece, rest) = remainder.split_at(character.len_utf8());
                remainder = rest;
                assert_eq!(piece.text.as_ref(), character.to_string());
                assert_eq!(piece.len(), character.len_utf8());
                if character == separator {
                    assert_eq!(piece.width, px(0.));
                }
            }
        }
    }
    Ok(())
}

#[test]
fn default_typography_retains_direct_write_ligature_features() -> Result<()> {
    check_typography(FontFeatures::default(), &[1, 1, 1])
}

#[test]
fn explicit_disabled_ligature_features_override_native_defaults() -> Result<()> {
    for (tag, expected) in [
        ("liga", [0, 1, 1]),
        ("clig", [1, 0, 1]),
        ("calt", [1, 1, 0]),
    ] {
        check_typography(FontFeatures(Arc::new(vec![(tag.into(), 0)])), &expected)?;
    }
    Ok(())
}

fn check_typography(features: FontFeatures, expected: &[u32; 3]) -> Result<()> {
    let devices = DirectXDevices::new()?;
    let platform = DirectWriteTextSystem::new(&devices)?;
    let typography = unsafe { platform.components.factory.CreateTypography()? };
    apply_font_features(&typography, &features)?;
    assert_eq!(unsafe { typography.GetFontFeatureCount() }, 3);
    for (index, (tag, expected)) in ["liga", "clig", "calt"]
        .into_iter()
        .zip(expected)
        .enumerate()
    {
        let feature = unsafe { typography.GetFontFeature(index as u32)? };
        assert_eq!(feature.nameTag, make_direct_write_tag(tag));
        assert_eq!(feature.parameter, *expected, "{tag}");
    }
    Ok(())
}

#[test]
fn optional_native_arrays_reject_nonempty_null_pointers() {
    let empty = unsafe { slice_from_nullable::<u16>(std::ptr::null(), 0, "missing array") }
        .expect("a zero-length optional native array may be null");
    assert!(empty.is_empty());
    let error = unsafe { slice_from_nullable::<u16>(std::ptr::null(), 1, "missing array") }
        .expect_err("nonempty native arrays must have a pointer");
    assert_eq!(error.code(), windows::Win32::Foundation::E_INVALIDARG);
    let values = [3_u16, 5];
    let actual = unsafe { slice_from_nullable(values.as_ptr(), values.len(), "missing array") }
        .expect("a live native array retains its elements");
    assert_eq!(actual, values.as_slice());
}
