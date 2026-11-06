use open_gpui::{NoopTextSystem, TextSystem, font};
use std::sync::Arc;

#[test]
fn downstream_noop_font_inventory_stays_empty_after_prewarming() {
    let platform = Arc::new(NoopTextSystem::new());
    let platform_lifetime = Arc::downgrade(&platform);
    let text_system = Arc::new(TextSystem::new(platform.clone()));
    let text_lifetime = Arc::downgrade(&text_system);
    assert!(text_system.all_font_names().is_empty());
    text_system.prewarm_fonts(&[]);
    text_system.prewarm_fonts(&[font("downstream-noop"), font(".SystemUIFont")]);
    assert!(text_system.all_font_names().is_empty());
    assert_eq!(
        text_system.resolve_font(&font("downstream-noop")),
        text_system.resolve_font(&font(".SystemUIFont")),
    );
    drop(text_system);
    assert!(text_lifetime.upgrade().is_none());
    drop(platform);
    assert!(platform_lifetime.upgrade().is_none());
}

#[cfg(feature = "wgpu-renderer")]
mod cosmic {
    use open_gpui::{
        Font, FontFallbacks, FontId, GlyphId, Pixels, PlatformTextSystem, Point, RenderGlyphParams,
        ShapedLine, TextRun, TextSystem, WindowTextSystem, font, point, px,
    };
    use open_gpui_wgpu::CosmicTextSystem;
    use std::{borrow::Cow, sync::Arc};

    const IBM_PLEX: &[u8] = include_bytes!("../../fonts/IBMPlexSans-Regular.ttf");
    const LILEX: &[u8] = include_bytes!("../../fonts/Lilex-Regular.ttf");
    const LILEX_BOLD: &[u8] = include_bytes!("../../fonts/Lilex-Bold.ttf");
    const NOTO_EMOJI: &[u8] = include_bytes!("../../fonts/NotoEmoji-Regular.ttf");
    const NOTO_HEBREW: &[u8] = include_bytes!("../../fonts/NotoSansHebrew-Regular.ttf");
    const SOURCE: &str = "miéאב😀";

    #[derive(Clone, Debug, PartialEq)]
    struct GlyphObservation {
        font_id: FontId,
        id: GlyphId,
        position: Point<Pixels>,
        index: usize,
        is_emoji: bool,
    }

    fn glyphs(line: &ShapedLine) -> Vec<GlyphObservation> {
        line.runs
            .iter()
            .flat_map(|run| {
                run.glyphs.iter().map(move |glyph| GlyphObservation {
                    font_id: run.font_id,
                    id: glyph.id,
                    position: glyph.position,
                    index: glyph.index,
                    is_emoji: glyph.is_emoji,
                })
            })
            .collect()
    }

    fn assert_real_layout(platform: &CosmicTextSystem, line: &ShapedLine) {
        assert_eq!(line.text.as_ref(), SOURCE);
        assert_eq!(line.len(), SOURCE.len());
        assert!(line.width > px(0.) && line.width.as_f32().is_finite());
        assert_eq!(line.font_size, px(24.));
        assert!(line.ascent > px(0.));
        let shaped = glyphs(line);
        for (index, character) in SOURCE.char_indices() {
            let glyph = shaped
                .iter()
                .find(|glyph| glyph.index == index)
                .expect("each fixture character must retain its source byte index");
            assert_ne!(glyph.id, GlyphId(0));
            assert_eq!(
                platform.glyph_for_char(glyph.font_id, character),
                Some(glyph.id),
            );
            if matches!(character, 'א' | 'ב' | '😀') {
                assert!(platform.glyph_for_char(glyph.font_id, 'm').is_none());
            }
        }
        for glyph in shaped {
            assert!(glyph.index < SOURCE.len() && SOURCE.is_char_boundary(glyph.index));
            assert!(glyph.position.x.as_f32().is_finite());
            assert!(glyph.position.y.as_f32().is_finite());
        }
    }

