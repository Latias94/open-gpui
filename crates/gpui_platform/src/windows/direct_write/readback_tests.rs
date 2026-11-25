use super::DirectWriteTextSystem;
use anyhow::{Context, Result};
use open_gpui::{
    Bounds, DevicePixels, Font, FontRun, PlatformTextSystem, RenderGlyphParams, point, px, size,
};
use windows::Win32::Graphics::DirectWrite::*;

#[test]
fn directwrite_color_bounds_union_every_layer_at_the_recommended_mode() -> Result<()> {
    let platform = DirectWriteTextSystem::new()?;
    let font_id = platform.font_id(&Font {
        family: "Segoe UI Emoji".into(),
        ..Default::default()
    })?;
    let glyph_id = platform
        .glyph_for_char(font_id, '😀')
        .context("Segoe UI Emoji must contain the color fixture")?;
    assert_ne!(glyph_id.0, 0);
    let state = platform.state.read();
    for (font_size, scale_factor, subpixel_x) in [(12.0, 1.0, 0), (32.0, 1.5, 1), (48.0, 2.0, 3)] {
        let params = RenderGlyphParams {
            font_id,
            glyph_id,
            font_size: px(font_size),
            subpixel_variant: point(subpixel_x, 0),
            scale_factor,
            is_emoji: true,
            subpixel_rendering: false,
            dilation: 0,
        };
        let mut rendering_mode = DWRITE_RENDERING_MODE1::default();
        let mut grid_fit_mode = DWRITE_GRID_FIT_MODE::default();
        unsafe {
            state.fonts[font_id.0]
                .font_face
                .GetRecommendedRenderingMode(
                    font_size * scale_factor,
                    96.0,
                    96.0,
                    None,
                    false,
                    DWRITE_OUTLINE_THRESHOLD_ANTIALIASED,
                    DWRITE_MEASURING_MODE_NATURAL,
                    None,
                    &mut rendering_mode,
                    &mut grid_fit_mode,
                )?;
        }
        if rendering_mode == DWRITE_RENDERING_MODE1_OUTLINE {
            rendering_mode = DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC;
        }
        assert_eq!(
            state.rendering_mode(&params)?,
            (rendering_mode, grid_fit_mode)
        );
        state.with_glyph_run(&params, |run, origin| {
            assert_eq!(run.fontEmSize, font_size * scale_factor);
            assert_eq!(
                origin.X,
                subpixel_x as f32 / open_gpui::SUBPIXEL_VARIANTS_X as f32,
            );
            assert_eq!(origin.Y, 0.0);
        });
        let layers = state.color_layers(&platform.components, &params)?;
        assert!(
            !layers.is_empty(),
            "the fixture must enumerate actual COLR layers",
        );
        let mut expected: Option<Bounds<DevicePixels>> = None;
        for layer in layers {
            let rect = unsafe {
                layer
                    .analysis
                    .GetAlphaTextureBounds(DWRITE_TEXTURE_ALIASED_1x1)?
            };
            let layer_bounds = Bounds::from_corners(
                point(DevicePixels(rect.left), DevicePixels(rect.top)),
                point(DevicePixels(rect.right), DevicePixels(rect.bottom)),
            );
            if !layer_bounds.is_empty() {
                expected = Some(expected.map_or(layer_bounds, |union| union.union(&layer_bounds)));
            }
        }
        let expected = expected.context("at least one COLR layer must have ink")?;
        let actual = platform.glyph_raster_bounds(&params)?;
        assert_eq!(actual, expected);
        let (raster_size, bytes) = platform.rasterize_glyph(&params, actual)?;
        assert_eq!(raster_size, expected.size);
        assert_eq!(
            bytes.len(),
            (raster_size.width.0 * raster_size.height.0) as usize * 4,
        );
        assert!(bytes.chunks_exact(4).any(|pixel| pixel[3] > 0));
    }
    Ok(())
}

#[test]
fn a_glyph_without_colr_layers_keeps_the_monochrome_fallback() -> Result<()> {
    let platform = DirectWriteTextSystem::new()?;
    let font_id = platform.font_id(&open_gpui::font("Segoe UI"))?;
    let glyph_id = platform
        .glyph_for_char(font_id, 'm')
        .context("Segoe UI must contain m")?;
    let mut params = RenderGlyphParams {
        font_id,
        glyph_id,
        font_size: px(24.0),
        subpixel_variant: point(0, 0),
        scale_factor: 1.5,
        is_emoji: true,
        subpixel_rendering: false,
        dilation: 0,
    };
    let state = platform.state.read();
    assert!(
        state
            .color_layers(&platform.components, &params)?
            .is_empty()
    );
    let bounds = platform.glyph_raster_bounds(&params)?;
    let (_, color) = platform.rasterize_glyph(&params, bounds)?;
    params.is_emoji = false;
    assert_eq!(platform.glyph_raster_bounds(&params)?, bounds);
    let (_, monochrome) = platform.rasterize_glyph(&params, bounds)?;
    assert!(monochrome.iter().any(|coverage| *coverage > 0));
    assert_eq!(
        color,
        monochrome
            .into_iter()
            .flat_map(|coverage| [0, 0, 0, coverage])
            .collect::<Vec<_>>(),
    );
    Ok(())
}

