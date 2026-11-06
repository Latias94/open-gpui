use super::*;
use crate::{GlyphId, NoopTextSystem};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Default)]
pub(crate) struct DynamicPlatform {
    installed: AtomicBool,
    lookups: AtomicUsize,
    layouts: AtomicUsize,
    descriptors: Mutex<Vec<Font>>,
    prewarmed: Mutex<Vec<Vec<FontId>>>,
}

impl PlatformTextSystem for DynamicPlatform {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        if fonts.iter().any(|bytes| bytes.as_ref() == b"reject") {
            anyhow::bail!("font rejected");
        }
        self.installed.store(true, Ordering::Release);
        Ok(())
    }

    fn all_font_names(&self) -> Vec<String> {
        vec!["primary".into(), "Arial".into(), "primary".into()]
    }

    fn font_id(&self, descriptor: &Font) -> Result<FontId> {
        self.lookups.fetch_add(1, Ordering::Relaxed);
        self.descriptors.lock().push(descriptor.clone());
        match descriptor.family.as_str() {
            "primary" | "Arial" => Ok(FontId(1)),
            "late" if self.installed.load(Ordering::Acquire) => Ok(FontId(2)),
            _ => anyhow::bail!("font not installed"),
        }
    }

    fn prewarm_fonts(&self, font_ids: &[FontId]) {
        self.prewarmed.lock().push(font_ids.to_vec());
    }

    fn font_metrics(&self, id: FontId) -> FontMetrics {
        NoopTextSystem.font_metrics(id)
    }

    fn typographic_bounds(&self, id: FontId, glyph: GlyphId) -> Result<Bounds<f32>> {
        NoopTextSystem.typographic_bounds(id, glyph)
    }

    fn advance(&self, id: FontId, glyph: GlyphId) -> Result<Size<f32>> {
        NoopTextSystem.advance(id, glyph)
    }

    fn glyph_for_char(&self, id: FontId, character: char) -> Option<GlyphId> {
        NoopTextSystem.glyph_for_char(id, character)
    }

    fn glyph_raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        NoopTextSystem.glyph_raster_bounds(params)
    }

    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        NoopTextSystem.rasterize_glyph(params, bounds)
    }

    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout {
        self.layouts.fetch_add(1, Ordering::Relaxed);
        let scale = if self.installed.load(Ordering::Acquire) {
            2.
        } else {
            1.
        };
        NoopTextSystem.layout_line(text, font_size * scale, runs)
    }

    fn recommended_rendering_mode(&self, id: FontId, font_size: Pixels) -> TextRenderingMode {
        NoopTextSystem.recommended_rendering_mode(id, font_size)
    }
}

pub(super) fn fixture() -> (Arc<DynamicPlatform>, Arc<TextSystem>, WindowTextSystem) {
    let platform = Arc::new(DynamicPlatform::default());
    let text = Arc::new(TextSystem::new(platform.clone()));
    let window = WindowTextSystem::new(text.clone());
    (platform, text, window)
}

fn run(text: &str) -> TextRun {
    TextRun {
        len: text.len(),
        font: font("primary"),
        ..Default::default()
    }
}

#[test]
fn all_font_names_reports_only_backend_families() {
    let (_, text, _) = fixture();
    assert_eq!(text.all_font_names(), ["Arial", "primary"]);
}

#[test]
fn all_font_names_keeps_an_empty_backend_empty() {
    let text = TextSystem::new(Arc::new(NoopTextSystem));
    assert!(text.all_font_names().is_empty());
}

#[test]
fn prewarming_resolves_complete_descriptors_and_deduplicates_platform_faces() {
    let (platform, text, window) = fixture();
    let descriptors = [
        font("primary"),
        font("primary").bold(),
        font("primary"),
        font("late"),
        font("late").italic(),
    ];
    text.prewarm_fonts(&descriptors);
    assert_eq!(*platform.prewarmed.lock(), [vec![FontId(1)]]);
    let lookups = platform.lookups.load(Ordering::Relaxed);
    for descriptor in &descriptors {
        assert_eq!(text.resolve_font(descriptor), FontId(1));
        let line = window.shape_line(
            "prepared".into(),
            px(16.),
            &[TextRun {
                len: "prepared".len(),
                font: descriptor.clone(),
                ..Default::default()
            }],
            None,
        );
        assert_eq!(line.text.as_ref(), "prepared");
    }
    assert_eq!(platform.lookups.load(Ordering::Relaxed), lookups);
    let observed = platform.descriptors.lock();
    for descriptor in &descriptors {
        assert_eq!(
            observed.iter().filter(|font| *font == descriptor).count(),
            1
        );
    }
}

