use super::*;
use open_gpui::{HeadlessAppContext, TextRun, TextSystem, WindowTextSystem, font, px};
use parking_lot::Mutex;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Weak,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const IBM_PLEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/fonts/IBMPlexSans-Regular.ttf"
));
const LILEX: &[u8] = include_bytes!(concat!(
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

fn text_system() -> Result<CosmicTextSystem> {
    let platform = CosmicTextSystem::new_without_system_fonts("IBM Plex Sans");
    platform.add_fonts(vec![Cow::Borrowed(IBM_PLEX)])?;
    Ok(platform)
}

#[test]
fn font_properties_describe_the_selected_face() -> Result<()> {
    let text_system = text_system()?;
    let regular = open_gpui::font("IBM Plex Sans");
    let regular_id = text_system.font_id(&regular)?;
    for (weight, style) in [
        (open_gpui::FontWeight::MEDIUM, open_gpui::FontStyle::Normal),
        (open_gpui::FontWeight::BOLD, open_gpui::FontStyle::Italic),
        (open_gpui::FontWeight::NORMAL, open_gpui::FontStyle::Oblique),
    ] {
        let requested = Font {
            weight,
            style,
            ..regular.clone()
        };
        let font_id = text_system.font_id(&requested)?;
        assert_eq!(font_id, regular_id);
        assert_eq!(
            text_system.font_weight_and_style(font_id)?,
            (open_gpui::FontWeight::NORMAL, open_gpui::FontStyle::Normal)
        );
    }
    Ok(())
}

#[test]
fn font_properties_reject_invalid_font_ids() -> Result<()> {
    let platform = text_system()?;
    platform.font_id(&font("IBM Plex Sans"))?;
    let loaded_count = platform.0.read().loaded_fonts.len();

    for font_id in [FontId(loaded_count), FontId(usize::MAX)] {
        assert_eq!(
            platform
                .font_weight_and_style(font_id)
                .unwrap_err()
                .to_string(),
            "invalid font ID"
        );
    }
    Ok(())
}

#[test]
fn font_properties_follow_reselection_and_preserve_retained_faces() -> Result<()> {
    let platform = CosmicTextSystem::new_without_system_fonts("Lilex");
    platform.add_fonts(vec![Cow::Borrowed(LILEX)])?;
    let requested = font("Lilex").bold().italic();
    let retained_id = platform.font_id(&requested)?;
    assert_eq!(
        platform.font_weight_and_style(retained_id)?,
        (open_gpui::FontWeight::NORMAL, open_gpui::FontStyle::Normal)
    );

    platform.add_fonts(vec![Cow::Borrowed(LILEX_BOLD)])?;

    let selected_id = platform.font_id(&requested)?;
    assert_ne!(selected_id, retained_id);
    assert_eq!(platform.font_id(&requested)?, selected_id);
    assert_eq!(
        platform.font_weight_and_style(selected_id)?,
        (open_gpui::FontWeight::BOLD, open_gpui::FontStyle::Normal)
    );
    assert_eq!(platform.font_id(&font("Lilex"))?, retained_id);
    assert_eq!(
        platform.font_weight_and_style(retained_id)?,
        (open_gpui::FontWeight::NORMAL, open_gpui::FontStyle::Normal)
    );
    Ok(())
}

#[derive(Default)]
struct RecordingSink(Mutex<Vec<MissingGlyph>>);

impl MissingGlyphSink for RecordingSink {
    fn report(&self, missing_glyphs: Vec<MissingGlyph>) {
        self.0.lock().extend(missing_glyphs);
    }
}

#[test]
fn installed_font_sources_retain_borrowed_and_owned_storage_without_copying() -> Result<()> {
    let platform = text_system()?;
    let owned = LILEX.to_vec();
    let owned_pointer = owned.as_ptr();
    platform.add_fonts(vec![Cow::Owned(owned)])?;

    let state = platform.0.read();
    for (family, expected_pointer) in [
        ("IBM Plex Sans", IBM_PLEX.as_ptr()),
        ("Lilex", owned_pointer),
    ] {
        let face = state
            .font_system
            .db()
            .faces()
            .find(|face| face.families.iter().any(|entry| entry.0 == family))
            .context("installed fixture family is missing")?;
        let cosmic_text::fontdb::Source::Binary(data) = &face.source else {
            anyhow::bail!("installed font must retain its binary source");
        };
        let data: &[u8] = data.as_ref().as_ref();
        assert_eq!(data.as_ptr(), expected_pointer);
    }
    Ok(())
}

#[test]
fn installing_fonts_retries_cached_missing_families_and_preserves_loaded_faces() -> Result<()> {
    let platform = text_system()?;
    let primary = font("IBM Plex Sans");
    let primary_id = platform.font_id(&primary)?;
    let face = platform.0.read().loaded_font(primary_id).font.clone();
    let metrics = platform.font_metrics(primary_id);
    let glyph = platform
        .glyph_for_char(primary_id, 'm')
        .context("fixture must contain the measurement glyph")?;
    let advance = platform.advance(primary_id, glyph)?;
    let bounds = platform.typographic_bounds(primary_id, glyph)?;
    let loaded_count = platform.0.read().loaded_fonts.len();
    assert!(platform.font_id(&font("Lilex")).is_err());

    platform.add_fonts(vec![Cow::Borrowed(LILEX)])?;

    assert!(platform.0.read().font_ids_by_family_cache.is_empty());
    assert_eq!(platform.font_id(&primary)?, primary_id);
    assert_eq!(platform.0.read().loaded_fonts.len(), loaded_count);
    assert!(Arc::ptr_eq(
        &face,
        &platform.0.read().loaded_font(primary_id).font
    ));
    let after = platform.font_metrics(primary_id);
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
        ]
    );
    assert_eq!(after.bounding_box, metrics.bounding_box);
    assert_eq!(platform.glyph_for_char(primary_id, 'm'), Some(glyph));
    assert_eq!(platform.advance(primary_id, glyph)?, advance);
    assert_eq!(platform.typographic_bounds(primary_id, glyph)?, bounds);
    assert_ne!(platform.font_id(&font("Lilex"))?, primary_id);
    assert!(platform.all_font_names().iter().any(|name| name == "Lilex"));
    Ok(())
}

