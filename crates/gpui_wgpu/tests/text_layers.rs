//! Public layered-text consumer for Zed bda9c0bd43, reconciled against
//! 635026fabc38440fce5e1767d125026a17b12012.
//!
//! Cosmic and Swash shape and rasterize the tracked fixture fonts. The test platform
//! accepts framework paint without exercising a GPU renderer or native presentation.

#![cfg(not(target_family = "wasm"))]

use anyhow::Result;
use open_gpui::{
    App, AppContext as _, Bounds, Context, DevicePixels, Element, ElementId, Font, FontFeatures,
    FontId, FontMetrics, FontRun, GlobalElementId, GlyphId, HeadlessAppContext, InspectorElementId,
    IntoElement, LayoutId, LineLayout, ParentElement as _, Pixels, PlatformTextSystem,
    PreparedSubtreeClip, QuitMode, Render, RenderGlyphParams, SharedString, Size,
    StrikethroughStyle, Styled as _, StyledText, SubtreeClip, TextAlign, TextLayout,
    TextRenderingMode, TextRun, UnderlineStyle, Window, WindowHandle, div, fill, font, point, px,
    rgb, size,
};
use open_gpui_wgpu::CosmicTextSystem;
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

const LILEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/fonts/Lilex-Regular.ttf"
));
const IBM_PLEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/fonts/IBMPlexSans-Regular.ttf"
));
const SOURCE: &str = "alpha beta gamma delta café\n\nlast longer line z\n";
const FINAL_SOURCE: &str = "updated café e\u{301} alpha beta gamma delta\n\nfinal exact text z\n";
const BACKGROUND: usize = 1;
const HIGHLIGHT: usize = 2;
const FOREGROUND: usize = 3;
const COMBINED: usize = 4;

struct ObservedTextSystem {
    cosmic: CosmicTextSystem,
    phase: AtomicUsize,
    fail_raster_bounds: AtomicBool,
    error_checked: AtomicBool,
    rasters: Mutex<Vec<RenderGlyphParams>>,
}

impl ObservedTextSystem {
    fn new() -> Self {
        Self {
            cosmic: CosmicTextSystem::new_without_system_fonts("Lilex"),
            phase: AtomicUsize::new(0),
            fail_raster_bounds: AtomicBool::new(false),
            error_checked: AtomicBool::new(false),
            rasters: Mutex::new(Vec::new()),
        }
    }

