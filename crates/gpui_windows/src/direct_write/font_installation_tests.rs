use super::{DirectWriteTextSystem, font_face_to_font};
use crate::directx_devices::DirectXDevices;
use anyhow::{Context, Result};
use open_gpui::{
    FontFallbacks, FontFeatures, FontRun, PlatformTextSystem, RenderGlyphParams, SharedString,
    TextRun, TextSystem, WindowTextSystem, font, point, px,
};
use std::{borrow::Cow, sync::Arc};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_SIMULATIONS_BOLD, DWRITE_FONT_SIMULATIONS_NONE, DWRITE_FONT_STRETCH_NORMAL,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_BOLD,
};
use windows::core::HSTRING;

const LILEX_REGULAR: &[u8] = include_bytes!("../../../../tests/fixtures/fonts/Lilex-Regular.ttf");
const LILEX_BOLD: &[u8] = include_bytes!("../../../../tests/fixtures/fonts/Lilex-Bold.ttf");
const IBM_PLEX: &[u8] = include_bytes!("../../../../tests/fixtures/fonts/IBMPlexSans-Regular.ttf");
const NOTO_EMOJI: &[u8] = include_bytes!("../../../../tests/fixtures/fonts/NotoEmoji-Regular.ttf");

#[test]
fn loading_an_alias_preserves_the_first_observed_face_identity() -> Result<()> {
    let devices = DirectXDevices::new()?;
    let platform = DirectWriteTextSystem::new(Some(&devices))?;
    platform.add_fonts(vec![Cow::Borrowed(IBM_PLEX)])?;
    let descriptor = font("IBM Plex Sans");
    let original = platform.font_id(&descriptor)?;
    let runs = [FontRun {
        len: 2,
        font_id: original,
    }];
    let before = platform.layout_line("mi", px(18.), &runs);
    assert_eq!(before.runs.len(), 1);
    assert_eq!(before.runs[0].font_id, original);
    let alias = platform.font_id(&font(".ZedSans"))?;
    assert_ne!(alias, original);
    assert_eq!(platform.font_id(&descriptor)?, original);
    let after = platform.layout_line("mi", px(18.), &runs);
    assert_eq!(after.runs.len(), 1);
    assert_eq!(after.runs[0].font_id, original);
    assert_eq!(after.width, before.width);
    assert_eq!(after.runs[0].glyphs.len(), before.runs[0].glyphs.len());
    for (actual, expected) in after.runs[0].glyphs.iter().zip(&before.runs[0].glyphs) {
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.position, expected.position);
        assert_eq!(actual.index, expected.index);
    }
    Ok(())
}

#[test]
fn installed_weight_reselects_a_matched_family_and_preserves_old_glyphs() -> Result<()> {
    let devices = DirectXDevices::new()?;
    let platform = Arc::new(DirectWriteTextSystem::new(Some(&devices))?);
    let text = Arc::new(TextSystem::new(platform.clone()));
    text.add_fonts(vec![Cow::Borrowed(LILEX_REGULAR)])?;
    let descriptor = font("Lilex").bold();
    let old_id = text.resolve_font(&descriptor);
    assert_eq!(
        unsafe {
            platform.state.read().fonts[old_id.0]
                .font_face
                .GetSimulations()
        },
        DWRITE_FONT_SIMULATIONS_BOLD,
    );
    let glyph = platform
        .glyph_for_char(old_id, 'm')
        .context("Lilex must contain m")?;
    let old_advance = platform.advance(old_id, glyph)?;
    let old_bounds = platform.typographic_bounds(old_id, glyph)?;
    let old_metrics = platform.font_metrics(old_id);
    let window_text = WindowTextSystem::new(text.clone());
    let source: SharedString = "office é".into();
    let runs = [TextRun {
        len: source.len(),
        font: descriptor.clone(),
        ..Default::default()
    }];
    let old_layout = window_text.shape_line(source.clone(), px(18.), &runs, None);
    assert!(old_layout.runs.iter().all(|run| run.font_id == old_id));

    text.add_fonts(vec![Cow::Borrowed(LILEX_BOLD)])?;
    let new_id = text.resolve_font(&descriptor);
    assert_eq!(
        unsafe { platform.state.read().fonts[new_id.0].font_face.GetWeight() },
        DWRITE_FONT_WEIGHT_BOLD,
        "installing a requested weight must replace the earlier closest family match",
    );
    assert_eq!(
        unsafe {
            platform.state.read().fonts[new_id.0]
                .font_face
                .GetSimulations()
        },
        DWRITE_FONT_SIMULATIONS_NONE,
        "the newly installed face must replace DirectWrite's synthesized Bold",
    );
    assert_ne!(new_id, old_id);
    let new_layout = window_text.shape_line(source, px(18.), &runs, None);
    assert!(!Arc::ptr_eq(&*old_layout, &*new_layout));
    assert!(new_layout.runs.iter().all(|run| run.font_id == new_id));
    assert_eq!(platform.advance(old_id, glyph)?, old_advance);
    assert_eq!(platform.typographic_bounds(old_id, glyph)?, old_bounds);
    let retained_metrics = platform.font_metrics(old_id);
    assert_eq!(retained_metrics.units_per_em, old_metrics.units_per_em);
    assert_eq!(retained_metrics.ascent, old_metrics.ascent);
    assert_eq!(retained_metrics.descent, old_metrics.descent);
    assert!(old_layout.runs.iter().all(|run| run.font_id == old_id));
    assert_eq!(
        unsafe {
            platform.state.read().fonts[old_id.0]
                .font_face
                .GetSimulations()
        },
        DWRITE_FONT_SIMULATIONS_BOLD,
    );
    Ok(())
}

