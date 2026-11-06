use super::{FontRun, LineLayout};
use crate::{
    Bounds, DevicePixels, Font, FontId, FontMetrics, GlyphId, Pixels, PlatformTextSystem,
    RenderGlyphParams, Size, TextRenderingMode, TextRun, TextSystem, WindowTextSystem, WrappedLine,
    font, px, text_system::font_installation_tests::DynamicPlatform,
};
use anyhow::Result;
use std::{
    borrow::Cow,
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

thread_local! {
    static ON_LAYOUT: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::default();
}

struct OnLayout;

impl OnLayout {
    fn new(callback: impl FnOnce() + 'static) -> Self {
        ON_LAYOUT.with(|slot| {
            assert!(slot.borrow_mut().replace(Box::new(callback)).is_none());
        });
        Self
    }
}

impl Drop for OnLayout {
    fn drop(&mut self) {
        ON_LAYOUT.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}

#[derive(Default)]
struct ReentrantPlatform {
    inner: DynamicPlatform,
    layouts: AtomicUsize,
}

impl PlatformTextSystem for ReentrantPlatform {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        self.inner.add_fonts(fonts)
    }

    fn all_font_names(&self) -> Vec<String> {
        self.inner.all_font_names()
    }

    fn font_id(&self, descriptor: &Font) -> Result<FontId> {
        self.inner.font_id(descriptor)
    }

    fn font_metrics(&self, id: FontId) -> FontMetrics {
        self.inner.font_metrics(id)
    }

    fn typographic_bounds(&self, id: FontId, glyph: GlyphId) -> Result<Bounds<f32>> {
        self.inner.typographic_bounds(id, glyph)
    }

    fn advance(&self, id: FontId, glyph: GlyphId) -> Result<Size<f32>> {
        self.inner.advance(id, glyph)
    }

    fn glyph_for_char(&self, id: FontId, character: char) -> Option<GlyphId> {
        self.inner.glyph_for_char(id, character)
    }

    fn glyph_raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        self.inner.glyph_raster_bounds(params)
    }

    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        self.inner.rasterize_glyph(params, bounds)
    }

    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout {
        self.layouts.fetch_add(1, Ordering::Relaxed);
        let mut layout = self.inner.layout_line(text, font_size, runs);
        if let Some(font_run) = runs.first() {
            for run in &mut layout.runs {
                run.font_id = font_run.font_id;
            }
        }
        let callback = ON_LAYOUT.with(|slot| slot.borrow_mut().take());
        if let Some(callback) = callback {
            callback();
        }
        layout
    }

    fn recommended_rendering_mode(&self, id: FontId, font_size: Pixels) -> TextRenderingMode {
        self.inner.recommended_rendering_mode(id, font_size)
    }
}

fn fixture() -> (
    Arc<ReentrantPlatform>,
    Arc<TextSystem>,
    Rc<WindowTextSystem>,
) {
    let platform = Arc::new(ReentrantPlatform::default());
    let text = Arc::new(TextSystem::new(platform.clone()));
    let window = Rc::new(WindowTextSystem::new(text.clone()));
    (platform, text, window)
}

fn run(source: &str) -> TextRun {
    TextRun {
        len: source.len(),
        font: font("primary"),
        ..Default::default()
    }
}

fn shape_wrapped(window: &WindowTextSystem, source: &str) -> WrappedLine {
    let mut lines = window
        .shape_text(
            source.to_owned().into(),
            px(16.),
            &[run(source)],
            Some(px(40.)),
            None,
        )
        .unwrap();
    assert_eq!(lines.len(), 1);
    lines.pop().unwrap()
}

#[test]
fn backend_reentry_can_shape_a_different_line_in_the_same_window() {
    let (platform, _, window) = fixture();
    let nested = Rc::new(RefCell::new(None));
    let callback_window = window.clone();
    let callback_result = nested.clone();
    let _callback = OnLayout::new(move || {
        callback_result.replace(Some(shape_wrapped(&callback_window, "inner 中 text")));
    });

    let source = "outer é🙂";
    let outer = window.shape_line(source.into(), px(16.), &[run(source)], None);
    let nested = nested.borrow();
    let inner = nested.as_ref().unwrap();
    assert_eq!(outer.text.as_ref(), source);
    assert_eq!(outer.layout.len, source.len());
    assert_eq!(inner.text.as_ref(), "inner 中 text");
    assert_eq!(inner.layout.len(), inner.text.len());
    assert!(!inner.layout.wrap_boundaries.is_empty());
    assert!(Arc::ptr_eq(
        &outer.layout,
        &window.layout_line(source, px(16.), &[run(source)], None)
    ));
    assert!(Arc::ptr_eq(
        &inner.layout,
        &shape_wrapped(&window, "inner 中 text").layout
    ));
    assert_eq!(platform.layouts.load(Ordering::Relaxed), 2);
    let index = window.layout_index();
    assert_eq!(index.lines_index, 2);
    assert_eq!(index.wrapped_lines_index, 1);
}

