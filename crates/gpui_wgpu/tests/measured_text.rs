//! Public measured-text consumer for Zed c7ad65e46817b8bdc71c79ba3f15c898c0bc5678,
//! reconciled against fixed target 635026fabc38440fce5e1767d125026a17b12012.
//!
//! The test drives measurement, prepaint and paint through HeadlessAppContext.
//! Cosmic and Swash use the tracked font; TestPlatform accepts framework frames.
//! This harness does not exercise GPU rendering or native presentation.

#![cfg(not(target_family = "wasm"))]

use anyhow::Result;
use open_gpui::{
    App, AppContext as _, Bounds, Context, DevicePixels, Element, ElementId, Font, FontId,
    FontMetrics, FontRun, GlobalElementId, GlyphId, HeadlessAppContext, InspectorElementId,
    IntoElement, LayoutId, LineLayout, ParentElement as _, Pixels, PlatformTextSystem, Point,
    QuitMode, Render, RenderGlyphParams, SharedString, Size, Styled as _, StyledText, TextLayout,
    TextRenderingMode, Window, WindowHandle, WrappedLineLayout, div, point, px, size,
};
use open_gpui_wgpu::CosmicTextSystem;
use smallvec::SmallVec;
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

const IBM_PLEX: &[u8] = include_bytes!("../../../tests/fixtures/fonts/IBMPlexSans-Regular.ttf");
const SOURCE: &str = "é alpha beta gamma delta epsilon\n\nlast café e\u{301} longer line\n";
const UPDATED_SOURCE: &str = "updated café e\u{301} alpha beta gamma\n\nfinal exact source z\n";
const FONT_SIZE: f32 = 20.;
const LINE_HEIGHT: f32 = 28.;

struct ObservedTextSystem {
    cosmic: CosmicTextSystem,
    rasters: AtomicUsize,
    layouts: AtomicUsize,
}

impl PlatformTextSystem for ObservedTextSystem {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        self.cosmic.add_fonts(fonts)
    }

    fn all_font_names(&self) -> Vec<String> {
        self.cosmic.all_font_names()
    }

    fn font_id(&self, descriptor: &Font) -> Result<FontId> {
        self.cosmic.font_id(descriptor)
    }

    fn prewarm_fonts(&self, font_ids: &[FontId]) {
        self.cosmic.prewarm_fonts(font_ids);
    }

    fn font_metrics(&self, font_id: FontId) -> FontMetrics {
        self.cosmic.font_metrics(font_id)
    }

    fn typographic_bounds(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Bounds<f32>> {
        self.cosmic.typographic_bounds(font_id, glyph_id)
    }

    fn advance(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>> {
        self.cosmic.advance(font_id, glyph_id)
    }

    fn glyph_for_char(&self, font_id: FontId, character: char) -> Option<GlyphId> {
        self.cosmic.glyph_for_char(font_id, character)
    }

    fn glyph_raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        self.cosmic.glyph_raster_bounds(params)
    }

    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        let raster = self.cosmic.rasterize_glyph(params, bounds)?;
        assert_eq!(raster.0, bounds.size);
        assert!(bounds.size.width.0 > 0 && bounds.size.height.0 > 0);
        assert!(raster.1.iter().any(|byte| *byte != 0));
        self.rasters.fetch_add(1, Ordering::Relaxed);
        Ok(raster)
    }

    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout {
        self.layouts.fetch_add(1, Ordering::Relaxed);
        self.cosmic.layout_line(text, font_size, runs)
    }

    fn recommended_rendering_mode(&self, font_id: FontId, font_size: Pixels) -> TextRenderingMode {
        self.cosmic.recommended_rendering_mode(font_id, font_size)
    }
}

#[derive(Clone)]
struct PaintedMeasurement {
    version: usize,
    source: SharedString,
    width: Pixels,
    bounds: Bounds<Pixels>,
    clip: Bounds<Pixels>,
    lines: SmallVec<[Arc<WrappedLineLayout>; 1]>,
    wrapped: String,
    previous_revision: u64,
}

impl PaintedMeasurement {
    fn rows(&self) -> usize {
        self.lines
            .iter()
            .map(|line| line.wrap_boundaries().len() + 1)
            .sum()
    }
}