#[test]
fn installed_named_fallback_reselects_the_primary_and_shapes_the_private_face() -> Result<()> {
    let devices = DirectXDevices::new()?;
    let platform = Arc::new(DirectWriteTextSystem::new(Some(&devices))?);
    let text = Arc::new(TextSystem::new(platform.clone()));
    text.add_fonts(vec![Cow::Borrowed(IBM_PLEX)])?;
    let mut descriptor = font("IBM Plex Sans");
    descriptor.fallbacks = Some(FontFallbacks::from_fonts(vec!["Noto Emoji".into()]));
    let old_id = text.resolve_font(&descriptor);
    let source: SharedString = "A😀B".into();
    let runs = [TextRun {
        len: source.len(),
        font: descriptor.clone(),
        ..Default::default()
    }];
    let window_text = WindowTextSystem::new(text.clone());
    let before = window_text.shape_line(source.clone(), px(24.), &runs, None);
    let before_fallback = before
        .runs
        .iter()
        .find(|run| run.glyphs.iter().any(|glyph| glyph.index == 1))
        .context("the baseline must retain the emoji's source index")?;
    let before_family = {
        let state = platform.state.read();
        font_face_to_font(
            &state.fonts[before_fallback.font_id.0].font_face,
            &platform.components.locale,
        )
        .context("the baseline fallback face must have a family")?
        .family
    };
    assert_ne!(
        before_family.as_ref(),
        "Noto Emoji",
        "the named fallback fixture must be absent from the OS font collection initially",
    );
    let old_glyph = platform
        .glyph_for_char(old_id, 'A')
        .context("IBM Plex must contain A")?;
    let old_advance = platform.advance(old_id, old_glyph)?;

    text.add_fonts(vec![Cow::Borrowed(NOTO_EMOJI)])?;
    let expected = text.resolve_font(&font("Noto Emoji"));
    let after = window_text.shape_line(source, px(24.), &runs, None);
    let (fallback, glyph) = after
        .runs
        .iter()
        .find_map(|run| {
            run.glyphs
                .iter()
                .find(|glyph| glyph.index == 1)
                .map(|glyph| (run.font_id, glyph))
        })
        .context("the installed fallback must preserve the emoji's source index")?;
    assert_eq!(
        fallback, expected,
        "named private fallback must precede system fallback"
    );
    assert_eq!(
        glyph.id,
        platform
            .glyph_for_char(expected, '😀')
            .context("Noto must contain emoji")?
    );
    assert_ne!(glyph.id.0, 0);
    assert_eq!(
        after
            .runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .map(|glyph| glyph.index)
            .collect::<Vec<_>>(),
        [0, 1, 5],
    );
    for scale_factor in [1., 1.5] {
        let params = RenderGlyphParams {
            font_id: fallback,
            glyph_id: glyph.id,
            font_size: px(24.),
            subpixel_variant: point(0, 0),
            scale_factor,
            is_emoji: glyph.is_emoji,
            subpixel_rendering: false,
            dilation: 0,
        };
        let bounds = platform.glyph_raster_bounds(&params)?;
        assert!(bounds.size.width.0 > 0 && bounds.size.height.0 > 0);
        let (size, bytes) = platform.rasterize_glyph(&params, bounds)?;
        assert_eq!(size, bounds.size);
        assert!(bytes.iter().any(|byte| *byte != 0));
        assert_eq!(platform.rasterize_glyph(&params, bounds)?, (size, bytes));
    }
    assert_eq!(platform.advance(old_id, old_glyph)?, old_advance);
    Ok(())
}