    fn assert_font_names(text_system: &TextSystem, platform: &CosmicTextSystem, expected: &[&str]) {
        let mut backend_names = platform.all_font_names();
        backend_names.sort();
        backend_names.dedup();
        assert_eq!(text_system.all_font_names(), backend_names);
        assert_eq!(backend_names, expected);
    }

    #[test]
    fn downstream_cosmic_prewarms_installs_and_reshapes_without_retiring_old_glyphs() {
        let platform = Arc::new(CosmicTextSystem::new_without_system_fonts("IBM Plex Sans"));
        let platform_lifetime = Arc::downgrade(&platform);
        let text_system = Arc::new(TextSystem::new(platform.clone()));
        let text_lifetime = Arc::downgrade(&text_system);
        assert_font_names(&text_system, &platform, &[]);
        text_system.prewarm_fonts(&[]);
        assert_font_names(&text_system, &platform, &[]);
        text_system
            .add_fonts(vec![
                Cow::Borrowed(NOTO_HEBREW),
                Cow::Borrowed(IBM_PLEX),
                Cow::Borrowed(NOTO_EMOJI),
            ])
            .expect("the independent consumer must install its tracked font bytes");
        let initial_names = ["IBM Plex Sans", "Noto Emoji", "Noto Sans Hebrew"];
        assert_font_names(&text_system, &platform, &initial_names);
        let primary = Font {
            fallbacks: Some(FontFallbacks::from_fonts(vec![
                "Noto Sans Hebrew".into(),
                "Noto Emoji".into(),
            ])),
            ..font("IBM Plex Sans")
        };
        let aliases = [".SystemUIFont", ".OpenGpuiSans", ".UiSans", ".ZedSans"].map(font);
        text_system.prewarm_fonts(&aliases);
        let primary_id = text_system.resolve_font(&primary);
        let primary_m = platform
            .glyph_for_char(primary_id, 'm')
            .expect("the Latin primary must contain m");
        for alias in &aliases {
            let alias_id = platform
                .font_id(alias)
                .expect("virtual aliases must resolve directly without entering the inventory");
            assert_eq!(text_system.resolve_font(alias), alias_id);
            assert_eq!(platform.glyph_for_char(alias_id, 'm'), Some(primary_m));
        }
        assert_font_names(&text_system, &platform, &initial_names);
        assert!(platform.font_id(&font("Lilex")).is_err());
        let window_text = WindowTextSystem::new(text_system.clone());

        for (requested, bytes) in [(font("Lilex"), LILEX), (font("Lilex").bold(), LILEX_BOLD)] {
            text_system.prewarm_fonts(&[requested.clone(), primary.clone()]);
            let previous_font = text_system.resolve_font(&requested);
            let runs = [
                TextRun {
                    len: "m".len(),
                    font: requested.clone(),
                    ..Default::default()
                },
                TextRun {
                    len: SOURCE.len() - "m".len(),
                    font: primary.clone(),
                    ..Default::default()
                },
            ];
            let original = window_text.shape_line(SOURCE.into(), px(24.), &runs, None);
            assert_real_layout(&platform, &original);
            let original_glyphs = glyphs(&original);
            let original_measurements = [
                original.font_size,
                original.width,
                original.ascent,
                original.descent,
            ];
            let original_m = original_glyphs
                .iter()
                .find(|glyph| glyph.index == 0)
                .expect("the requested Latin run must shape its own m");
            assert_eq!(original_m.font_id, previous_font);
            let original_metrics = platform.font_metrics(previous_font);
            let original_advance = platform
                .advance(previous_font, original_m.id)
                .expect("the original m must have an advance");
            let original_outline = platform
                .typographic_bounds(previous_font, original_m.id)
                .expect("the original m must have typographic bounds");
            let params = RenderGlyphParams {
                font_id: previous_font,
                glyph_id: original_m.id,
                font_size: px(24.),
                subpixel_variant: point(0, 0),
                scale_factor: 1.5,
                is_emoji: original_m.is_emoji,
                subpixel_rendering: false,
                dilation: 0,
            };
            let original_bounds = platform
                .glyph_raster_bounds(&params)
                .expect("the original glyph must expose real software raster bounds");
            let original_raster = platform
                .rasterize_glyph(&params, original_bounds)
                .expect("the original glyph must produce a real software raster");
            assert_eq!(original_raster.0, original_bounds.size);
            assert!(original_bounds.size.width.0 > 0 && original_bounds.size.height.0 > 0);
            assert!(original_raster.1.iter().any(|byte| *byte != 0));

            text_system
                .add_fonts(vec![Cow::Borrowed(bytes)])
                .expect("the same consumer TextSystem must install the next Lilex face");
            let installed_names = ["IBM Plex Sans", "Lilex", "Noto Emoji", "Noto Sans Hebrew"];
            assert_font_names(&text_system, &platform, &installed_names);
            text_system.prewarm_fonts(&[requested.clone(), primary.clone()]);
            assert_font_names(&text_system, &platform, &installed_names);
            let installed_font = platform
                .font_id(&requested)
                .expect("the installed face must resolve directly in the real backend");
            assert_ne!(installed_font, previous_font);
            assert_eq!(text_system.resolve_font(&requested), installed_font);
            let installed = window_text.shape_line(SOURCE.into(), px(24.), &runs, None);
            assert_real_layout(&platform, &installed);
            assert!(!Arc::ptr_eq(&*original, &*installed));
            let installed_glyphs = glyphs(&installed);
            let installed_m = installed_glyphs
                .iter()
                .find(|glyph| glyph.index == 0)
                .expect("the installed Latin run must retain m's original byte index");
            assert_eq!(installed_m.font_id, installed_font);
            assert_eq!(installed_m.id, GlyphId(281));
            assert_eq!(platform.font_metrics(installed_font).units_per_em, 1_000);
            assert_eq!(
                platform
                    .advance(installed_font, installed_m.id)
                    .unwrap()
                    .width,
                600.,
            );
            assert_eq!(text_system.resolve_font(&primary), primary_id);
            let mono_alias = font(".ZedMono");
            let alias_id = platform
                .font_id(&mono_alias)
                .expect("the installed Lilex family must also resolve through its virtual alias");
            assert_eq!(text_system.resolve_font(&mono_alias), alias_id);
            assert_eq!(platform.glyph_for_char(alias_id, 'm'), Some(GlyphId(281)));

            assert_eq!(glyphs(&original), original_glyphs);
            assert_eq!(
                [
                    original.font_size,
                    original.width,
                    original.ascent,
                    original.descent,
                ],
                original_measurements,
            );
            assert_real_layout(&platform, &original);
            let retained_metrics = platform.font_metrics(previous_font);
            assert_eq!(retained_metrics.units_per_em, original_metrics.units_per_em);
            assert_eq!(retained_metrics.bounding_box, original_metrics.bounding_box);
            assert_eq!(retained_metrics.ascent, original_metrics.ascent);
            assert_eq!(retained_metrics.descent, original_metrics.descent);
            assert_eq!(
                platform.advance(previous_font, original_m.id).unwrap(),
                original_advance,
            );
            assert_eq!(
                platform
                    .typographic_bounds(previous_font, original_m.id)
                    .unwrap(),
                original_outline,
            );
            assert_eq!(
                platform.glyph_raster_bounds(&params).unwrap(),
                original_bounds
            );
            assert_eq!(
                platform.rasterize_glyph(&params, original_bounds).unwrap(),
                original_raster,
            );
        }

        drop(text_system);
        assert!(text_lifetime.upgrade().is_some());
        assert!(platform_lifetime.upgrade().is_some());
        drop(window_text);
        assert!(text_lifetime.upgrade().is_none());
        assert!(platform_lifetime.upgrade().is_some());
        drop(platform);
        assert!(platform_lifetime.upgrade().is_none());
    }
}
