use super::*;
use open_gpui::{TextSystem, font, px};
use parking_lot::Mutex;
use swash::scale::image::{Content, Image};

const IBM_PLEX: &[u8] = include_bytes!("../../../../tests/fixtures/fonts/IBMPlexSans-Regular.ttf");
const LILEX: &[u8] = include_bytes!("../../../../tests/fixtures/fonts/Lilex-Regular.ttf");
const LILEX_BOLD: &[u8] = include_bytes!("../../../../tests/fixtures/fonts/Lilex-Bold.ttf");
const HEBREW: &[u8] = include_bytes!("../../../../tests/fixtures/fonts/NotoSansHebrew-Regular.ttf");
const NOTO_EMOJI: &[u8] = include_bytes!("../../../../tests/fixtures/fonts/NotoEmoji-Regular.ttf");

fn platform() -> Result<CosmicTextSystem> {
    let platform = CosmicTextSystem::new_without_system_fonts("IBM Plex Sans");
    platform.add_fonts(vec![
        Cow::Borrowed(IBM_PLEX),
        Cow::Borrowed(LILEX),
        Cow::Borrowed(LILEX_BOLD),
        Cow::Borrowed(HEBREW),
        Cow::Borrowed(NOTO_EMOJI),
    ])?;
    Ok(platform)
}

fn render_params(platform: &CosmicTextSystem, character: char) -> Result<RenderGlyphParams> {
    let font_id = platform.font_id(&font("IBM Plex Sans"))?;
    Ok(RenderGlyphParams {
        font_id,
        glyph_id: platform
            .glyph_for_char(font_id, character)
            .context("fixture must contain the requested glyph")?,
        font_size: px(32.),
        subpixel_variant: point(0, 0),
        scale_factor: 1.,
        is_emoji: false,
        subpixel_rendering: false,
        dilation: 0,
    })
}

fn cold_image(platform: &CosmicTextSystem, params: &RenderGlyphParams) -> Result<Image> {
    let font = platform.0.read().loaded_font(params.font_id).font.clone();
    let mut context = ScaleContext::new();
    let mut scaler = context
        .builder(font.as_swash())
        .size(f32::from(params.font_size) * params.scale_factor)
        .hint(true)
        .build();
    let sources: &[Source] = if params.is_emoji {
        &[
            Source::ColorOutline(0),
            Source::ColorBitmap(StrikeWith::BestFit),
            Source::Outline,
        ]
    } else {
        &[Source::Bitmap(StrikeWith::ExactSize), Source::Outline]
    };
    Render::new(sources)
        .format(if params.subpixel_rendering {
            Format::subpixel_bgra()
        } else {
            Format::Alpha
        })
        .offset(Vector::new(
            f32::from(params.subpixel_variant.x)
                / f32::from(SUBPIXEL_VARIANTS_X)
                / params.scale_factor,
            f32::from(params.subpixel_variant.y)
                / f32::from(SUBPIXEL_VARIANTS_Y)
                / params.scale_factor,
        ))
        .render(&mut scaler, params.glyph_id.0.try_into()?)
        .context("independent cold Swash rendering failed")
}

fn image_bounds(image: &Image) -> Bounds<DevicePixels> {
    Bounds {
        origin: point(image.placement.left.into(), (-image.placement.top).into()),
        size: size(image.placement.width.into(), image.placement.height.into()),
    }
}

fn expected_pixels(image: &Image, subpixel_rendering: bool) -> Vec<u8> {
    match image.content {
        Content::Color | Content::SubpixelMask => image
            .data
            .chunks_exact(4)
            .flat_map(|rgba| [rgba[2], rgba[1], rgba[0], rgba[3]])
            .collect(),
        Content::Mask if subpixel_rendering => image
            .data
            .iter()
            .flat_map(|&alpha| [alpha, alpha, alpha, alpha])
            .collect(),
        Content::Mask => image.data.clone(),
    }
}

fn assert_pending_budget(platform: &CosmicTextSystem) {
    let state = platform.0.read();
    assert!(state.pending_glyph_images.len() <= MAX_PENDING_GLYPH_IMAGES);
    assert!(state.pending_glyph_image_bytes <= MAX_PENDING_GLYPH_IMAGE_BYTES);
    assert_eq!(
        state.pending_glyph_image_bytes,
        state
            .pending_glyph_images
            .values()
            .map(|image| image.data.capacity())
            .sum::<usize>()
    );
}