#[test]
fn installing_a_missing_fallback_refreshes_its_chain_without_replacing_the_primary() -> Result<()> {
    let platform = text_system()?;
    let plain_id = platform.font_id(&font("IBM Plex Sans"))?;
    let requested = Font {
        fallbacks: Some(FontFallbacks::from_fonts(vec!["Lilex".into()])),
        ..font("IBM Plex Sans")
    };
    let primary_id = platform.font_id(&requested)?;
    let original_chain = platform
        .0
        .read()
        .loaded_font(primary_id)
        .user_fallback_chain
        .clone();
    assert_ne!(primary_id, plain_id);
    assert!(original_chain.is_empty());

    platform.add_fonts(vec![Cow::Borrowed(LILEX)])?;

    assert_eq!(platform.font_id(&requested)?, primary_id);
    let state = platform.0.read();
    let refreshed = &state.loaded_font(primary_id).user_fallback_chain;
    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed[0].1.as_str(), "Lilex");
    assert!(state.loaded_font(plain_id).user_fallback_chain.is_empty());
    assert!(!Arc::ptr_eq(&original_chain, refreshed));
    assert!(original_chain.is_empty());
    Ok(())
}

#[test]
fn loaded_identity_keeps_feature_and_fallback_configurations_separate_after_installation()
-> Result<()> {
    let platform = text_system()?;
    let descriptors = [
        font("IBM Plex Sans"),
        Font {
            features: FontFeatures::disable_ligatures(),
            ..font("IBM Plex Sans")
        },
        Font {
            fallbacks: Some(FontFallbacks::from_fonts(vec!["Lilex".into()])),
            ..font("IBM Plex Sans")
        },
    ];
    let ids = descriptors
        .iter()
        .map(|descriptor| platform.font_id(descriptor))
        .collect::<Result<Vec<_>>>()?;
    assert_ne!(ids[0], ids[1]);
    assert_ne!(ids[0], ids[2]);
    assert_ne!(ids[1], ids[2]);

    platform.add_fonts(vec![Cow::Borrowed(LILEX)])?;

    for (descriptor, id) in descriptors.iter().zip(ids) {
        assert_eq!(platform.font_id(descriptor)?, id);
    }
    Ok(())
}