    fn assert_foreground(&self) {
        assert!(matches!(
            self.phase.load(Ordering::SeqCst),
            FOREGROUND | COMBINED
        ));
    }
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
        self.assert_foreground();
        anyhow::ensure!(
            !self.fail_raster_bounds.load(Ordering::SeqCst),
            "layered consumer raster bounds failure"
        );
        self.cosmic.glyph_raster_bounds(params)
    }

    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        self.assert_foreground();
        let raster = self.cosmic.rasterize_glyph(params, bounds)?;
        assert_eq!(raster.0, bounds.size);
        assert!(bounds.size.width.0 > 0 && bounds.size.height.0 > 0);
        assert!(raster.1.iter().any(|byte| *byte != 0));
        self.rasters.lock().unwrap().push(params.clone());
        Ok(raster)
    }

    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout {
        self.cosmic.layout_line(text, font_size, runs)
    }

    fn recommended_rendering_mode(&self, font_id: FontId, font_size: Pixels) -> TextRenderingMode {
        self.cosmic.recommended_rendering_mode(font_id, font_size)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Case {
    align: TextAlign,
    width_factor: f32,
    clipped: bool,
    overlapping: bool,
    empty: bool,
}

const CASES: [Case; 6] = [
    Case {
        align: TextAlign::Center,
        width_factor: 0.5,
        clipped: false,
        overlapping: false,
        empty: false,
    },
    Case {
        align: TextAlign::Center,
        width_factor: 1.5,
        clipped: false,
        overlapping: false,
        empty: false,
    },
    Case {
        align: TextAlign::Right,
        width_factor: 0.5,
        clipped: false,
        overlapping: true,
        empty: false,
    },
    Case {
        align: TextAlign::Right,
        width_factor: 1.5,
        clipped: false,
        overlapping: false,
        empty: false,
    },
    Case {
        align: TextAlign::Right,
        width_factor: 1.5,
        clipped: true,
        overlapping: false,
        empty: false,
    },
    Case {
        align: TextAlign::Center,
        width_factor: 1.5,
        clipped: false,
        overlapping: false,
        empty: true,
    },
];

#[derive(Clone, Debug, PartialEq)]
struct PaintedText {
    version: usize,
    case: Case,
    text: String,
    wrapped: String,
    bounds: Bounds<Pixels>,
    line_height: Pixels,
    rows: Vec<Bounds<Pixels>>,
    clip: Bounds<Pixels>,
    scale_factor: f32,
}

fn row_bounds(layout: &TextLayout, text: &str, align: TextAlign) -> Vec<Bounds<Pixels>> {
    let bounds = layout.bounds();
    let line_height = layout.line_height();
    let mut rows = Vec::new();
    let mut logical_offset = 0;
    let mut y = bounds.top();
    for paragraph in text.split('\n') {
        let line = layout
            .line_layout_for_index(logical_offset)
            .expect("each logical line, including empty lines, must retain its layout");
        assert_eq!(line.len(), paragraph.len());
        let starts = std::iter::once(px(0.))
            .chain(line.wrap_boundaries().iter().map(|boundary| {
                let glyph = &line.unwrapped_layout.runs[boundary.run_ix].glyphs[boundary.glyph_ix];
                assert!(paragraph.is_char_boundary(glyph.index));
                glyph.position.x
            }))
            .chain(std::iter::once(line.unwrapped_layout.width))
            .collect::<Vec<_>>();
        for ends in starts.windows(2) {
            let width = ends[1] - ends[0];
            let remaining = bounds.size.width - width;
            let x = bounds.left()
                + match align {
                    TextAlign::Left => px(0.),
                    TextAlign::Center => remaining / 2.,
                    TextAlign::Right => remaining,
                };
            rows.push(Bounds::new(point(x, y), size(width, line_height)));
            y += line_height;
        }
        logical_offset += paragraph.len() + 1;
    }
    assert_eq!(rows.len(), layout.wrapped_text().split('\n').count());
    assert_eq!(y - bounds.top(), bounds.size.height);
    rows
}

struct LayeredText {
    label: StyledText,
    source: SharedString,
    case: Case,
    align_width: Pixels,
    version: usize,
    combined: bool,
    error_probe: bool,
    backend: Arc<ObservedTextSystem>,
    paints: Rc<RefCell<Vec<PaintedText>>>,
}

impl Element for LayeredText {
    type RequestLayoutState = ();
    type PrepaintState = Option<PreparedSubtreeClip>;

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
        for paint in [TextLayout::paint_background, TextLayout::paint_foreground] {
            assert_eq!(
                paint(&layout, window, cx).unwrap_err().to_string(),
                "prepaint has not been performed"
            );
        }
        let text_bounds = Bounds::new(
            bounds.origin + point(px(64.), px(0.)),
            size(self.align_width, bounds.size.height),
        );
        self.label
            .prepaint(id, inspector_id, text_bounds, state, window, cx);
        self.case.clipped.then(|| {
            let clip = SubtreeClip::try_rect(Bounds::new(
                point(self.align_width * 0.35, layout.line_height() * 0.5),
                size(self.align_width * 0.5, layout.line_height() * 1.5),
            ))
            .expect("the consumer clip is finite and positive");
            window.prepare_subtree_clip(&clip, text_bounds)
        })
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        clip: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let layout = self.label.layout().clone();
        assert_eq!(layout.text(), self.source.as_ref());
        assert_eq!(layout.len(), self.source.len());
        let rows = row_bounds(&layout, &self.source, self.case.align);
        let mut paint = |window: &mut Window| {
            if self.combined {
                self.backend.phase.store(COMBINED, Ordering::SeqCst);
                self.label
                    .paint(id, inspector_id, bounds, state, &mut (), window, cx);
            } else {
                self.backend.phase.store(BACKGROUND, Ordering::SeqCst);
                let before = self.backend.rasters.lock().unwrap().len();
                layout
                    .paint_background(window, cx)
                    .expect("background pass");
                assert_eq!(self.backend.rasters.lock().unwrap().len(), before);
                self.backend.phase.store(HIGHLIGHT, Ordering::SeqCst);
                for row in rows.iter().filter(|row| row.size.width > px(0.)) {
                    window.paint_quad(fill(*row, rgb(0x287d3e)));
                }
                self.backend.phase.store(FOREGROUND, Ordering::SeqCst);
                if self.error_probe && !self.backend.error_checked.swap(true, Ordering::SeqCst) {
                    self.backend
                        .fail_raster_bounds
                        .store(true, Ordering::SeqCst);
                    let result = layout.paint_foreground(window, cx);
                    self.backend
                        .fail_raster_bounds
                        .store(false, Ordering::SeqCst);
                    assert!(
                        result
                            .unwrap_err()
                            .to_string()
                            .contains("layered consumer raster bounds failure")
                    );
                }
                layout
                    .paint_foreground(window, cx)
                    .expect("foreground pass");
            }
            self.backend.phase.store(0, Ordering::SeqCst);
            self.paints.borrow_mut().push(PaintedText {
                version: self.version,
                case: self.case,
                text: layout.text(),
                wrapped: layout.wrapped_text(),
                bounds: layout.bounds(),
                line_height: layout.line_height(),
                rows: rows.clone(),
                clip: window.clip_bounds(),
                scale_factor: window.scale_factor(),
            });
        };
        if let Some(clip) = clip {
            window
                .with_prepared_subtree_clip(clip, paint)
                .expect("current consumer clip");
        } else {
            paint(window);
        }
    }
}