fn assert_measured_queries(
    layout: &TextLayout,
    source: &SharedString,
    width: Pixels,
    window: &Window,
) -> SmallVec<[Arc<WrappedLineLayout>; 1]> {
    let lines: SmallVec<[Arc<WrappedLineLayout>; 1]> = layout.line_layouts();
    assert_eq!(layout.text(), source.as_ref());
    assert_eq!(layout.len(), source.len());
    assert_eq!(layout.line_height(), px(LINE_HEIGHT));
    assert_eq!(lines.len(), source.split('\n').count());
    let repeated = layout.line_layouts();
    let mut start = 0;
    for ((line, repeated), text) in lines.iter().zip(&repeated).zip(source.split('\n')) {
        assert!(Arc::ptr_eq(line, repeated));
        assert_eq!(line.len(), text.len());
        assert_eq!(line.wrap_width, Some(width));
        let reference = window
            .text_system()
            .shape_text(
                text.to_owned().into(),
                px(FONT_SIZE),
                &[window.text_style().to_run(text.len())],
                Some(width),
                None,
            )
            .expect("each original logical line must shape in the same window namespace");
        assert_eq!(reference.len(), 1);
        assert_eq!(reference[0].text.as_ref(), text);
        assert!(Arc::ptr_eq(line, &reference[0]));
        for index in start..=start + text.len() {
            assert!(Arc::ptr_eq(
                line,
                &layout.line_layout_for_index(index).unwrap(),
            ));
        }
        for run in line.runs() {
            for glyph in &run.glyphs {
                assert_ne!(glyph.id, GlyphId(0));
                assert!(glyph.index < text.len());
                assert!(text.is_char_boundary(glyph.index));
            }
        }
        for boundary in line.wrap_boundaries() {
            let glyph = &line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix];
            assert!(text.is_char_boundary(glyph.index));
            assert!(Arc::ptr_eq(
                line,
                &layout.line_layout_for_index(start + glyph.index).unwrap(),
            ));
        }
        start += text.len() + 1;
    }
    assert_eq!(start - 1, source.len());
    assert!(layout.line_layout_for_index(source.len() + 1).is_none());
    assert!(layout.line_layout_for_index(usize::MAX).is_none());
    assert_eq!(
        layout.wrapped_text().replace('\n', ""),
        source.replace('\n', ""),
    );
    lines
}

struct MeasuredText {
    label: StyledText,
    source: SharedString,
    width: Pixels,
    version: usize,
    paints: Rc<RefCell<Vec<PaintedMeasurement>>>,
}

impl Element for MeasuredText {
    type RequestLayoutState = ();
    type PrepaintState = SmallVec<[Arc<WrappedLineLayout>; 1]>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.label.request_layout(id, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let layout = self.label.layout().clone();
        let measured = assert_measured_queries(&layout, &self.source, self.width, window);
        for paint in [TextLayout::paint_background, TextLayout::paint_foreground] {
            assert_eq!(
                paint(&layout, window, cx).unwrap_err().to_string(),
                "prepaint has not been performed",
            );
        }
        self.label
            .prepaint(id, inspector_id, bounds, state, window, cx);
        assert_eq!(layout.bounds(), bounds);
        measured
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        measured: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let layout = self.label.layout().clone();
        let painted = assert_measured_queries(&layout, &self.source, self.width, window);
        for (before, after) in measured.iter().zip(&painted) {
            assert!(Arc::ptr_eq(before, after));
        }
        assert_eq!(layout.bounds(), bounds);
        assert!(bounds.origin.x > px(0.) && bounds.origin.y > px(0.));
        assert_eq!(
            layout.index_for_position(bounds.origin - point(px(0.), px(1.))),
            Err(0),
        );
        let mut start = 0;
        let mut origin = bounds.origin;
        for (line, text) in measured.iter().zip(self.source.split('\n')) {
            for index in text
                .char_indices()
                .map(|(index, _)| index)
                .chain([text.len()])
            {
                let local = line
                    .position_for_index(index, layout.line_height())
                    .unwrap();
                assert_eq!(
                    layout.position_for_index(start + index),
                    Some(origin + local),
                );
            }
            start += text.len() + 1;
            origin.y += line.size(layout.line_height()).height;
        }
        assert_eq!(origin.y, bounds.bottom());
        self.label
            .paint(id, inspector_id, bounds, state, &mut (), window, cx);
        self.paints.borrow_mut().push(PaintedMeasurement {
            version: self.version,
            source: self.source.clone(),
            width: self.width,
            bounds,
            clip: window.clip_bounds(),
            lines: painted,
            wrapped: layout.wrapped_text(),
            previous_revision: window.rendered_frame_revision(),
        });
    }
}