#[test]
fn directwrite_color_emoji_rasterization_preserves_pixels_while_headless() -> Result<()> {
    let platform = DirectWriteTextSystem::new()?;
    let font = Font {
        family: "Segoe UI Emoji".into(),
        ..Default::default()
    };
    let text = "\u{1f600}";
    let font_size = px(32.);
    let font_id = platform.font_id(&font)?;
    let shaped = platform.state.write().layout_line(
        &platform.components,
        text,
        font_size,
        &[FontRun {
            len: text.len(),
            font_id,
        }],
    )?;
    assert_eq!(shaped.runs.len(), 1, "the emoji must shape into one run");
    let run = &shaped.runs[0];
    assert_eq!(run.glyphs.len(), 1, "the emoji must shape into one glyph");
    let glyph = &run.glyphs[0];
    assert!(glyph.is_emoji, "the shaped glyph must use the color font");
    let params = RenderGlyphParams {
        font_id: run.font_id,
        glyph_id: glyph.id,
        font_size,
        subpixel_variant: point(0, 0),
        scale_factor: 1.,
        is_emoji: glyph.is_emoji,
        subpixel_rendering: false,
        dilation: 0,
    };
    let state = platform.state.read();
    let bounds = state.raster_bounds(&platform.components, &params)?;
    assert!(bounds.size.width.0 > 0 && bounds.size.height.0 > 0);
    let first = state
        .rasterize_color(&platform.components, &params, bounds)
        .context("rasterizing the first headless DirectWrite color emoji")?;
    assert_eq!(
        first.len(),
        usize::try_from(bounds.size.width.0)? * usize::try_from(bounds.size.height.0)? * 4,
    );
    assert!(
        first.chunks_exact(4).any(|pixel| pixel[3] != 0),
        "the color emoji must contain visible BGRA pixels",
    );
    assert!(
        first.chunks_exact(4).any(|pixel| pixel[3] > 0
            && pixel[0].max(pixel[1]).max(pixel[2]) - pixel[0].min(pixel[1]).min(pixel[2]) > 32),
        "the headless glyph must contain actual palette colors",
    );
    assert_eq!(
        platform.rasterize_glyph(&params, bounds)?,
        (bounds.size, first.clone()),
    );

    let padding = DevicePixels(4);
    let padded_bounds = open_gpui::bounds(
        bounds.origin - point(padding, padding),
        bounds.size + size(padding + padding, padding + padding),
    );
    let padded = state
        .rasterize_color(&platform.components, &params, padded_bounds)
        .context("rasterizing a DirectWrite color emoji with transparent padding")?;
    let padded_width = usize::try_from(padded_bounds.size.width.0)?;
    let padded_height = usize::try_from(padded_bounds.size.height.0)?;
    let padding = usize::try_from(padding.0)?;
    assert_eq!(padded.len(), padded_width * padded_height * 4);
    for (y, row) in padded.chunks_exact(padded_width * 4).enumerate() {
        for (x, pixel) in row.chunks_exact(4).enumerate() {
            if x < padding
                || y < padding
                || x >= padded_width - padding
                || y >= padded_height - padding
            {
                assert_eq!(
                    pixel,
                    [0, 0, 0, 0],
                    "uncovered color glyph pixel at ({x}, {y}) must be transparent black",
                );
            } else {
                let source = ((y - padding) * bounds.size.width.0 as usize + x - padding) * 4;
                assert_eq!(
                    pixel,
                    &first[source..source + 4],
                    "padding must preserve the glyph at ({x}, {y})",
                );
            }
        }
    }
    assert!(
        padded.chunks_exact(4).any(|pixel| pixel[3] != 0),
        "transparent padding must preserve visible color glyph content",
    );

    for iteration in 0..3 {
        let repeated_bounds = state.raster_bounds(&platform.components, &params)?;
        let repeated = state
            .rasterize_color(&platform.components, &params, repeated_bounds)
            .with_context(|| {
                format!("rasterizing headless DirectWrite color emoji on iteration {iteration}")
            })?;
        assert_eq!(repeated_bounds, bounds);
        assert_eq!(
            repeated, first,
            "DirectWrite color emoji on iteration {iteration}"
        );
    }

    drop(state);
    assert_eq!(platform.font_id(&font)?, font_id);
    let reshaped = platform.layout_line(
        text,
        font_size,
        &[FontRun {
            len: text.len(),
            font_id,
        }],
    );
    assert_eq!(reshaped.width, shaped.width);
    assert_eq!(reshaped.runs[0].font_id, run.font_id);
    assert_eq!(reshaped.runs[0].glyphs[0].id, glyph.id);
    assert_eq!(
        platform
            .state
            .read()
            .rasterize_color(&platform.components, &params, bounds)?,
        first
    );

    Ok(())
}