#[test]
fn failed_batch_keeps_the_published_collection_until_a_later_success() -> Result<()> {
    let devices = DirectXDevices::new()?;
    let platform = Arc::new(DirectWriteTextSystem::new(Some(&devices))?);
    let text = TextSystem::new(platform.clone());
    text.add_fonts(vec![Cow::Borrowed(LILEX_REGULAR)])?;
    let descriptor = font("Lilex").bold();
    let before = text.resolve_font(&descriptor);
    let before_count = unsafe {
        platform
            .state
            .read()
            .custom_font_collection
            .GetFontSet()?
            .GetFontCount()
    };
    assert!(
        text.add_fonts(vec![
            Cow::Borrowed(LILEX_BOLD),
            Cow::Borrowed(b"not a font")
        ])
        .is_err()
    );
    assert_eq!(text.resolve_font(&descriptor), before);
    assert_eq!(
        unsafe {
            platform
                .state
                .read()
                .custom_font_collection
                .GetFontSet()?
                .GetFontCount()
        },
        before_count,
    );
    assert_eq!(
        unsafe {
            platform.state.read().fonts[before.0]
                .font_face
                .GetSimulations()
        },
        DWRITE_FONT_SIMULATIONS_BOLD,
    );
    text.add_fonts(Vec::new())?;
    let after = text.resolve_font(&descriptor);
    assert_eq!(
        unsafe { platform.state.read().fonts[after.0].font_face.GetWeight() },
        DWRITE_FONT_WEIGHT_BOLD,
    );
    assert_eq!(
        unsafe {
            platform.state.read().fonts[after.0]
                .font_face
                .GetSimulations()
        },
        DWRITE_FONT_SIMULATIONS_NONE,
    );
    assert_eq!(
        unsafe {
            platform.state.read().fonts[before.0]
                .font_face
                .GetSimulations()
        },
        DWRITE_FONT_SIMULATIONS_BOLD,
    );
    Ok(())
}

#[test]
fn a_cold_native_face_cache_preserves_a_retained_synthetic_run_after_real_bold_installation()
-> Result<()> {
    let devices = DirectXDevices::new()?;
    let platform = DirectWriteTextSystem::new(Some(&devices))?;
    platform.add_fonts(vec![Cow::Borrowed(LILEX_REGULAR)])?;
    let descriptor = font("Lilex").bold();
    let old_id = platform.font_id(&descriptor)?;
    let old_glyph = platform
        .glyph_for_char(old_id, 'm')
        .context("the simulated face must contain m")?;
    let old_advance = platform.advance(old_id, old_glyph)?;
    let old_bounds = platform.typographic_bounds(old_id, old_glyph)?;
    platform.add_fonts(vec![Cow::Borrowed(LILEX_BOLD)])?;
    platform.state.write().font_to_font_id.remove(&descriptor);
    let real_id = platform.font_id(&descriptor)?;
    assert_ne!(real_id, old_id);
    assert_eq!(
        unsafe {
            platform.state.read().fonts[real_id.0]
                .font_face
                .GetSimulations()
        },
        DWRITE_FONT_SIMULATIONS_NONE,
    );
    platform.state.write().font_info_cache.clear();

    let retained = platform.layout_line(
        "m",
        px(18.),
        &[FontRun {
            len: 1,
            font_id: old_id,
        }],
    );
    assert_eq!(retained.runs.len(), 1);
    let actual_id = retained.runs[0].font_id;
    let actual_glyph = retained.runs[0].glyphs[0].id;
    assert_eq!(
        unsafe {
            platform.state.read().fonts[actual_id.0]
                .font_face
                .GetSimulations()
        },
        DWRITE_FONT_SIMULATIONS_BOLD,
        "a callback's actual simulated face must not be rebound to a newer named selection",
    );
    assert_eq!(actual_glyph, old_glyph);
    assert_eq!(platform.advance(actual_id, actual_glyph)?, old_advance);
    assert_eq!(
        platform.typographic_bounds(actual_id, actual_glyph)?,
        old_bounds
    );
    let repeated = platform.layout_line(
        "m",
        px(18.),
        &[FontRun {
            len: 1,
            font_id: actual_id,
        }],
    );
    assert_eq!(repeated.runs.len(), 1);
    assert_eq!(repeated.runs[0].glyphs[0].id, old_glyph);
    assert_eq!(
        platform.advance(repeated.runs[0].font_id, old_glyph)?,
        old_advance
    );
    Ok(())
}

