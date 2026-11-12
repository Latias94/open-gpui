use super::*;
use crate::{
    App, AppContext as _, Context, GlyphId, HeadlessAppContext, IntoElement, NoopTextSystem,
    Render, Styled, Window, div, font, size,
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::{AtomicUsize, Ordering},
};

#[derive(Default)]
struct CountingTextSystem {
    layouts: AtomicUsize,
}

impl PlatformTextSystem for CountingTextSystem {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        NoopTextSystem.add_fonts(fonts)
    }

    fn all_font_names(&self) -> Vec<String> {
        vec!["primary".into(), "secondary".into()]
    }

    fn font_id(&self, descriptor: &Font) -> Result<FontId> {
        Ok(FontId(if descriptor.family.as_str() == "secondary" {
            2
        } else {
            1
        }))
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
        let font_width = runs.first().map_or(1, |run| run.font_id.0);
        NoopTextSystem.layout_line(text, font_size * font_width as f32, runs)
    }

    fn recommended_rendering_mode(&self, id: FontId, font_size: Pixels) -> TextRenderingMode {
        NoopTextSystem.recommended_rendering_mode(id, font_size)
    }
}

#[test]
fn pooled_line_wrappers_do_not_retain_text_system() {
    let platform = Arc::new(NoopTextSystem::new());
    let text_system = Arc::new(TextSystem::new(platform.clone()));
    let weak_platform = Arc::downgrade(&platform);
    let weak_text_system = Arc::downgrade(&text_system);
    let mut wrapper = text_system.line_wrapper(font(".OpenGpuiMono"), px(16.));
    let fragments = [LineFragment::text("hello world 你好")];
    wrapper
        .wrap_line(&fragments, px(40.), IndentAdjustment::default())
        .for_each(drop);
    drop(wrapper);
    drop(text_system);
    drop(platform);

    assert!(weak_text_system.upgrade().is_none());
    assert!(weak_platform.upgrade().is_none());
}

#[test]
fn checked_out_wrapper_keeps_text_system_alive_until_its_last_handle_drops() {
    let text_system = Arc::new(TextSystem::new(Arc::new(NoopTextSystem)));
    let weak = Arc::downgrade(&text_system);
    let mut first = text_system.line_wrapper(font("primary"), px(16.));
    let mut second = text_system.line_wrapper(font("primary"), px(16.));
    drop(text_system);

    let fragments = [LineFragment::text("hello world 你好")];
    let expected = first
        .wrap_line(&fragments, px(40.), IndentAdjustment::default())
        .collect::<Vec<_>>();
    assert!(!expected.is_empty());
    drop(first);
    assert!(weak.upgrade().is_some());
    assert_eq!(
        second
            .wrap_line(&fragments, px(40.), IndentAdjustment::default())
            .collect::<Vec<_>>(),
        expected
    );
    drop(second);
    assert!(weak.upgrade().is_none());
}

