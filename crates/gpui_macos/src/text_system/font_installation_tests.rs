use super::*;
use anyhow::Context;
use open_gpui::font;

const IBM_PLEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/fonts/IBMPlexSans-Regular.ttf"
));
const LILEX_REGULAR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/fonts/Lilex-Regular.ttf"
));
const LILEX_BOLD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/fonts/Lilex-Bold.ttf"
));
const NOTO_EMOJI: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/fonts/NotoEmoji-Regular.ttf"
));

fn loaded_fonts(platform: &MacTextSystem) -> Vec<(CTFont, FontMetrics)> {
    platform
        .0
        .read()
        .fonts
        .iter()
        .map(|font| {
            (
                font.native_font(),
                font_kit_metrics_to_metrics(font.metrics()),
            )
        })
        .collect()
}

fn assert_loaded_fonts_retained(platform: &MacTextSystem, before: &[(CTFont, FontMetrics)]) {
    let state = platform.0.read();
    assert!(state.fonts.len() >= before.len());
    for (font, (native, metrics)) in state.fonts.iter().zip(before) {
        assert_eq!(
            font.native_font().as_concrete_TypeRef(),
            native.as_concrete_TypeRef(),
        );
        let after = font_kit_metrics_to_metrics(font.metrics());
        assert_eq!(after.units_per_em, metrics.units_per_em);
        assert_eq!(
            [
                after.ascent,
                after.descent,
                after.line_gap,
                after.underline_position,
                after.underline_thickness,
                after.cap_height,
                after.x_height,
            ],
            [
                metrics.ascent,
                metrics.descent,
                metrics.line_gap,
                metrics.underline_position,
                metrics.underline_thickness,
                metrics.cap_height,
                metrics.x_height,
            ],
        );
        assert_eq!(after.bounding_box, metrics.bounding_box);
    }
}

fn postscript_name(platform: &MacTextSystem, font_id: FontId) -> String {
    platform.0.read().postscript_names_by_font_id[&font_id].clone()
}

fn rasterized_layout(
    platform: &MacTextSystem,
    layout: &LineLayout,
) -> Result<Vec<(Size<DevicePixels>, Vec<u8>)>> {
    let mut rasters = Vec::new();
    for run in &layout.runs {
        for glyph in &run.glyphs {
            let params = RenderGlyphParams {
                font_id: run.font_id,
                glyph_id: glyph.id,
                font_size: layout.font_size,
                subpixel_variant: point(0, 0),
                scale_factor: 1.0,
                is_emoji: glyph.is_emoji,
                subpixel_rendering: false,
                dilation: 0,
            };
            let bounds = platform.glyph_raster_bounds(&params)?;
            let raster = platform.rasterize_glyph(&params, bounds)?;
            assert!(raster.1.iter().any(|byte| *byte != 0));
            rasters.push(raster);
        }
    }
    assert!(!rasters.is_empty());
    Ok(rasters)
}

fn cascade_descriptors(platform: &MacTextSystem, font_id: FontId) -> Vec<CTFontDescriptor> {
    let descriptor = platform.0.read().fonts[font_id.0]
        .native_font()
        .copy_descriptor();
    let key = unsafe { CFString::wrap_under_get_rule(kCTFontCascadeListAttribute) };
    let attributes = descriptor.attributes();
    let array = attributes
        .get(&key)
        .downcast::<CFArray>()
        .expect("the configured cascade must be a CFArray");
    let array: CFArray<CTFontDescriptor> =
        unsafe { CFArray::wrap_under_get_rule(array.as_concrete_TypeRef()) };
    array.iter().map(|descriptor| descriptor.clone()).collect()
}