#[test]
fn measured_masks_match_cold_pixels_and_are_consumed_after_one_generation() -> Result<()> {
    for scale_factor in [1., 1.5] {
        for subpixel_rendering in [false, true] {
            let platform = platform()?;
            let reference = self::platform()?;
            let params = RenderGlyphParams {
                scale_factor,
                subpixel_rendering,
                subpixel_variant: point(1, 0),
                ..render_params(&platform, 'm')?
            };
            assert_eq!(reference.font_id(&font("IBM Plex Sans"))?, params.font_id);
            let image = cold_image(&reference, &params)?;
            assert_eq!(
                image.content,
                if subpixel_rendering {
                    Content::SubpixelMask
                } else {
                    Content::Mask
                }
            );
            let expected = (
                image_bounds(&image).size,
                expected_pixels(&image, subpixel_rendering),
            );
            let bounds = platform.glyph_raster_bounds(&params)?;
            assert_eq!(bounds, image_bounds(&image));
            assert_eq!(platform.0.read().glyph_image_generation_count, 1);
            assert_eq!(platform.0.read().pending_glyph_images.len(), 1);
            assert_pending_budget(&platform);

            assert_eq!(platform.rasterize_glyph(&params, bounds)?, expected);
            assert_eq!(platform.0.read().glyph_image_generation_count, 1);
            assert!(platform.0.read().pending_glyph_images.is_empty());
            assert_eq!(platform.0.read().pending_glyph_image_bytes, 0);
            assert_eq!(platform.rasterize_glyph(&params, bounds)?, expected);
            assert_eq!(platform.0.read().glyph_image_generation_count, 2);
            assert_eq!(reference.rasterize_glyph(&params, bounds)?, expected);
            assert_eq!(reference.0.read().glyph_image_generation_count, 1);
            assert_pending_budget(&reference);
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[test]
fn actual_windows_color_glyphs_match_cold_bgra_pixels_at_both_scales() -> Result<()> {
    let windows = std::env::var_os("WINDIR").context("Windows font directory is required")?;
    let bytes = std::fs::read(std::path::PathBuf::from(windows).join("Fonts/seguiemj.ttf"))?;
    let platform = platform()?;
    let reference = self::platform()?;
    let mut ids = Vec::new();
    for platform in [&platform, &reference] {
        platform.add_fonts(vec![Cow::Owned(bytes.clone())])?;
        let mut state = platform.0.write();
        let database_id = state
            .font_system
            .db()
            .faces()
            .find(|face| {
                face.families
                    .iter()
                    .any(|family| family.0 == "Segoe UI Emoji")
            })
            .context("Windows must provide the Segoe UI Emoji color font")?
            .id;
        ids.push(state.font_id_for_cosmic_id(database_id)?);
    }
    assert_eq!(ids[0], ids[1]);
    let glyph_id = platform
        .glyph_for_char(ids[0], '😀')
        .context("Windows color font must contain the emoji fixture")?;
    for scale_factor in [1., 1.5] {
        for subpixel_rendering in [false, true] {
            let params = RenderGlyphParams {
                font_id: ids[0],
                glyph_id,
                scale_factor,
                is_emoji: true,
                subpixel_rendering,
                subpixel_variant: point(1, 0),
                ..render_params(&platform, 'm')?
            };
            let image = cold_image(&reference, &params)?;
            assert_eq!(image.content, Content::Color);
            assert!(image.data.chunks_exact(4).any(|rgba| rgba[0] != rgba[2]));
            let before = platform.0.read().glyph_image_generation_count;
            let bounds = platform.glyph_raster_bounds(&params)?;
            assert_eq!(bounds, image_bounds(&image));
            let expected = (bounds.size, expected_pixels(&image, subpixel_rendering));
            assert_eq!(platform.rasterize_glyph(&params, bounds)?, expected);
            assert_eq!(platform.0.read().glyph_image_generation_count, before + 1);
            assert_eq!(platform.rasterize_glyph(&params, bounds)?, expected);
            assert_eq!(platform.0.read().glyph_image_generation_count, before + 2);
            assert!(platform.0.read().pending_glyph_images.is_empty());
            assert_pending_budget(&platform);
        }
    }
    Ok(())
}

#[test]
fn every_render_parameter_partitions_pending_images_and_interleaving_preserves_hits() -> Result<()>
{
    let platform = platform()?;
    let base = render_params(&platform, 'm')?;
    let feature_font = platform.font_id(&Font {
        features: FontFeatures::disable_ligatures(),
        ..font("IBM Plex Sans")
    })?;
    let other_glyph = platform
        .glyph_for_char(base.font_id, 'A')
        .context("fixture must contain A")?;
    let variants = [
        RenderGlyphParams {
            font_id: feature_font,
            ..base.clone()
        },
        RenderGlyphParams {
            glyph_id: other_glyph,
            ..base.clone()
        },
        RenderGlyphParams {
            font_size: px(33.),
            ..base.clone()
        },
        RenderGlyphParams {
            subpixel_variant: point(1, 0),
            ..base.clone()
        },
        RenderGlyphParams {
            subpixel_variant: point(0, 1),
            ..base.clone()
        },
        RenderGlyphParams {
            scale_factor: 1.5,
            ..base.clone()
        },
        RenderGlyphParams {
            is_emoji: true,
            ..base.clone()
        },
        RenderGlyphParams {
            subpixel_rendering: true,
            ..base.clone()
        },
        RenderGlyphParams {
            dilation: 1,
            ..base.clone()
        },
    ];
    let bounds = platform.glyph_raster_bounds(&base)?;
    for (index, params) in variants.iter().enumerate() {
        let image = cold_image(&platform, params)?;
        let expected = (
            image_bounds(&image).size,
            expected_pixels(&image, params.subpixel_rendering),
        );
        assert_eq!(
            platform.rasterize_glyph(params, image_bounds(&image))?,
            expected
        );
        assert_eq!(platform.0.read().glyph_image_generation_count, index + 2);
        assert_eq!(platform.0.read().pending_glyph_images.len(), 1);
        assert!(platform.0.read().pending_glyph_images.contains_key(&base));
        assert_pending_budget(&platform);
    }
    let interleaved = &variants[1];
    let interleaved_bounds = platform.glyph_raster_bounds(interleaved)?;
    let before = platform.0.read().glyph_image_generation_count;
    assert_eq!(platform.0.read().pending_glyph_images.len(), 2);
    let base_image = cold_image(&platform, &base)?;
    assert_eq!(
        platform.rasterize_glyph(&base, bounds)?,
        (
            bounds.size,
            expected_pixels(&base_image, base.subpixel_rendering)
        )
    );
    assert_eq!(platform.0.read().pending_glyph_images.len(), 1);
    let interleaved_image = cold_image(&platform, interleaved)?;
    assert_eq!(
        platform.rasterize_glyph(interleaved, interleaved_bounds)?,
        (
            interleaved_bounds.size,
            expected_pixels(&interleaved_image, interleaved.subpixel_rendering)
        )
    );
    assert_eq!(platform.0.read().glyph_image_generation_count, before);
    assert!(platform.0.read().pending_glyph_images.is_empty());
    assert_pending_budget(&platform);
    Ok(())
}

#[test]
fn empty_and_failed_requests_retain_no_images_and_count_only_swash_attempts() -> Result<()> {
    let platform = platform()?;
    let space = render_params(&platform, ' ')?;
    if let std::result::Result::Ok(bounds) = platform.glyph_raster_bounds(&space) {
        assert!(bounds.size.width.0 == 0 || bounds.size.height.0 == 0);
    }
    assert_eq!(platform.0.read().glyph_image_generation_count, 1);
    assert!(platform.0.read().pending_glyph_images.is_empty());
    assert!(platform.rasterize_glyph(&space, Bounds::default()).is_err());
    assert_eq!(platform.0.read().glyph_image_generation_count, 1);

    let invalid = RenderGlyphParams {
        glyph_id: GlyphId(u32::MAX),
        ..space.clone()
    };
    assert!(platform.glyph_raster_bounds(&invalid).is_err());
    let nonempty = Bounds {
        origin: point(0.into(), 0.into()),
        size: size(1.into(), 1.into()),
    };
    assert!(platform.rasterize_glyph(&invalid, nonempty).is_err());
    assert_eq!(platform.0.read().glyph_image_generation_count, 1);

    let absent = RenderGlyphParams {
        glyph_id: GlyphId(u32::from(u16::MAX)),
        ..space
    };
    assert!(platform.glyph_raster_bounds(&absent).is_err());
    assert_eq!(platform.0.read().glyph_image_generation_count, 2);
    assert!(platform.0.read().pending_glyph_images.is_empty());
    assert_pending_budget(&platform);

    let params = render_params(&platform, 'm')?;
    for empty_width in [true, false] {
        let bounds = platform.glyph_raster_bounds(&params)?;
        let mut empty_bounds = bounds;
        if empty_width {
            empty_bounds.size.width = DevicePixels(0);
        } else {
            empty_bounds.size.height = DevicePixels(0);
        }
        let before = platform.0.read().glyph_image_generation_count;
        assert!(platform.rasterize_glyph(&params, empty_bounds).is_err());
        assert_eq!(platform.0.read().glyph_image_generation_count, before);
        assert!(platform.0.read().pending_glyph_images.is_empty());
        assert_pending_budget(&platform);
        platform.rasterize_glyph(&params, bounds)?;
        assert_eq!(platform.0.read().glyph_image_generation_count, before + 1);
    }
    Ok(())
}

#[test]
fn repeated_measurement_replaces_capacity_and_consumption_deducts_it() -> Result<()> {
    let platform = platform()?;
    let params = render_params(&platform, 'm')?;
    let mut capacity = None;
    for _ in 0..4 {
        platform.glyph_raster_bounds(&params)?;
        let state = platform.0.read();
        assert_eq!(state.pending_glyph_images.len(), 1);
        let actual = state.pending_glyph_images[&params].data.capacity();
        assert_eq!(state.pending_glyph_image_bytes, actual);
        assert_eq!(*capacity.get_or_insert(actual), actual);
    }
    let bounds = platform.glyph_raster_bounds(&params)?;
    platform.rasterize_glyph(&params, bounds)?;
    assert_eq!(platform.0.read().glyph_image_generation_count, 5);
    assert!(platform.0.read().pending_glyph_images.is_empty());
    assert_pending_budget(&platform);
    Ok(())
}

#[test]
fn spare_image_capacity_is_released_on_replacement_consumption_and_budget_clear() -> Result<()> {
    let platform = platform()?;
    let base = render_params(&platform, 'm')?;
    let other = render_params(&platform, 'A')?;
    let bounds = platform.glyph_raster_bounds(&base)?;
    platform.glyph_raster_bounds(&other)?;
    let image = cold_image(&platform, &base)?;
    for replace in [true, false] {
        {
            let mut state = platform.0.write();
            let other_bytes = state.pending_glyph_images[&other].data.capacity();
            let image = state
                .pending_glyph_images
                .get_mut(&base)
                .context("pending base")?;
            let mut allocated = Vec::with_capacity(MAX_PENDING_GLYPH_IMAGE_BYTES - other_bytes);
            allocated.extend_from_slice(&image.data);
            image.data = allocated;
            assert!(image.data.capacity() > image.data.len());
            state.pending_glyph_image_bytes = state
                .pending_glyph_images
                .values()
                .map(|image| image.data.capacity())
                .sum();
            assert_eq!(
                state.pending_glyph_image_bytes,
                MAX_PENDING_GLYPH_IMAGE_BYTES
            );
        }
        if replace {
            assert_eq!(platform.glyph_raster_bounds(&base)?, bounds);
            assert_eq!(platform.0.read().pending_glyph_images.len(), 2);
        } else {
            assert_eq!(
                platform.rasterize_glyph(&base, bounds)?,
                (bounds.size, expected_pixels(&image, false))
            );
            assert_eq!(platform.0.read().pending_glyph_images.len(), 1);
        }
        assert_pending_budget(&platform);
    }
    platform.glyph_raster_bounds(&base)?;
    {
        let mut state = platform.0.write();
        let other_bytes = state.pending_glyph_images[&other].data.capacity();
        let image = state
            .pending_glyph_images
            .get_mut(&base)
            .context("pending base")?;
        let mut allocated = Vec::with_capacity(MAX_PENDING_GLYPH_IMAGE_BYTES - other_bytes);
        allocated.extend_from_slice(&image.data);
        image.data = allocated;
        state.pending_glyph_image_bytes = MAX_PENDING_GLYPH_IMAGE_BYTES;
    }
    let newest = render_params(&platform, 'B')?;
    platform.glyph_raster_bounds(&newest)?;
    assert_eq!(platform.0.read().pending_glyph_images.len(), 1);
    assert!(platform.0.read().pending_glyph_images.contains_key(&newest));
    assert_pending_budget(&platform);
    Ok(())
}

#[test]
fn measurement_only_requests_clear_at_the_entry_budget_and_evicted_rasters_recompute() -> Result<()>
{
    let platform = platform()?;
    let base = render_params(&platform, 'm')?;
    let first_bounds = platform.glyph_raster_bounds(&base)?;
    for index in 1..MAX_PENDING_GLYPH_IMAGES {
        let params = RenderGlyphParams {
            font_size: px(32. + index as f32 / 16.),
            ..base.clone()
        };
        platform.glyph_raster_bounds(&params)?;
        assert_pending_budget(&platform);
    }
    assert_eq!(
        platform.0.read().pending_glyph_images.len(),
        MAX_PENDING_GLYPH_IMAGES
    );
    let newest = RenderGlyphParams {
        font_size: px(64.),
        ..base.clone()
    };
    platform.glyph_raster_bounds(&newest)?;
    assert_eq!(platform.0.read().pending_glyph_images.len(), 1);
    assert!(!platform.0.read().pending_glyph_images.contains_key(&base));
    let image = cold_image(&platform, &base)?;
    assert_eq!(
        platform.rasterize_glyph(&base, first_bounds)?,
        (first_bounds.size, expected_pixels(&image, false))
    );
    assert_eq!(
        platform.0.read().glyph_image_generation_count,
        MAX_PENDING_GLYPH_IMAGES + 2
    );
    assert_eq!(platform.0.read().pending_glyph_images.len(), 1);
    assert_pending_budget(&platform);
    Ok(())
}

#[test]
fn measurement_only_requests_clear_at_the_byte_budget_before_the_entry_budget() -> Result<()> {
    let platform = platform()?;
    let base = RenderGlyphParams {
        font_size: px(512.),
        subpixel_rendering: true,
        ..render_params(&platform, 'm')?
    };
    let first_bounds = platform.glyph_raster_bounds(&base)?;
    let mut cleared = false;
    for dilation in 1..=128 {
        let params = RenderGlyphParams {
            dilation,
            ..base.clone()
        };
        platform.glyph_raster_bounds(&params)?;
        assert_pending_budget(&platform);
        let state = platform.0.read();
        if !state.pending_glyph_images.contains_key(&base) {
            assert_eq!(state.pending_glyph_images.len(), 1);
            assert!(usize::from(dilation) < MAX_PENDING_GLYPH_IMAGES);
            cleared = true;
            break;
        }
    }
    assert!(
        cleared,
        "actual image capacities must exercise the byte budget"
    );
    let before = platform.0.read().glyph_image_generation_count;
    let image = cold_image(&platform, &base)?;
    assert_eq!(
        platform.rasterize_glyph(&base, first_bounds)?,
        (first_bounds.size, expected_pixels(&image, true))
    );
    assert_eq!(platform.0.read().glyph_image_generation_count, before + 1);
    assert_pending_budget(&platform);
    Ok(())
}

#[test]
fn oversized_measurements_release_pending_images_and_still_rasterize() -> Result<()> {
    let platform = platform()?;
    let small = render_params(&platform, 'm')?;
    platform.glyph_raster_bounds(&small)?;
    let oversized = RenderGlyphParams {
        font_size: px(4096.),
        subpixel_rendering: true,
        ..small
    };
    let bounds = platform.glyph_raster_bounds(&oversized)?;
    let byte_len =
        usize::try_from(bounds.size.width.0)? * usize::try_from(bounds.size.height.0)? * 4;
    assert!(byte_len > MAX_PENDING_GLYPH_IMAGE_BYTES);
    assert!(platform.0.read().pending_glyph_images.is_empty());
    assert_pending_budget(&platform);
    let (size, pixels) = platform.rasterize_glyph(&oversized, bounds)?;
    assert_eq!(size, bounds.size);
    assert_eq!(pixels.len(), byte_len);
    assert!(pixels.iter().any(|channel| *channel != 0));
    let image = cold_image(&platform, &oversized)?;
    assert_eq!(bounds, image_bounds(&image));
    assert_eq!(
        pixels,
        expected_pixels(&image, oversized.subpixel_rendering)
    );
    assert_eq!(platform.0.read().glyph_image_generation_count, 3);
    assert_pending_budget(&platform);
    Ok(())
}

#[test]
fn installation_preserves_old_ids_sources_and_pending_and_repeated_raster_bytes() -> Result<()> {
    let platform = Arc::new(CosmicTextSystem::new_without_system_fonts("IBM Plex Sans"));
    platform.add_fonts(vec![Cow::Borrowed(IBM_PLEX)])?;
    let text = TextSystem::new(platform.clone());
    let descriptor = font("IBM Plex Sans");
    let old_id = text.resolve_font(&descriptor);
    text.prewarm_fonts(std::slice::from_ref(&descriptor));
    let params = RenderGlyphParams {
        scale_factor: 1.5,
        ..render_params(&platform, 'm')?
    };
    let face = platform.0.read().loaded_font(old_id).font.clone();
    let image = cold_image(&platform, &params)?;
    let expected = (image_bounds(&image).size, expected_pixels(&image, false));
    let bounds = platform.glyph_raster_bounds(&params)?;

    text.add_fonts(vec![Cow::Borrowed(LILEX), Cow::Borrowed(NOTO_EMOJI)])?;
    text.prewarm_fonts(std::slice::from_ref(&descriptor));

    assert_eq!(text.resolve_font(&descriptor), old_id);
    assert!(Arc::ptr_eq(
        &face,
        &platform.0.read().loaded_font(old_id).font
    ));
    assert_eq!(
        face.data().as_ptr(),
        platform.0.read().loaded_font(old_id).font.data().as_ptr()
    );
    assert_ne!(text.resolve_font(&font("Lilex")), old_id);
    assert_eq!(platform.rasterize_glyph(&params, bounds)?, expected);
    assert_eq!(platform.0.read().glyph_image_generation_count, 1);
    assert_eq!(platform.rasterize_glyph(&params, bounds)?, expected);
    assert_eq!(platform.0.read().glyph_image_generation_count, 2);
    assert_pending_budget(&platform);
    Ok(())
}

#[test]
fn public_prewarming_populates_real_primary_and_fallback_matches_with_shared_attributes()
-> Result<()> {
    let platform = Arc::new(platform()?);
    let text = TextSystem::new(platform.clone());
    let features = FontFeatures(Arc::new(vec![
        ("liga".into(), 0),
        ("kern".into(), 0),
        ("ss01".into(), 2),
    ]));
    let descriptor = Font {
        features: features.clone(),
        fallbacks: Some(FontFallbacks::from_fonts(vec![
            "Missing Font Work Fixture".into(),
            "IBM Plex Sans".into(),
        ])),
        ..font("Lilex").bold().italic()
    };
    let primary_id = text.resolve_font(&descriptor);
    let (fallback_id, stretch, style, weight) = {
        let mut state = platform.0.write();
        let properties = state
            .font_match_properties(primary_id)
            .context("primary properties")?;
        assert_eq!(properties.fallback_chain.len(), 1);
        assert_eq!(properties.primary_family_name.as_str(), "Lilex");
        assert_eq!(properties.weight, Weight::BOLD);
        assert_eq!(properties.style, Style::Normal);
        state.font_system.db_mut();
        (
            properties.fallback_chain[0].0,
            properties.stretch,
            properties.style,
            properties.weight,
        )
    };

    text.prewarm_fonts(&[descriptor.clone(), descriptor]);

    let mut state = platform.0.write();
    let prewarmed = state.prewarmed_font_matches.clone();
    assert_eq!(prewarmed.len(), 2);
    for ((actual, cached), (font_id, family)) in prewarmed
        .iter()
        .zip([(primary_id, "Lilex"), (fallback_id, "IBM Plex Sans")])
    {
        let expected = Attrs::new()
            .metadata(font_id.0)
            .family(Family::Name(family))
            .stretch(stretch)
            .style(style)
            .weight(weight)
            .font_features(cosmic_font_features(&features)?);
        assert_eq!(*actual, cosmic_text::AttrsOwned::new(&expected));
        let cached = cached
            .upgrade()
            .context("prewarming must retain the actual match entry")?;
        assert!(!cached.is_empty());
        let queried = state.font_system.get_font_matches(&expected);
        assert!(Arc::ptr_eq(&cached, &queried));
    }
    assert_eq!(state.glyph_image_generation_count, 0);
    assert!(state.pending_glyph_images.is_empty());
    Ok(())
}

#[test]
fn available_non_latin_families_remain_fallbacks_when_prewarming_primary_descriptors() -> Result<()>
{
    let platform = Arc::new(platform()?);
    let text = TextSystem::new(platform.clone());
    for family in ["Noto Emoji", "Noto Sans Hebrew"] {
        assert!(text.all_font_names().iter().any(|name| name == family));
        let descriptor = font(family);
        assert!(platform.font_id(&descriptor).is_err());
        text.prewarm_fonts(std::slice::from_ref(&descriptor));
        let primary_id = text.resolve_font(&descriptor);
        assert!(platform.glyph_for_char(primary_id, 'm').is_some());
        let state = platform.0.read();
        let (attributes, matches) = state
            .prewarmed_font_matches
            .last()
            .context("resolved primary must be warmed")?;
        assert_eq!(attributes.metadata, primary_id.0);
        assert_ne!(attributes.family_owned.as_family(), Family::Name(family));
        assert!(matches.upgrade().is_some());
    }
    Ok(())
}

#[derive(Default)]
struct RecordingSink(Mutex<Vec<MissingGlyph>>);

impl MissingGlyphSink for RecordingSink {
    fn report(&self, missing: Vec<MissingGlyph>) {
        self.0.lock().extend(missing);
    }
}

fn assert_same_layout(actual: &LineLayout, expected: &LineLayout) {
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
fn warmed_and_cold_shaping_preserve_features_paragraphs_fallbacks_and_missing_reports() -> Result<()>
{
    let warm = Arc::new(platform()?);
    let cold = Arc::new(platform()?);
    let warm_text = TextSystem::new(warm.clone());
    let cold_text = TextSystem::new(cold.clone());
    let descriptors = [
        Font {
            fallbacks: Some(FontFallbacks::from_fonts(vec!["IBM Plex Sans".into()])),
            ..font("Lilex")
        },
        Font {
            features: FontFeatures(Arc::new(vec![("liga".into(), 0), ("kern".into(), 0)])),
            ..font("IBM Plex Sans")
        },
    ];
    warm_text.prewarm_fonts(&descriptors);
    let warm_ids = descriptors
        .iter()
        .map(|font| warm_text.resolve_font(font))
        .collect::<Vec<_>>();
    let cold_ids = descriptors
        .iter()
        .map(|font| cold_text.resolve_font(font))
        .collect::<Vec<_>>();
    assert_eq!(warm_ids, cold_ids);
    assert!(cold.0.read().prewarmed_font_matches.is_empty());
    let warm_sink = Arc::new(RecordingSink::default());
    let cold_sink = Arc::new(RecordingSink::default());
    warm.set_missing_glyph_sink(Some(warm_sink.clone()));
    cold.set_missing_glyph_sink(Some(cold_sink.clone()));
    for source in [
        "",
        "office ffi AV -> != =>",
        "AאבB",
        "אב\u{001c}A\u{0085}B",
        "A😀B",
        "界\u{0301}",
    ] {
        let first_len = source.chars().next().map_or(0, char::len_utf8);
        let runs = [
            FontRun {
                len: first_len,
                font_id: warm_ids[0],
            },
            FontRun {
                len: source.len() - first_len,
                font_id: warm_ids[1],
            },
        ];
        let actual = warm.layout_line(source, px(18.), &runs);
        let expected = cold.layout_line(source, px(18.), &runs);
        assert_same_layout(&actual, &expected);
        if source == "A😀B" {
            let fallback = actual
                .runs
                .iter()
                .find(|run| {
                    run.glyphs
                        .iter()
                        .any(|glyph| glyph.index == 1 && glyph.id.0 != 0)
                })
                .context("real emoji fallback must survive prewarming")?;
            assert!(warm.glyph_for_char(fallback.font_id, 'm').is_none());
            assert!(warm.glyph_for_char(fallback.font_id, '😀').is_some());
        }
    }
    assert_eq!(*warm_sink.0.lock(), *cold_sink.0.lock());
    assert!(!warm_sink.0.lock().is_empty());
    let mut state = warm.0.write();
    for (attributes, cached) in state.prewarmed_font_matches.clone() {
        let cached = cached
            .upgrade()
            .context("shaping must retain prewarmed matches")?;
        assert!(Arc::ptr_eq(
            &cached,
            &state.font_system.get_font_matches(&attributes.as_attrs())
        ));
    }
    assert_eq!(state.glyph_image_generation_count, 0);
    Ok(())
}