#[test]
fn successful_installation_changes_a_real_cached_fallback_to_the_direct_face() -> Result<()> {
    let platform = Arc::new(text_system()?);
    let text = TextSystem::new(platform.clone());
    let requested = font("Lilex").bold().italic();
    let fallback = text.resolve_font(&requested);
    assert_eq!(text.get_font_for_id(fallback), Some(font(".OpenGpuiSans")));

    text.add_fonts(vec![Cow::Borrowed(LILEX)])?;

    let direct = platform.font_id(&requested)?;
    assert_ne!(direct, fallback);
    assert_eq!(text.resolve_font(&requested), direct);
    assert_eq!(text.get_font_for_id(direct), Some(requested));
    assert!(platform.glyph_for_char(fallback, 'm').is_some());
    Ok(())
}

#[test]
fn cached_font_resolution_does_not_wait_for_cold_cosmic_lookups() -> Result<()> {
    for family in [".OpenGpuiSans", "Missing Font"] {
        let platform = Arc::new(text_system()?);
        let text = TextSystem::new(platform.clone());
        let requested = font(family);
        let expected = platform.font_id(&font(".OpenGpuiSans"))?;
        assert_eq!(text.resolve_font(&requested), expected);
        let (sender, receiver) = mpsc::channel();

        thread::scope(|scope| {
            let backend = platform.0.read();
            let deadline = Instant::now() + Duration::from_secs(5);
            let cold = scope.spawn(|| text.resolve_font(&requested.clone().bold()));
            while !platform.0.is_locked_exclusive() && Instant::now() < deadline {
                thread::yield_now();
            }
            let cold_waiting = platform.0.is_locked_exclusive();
            let warm = scope.spawn(|| sender.send(text.resolve_font(&requested)));
            let result = receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()));
            drop(backend);
            let cold_result = cold.join();
            let warm_result = warm.join();

            assert!(cold_waiting);
            assert_eq!(result.expect("cached lookup blocked"), expected);
            assert_eq!(cold_result.expect("cold lookup panicked"), expected);
            warm_result
                .expect("warm lookup panicked")
                .expect("receiver dropped");
        });
    }
    Ok(())
}

#[test]
fn installing_fonts_invalidates_real_cached_layouts_without_reloading_unchanged_faces() -> Result<()>
{
    let platform = Arc::new(text_system()?);
    let text = Arc::new(TextSystem::new(platform.clone()));
    let window = WindowTextSystem::new(text.clone());
    let source: SharedString = "cached text".into();
    let runs = [TextRun {
        len: source.len(),
        font: font("IBM Plex Sans"),
        ..Default::default()
    }];
    let initial = window.shape_line(source.clone(), px(14.), &runs, None);
    let cached = window.shape_line(source.clone(), px(14.), &runs, None);
    assert!(std::ptr::eq::<LineLayout>(&**initial, &**cached));
    let loaded_count = platform.0.read().loaded_fonts.len();

    text.add_fonts(vec![Cow::Borrowed(LILEX)])?;

    let refreshed = window.shape_line(source, px(14.), &runs, None);
    assert!(!std::ptr::eq::<LineLayout>(&**initial, &**refreshed));
    assert_eq!(initial.width, refreshed.width);
    assert_eq!(initial.runs[0].font_id, refreshed.runs[0].font_id);
    assert_eq!(platform.0.read().loaded_fonts.len(), loaded_count);
    Ok(())
}