#[test]
fn installing_bold_reselects_matched_descriptors_and_retains_loaded_fonts() -> Result<()> {
    for owned in [false, true] {
        let platform = MacTextSystem::new();
        platform.add_fonts(vec![Cow::Borrowed(LILEX_REGULAR)])?;
        let requested = [font("Lilex").bold(), font("Lilex").bold().italic()];
        let old_id = platform.font_id(&requested[0])?;
        assert_eq!(platform.font_id(&requested[1])?, old_id);
        assert_eq!(postscript_name(&platform, old_id), "Lilex-Regular");
        let glyph = platform
            .glyph_for_char(old_id, 'm')
            .context("fixture glyph")?;
        let advance = platform.advance(old_id, glyph)?;
        let bounds = platform.typographic_bounds(old_id, glyph)?;
        let old_layout = platform.layout_line(
            "m",
            px(16.),
            &[FontRun {
                font_id: old_id,
                len: 1,
            }],
        );
        let old_rasters = rasterized_layout(&platform, &old_layout)?;
        let before = loaded_fonts(&platform);

        let bold = if owned {
            Cow::Owned(LILEX_BOLD.to_vec())
        } else {
            Cow::Borrowed(LILEX_BOLD)
        };
        platform.add_fonts(vec![bold])?;

        assert!(platform.0.read().font_selections.is_empty());
        assert!(platform.0.read().font_ids_by_font_key.is_empty());
        for descriptor in &requested {
            let selected = platform.font_id(descriptor)?;
            assert_ne!(selected, old_id);
            assert_eq!(postscript_name(&platform, selected), "Lilex-Bold");
            assert_eq!(
                platform.0.read().fonts[selected.0].properties().weight,
                FontkitWeight(FontWeight::BOLD.0),
            );
            let layout = platform.layout_line(
                "m",
                px(16.),
                &[FontRun {
                    font_id: selected,
                    len: 1,
                }],
            );
            assert_eq!(layout.runs.len(), 1);
            assert_eq!(layout.runs[0].font_id, selected);
            rasterized_layout(&platform, &layout)?;
        }
        assert_eq!(platform.font_id(&font("Lilex"))?, old_id);
        assert_eq!(platform.glyph_for_char(old_id, 'm'), Some(glyph));
        assert_eq!(platform.advance(old_id, glyph)?, advance);
        assert_eq!(platform.typographic_bounds(old_id, glyph)?, bounds);
        assert_loaded_fonts_retained(&platform, &before);
        assert_eq!(rasterized_layout(&platform, &old_layout)?, old_rasters);
    }
    Ok(())
}