struct LayeredView {
    backend: Arc<ObservedTextSystem>,
    paints: Rc<RefCell<Vec<PaintedText>>>,
    width: Pixels,
    font_size: Pixels,
    version: usize,
    source: SharedString,
    combined: bool,
    error_probe: bool,
}

impl Render for LayeredView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for paint in [TextLayout::paint_background, TextLayout::paint_foreground] {
            assert_eq!(
                paint(&TextLayout::default(), window, cx)
                    .unwrap_err()
                    .to_string(),
                "measurement has not been performed"
            );
        }
        let paragraphs = CASES
            .into_iter()
            .enumerate()
            .map(|(index, case)| {
                let source = if case.empty {
                    SharedString::default()
                } else {
                    self.source.clone()
                };
                let split = source.find(' ').unwrap_or(source.len());
                let runs = [(split, "IBM Plex Sans"), (source.len() - split, "Lilex")]
                    .into_iter()
                    .filter(|(len, _)| *len > 0)
                    .map(|(len, family)| TextRun {
                        len,
                        font: Font {
                            features: FontFeatures::disable_ligatures(),
                            ..font(family)
                        },
                        color: rgb(0xffffff).into(),
                        background_color: Some(rgb(0x602040).into()),
                        underline: Some(UnderlineStyle {
                            color: Some(rgb(0xffd020).into()),
                            thickness: px(2.),
                            wavy: false,
                        }),
                        strikethrough: Some(StrikethroughStyle {
                            color: Some(rgb(0x40a0ff).into()),
                            thickness: px(1.),
                        }),
                    })
                    .collect();
                let label = StyledText::new(source.clone()).with_runs(runs);
                div()
                    .w(self.width)
                    .flex_shrink_0()
                    .text_align(case.align)
                    .line_height(self.font_size * if case.overlapping { 0.5 } else { 1.5 })
                    .child(LayeredText {
                        label,
                        source,
                        case,
                        align_width: self.width * case.width_factor,
                        version: self.version,
                        combined: self.combined,
                        error_probe: self.error_probe && index == 0,
                        backend: self.backend.clone(),
                        paints: self.paints.clone(),
                    })
            })
            .collect::<Vec<_>>();
        div()
            .flex()
            .flex_col()
            .size_full()
            .font_family("Lilex")
            .font_features(FontFeatures::disable_ligatures())
            .text_size(self.font_size)
            .children(paragraphs)
    }
}