#[test]
fn missing_indices_report_whole_graphemes_once_with_each_source_font_class() -> Result<()> {
    let platform = text_system()?;
    platform.add_fonts(vec![Cow::Borrowed(LILEX)])?;
    let proportional = "x\u{0301}";
    let monospace = "👩‍💻";
    let source = format!("{proportional}{monospace}");
    let runs = [
        FontRun {
            len: proportional.len(),
            font_id: platform.font_id(&font("IBM Plex Sans"))?,
        },
        FontRun {
            len: monospace.len(),
            font_id: platform.font_id(&font("Lilex"))?,
        },
    ];
    let reports = platform.missing_glyphs(
        &source,
        &runs,
        [
            proportional.len() + "👩‍".len(),
            1,
            proportional.len(),
            0,
            1,
            source.len(),
        ],
    );
    assert_eq!(
        reports,
        vec![
            MissingGlyph::new(proportional.into(), FallbackFontClass::Proportional),
            MissingGlyph::new(monospace.into(), FallbackFontClass::Monospace),
        ]
    );
    Ok(())
}

#[test]
fn missing_indices_without_font_runs_use_proportional_and_ignore_out_of_source_indices() {
    let platform = CosmicTextSystem::new_without_system_fonts("unused");
    assert!(platform.missing_glyphs("", &[], [0, usize::MAX]).is_empty());
    assert!(platform.missing_glyphs("界", &[], []).is_empty());
    assert_eq!(
        platform.missing_glyphs("界", &[], [usize::MAX, 0, 0, "界".len()]),
        vec![MissingGlyph::new(
            "界".into(),
            FallbackFontClass::Proportional
        )]
    );
}

#[test]
fn real_shaping_reports_unresolved_combining_and_zwj_graphemes_with_source_classes() -> Result<()> {
    let platform = text_system()?;
    platform.add_fonts(vec![Cow::Borrowed(LILEX)])?;
    let proportional = "界\u{0301}";
    let monospace = "👩‍💻";
    let source = format!("{proportional}{monospace}");
    let runs = [
        FontRun {
            len: proportional.len(),
            font_id: platform.font_id(&font("IBM Plex Sans"))?,
        },
        FontRun {
            len: monospace.len(),
            font_id: platform.font_id(&font("Lilex"))?,
        },
    ];
    assert!(platform.glyph_for_char(runs[0].font_id, '界').is_none());
    assert!(platform.glyph_for_char(runs[1].font_id, '👩').is_none());
    let sink = Arc::new(RecordingSink::default());
    platform.set_missing_glyph_sink(Some(sink.clone()));

    let layout = platform.layout_line(&source, px(14.), &runs);

    assert_eq!(layout.len, source.len());
    assert!(
        layout
            .runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .any(|glyph| glyph.id.0 == 0)
    );
    assert_eq!(
        sink.0.lock().as_slice(),
        &[
            MissingGlyph::new(proportional.into(), FallbackFontClass::Proportional),
            MissingGlyph::new(monospace.into(), FallbackFontClass::Monospace),
        ]
    );
    Ok(())
}