#[test]
fn installing_fallback_rebuilds_the_cascade_with_the_installed_face() -> Result<()> {
    for owned in [false, true] {
        let platform = MacTextSystem::new();
        platform.add_fonts(vec![Cow::Borrowed(IBM_PLEX)])?;
        let plain_id = platform.font_id(&font("IBM Plex Sans"))?;
        let requested = Font {
            fallbacks: Some(FontFallbacks::from_fonts(vec!["Noto Emoji".into()])),
            ..font("IBM Plex Sans")
        };
        let old_id = platform.font_id(&requested)?;
        let old_cascade = cascade_descriptors(&platform, old_id);
        let text = "m😀m";
        let mut old_layouts = Vec::new();
        for font_size in [px(12.), px(16.), px(32.)] {
            let layout = platform.layout_line(
                text,
                font_size,
                &[FontRun {
                    font_id: old_id,
                    len: text.len(),
                }],
            );
            let rasters = rasterized_layout(&platform, &layout)?;
            old_layouts.push((layout, rasters));
        }
        let before = loaded_fonts(&platform);

        let fallback = if owned {
            Cow::Owned(NOTO_EMOJI.to_vec())
        } else {
            Cow::Borrowed(NOTO_EMOJI)
        };
        platform.add_fonts(vec![fallback])?;

        let selected = platform.font_id(&requested)?;
        assert_ne!(selected, old_id);
        assert_eq!(platform.font_id(&font("IBM Plex Sans"))?, plain_id);
        let fixture = platform
            .0
            .read()
            .memory_source
            .select_by_postscript_name("NotoEmoji")?
            .load()?;
        assert!(fixture.glyph_for_char('m').is_none());
        let cascade = cascade_descriptors(&platform, selected);
        assert!(!cascade.is_empty());
        assert_eq!(
            cascade[0].attributes().as_CFType(),
            fixture
                .native_font()
                .copy_descriptor()
                .attributes()
                .as_CFType(),
        );
        assert_eq!(cascade_descriptors(&platform, old_id), old_cascade);
        assert!(platform.font_id(&font("Noto Emoji")).is_err());
        assert_eq!(platform.font_id(&requested)?, selected);

        let mut expected_fallback_id = None;
        for (old_layout, old_rasters) in &old_layouts {
            let layout = platform.layout_line(
                text,
                old_layout.font_size,
                &[FontRun {
                    font_id: selected,
                    len: text.len(),
                }],
            );
            let (fallback_id, glyph) = layout
                .runs
                .iter()
                .find_map(|run| {
                    run.glyphs
                        .iter()
                        .find(|glyph| glyph.index == 1)
                        .map(|glyph| (run.font_id, glyph.id))
                })
                .context("CoreText must shape the emoji at its original UTF-8 byte index")?;
            assert_eq!(postscript_name(&platform, fallback_id), "NotoEmoji");
            assert_eq!(Some(glyph.0), fixture.glyph_for_char('😀'));
            assert_ne!(fallback_id, selected);
            assert_eq!(
                *expected_fallback_id.get_or_insert(fallback_id),
                fallback_id
            );
            assert_eq!(
                layout
                    .runs
                    .iter()
                    .flat_map(|run| run
                        .glyphs
                        .iter()
                        .map(move |glyph| (run.font_id, glyph.index)))
                    .collect::<Vec<_>>(),
                [(selected, 0), (fallback_id, 1), (selected, "m😀".len())],
            );
            rasterized_layout(&platform, &layout)?;
            assert_eq!(&rasterized_layout(&platform, old_layout)?, old_rasters);
        }
        assert_loaded_fonts_retained(&platform, &before);
    }
    Ok(())
}

#[test]
fn native_run_identity_preserves_features_and_cascades_across_sizes_and_installation() -> Result<()>
{
    let platform = MacTextSystem::new();
    platform.add_fonts(vec![Cow::Borrowed(IBM_PLEX), Cow::Borrowed(NOTO_EMOJI)])?;
    let disabled = FontFeatures(Arc::new(vec![("liga".into(), 0)]));
    let fallbacks = Some(FontFallbacks::from_fonts(vec!["Noto Emoji".into()]));
    let descriptors = [
        font("IBM Plex Sans"),
        Font {
            features: disabled.clone(),
            ..font("IBM Plex Sans")
        },
        Font {
            fallbacks: fallbacks.clone(),
            ..font("IBM Plex Sans")
        },
        Font {
            features: disabled,
            fallbacks,
            ..font("IBM Plex Sans")
        },
    ];
    let ids = descriptors
        .iter()
        .map(|descriptor| platform.font_id(descriptor))
        .collect::<Result<Vec<_>>>()?;
    for (index, id) in ids.iter().enumerate() {
        assert!(!ids[..index].contains(id));
    }
    for font_size in [px(12.), px(16.), px(32.)] {
        for (index, id) in ids.iter().copied().enumerate() {
            let native = platform.0.read().fonts[id.0].native_font();
            assert_eq!(
                platform
                    .0
                    .write()
                    .id_for_native_font(native.clone_with_font_size(font_size.into())),
                id,
            );
            let layout = platform.layout_line(
                "fi",
                font_size,
                &[FontRun {
                    font_id: id,
                    len: 2,
                }],
            );
            assert_eq!(layout.runs.len(), 1);
            assert_eq!(layout.runs[0].font_id, id);
            assert_eq!(
                layout.runs[0].glyphs.len(),
                if index % 2 == 0 { 1 } else { 2 }
            );
        }
    }
    let before = loaded_fonts(&platform);
    platform.add_fonts(vec![
        Cow::Borrowed(LILEX_REGULAR),
        Cow::Borrowed(LILEX_BOLD),
        Cow::Borrowed(IBM_PLEX),
    ])?;

    for (descriptor, id) in descriptors.iter().zip(ids) {
        assert_eq!(platform.font_id(descriptor)?, id);
    }
    assert_eq!(platform.0.read().fonts.len(), before.len());
    assert_loaded_fonts_retained(&platform, &before);
    Ok(())
}