#[test]
fn same_key_plain_reentry_preserves_the_inner_canonical_layout() {
    for finish_inner_frame in [false, true] {
        let (platform, _, window) = fixture();
        let source = "same é🙂";
        let nested = Rc::new(RefCell::new(None));
        let callback_window = window.clone();
        let callback_result = nested.clone();
        let _callback = OnLayout::new(move || {
            let inner = callback_window.shape_line(source.into(), px(16.), &[run(source)], None);
            callback_result.replace(Some(inner));
            if finish_inner_frame {
                callback_window.finish_frame();
            }
        });

        let outer = window.shape_line(source.into(), px(16.), &[run(source)], None);
        let nested = nested.borrow();
        let inner = nested.as_ref().unwrap();
        assert_eq!(outer.text, inner.text);
        assert_eq!(outer.layout.len, source.len());
        assert!(Arc::ptr_eq(&outer.layout, &inner.layout));
        assert!(Arc::ptr_eq(
            &outer.layout,
            &window.layout_line(source, px(16.), &[run(source)], None)
        ));
        assert_eq!(window.layout_index().lines_index, 1);
        window.finish_frame();
        assert!(Arc::ptr_eq(
            &inner.layout,
            &window.layout_line(source, px(16.), &[run(source)], None)
        ));
        assert_eq!(platform.layouts.load(Ordering::Relaxed), 2);
    }
}

#[test]
fn same_key_wrapped_reentry_preserves_the_inner_canonical_layout() {
    for finish_inner_frame in [false, true] {
        let (platform, _, window) = fixture();
        let source = "same é🙂 text";
        let nested = Rc::new(RefCell::new(None));
        let callback_window = window.clone();
        let callback_result = nested.clone();
        let _callback = OnLayout::new(move || {
            callback_result.replace(Some(shape_wrapped(&callback_window, source)));
            if finish_inner_frame {
                callback_window.finish_frame();
            }
        });

        let outer = shape_wrapped(&window, source);
        let nested = nested.borrow();
        let inner = nested.as_ref().unwrap();
        assert_eq!(outer.text, inner.text);
        assert_eq!(outer.layout.len(), source.len());
        assert!(!outer.layout.wrap_boundaries.is_empty());
        assert!(Arc::ptr_eq(&outer.layout, &inner.layout));
        assert!(Arc::ptr_eq(
            &outer.layout.unwrapped_layout,
            &inner.layout.unwrapped_layout
        ));
        assert!(Arc::ptr_eq(
            &inner.layout,
            &shape_wrapped(&window, source).layout
        ));
        let index = window.layout_index();
        assert_eq!(index.lines_index, 1);
        assert_eq!(index.wrapped_lines_index, 1);
        assert_eq!(platform.layouts.load(Ordering::Relaxed), 2);
    }
}

#[test]
fn backend_installation_retains_in_flight_font_ids_without_caching_stale_runs() {
    let (platform, text, window) = fixture();
    let source = "late é🙂";
    let runs = [TextRun {
        font: font("late"),
        ..run(source)
    }];
    let nested = Rc::new(RefCell::new(None));
    let callback_window = window.clone();
    let callback_result = nested.clone();
    let callback_text = text.clone();
    let callback_runs = runs.clone();
    let _callback = OnLayout::new(move || {
        callback_text
            .add_fonts(vec![Cow::Borrowed(b"font")])
            .unwrap();
        let current = callback_window.shape_line(source.into(), px(16.), &callback_runs, None);
        callback_result.replace(Some(current));
    });

    let old = window.shape_line(source.into(), px(16.), &runs, None);
    let nested = nested.borrow();
    let current = nested.as_ref().unwrap();
    assert_eq!(text.font_generation(), 1);
    assert_eq!(old.text, current.text);
    assert_eq!(old.layout.len, source.len());
    assert_eq!(old.layout.runs[0].font_id, FontId(1));
    assert_eq!(current.layout.runs[0].font_id, FontId(2));
    assert_eq!(text.resolve_font(&font("late")), FontId(2));
    assert_eq!(current.width, old.width * 2.);
    assert!(!Arc::ptr_eq(&old.layout, &current.layout));
    assert!(Arc::ptr_eq(
        &current.layout,
        &window.layout_line(source, px(16.), &runs, None)
    ));
    assert_eq!(platform.layouts.load(Ordering::Relaxed), 2);
    let state = window.line_layout_cache.state();
    assert_eq!(state.font_generation, 1);
    assert_eq!(state.current_frame.lines.len(), 1);
    assert_eq!(state.current_frame.used_lines.len(), 1);
    assert!(
        state
            .current_frame
            .lines
            .keys()
            .all(|key| key.runs[0].font_id == FontId(2))
    );
    assert!(state.previous_frame.lines.is_empty());
}