struct MeasuredTextView {
    paints: Rc<RefCell<Vec<PaintedMeasurement>>>,
    source: SharedString,
    width: Pixels,
    version: usize,
}

impl Render for MeasuredTextView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p(px(16.)).child(
            div()
                .w(self.width)
                .overflow_hidden()
                .font_family("IBM Plex Sans")
                .text_size(px(FONT_SIZE))
                .line_height(px(LINE_HEIGHT))
                .child(MeasuredText {
                    label: StyledText::new(self.source.clone()),
                    source: self.source.clone(),
                    width: self.width,
                    version: self.version,
                    paints: self.paints.clone(),
                }),
        )
    }
}

fn open_consumer(
    app: &mut HeadlessAppContext,
    paints: &Rc<RefCell<Vec<PaintedMeasurement>>>,
    source: &str,
    width: Pixels,
    version: usize,
) -> Result<WindowHandle<MeasuredTextView>> {
    let paints = paints.clone();
    let source = SharedString::new(source);
    app.open_window(size(px(640.), px(1024.)), move |_, cx| {
        cx.new(|_| MeasuredTextView {
            paints,
            source,
            width,
            version,
        })
    })
}

fn observe_frame(
    app: &mut HeadlessAppContext,
    handle: WindowHandle<MeasuredTextView>,
    paints: &Rc<RefCell<Vec<PaintedMeasurement>>>,
    before: usize,
) -> Result<PaintedMeasurement> {
    let revision = app.update_window(handle.into(), |_, window, _| {
        let revision = window.rendered_frame_revision();
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            Some(revision),
        );
        revision
    })?;
    let paints = paints.borrow();
    assert!(paints.len() > before);
    let painted = paints
        .last()
        .expect("an accepted frame must consume measured text")
        .clone();
    assert_eq!(painted.previous_revision.checked_add(1), Some(revision));
    assert_eq!(painted.wrapped.split('\n').count(), painted.rows());
    assert_eq!(painted.bounds.size.height, px(LINE_HEIGHT) * painted.rows());
    Ok(painted)
}

fn draw_frame(
    app: &mut HeadlessAppContext,
    handle: WindowHandle<MeasuredTextView>,
    paints: &Rc<RefCell<Vec<PaintedMeasurement>>>,
) -> Result<PaintedMeasurement> {
    let before = paints.borrow().len();
    app.update_window(handle.into(), |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })?;
    observe_frame(app, handle, paints, before)
}

fn glyphs(line: &LineLayout) -> Vec<(FontId, GlyphId, Point<Pixels>, usize, bool)> {
    line.runs
        .iter()
        .flat_map(|run| {
            run.glyphs.iter().map(move |glyph| {
                (
                    run.font_id,
                    glyph.id,
                    glyph.position,
                    glyph.index,
                    glyph.is_emoji,
                )
            })
        })
        .collect()
}

fn assert_same_unwrapped(before: &LineLayout, after: &LineLayout) {
    assert_eq!(
        (
            before.font_size,
            before.width,
            before.ascent,
            before.descent,
            before.len
        ),
        (
            after.font_size,
            after.width,
            after.ascent,
            after.descent,
            after.len
        ),
    );
    assert_eq!(
        before
            .runs
            .iter()
            .map(|run| (run.font_id, run.glyphs.len()))
            .collect::<Vec<_>>(),
        after
            .runs
            .iter()
            .map(|run| (run.font_id, run.glyphs.len()))
            .collect::<Vec<_>>(),
    );
    assert_eq!(glyphs(before), glyphs(after));
}

fn assert_same_measurement(before: &PaintedMeasurement, after: &PaintedMeasurement, reused: bool) {
    assert_eq!(before.version, after.version);
    assert_eq!(before.source, after.source);
    assert_eq!(before.width, after.width);
    assert_eq!(before.bounds, after.bounds);
    assert_eq!(before.clip, after.clip);
    assert_eq!(before.wrapped, after.wrapped);
    assert_eq!(before.lines.len(), after.lines.len());
    for (before, after) in before.lines.iter().zip(&after.lines) {
        assert_eq!(Arc::ptr_eq(before, after), reused);
        assert_eq!(before.len(), after.len());
        assert_eq!(before.wrap_width, after.wrap_width);
        assert_eq!(before.wrap_boundaries, after.wrap_boundaries);
        assert_same_unwrapped(&before.unwrapped_layout, &after.unwrapped_layout);
    }
}