#[test]
fn empty_and_rejected_installs_preserve_native_selection_state() -> Result<()> {
    let platform = MacTextSystem::new();
    platform.add_fonts(vec![Cow::Borrowed(LILEX_REGULAR)])?;
    let requested = font("Lilex").bold();
    let old_id = platform.font_id(&requested)?;
    let before = loaded_fonts(&platform);
    let selections = platform.0.read().font_selections.clone();
    let families = platform.0.read().font_ids_by_font_key.clone();

    platform.add_fonts(Vec::new())?;
    assert!(
        platform
            .add_fonts(vec![Cow::Borrowed(b"invalid font")])
            .is_err()
    );
    assert!(
        platform
            .add_fonts(vec![
                Cow::Owned(b"invalid font".to_vec()),
                Cow::Borrowed(LILEX_BOLD)
            ])
            .is_err()
    );

    assert_eq!(platform.0.read().font_selections, selections);
    assert!(platform.0.read().font_ids_by_font_key == families);
    assert!(
        platform
            .0
            .read()
            .memory_source
            .select_by_postscript_name("Lilex-Bold")
            .is_err()
    );
    assert_eq!(platform.font_id(&requested)?, old_id);
    assert_eq!(platform.0.read().fonts.len(), before.len());
    assert_loaded_fonts_retained(&platform, &before);
    Ok(())
}

#[test]
fn invalid_owned_tail_is_validated_before_installation_and_valid_retry_reselects_fonts()
-> Result<()> {
    let platform = MacTextSystem::new();
    platform.add_fonts(vec![Cow::Borrowed(IBM_PLEX), Cow::Borrowed(LILEX_REGULAR)])?;
    let requested = font("Lilex").bold();
    let old_id = platform.font_id(&requested)?;
    let before = loaded_fonts(&platform);
    let selections = platform.0.read().font_selections.clone();
    let families = platform.0.read().font_ids_by_font_key.clone();

    assert!(
        platform
            .add_fonts(vec![
                Cow::Owned(NOTO_EMOJI.to_vec()),
                Cow::Owned(LILEX_BOLD.to_vec()),
                Cow::Owned(b"invalid font".to_vec()),
            ])
            .is_err()
    );

    assert_eq!(platform.0.read().font_selections, selections);
    assert!(platform.0.read().font_ids_by_font_key == families);
    assert_eq!(
        platform.0.read().memory_source.all_families()?,
        ["IBM Plex Sans", "Lilex"],
    );
    assert!(
        platform
            .0
            .read()
            .memory_source
            .select_by_postscript_name("Lilex-Bold")
            .is_err()
    );
    assert_eq!(platform.font_id(&requested)?, old_id);
    assert_eq!(platform.0.read().fonts.len(), before.len());
    assert_loaded_fonts_retained(&platform, &before);

    platform.add_fonts(vec![
        Cow::Owned(NOTO_EMOJI.to_vec()),
        Cow::Owned(LILEX_BOLD.to_vec()),
    ])?;

    assert!(platform.0.read().font_selections.is_empty());
    assert!(platform.0.read().font_ids_by_font_key.is_empty());
    assert_eq!(
        platform.0.read().memory_source.all_families()?,
        ["IBM Plex Sans", "Lilex", "Noto Emoji"],
    );
    let selected = platform.font_id(&requested)?;
    assert_ne!(selected, old_id);
    assert_eq!(postscript_name(&platform, selected), "Lilex-Bold");
    assert_eq!(platform.font_id(&font("Lilex"))?, old_id);
    assert_loaded_fonts_retained(&platform, &before);
    Ok(())
}