#[test]
fn rejected_no_latin_primary_retains_its_face_for_actual_fallback_and_raster() -> Result<()> {
    let source = "A😀B";
    let control = text_system()?;
    let control_id = control.font_id(&font("IBM Plex Sans"))?;
    assert!(control.glyph_for_char(control_id, '😀').is_none());
    let control_sink = Arc::new(RecordingSink::default());
    control.set_missing_glyph_sink(Some(control_sink.clone()));
    let control_layout = control.layout_line(
        source,
        px(32.),
        &[FontRun {
            len: source.len(),
            font_id: control_id,
        }],
    );
    assert_eq!(control_layout.len, source.len());
    assert!(
        control_layout
            .runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .any(|glyph| glyph.index == 1 && glyph.id.0 == 0)
    );
    assert_eq!(
        control_sink.0.lock().as_slice(),
        &[MissingGlyph::new(
            "😀".into(),
            FallbackFontClass::Proportional
        )]
    );

    let platform = text_system()?;
    platform.add_fonts(vec![Cow::Borrowed(NOTO_EMOJI)])?;
    let primary_id = platform.font_id(&font("IBM Plex Sans"))?;
    let noto_database_id = {
        let mut state = platform.0.write();
        assert_eq!(state.font_system.db().faces().count(), 2);
        let database_id = state
            .font_system
            .db()
            .faces()
            .find(|face| face.families.iter().any(|entry| entry.0 == "Noto Emoji"))
            .context("Noto Emoji fixture family is missing")?
            .id;
        let face = state
            .font_system
            .get_font(database_id, cosmic_text::Weight::NORMAL)
            .context("Noto Emoji fixture could not be loaded")?;
        assert_eq!(face.as_swash().charmap().map('m'), 0);
        assert_eq!(face.as_swash().charmap().map('😀'), 878);
        database_id
    };
    assert!(platform.font_id(&font("Noto Emoji")).is_err());
    let face_retained = platform
        .0
        .read()
        .font_system
        .db()
        .face(noto_database_id)
        .is_some();
    let sink = Arc::new(RecordingSink::default());
    platform.set_missing_glyph_sink(Some(sink.clone()));

    let layout = platform.layout_line(
        source,
        px(32.),
        &[FontRun {
            len: source.len(),
            font_id: primary_id,
        }],
    );

    assert!(
        face_retained,
        "a rejected primary must remain in the fallback database"
    );
    assert_eq!(layout.len, source.len());
    assert!(layout.width > Pixels::ZERO);
    assert!(f32::from(layout.width).is_finite());
    let glyphs = layout
        .runs
        .iter()
        .flat_map(|run| run.glyphs.iter().map(move |glyph| (run.font_id, glyph)))
        .collect::<Vec<_>>();
    assert_eq!(
        glyphs
            .iter()
            .map(|(_, glyph)| glyph.index)
            .collect::<Vec<_>>(),
        vec![0, 1, 5]
    );
    assert!(glyphs.iter().all(|(_, glyph)| glyph.id.0 != 0));
    assert!(
        glyphs
            .windows(2)
            .all(|pair| pair[0].1.position.x < pair[1].1.position.x)
    );
    assert_eq!(glyphs[0].0, primary_id);
    assert_eq!(glyphs[2].0, primary_id);
    let (fallback_id, emoji) = glyphs[1];
    assert_ne!(fallback_id, primary_id);
    assert_eq!(emoji.id, GlyphId(878));
    assert!(!emoji.is_emoji);
    assert_eq!(
        platform.0.read().loaded_font(fallback_id).font.id(),
        noto_database_id
    );
    assert_eq!(platform.glyph_for_char(fallback_id, '😀'), Some(emoji.id));
    assert!(platform.glyph_for_char(fallback_id, 'm').is_none());
    assert!(platform.font_id(&font("Noto Emoji")).is_err());
    assert!(sink.0.lock().is_empty());

    for scale_factor in [1., 1.5] {
        let params = RenderGlyphParams {
            font_id: fallback_id,
            glyph_id: emoji.id,
            font_size: px(32.),
            subpixel_variant: point(0, 0),
            scale_factor,
            is_emoji: emoji.is_emoji,
            subpixel_rendering: false,
            dilation: 0,
        };
        let bounds = platform.glyph_raster_bounds(&params)?;
        assert!(bounds.size.width.0 > 0);
        assert!(bounds.size.height.0 > 0);
        let (size, pixels) = platform.rasterize_glyph(&params, bounds)?;
        assert_eq!(size, bounds.size);
        assert_eq!(
            pixels.len(),
            usize::try_from(size.width.0)? * usize::try_from(size.height.0)?
        );
        assert!(pixels.iter().any(|alpha| *alpha != 0));
        assert!(pixels.contains(&0));
        let (repeated_size, repeated_pixels) = platform.rasterize_glyph(&params, bounds)?;
        assert_eq!(repeated_size, size);
        assert_eq!(repeated_pixels, pixels);
    }
    Ok(())
}