#[test]
fn unchanged_selections_reuse_loaded_ids_and_keep_feature_configs_after_multiple_installs()
-> Result<()> {
    let devices = DirectXDevices::new()?;
    let platform = DirectWriteTextSystem::new(Some(&devices))?;
    platform.add_fonts(vec![Cow::Borrowed(IBM_PLEX)])?;
    let descriptors = [
        font("IBM Plex Sans"),
        open_gpui::Font {
            features: FontFeatures(Arc::new(vec![("liga".into(), 0)])),
            ..font("IBM Plex Sans")
        },
    ];
    let before = [
        platform.font_id(&descriptors[0])?,
        platform.font_id(&descriptors[1])?,
    ];
    assert_ne!(before[0], before[1]);
    let loaded = platform.state.read().fonts.len();
    platform.add_fonts(vec![Cow::Borrowed(LILEX_REGULAR)])?;
    platform.add_fonts(vec![Cow::Borrowed(NOTO_EMOJI)])?;
    for (index, descriptor) in descriptors.iter().enumerate() {
        let font_id = platform.font_id(descriptor)?;
        assert_eq!(font_id, before[index]);
        let layout = platform.layout_line("fi", px(18.), &[FontRun { len: 2, font_id }]);
        assert_eq!(
            layout
                .runs
                .iter()
                .map(|run| run.glyphs.len())
                .sum::<usize>(),
            if index == 0 { 1 } else { 2 },
        );
    }
    assert_eq!(platform.state.read().fonts.len(), loaded);
    Ok(())
}

#[test]
fn system_collection_refresh_revalidates_existing_selections_without_losing_loaded_ids()
-> Result<()> {
    let devices = DirectXDevices::new()?;
    let platform = DirectWriteTextSystem::new(Some(&devices))?;
    platform.add_fonts(vec![Cow::Borrowed(IBM_PLEX)])?;
    let descriptor = font("IBM Plex Sans");
    let original = platform.font_id(&descriptor)?;
    let generation = platform.state.read().collection_generation;
    assert!(
        platform
            .font_id(&font("Open GPUI missing font refresh fixture 9C823A"))
            .is_err(),
    );
    {
        let state = platform.state.read();
        assert!(state.collection_generation > generation);
        assert!(state.cached_font(&descriptor).is_none());
    }
    assert_eq!(platform.font_id(&descriptor)?, original);
    assert_eq!(
        platform.state.read().cached_font(&descriptor),
        Some(original)
    );
    let glyph = platform
        .glyph_for_char(original, 'm')
        .context("IBM Plex must retain m")?;
    assert_ne!(glyph.0, 0);
    assert!(platform.advance(original, glyph)?.width > 0.);
    Ok(())
}

#[test]
fn an_unregistered_native_synthetic_face_keeps_its_simulation_when_shaped_again() -> Result<()> {
    let devices = DirectXDevices::new()?;
    let platform = DirectWriteTextSystem::new(Some(&devices))?;
    platform.add_fonts(vec![Cow::Borrowed(LILEX_REGULAR)])?;
    let native = unsafe {
        platform
            .state
            .read()
            .custom_font_collection
            .GetFontSet()?
            .GetMatchingFonts(
                &HSTRING::from("Lilex"),
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STRETCH_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
            )?
            .GetFontFaceReference(0)?
            .CreateFontFace()?
    };
    assert_eq!(
        unsafe { native.GetSimulations() },
        DWRITE_FONT_SIMULATIONS_BOLD
    );
    assert!(platform.state.read().fonts.is_empty());
    let observed = platform
        .state
        .write()
        .cache_observed_font_face(&platform.components, &native)?;
    let glyph = platform
        .glyph_for_char(observed, 'm')
        .context("the native simulated face must contain m")?;
    let advance = platform.advance(observed, glyph)?;
    let bounds = platform.typographic_bounds(observed, glyph)?;
    let shaped = platform.layout_line(
        "m",
        px(18.),
        &[FontRun {
            len: 1,
            font_id: observed,
        }],
    );
    assert_eq!(shaped.runs.len(), 1);
    let actual = shaped.runs[0].font_id;
    assert_eq!(
        unsafe {
            platform.state.read().fonts[actual.0]
                .font_face
                .GetSimulations()
        },
        DWRITE_FONT_SIMULATIONS_BOLD
    );
    assert_eq!(shaped.runs[0].glyphs[0].id, glyph);
    assert_eq!(platform.advance(actual, glyph)?, advance);
    assert_eq!(platform.typographic_bounds(actual, glyph)?, bounds);
    Ok(())
}