#[test]
fn returned_ascii_and_unicode_width_caches_are_reused_by_overlapping_handles() {
    let platform = Arc::new(CountingTextSystem::default());
    let text_system = Arc::new(TextSystem::new(platform.clone()));
    let fragments = [LineFragment::text("a a 中 中 é é")];
    let mut first = text_system.line_wrapper(font("primary"), px(16.));
    let mut second = text_system.line_wrapper(font("primary"), px(16.));
    let expected = first
        .wrap_line(&fragments, px(40.), IndentAdjustment::default())
        .collect::<Vec<_>>();
    assert!(!expected.is_empty());
    let cold_layouts = platform.layouts.load(Ordering::Relaxed);
    assert!(cold_layouts > 0);
    drop(first);

    let mut reused = text_system.line_wrapper(font("primary"), px(16.));
    assert_eq!(
        reused
            .wrap_line(&fragments, px(40.), IndentAdjustment::default())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(platform.layouts.load(Ordering::Relaxed), cold_layouts);
    assert_eq!(
        second
            .wrap_line(&fragments, px(40.), IndentAdjustment::default())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(platform.layouts.load(Ordering::Relaxed), cold_layouts * 2);
    drop(second);
    drop(reused);

    let mut first = text_system.line_wrapper(font("primary"), px(16.));
    let mut second = text_system.line_wrapper(font("primary"), px(16.));
    assert_eq!(
        first
            .wrap_line(&fragments, px(40.), IndentAdjustment::default())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        second
            .wrap_line(&fragments, px(40.), IndentAdjustment::default())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(platform.layouts.load(Ordering::Relaxed), cold_layouts * 2);
}

#[test]
fn pooled_width_caches_remain_partitioned_by_resolved_font_and_size() {
    let platform = Arc::new(CountingTextSystem::default());
    let text_system = Arc::new(TextSystem::new(platform.clone()));
    let fragments = [LineFragment::text("a a 中 中 é é")];
    let mut primary = text_system.line_wrapper(font("primary"), px(16.));
    let expected = primary
        .wrap_line(&fragments, px(40.), IndentAdjustment::default())
        .collect::<Vec<_>>();
    let cold_layouts = platform.layouts.load(Ordering::Relaxed);
    drop(primary);

    let mut secondary = text_system.line_wrapper(font("secondary"), px(16.));
    let secondary_breaks = secondary
        .wrap_line(&fragments, px(40.), IndentAdjustment::default())
        .collect::<Vec<_>>();
    assert_ne!(secondary_breaks, expected);
    assert_eq!(platform.layouts.load(Ordering::Relaxed), cold_layouts * 2);
    drop(secondary);

    let mut larger = text_system.line_wrapper(font("primary"), px(32.));
    assert_eq!(
        larger
            .wrap_line(&fragments, px(40.), IndentAdjustment::default())
            .collect::<Vec<_>>(),
        secondary_breaks
    );
    assert_eq!(platform.layouts.load(Ordering::Relaxed), cold_layouts * 3);
    drop(larger);

    let mut primary = text_system.line_wrapper(font("primary"), px(16.));
    assert_eq!(
        primary
            .wrap_line(&fragments, px(40.), IndentAdjustment::default())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(platform.layouts.load(Ordering::Relaxed), cold_layouts * 3);
}

#[test]
fn unwinding_returns_width_cache_without_retaining_the_text_system() {
    let platform = Arc::new(CountingTextSystem::default());
    let text_system = Arc::new(TextSystem::new(platform.clone()));
    let weak = Arc::downgrade(&text_system);
    let fragments = [LineFragment::text("hello world 你好")];
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut wrapper = text_system.line_wrapper(font("primary"), px(16.));
            wrapper
                .wrap_line(&fragments, px(40.), IndentAdjustment::default())
                .for_each(drop);
            panic!("abandoned text layout");
        }))
        .is_err()
    );
    let layouts = platform.layouts.load(Ordering::Relaxed);
    let mut wrapper = text_system.line_wrapper(font("primary"), px(16.));
    wrapper
        .wrap_line(&fragments, px(40.), IndentAdjustment::default())
        .for_each(drop);
    assert_eq!(platform.layouts.load(Ordering::Relaxed), layouts);
    drop(wrapper);
    drop(text_system);
    assert!(weak.upgrade().is_none());
}

struct WrappedTextRoot;

impl Render for WrappedTextRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        use crate::ParentElement as _;
        div().w(px(64.)).truncate().child("hello world 你好 ééé")
    }
}

#[test]
fn headless_app_reconstruction_releases_the_text_system_used_by_rendered_controls() {
    for _ in 0..3 {
        let platform = Arc::new(CountingTextSystem::default());
        let mut app = HeadlessAppContext::new(platform.clone());
        let system = app.update(|cx| cx.text_system().clone());
        let weak_system = Arc::downgrade(&system);
        let weak_platform = Arc::downgrade(&platform);
        app.open_window(size(px(160.), px(96.)), |_, cx: &mut App| {
            cx.new(|_| WrappedTextRoot)
        })
        .expect("wrapped text should render through the public headless context");
        assert!(platform.layouts.load(Ordering::Relaxed) > 0);
        assert!(
            system
                .wrapper_pool
                .lock()
                .values()
                .any(|caches| !caches.is_empty())
        );
        drop(system);
        drop(app);
        drop(platform);
        assert!(weak_system.upgrade().is_none());
        assert!(weak_platform.upgrade().is_none());
    }
}