#[test]
fn backend_installation_cannot_replace_reentrant_current_generation_wrapping() {
    let (platform, text, window) = fixture();
    let source = "same é🙂 text";
    let nested = Rc::new(RefCell::new(None));
    let callback_window = window.clone();
    let callback_result = nested.clone();
    let _callback = OnLayout::new(move || {
        text.add_fonts(vec![Cow::Borrowed(b"font")]).unwrap();
        callback_result.replace(Some(shape_wrapped(&callback_window, source)));
    });

    let old = shape_wrapped(&window, source);
    let nested = nested.borrow();
    let current = nested.as_ref().unwrap();
    assert_eq!(old.text, current.text);
    assert_eq!(old.layout.len(), source.len());
    assert_eq!(old.layout.runs()[0].font_id, FontId(1));
    assert_eq!(current.layout.runs()[0].font_id, FontId(1));
    assert_eq!(
        current.layout.unwrapped_layout.width,
        old.layout.unwrapped_layout.width * 2.
    );
    assert_ne!(old.layout.wrap_boundaries, current.layout.wrap_boundaries);
    assert!(!Arc::ptr_eq(&old.layout, &current.layout));
    assert!(!Arc::ptr_eq(
        &old.layout.unwrapped_layout,
        &current.layout.unwrapped_layout
    ));
    assert!(Arc::ptr_eq(
        &current.layout,
        &shape_wrapped(&window, source).layout
    ));
    assert_eq!(platform.layouts.load(Ordering::Relaxed), 2);
    let state = window.line_layout_cache.state();
    assert_eq!(state.font_generation, 1);
    assert_eq!(state.current_frame.used_lines.len(), 1);
    assert_eq!(state.current_frame.used_wrapped_lines.len(), 1);
    assert!(state.previous_frame.lines.is_empty());
    assert!(state.previous_frame.wrapped_lines.is_empty());
}

#[test]
fn backend_installation_without_reentry_refuses_both_publications_and_clears_frames() {
    let (platform, text, window) = fixture();
    shape_wrapped(&window, "committed");
    window.finish_frame();
    shape_wrapped(&window, "candidate");
    let _callback = OnLayout::new(move || {
        text.add_fonts(vec![Cow::Borrowed(b"font")]).unwrap();
    });

    let source = "in flight é🙂";
    let old = shape_wrapped(&window, source);
    assert_eq!(old.text.as_ref(), source);
    assert_eq!(old.layout.len(), source.len());
    {
        let state = window.line_layout_cache.state();
        assert_eq!(state.font_generation, 1);
        assert!(state.current_frame.lines.is_empty());
        assert!(state.current_frame.wrapped_lines.is_empty());
        assert!(state.current_frame.used_lines.is_empty());
        assert!(state.current_frame.used_wrapped_lines.is_empty());
        assert!(state.previous_frame.lines.is_empty());
        assert!(state.previous_frame.wrapped_lines.is_empty());
    }
    let current = shape_wrapped(&window, source);
    assert_eq!(
        current.layout.unwrapped_layout.width,
        old.layout.unwrapped_layout.width * 2.
    );
    assert!(!Arc::ptr_eq(&old.layout, &current.layout));
    assert!(Arc::ptr_eq(
        &current.layout,
        &shape_wrapped(&window, source).layout
    ));
    assert_eq!(platform.layouts.load(Ordering::Relaxed), 4);
}