fn draw_frame(
    app: &mut HeadlessAppContext,
    handle: WindowHandle<LayeredView>,
    paints: &Rc<RefCell<Vec<PaintedText>>>,
) -> Result<(u64, Vec<PaintedText>)> {
    let before = paints.borrow().len();
    let revision = app.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        let revision = window.rendered_frame_revision();
        assert!(revision > 0);
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            Some(revision)
        );
        revision
    })?;
    let paints = paints.borrow();
    assert!(paints.len() >= before + CASES.len());
    let frame = paints[paints.len() - CASES.len()..].to_vec();
    for (paint, case) in frame.iter().zip(CASES) {
        assert_eq!(paint.case, case);
        if case.empty {
            assert_eq!(paint.text, "");
            assert_eq!(paint.wrapped, "");
            assert_eq!(paint.rows.len(), 1);
            assert_eq!(paint.rows[0].size.width, px(0.));
        } else {
            assert!(paint.wrapped.split('\n').count() > paint.text.split('\n').count());
            assert!(paint.wrapped.contains("\n\n"));
            assert!(paint.wrapped.ends_with('\n'));
            assert!(paint.rows.iter().any(|row| row.size.width == px(0.)));
            if case.width_factor < 1. {
                assert!(
                    paint
                        .rows
                        .iter()
                        .any(|row| row.left() < paint.bounds.left())
                );
            } else {
                assert!(
                    paint
                        .rows
                        .iter()
                        .all(|row| row.right() <= paint.bounds.right() + px(0.01))
                );
            }
            if case.clipped {
                assert!(paint.clip.size.height < paint.bounds.size.height);
                assert!(paint.rows.iter().any(|row| !row.intersects(&paint.clip)));
                assert!(paint.rows.iter().any(|row| row.intersects(&paint.clip)));
            }
        }
    }
    Ok((revision, frame))
}