#[test]
fn prewarming_retains_failed_installation_and_refreshes_after_success() {
    let (platform, text, _) = fixture();
    let requested = font("late").bold().italic();
    text.prewarm_fonts(std::slice::from_ref(&requested));
    let lookups = platform.lookups.load(Ordering::Relaxed);
    assert!(text.add_fonts(vec![Cow::Borrowed(b"reject")]).is_err());
    text.prewarm_fonts(std::slice::from_ref(&requested));
    assert_eq!(platform.lookups.load(Ordering::Relaxed), lookups);
    assert_eq!(
        *platform.prewarmed.lock(),
        [vec![FontId(1)], vec![FontId(1)]]
    );

    text.add_fonts(vec![Cow::Borrowed(b"font")]).unwrap();
    text.prewarm_fonts(std::slice::from_ref(&requested));
    assert_eq!(platform.prewarmed.lock().last(), Some(&vec![FontId(2)]));
    let lookups = platform.lookups.load(Ordering::Relaxed);
    assert_eq!(text.resolve_font(&requested), FontId(2));
    assert_eq!(platform.lookups.load(Ordering::Relaxed), lookups);
    assert_eq!(text.get_font_for_id(FontId(2)), Some(requested));
}

#[test]
fn successful_installation_replaces_a_cached_fallback_with_the_direct_face() {
    let (_, text, _) = fixture();
    let requested = font("late").bold().italic();
    assert_eq!(text.resolve_font(&requested), FontId(1));
    assert_eq!(text.get_font_for_id(FontId(1)), Some(font("Arial")));
    text.add_fonts(vec![Cow::Borrowed(b"font")]).unwrap();
    assert_eq!(text.resolve_font(&requested), FontId(2));
    assert_eq!(text.get_font_for_id(FontId(2)), Some(requested));
}

#[test]
fn resolved_fallback_is_retained_as_the_requested_descriptor_result() {
    let platform = Arc::new(DynamicPlatform::default());
    let mut text = TextSystem::new(platform);
    let requested = font("late");
    assert_eq!(text.resolve_font(&requested), FontId(1));
    text.fallback_font_stack.clear();
    assert_eq!(text.resolve_font(&requested), FontId(1));
    assert_eq!(text.get_font_for_id(FontId(1)), Some(font("Arial")));
}

#[test]
fn installation_invalidates_plain_wrapped_and_shaped_layouts_with_stable_font_ids() {
    let (platform, text, window) = fixture();
    let source = "alpha beta 你好";
    let runs = [run(source)];
    let initial = window.layout_line(source, px(16.), &runs, None);
    let wrapped = window
        .shape_text(source.into(), px(16.), &runs, Some(px(70.)), None)
        .unwrap();
    let shaped = window.shape_line(source.into(), px(16.), &runs, None);
    assert_eq!(shaped.text.as_ref(), source);
    assert!(Arc::ptr_eq(&initial, &shaped.layout));
    window.finish_frame();
    text.add_fonts(vec![Cow::Borrowed(b"font")]).unwrap();
    assert_eq!(text.resolve_font(&font("primary")), FontId(1));
    let after = window.layout_line(source, px(16.), &runs, None);
    assert!(!Arc::ptr_eq(&initial, &after));
    assert_eq!(after.width, initial.width * 2.);
    let after_wrapped = window
        .shape_text(source.into(), px(16.), &runs, Some(px(70.)), None)
        .unwrap();
    assert!(!Arc::ptr_eq(&wrapped[0].layout, &after_wrapped[0].layout));
    let after_shaped = window.shape_line(source.into(), px(16.), &runs, None);
    assert_eq!(after_shaped.text.as_ref(), source);
    assert!(Arc::ptr_eq(&after, &after_shaped.layout));
    assert!(!Arc::ptr_eq(&shaped.layout, &after_shaped.layout));
    assert_eq!(after_shaped.width, shaped.width * 2.);
    assert_eq!(platform.layouts.load(Ordering::Relaxed), 2);
}

#[test]
fn failed_installation_preserves_resolution_and_layout_reuse() {
    let (platform, text, window) = fixture();
    let source = "unchanged";
    let runs = [run(source)];
    let initial = window.layout_line(source, px(16.), &runs, None);
    let before_lookups = platform.lookups.load(Ordering::Relaxed);
    let before_layouts = platform.layouts.load(Ordering::Relaxed);
    assert!(text.add_fonts(vec![Cow::Borrowed(b"reject")]).is_err());
    let after = window.layout_line(source, px(16.), &runs, None);
    assert!(Arc::ptr_eq(&initial, &after));
    assert_eq!(platform.lookups.load(Ordering::Relaxed), before_lookups);
    assert_eq!(platform.layouts.load(Ordering::Relaxed), before_layouts);
}