#[test]
fn backend_unwind_releases_window_scratch_and_cache_borrows_for_retry() {
    for wrapped in [false, true] {
        let (platform, _, window) = fixture();
        let committed = shape_wrapped(&window, "committed 中");
        window.finish_frame();
        let candidate = shape_wrapped(&window, "committed 中");
        assert!(Arc::ptr_eq(&committed.layout, &candidate.layout));

        let nested = Rc::new(RefCell::new(None));
        let callback_window = window.clone();
        let callback_result = nested.clone();
        let _callback = OnLayout::new(move || {
            callback_result.replace(Some(shape_wrapped(&callback_window, "nested short")));
            panic!("backend layout unwind");
        });
        let source = "retry after unwind é🙂";
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if wrapped {
                shape_wrapped(&window, source);
            } else {
                window.shape_line(source.into(), px(16.), &[run(source)], None);
            }
        }));
        let panic = failed.expect_err("backend layout should unwind");
        assert_eq!(
            panic.downcast_ref::<&str>().copied(),
            Some("backend layout unwind")
        );
        {
            let state = window.line_layout_cache.state();
            assert!(
                state
                    .current_frame
                    .lines
                    .keys()
                    .all(|key| key.text.as_ref() != source)
            );
            assert!(
                state
                    .current_frame
                    .wrapped_lines
                    .keys()
                    .all(|key| key.text.as_ref() != source)
            );
        }
        window.abort_frame();
        assert!(Arc::ptr_eq(
            &committed.layout,
            &shape_wrapped(&window, "committed 中").layout
        ));
        assert!(Arc::ptr_eq(
            &nested.borrow().as_ref().unwrap().layout,
            &shape_wrapped(&window, "nested short").layout
        ));
        let retried = window.shape_line(source.into(), px(16.), &[run(source)], None);
        let retried_wrapped = shape_wrapped(&window, source);
        assert_eq!(retried.text.as_ref(), source);
        assert_eq!(retried_wrapped.text.as_ref(), source);
        assert_eq!(retried.layout.len, source.len());
        assert!(Arc::ptr_eq(
            &retried.layout,
            &retried_wrapped.layout.unwrapped_layout
        ));
        assert_eq!(platform.layouts.load(Ordering::Relaxed), 4);
        let state = window.line_layout_cache.state();
        let key = state
            .current_frame
            .lines
            .keys()
            .find(|key| key.text.as_ref() == source)
            .unwrap();
        assert_eq!(
            key.runs.iter().map(|run| run.len).sum::<usize>(),
            source.len()
        );
        assert_eq!(key.runs[0].font_id, FontId(1));
    }
}

#[test]
fn abort_restores_moved_entries_and_preserves_committed_ranges_on_retry() {
    let (platform, _, window) = fixture();
    let cache = &window.line_layout_cache;
    let start = cache.layout_index();
    let plain = window.shape_line("plain é🙂".into(), px(16.), &[run("plain é🙂")], None);
    let wrapped = shape_wrapped(&window, "wrapped 中 text");
    let end = cache.layout_index();
    window.finish_frame();
    cache.reuse_layouts(start.clone()..end.clone());
    let candidate = window.shape_line("candidate".into(), px(16.), &[run("candidate")], None);
    let candidate_wrapped = shape_wrapped(&window, "candidate wrapped");
    cache.truncate_layouts(end.clone());
    window.abort_frame();
    {
        let state = cache.state();
        assert!(state.current_frame.lines.is_empty());
        assert!(state.current_frame.wrapped_lines.is_empty());
        assert!(state.current_frame.used_lines.is_empty());
        assert!(state.current_frame.used_wrapped_lines.is_empty());
        assert_eq!(state.previous_frame.used_lines.len(), end.lines_index);
        assert_eq!(
            state.previous_frame.used_wrapped_lines.len(),
            end.wrapped_lines_index
        );
        assert_eq!(state.previous_frame.lines.len(), 4);
        assert_eq!(state.previous_frame.wrapped_lines.len(), 2);
    }
    cache.reuse_layouts(start..end);
    assert!(Arc::ptr_eq(
        &plain.layout,
        &window.layout_line("plain é🙂", px(16.), &[run("plain é🙂")], None)
    ));
    assert!(Arc::ptr_eq(
        &wrapped.layout,
        &shape_wrapped(&window, "wrapped 中 text").layout
    ));
    assert!(Arc::ptr_eq(
        &candidate.layout,
        &window.layout_line("candidate", px(16.), &[run("candidate")], None)
    ));
    assert!(Arc::ptr_eq(
        &candidate_wrapped.layout,
        &shape_wrapped(&window, "candidate wrapped").layout
    ));
    assert_eq!(platform.layouts.load(Ordering::Relaxed), 4);
}