#[test]
fn replacing_and_disabling_a_sink_releases_the_previous_reporter() -> Result<()> {
    let platform = text_system()?;
    let runs = [FontRun {
        len: "界".len(),
        font_id: platform.font_id(&font("IBM Plex Sans"))?,
    }];
    assert!(platform.0.read().missing_glyph_sink.is_none());
    platform.layout_line("界", px(14.), &runs);
    let first = Arc::new(RecordingSink::default());
    let first_weak = Arc::downgrade(&first);
    platform.set_missing_glyph_sink(Some(first.clone()));
    platform.layout_line("界", px(14.), &runs);
    assert_eq!(first.0.lock().len(), 1);

    let second = Arc::new(RecordingSink::default());
    let second_weak = Arc::downgrade(&second);
    platform.set_missing_glyph_sink(Some(second.clone()));
    drop(first);
    assert!(first_weak.upgrade().is_none());
    platform.layout_line("界", px(14.), &runs);
    assert_eq!(second.0.lock().len(), 1);

    platform.set_missing_glyph_sink(None);
    drop(second);
    assert!(second_weak.upgrade().is_none());
    assert!(platform.0.read().missing_glyph_sink.is_none());
    platform.layout_line("界", px(14.), &runs);
    Ok(())
}

#[test]
fn replacing_and_disabling_sinks_drop_after_the_state_lock_is_released() -> Result<()> {
    struct ReentrantDropSink {
        platform: Weak<CosmicTextSystem>,
        reentered: Arc<AtomicBool>,
    }

    impl MissingGlyphSink for ReentrantDropSink {
        fn report(&self, _: Vec<MissingGlyph>) {}
    }

    impl Drop for ReentrantDropSink {
        fn drop(&mut self) {
            let Some(platform) = self.platform.upgrade() else {
                return;
            };
            let unlocked = platform.0.try_read().is_some();
            if unlocked {
                let names = platform.all_font_names();
                self.reentered.store(
                    names.iter().any(|name| name == "IBM Plex Sans"),
                    Ordering::Release,
                );
            }
        }
    }

    let platform = Arc::new(text_system()?);
    for replace in [true, false] {
        let reentered = Arc::new(AtomicBool::new(false));
        platform.set_missing_glyph_sink(Some(Arc::new(ReentrantDropSink {
            platform: Arc::downgrade(&platform),
            reentered: reentered.clone(),
        })));
        let replacement =
            replace.then(|| Arc::new(RecordingSink::default()) as Arc<dyn MissingGlyphSink>);

        platform.set_missing_glyph_sink(replacement);

        assert!(
            reentered.load(Ordering::Acquire),
            "the previous sink must reenter font queries after the state lock is released"
        );
        assert_eq!(platform.0.read().missing_glyph_sink.is_some(), replace);
    }
    Ok(())
}

#[test]
fn headless_app_observer_receives_cosmic_reports_from_the_apps_actual_text_system() -> Result<()> {
    let platform = Arc::new(CosmicTextSystem::new_without_system_fonts("IBM Plex Sans"));
    let mut app = HeadlessAppContext::new(platform);
    let text = app.update(|cx| cx.text_system().clone());
    text.add_fonts(vec![Cow::Borrowed(IBM_PLEX), Cow::Borrowed(LILEX)])?;
    let observed = Rc::new(RefCell::new(Vec::new()));
    let subscription = app.update(|cx| {
        let observed = observed.clone();
        cx.on_missing_glyphs(move |missing, _| observed.borrow_mut().extend_from_slice(missing))
    });
    let proportional = "界\u{0301}";
    let monospace = "👩‍💻";
    let source: SharedString = format!("{proportional}{monospace}").into();
    let runs = [
        TextRun {
            len: proportional.len(),
            font: font("IBM Plex Sans"),
            ..Default::default()
        },
        TextRun {
            len: monospace.len(),
            font: font("Lilex"),
            ..Default::default()
        },
    ];
    let window_text = WindowTextSystem::new(text);

    window_text.shape_line(source, px(14.), &runs, None);
    assert!(observed.borrow().is_empty());
    app.run_until_parked();

    assert_eq!(
        observed.borrow().as_slice(),
        &[
            MissingGlyph::new(proportional.into(), FallbackFontClass::Proportional),
            MissingGlyph::new(monospace.into(), FallbackFontClass::Monospace),
        ]
    );
    drop(subscription);
    Ok(())
}