#[test]
fn installation_refreshes_pooled_and_checked_out_character_widths() {
    let (_, text, _) = fixture();
    let mut borrowed = text.line_wrapper(font("primary"), px(16.));
    let mut pooled = text.line_wrapper(font("primary"), px(16.));
    assert!(
        borrowed
            .should_truncate_line("abcdefgh", px(90.), "", TruncateFrom::End)
            .is_none()
    );
    assert!(
        pooled
            .should_truncate_line("abcdefgh", px(90.), "", TruncateFrom::End)
            .is_none()
    );
    drop(pooled);
    text.add_fonts(vec![Cow::Borrowed(b"font")]).unwrap();
    let mut reused = text.line_wrapper(font("primary"), px(16.));
    assert!(
        reused
            .should_truncate_line("abcdefgh", px(90.), "", TruncateFrom::End)
            .is_some()
    );
    assert!(
        borrowed
            .should_truncate_line("abcdefgh", px(90.), "", TruncateFrom::End)
            .is_some()
    );
    drop(borrowed);
    drop(reused);
    let mut again = text.line_wrapper(font("primary"), px(16.));
    assert!(
        again
            .should_truncate_line("abcdefgh", px(90.), "", TruncateFrom::End)
            .is_some()
    );
}

#[test]
fn a_checked_out_wrapper_reresolves_its_requested_font_and_returns_under_the_new_id() {
    let (_, text, _) = fixture();
    let mut wrapper = text.line_wrapper(font("late"), px(16.));
    wrapper.should_truncate_line("abcdefgh", px(90.), "", TruncateFrom::End);
    assert_eq!(wrapper.font_id, FontId(1));
    text.add_fonts(vec![Cow::Borrowed(b"font")]).unwrap();
    wrapper.should_truncate_line("abcdefgh", px(90.), "", TruncateFrom::End);
    assert_eq!(wrapper.font_id, FontId(2));
    drop(wrapper);
    assert_eq!(text.line_wrapper(font("late"), px(16.)).font_id, FontId(2));
}

#[test]
fn an_old_font_selection_cannot_borrow_a_new_generation_width_cache_as_its_resolution() {
    let (_, text, _) = fixture();
    let generation = text.font_generation();
    let requested = font("late");
    let old_id = text.resolve_font(&requested);
    text.add_fonts(vec![Cow::Borrowed(b"font")]).unwrap();
    let mut primary = text.line_wrapper(font("primary"), px(16.));
    primary.should_truncate_line("abcdefgh", px(90.), "", TruncateFrom::End);
    drop(primary);
    let widths = text
        .wrapper_pool
        .lock()
        .get_mut(&FontIdWithSize {
            font_id: old_id,
            font_size: px(16.),
        })
        .unwrap()
        .pop()
        .unwrap();
    let mut wrapper =
        LineWrapper::new(requested, old_id, generation, px(16.), text.clone(), widths);
    wrapper.should_truncate_line("abcdefgh", px(90.), "", TruncateFrom::End);
    assert_eq!(wrapper.font_id, FontId(2));
}

#[test]
fn public_headless_text_system_is_the_apps_installation_and_layout_owner() {
    let mut app = crate::HeadlessAppContext::new(Arc::new(DynamicPlatform::default()));
    let actual = app.update(|cx| cx.text_system().clone());
    assert!(Arc::ptr_eq(app.text_system(), &actual));
    let window = WindowTextSystem::new(actual.clone());
    let source = "same owner";
    let runs = [run(source)];
    let initial = window.layout_line(source, px(16.), &runs, None);
    assert_eq!(actual.resolve_font(&font("late")), FontId(1));
    app.text_system()
        .add_fonts(vec![Cow::Borrowed(b"font")])
        .unwrap();
    assert_eq!(actual.resolve_font(&font("late")), FontId(2));
    let after = window.layout_line(source, px(16.), &runs, None);
    assert!(!Arc::ptr_eq(&initial, &after));
    assert_eq!(after.width, initial.width * 2.);
}

#[test]
fn public_test_context_text_system_is_the_apps_exact_instance() {
    let app = crate::TestAppContext::single();
    assert!(app.read(|cx| Arc::ptr_eq(app.text_system(), cx.text_system())));
}