#[test]
fn wrapped_cache_distinguishes_line_clamp_in_both_orders_and_after_frame_reuse() {
    for limits in [
        [None, Some(1), Some(2), None, Some(1)],
        [Some(1), Some(2), None, Some(1), None],
    ] {
        let (_, text, window) = fixture();
        let source = "abcdefgh";
        let runs = [run(source)];
        for (index, limit) in limits.into_iter().enumerate() {
            let fresh = WindowTextSystem::new(text.clone());
            let expected = fresh
                .shape_text(source.into(), px(16.), &runs, Some(px(20.)), limit)
                .unwrap();
            let actual = window
                .shape_text(source.into(), px(16.), &runs, Some(px(20.)), limit)
                .unwrap();
            assert_eq!(actual[0].text.as_ref(), source);
            assert_eq!(
                actual[0].wrap_boundaries, expected[0].wrap_boundaries,
                "cached wrapping must match a fresh owner for clamp {limit:?}"
            );
            if limit == Some(1) {
                assert!(actual[0].wrap_boundaries.is_empty());
            } else {
                assert!(!actual[0].wrap_boundaries.is_empty());
            }
            let repeated = window
                .shape_text(source.into(), px(16.), &runs, Some(px(20.)), limit)
                .unwrap();
            assert!(Arc::ptr_eq(&actual[0].layout, &repeated[0].layout));
            if index % 2 == 0 {
                window.finish_frame();
            } else {
                window.abort_frame();
            }
        }
    }
}

#[test]
fn content_keys_preserve_source_font_size_wrap_and_forced_width_identity() {
    let (platform, _, window) = fixture();
    let cache = &window.line_layout_cache;
    let source = "é🙂ab";
    let runs = [FontRun {
        len: source.len(),
        font_id: FontId(1),
    }];
    let initial = cache.layout_line(source, px(16.), &runs, None, 0);
    assert!(Arc::ptr_eq(
        &initial,
        &cache.layout_line(source.to_owned(), px(16.), &runs, None, 0)
    ));
    let different_text = cache.layout_line("é🙂cd", px(16.), &runs, None, 0);
    let different_font = cache.layout_line(
        source,
        px(16.),
        &[FontRun {
            font_id: FontId(2),
            ..runs[0]
        }],
        None,
        0,
    );
    let different_size = cache.layout_line(source, px(18.), &runs, None, 0);
    let forced = cache.layout_line(source, px(16.), &runs, Some(px(4.)), 0);
    for layout in [&different_text, &different_font, &different_size, &forced] {
        assert!(!Arc::ptr_eq(&initial, layout));
        assert_eq!(layout.len, source.len());
        assert!(
            layout
                .runs
                .iter()
                .flat_map(|run| &run.glyphs)
                .all(|glyph| source.is_char_boundary(glyph.index))
        );
    }
    assert_eq!(initial.runs[0].font_id, FontId(1));
    assert_eq!(different_font.runs[0].font_id, FontId(2));
    assert!(different_size.width > initial.width);
    assert_eq!(forced.runs[0].glyphs[1].position.x, px(4.));
    assert_ne!(initial.runs[0].glyphs[1].position.x, px(4.));
    assert!(Arc::ptr_eq(
        &forced,
        &cache.layout_line(source, px(16.), &runs, Some(px(4.)), 0)
    ));

    let narrow = cache.layout_wrapped_line(source, px(16.), &runs, Some(px(20.)), None, 0);
    let wide = cache.layout_wrapped_line(source, px(16.), &runs, Some(px(80.)), None, 0);
    let unwrapped = cache.layout_wrapped_line(source, px(16.), &runs, None, None, 0);
    assert!(!Arc::ptr_eq(&narrow, &wide));
    assert!(!Arc::ptr_eq(&wide, &unwrapped));
    assert!(!narrow.wrap_boundaries.is_empty());
    assert!(wide.wrap_boundaries.is_empty());
    assert!(unwrapped.wrap_boundaries.is_empty());
    for layout in [&narrow, &wide, &unwrapped] {
        assert!(Arc::ptr_eq(&initial, &layout.unwrapped_layout));
    }
    assert!(Arc::ptr_eq(
        &narrow,
        &cache.layout_wrapped_line(source.to_owned(), px(16.), &runs, Some(px(20.)), None, 0)
    ));
    assert_eq!(platform.layouts.load(Ordering::Relaxed), 5);
}