#[test]
fn public_measured_text_survives_notify_reflow_and_reconstruction() -> Result<()> {
    let backend = Arc::new(ObservedTextSystem {
        cosmic: CosmicTextSystem::new_without_system_fonts("IBM Plex Sans"),
        rasters: AtomicUsize::new(0),
        layouts: AtomicUsize::new(0),
    });
    let backend_lifetime = Arc::downgrade(&backend);
    let mut app = HeadlessAppContext::new(backend.clone());
    app.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        cx.set_text_rendering_mode(TextRenderingMode::Grayscale);
    });
    app.text_system().add_fonts(vec![Cow::Borrowed(IBM_PLEX)])?;
    let text_lifetime = Arc::downgrade(app.text_system());
    let paints = Rc::new(RefCell::new(Vec::new()));
    let handle = open_consumer(&mut app, &paints, SOURCE, px(240.), 0)?;
    let initial = draw_frame(&mut app, handle, &paints)?;
    assert_eq!(initial.source.as_ref(), SOURCE);
    assert!(initial.rows() > initial.lines.len());
    assert_eq!(initial.lines[1].len(), 0);
    assert_eq!(initial.lines.last().unwrap().len(), 0);
    assert!(backend.rasters.load(Ordering::Relaxed) > 0);
    let unchanged_layouts = backend.layouts.load(Ordering::Relaxed);
    assert_same_measurement(&initial, &draw_frame(&mut app, handle, &paints)?, true);
    assert_eq!(backend.layouts.load(Ordering::Relaxed), unchanged_layouts);

    let root = handle.root(&mut app)?;
    let notifications = Rc::new(Cell::new(0));
    let subscription = app.update(|cx| {
        let notifications = notifications.clone();
        cx.observe(&root, move |_, _| {
            notifications.set(notifications.get() + 1)
        })
    });
    let before = paints.borrow().len();
    let before_reflow_layouts = backend.layouts.load(Ordering::Relaxed);
    handle.update(&mut app, |view, _, cx| {
        view.width = px(90.);
        view.version = 1;
        cx.notify();
    })?;
    app.run_until_parked();
    assert_eq!(notifications.get(), 1);
    let narrow = observe_frame(&mut app, handle, &paints, before)?;
    assert_eq!(narrow.version, 1);
    assert_eq!(narrow.width, px(90.));
    assert_eq!(narrow.source, initial.source);
    assert!(narrow.rows() > initial.rows());
    eprintln!(
        "measured_text shaping calls: unchanged=0, reflow={}",
        backend.layouts.load(Ordering::Relaxed) - before_reflow_layouts,
    );
    for (before, after) in initial.lines.iter().zip(&narrow.lines) {
        assert!(!Arc::ptr_eq(before, after));
        assert_same_unwrapped(&before.unwrapped_layout, &after.unwrapped_layout);
    }

    let before = paints.borrow().len();
    handle.update(&mut app, |view, _, cx| {
        view.source = UPDATED_SOURCE.into();
        view.version = 2;
        cx.notify();
    })?;
    app.run_until_parked();
    assert_eq!(notifications.get(), 2);
    let updated = observe_frame(&mut app, handle, &paints, before)?;
    assert_eq!(updated.version, 2);
    assert_eq!(updated.source.as_ref(), UPDATED_SOURCE);
    assert_eq!(initial.source.as_ref(), SOURCE);
    assert_eq!(initial.lines[0].wrap_width, Some(px(240.)));
    assert_same_measurement(&updated, &draw_frame(&mut app, handle, &paints)?, true);

    drop(subscription);
    drop(root);
    app.update_window(handle.into(), |_, window, cx| window.remove_window(cx))?;
    app.run_until_parked();
    assert!(app.update(|cx| cx.windows().is_empty()));
    let rebuilt = open_consumer(&mut app, &paints, UPDATED_SOURCE, px(90.), 2)?;
    assert_ne!(rebuilt.window_id(), handle.window_id());
    let reconstructed = draw_frame(&mut app, rebuilt, &paints)?;
    assert_same_measurement(&updated, &reconstructed, false);
    app.update_window(rebuilt.into(), |_, window, cx| window.remove_window(cx))?;
    app.run_until_parked();
    assert!(app.update(|cx| cx.windows().is_empty()));
    drop(app);
    assert!(text_lifetime.upgrade().is_none());
    drop(backend);
    assert!(backend_lifetime.upgrade().is_none());
    assert_same_measurement(&updated, &reconstructed, false);
    Ok(())
}