#[test]
fn public_layered_text_survives_notify_reflow_refresh_and_reconstruction() -> Result<()> {
    let backend = Arc::new(ObservedTextSystem::new());
    let backend_lifetime = Arc::downgrade(&backend);
    let mut app = HeadlessAppContext::new(backend.clone());
    app.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        cx.set_text_rendering_mode(TextRenderingMode::Grayscale);
    });
    let text_system = app.text_system().clone();
    text_system.add_fonts(vec![Cow::Borrowed(LILEX), Cow::Borrowed(IBM_PLEX)])?;
    let paints = Rc::new(RefCell::new(Vec::new()));
    let handle = app.open_window(size(px(800.), px(4096.)), {
        let backend = backend.clone();
        let paints = paints.clone();
        move |_, cx| {
            cx.new(|_| LayeredView {
                backend,
                paints,
                width: px(180.),
                font_size: px(20.),
                version: 0,
                source: SOURCE.into(),
                combined: false,
                error_probe: true,
            })
        }
    })?;
    let root = handle.root(&mut app)?;
    let notifications = Rc::new(Cell::new(0));
    let subscription = app.update(|cx| {
        let notifications = notifications.clone();
        cx.observe(&root, move |_, _| {
            notifications.set(notifications.get() + 1)
        })
    });
    let (first_revision, first) = draw_frame(&mut app, handle, &paints)?;
    assert!(first.iter().all(|paint| paint.version == 0));
    assert!(
        first
            .iter()
            .filter(|paint| !paint.case.empty)
            .all(|paint| paint.text == SOURCE)
    );
    assert!(!backend.rasters.lock().unwrap().is_empty());
    assert!(backend.error_checked.load(Ordering::SeqCst));

    handle.update(&mut app, |view, _, cx| {
        view.font_size = px(24.);
        view.source = FINAL_SOURCE.into();
        view.version = 1;
        view.error_probe = false;
        cx.notify();
    })?;
    app.run_until_parked();
    assert_eq!(notifications.get(), 1);
    let (notified_revision, notified) = draw_frame(&mut app, handle, &paints)?;
    assert!(notified_revision > first_revision);
    assert!(notified.iter().all(|paint| paint.version == 1));
    assert!(
        notified
            .iter()
            .filter(|paint| !paint.case.empty)
            .all(|paint| paint.text == FINAL_SOURCE)
    );
    assert!(
        backend
            .rasters
            .lock()
            .unwrap()
            .iter()
            .any(|params| params.font_size == px(24.))
    );

    handle.update(&mut app, |view, _, cx| {
        view.width = px(90.);
        view.version = 2;
        cx.notify();
    })?;
    app.run_until_parked();
    assert_eq!(notifications.get(), 2);
    let (narrow_revision, narrow) = draw_frame(&mut app, handle, &paints)?;
    assert!(narrow_revision > notified_revision);
    for (new, old) in narrow.iter().zip(&notified) {
        assert_eq!(new.text, old.text);
        assert_eq!(new.bounds.size.width, px(90.) * new.case.width_factor);
        if !new.case.empty {
            assert!(new.rows.len() > old.rows.len());
        }
    }
    app.update_window(handle.into(), |_, window, _| window.refresh())?;
    let (refreshed_revision, refreshed) = draw_frame(&mut app, handle, &paints)?;
    assert!(refreshed_revision > narrow_revision);
    assert_eq!(refreshed, narrow);

    handle.update(&mut app, |view, _, cx| {
        view.combined = true;
        cx.notify();
    })?;
    app.run_until_parked();
    let (_, combined) = draw_frame(&mut app, handle, &paints)?;
    assert_eq!(combined, narrow);

    drop(subscription);
    drop(root);
    app.update_window(handle.into(), |_, window, cx| window.remove_window(cx))?;
    app.run_until_parked();
    assert!(app.update(|cx| cx.windows().is_empty()));
    assert!(Arc::ptr_eq(app.text_system(), &text_system));
    let rebuilt = app.open_window(size(px(800.), px(4096.)), {
        let backend = backend.clone();
        let paints = paints.clone();
        move |_, cx| {
            cx.new(|_| LayeredView {
                backend,
                paints,
                width: px(90.),
                font_size: px(24.),
                version: 2,
                source: FINAL_SOURCE.into(),
                combined: false,
                error_probe: false,
            })
        }
    })?;
    assert_ne!(rebuilt.window_id(), handle.window_id());
    let (_, reconstructed) = draw_frame(&mut app, rebuilt, &paints)?;
    assert_eq!(reconstructed, narrow);
    app.update_window(rebuilt.into(), |_, window, cx| window.remove_window(cx))?;
    app.run_until_parked();
    assert!(app.update(|cx| cx.windows().is_empty()));
    assert!(backend.rasters.lock().unwrap().iter().all(|params| {
        !params.is_emoji
            && !params.subpixel_rendering
            && params.scale_factor == narrow[0].scale_factor
    }));
    let text_lifetime = Arc::downgrade(&text_system);
    drop(text_system);
    drop(app);
    assert!(text_lifetime.upgrade().is_none());
    drop(backend);
    assert!(backend_lifetime.upgrade().is_none());
    Ok(())
}
