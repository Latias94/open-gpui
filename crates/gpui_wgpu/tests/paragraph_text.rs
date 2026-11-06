//! Public headless consumer for Zed c214057e086517920b214800725c5d16294ddf0d,
//! reconciled against fixed target 635026fabc38440fce5e1767d125026a17b12012.
//!
//! TestPlatform accepts the framework paint; Swash supplies real software glyph
//! rasters. This test does not exercise a GPU renderer or a native window.

#![cfg(not(target_family = "wasm"))]

use anyhow::Result;
use open_gpui::{
    AppContext as _, Bounds, Context, DevicePixels, Font, FontFallbacks, FontFeatures, FontId,
    FontMetrics, FontRun, GlyphId, HeadlessAppContext, IntoElement, LineLayout, ParentElement as _,
    Pixels, PlatformTextSystem, Point, QuitMode, Render, RenderGlyphParams, SharedString, Size,
    Styled as _, TextAlign, TextRenderingMode, TextRun, TextSystem, Window, WindowHandle, canvas,
    div, font, point, px, size,
};
use open_gpui_wgpu::CosmicTextSystem;
use std::sync::Mutex;
use std::{borrow::Cow, cell::RefCell, rc::Rc, sync::Arc};

const IBM_PLEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/fonts/IBMPlexSans-Regular.ttf"
));
const LILEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/fonts/Lilex-Regular.ttf"
));
const NOTO_HEBREW: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/fonts/NotoSansHebrew-Regular.ttf"
));

const FIRST_LINE: &str = "אב\u{001c}Aé\u{0085}גד\u{2029}BC";
const SECOND_LINE: &str = "de\u{001c}הו\u{0085}fĝ\u{2029}זח";
const SOURCE: &str = concat!(
    "אב\u{001c}Aé\u{0085}גד\u{2029}BC",
    "\n",
    "de\u{001c}הו\u{0085}fĝ\u{2029}זח",
);
const LTR_CONTROL: &str = "éAB\u{001c}CD\u{0085}EF\u{2029}GH";
const SANS: usize = 0;
const MONO: usize = 1;

struct ObservedTextSystem {
    platform: Arc<CosmicTextSystem>,
    prewarmed: Mutex<Vec<Vec<FontId>>>,
    rasters: Mutex<Vec<GlyphRasterObservation>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GlyphRasterPhase {
    Bounds,
    Raster,
}

#[derive(Clone, Debug, PartialEq)]
struct GlyphRasterObservation {
    phase: GlyphRasterPhase,
    params: RenderGlyphParams,
    bounds: Bounds<DevicePixels>,
}

impl PlatformTextSystem for ObservedTextSystem {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        self.platform.add_fonts(fonts)
    }

    fn set_missing_glyph_sink(&self, sink: Option<Arc<dyn open_gpui::MissingGlyphSink>>) {
        self.platform.set_missing_glyph_sink(sink);
    }

    fn all_font_names(&self) -> Vec<String> {
        self.platform.all_font_names()
    }

    fn font_id(&self, font: &Font) -> Result<FontId> {
        self.platform.font_id(font)
    }

    fn prewarm_fonts(&self, font_ids: &[FontId]) {
        self.platform.prewarm_fonts(font_ids);
        self.prewarmed.lock().unwrap().push(font_ids.to_vec());
    }

    fn font_metrics(&self, id: FontId) -> FontMetrics {
        self.platform.font_metrics(id)
    }

    fn typographic_bounds(&self, id: FontId, glyph: GlyphId) -> Result<Bounds<f32>> {
        self.platform.typographic_bounds(id, glyph)
    }

    fn advance(&self, id: FontId, glyph: GlyphId) -> Result<Size<f32>> {
        self.platform.advance(id, glyph)
    }

    fn glyph_for_char(&self, id: FontId, character: char) -> Option<GlyphId> {
        self.platform.glyph_for_char(id, character)
    }

    fn glyph_raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        let bounds = self.platform.glyph_raster_bounds(params)?;
        self.rasters.lock().unwrap().push(GlyphRasterObservation {
            phase: GlyphRasterPhase::Bounds,
            params: params.clone(),
            bounds,
        });
        Ok(bounds)
    }

    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        let raster = self.platform.rasterize_glyph(params, bounds)?;
        assert_eq!(raster.0, bounds.size);
        assert!(raster.1.iter().any(|byte| *byte != 0));
        self.rasters.lock().unwrap().push(GlyphRasterObservation {
            phase: GlyphRasterPhase::Raster,
            params: params.clone(),
            bounds,
        });
        Ok(raster)
    }

    fn layout_line(&self, source: &str, size: Pixels, runs: &[FontRun]) -> LineLayout {
        self.platform.layout_line(source, size, runs)
    }

    fn recommended_rendering_mode(&self, id: FontId, size: Pixels) -> TextRenderingMode {
        self.platform.recommended_rendering_mode(id, size)
    }
}

/// Checks fresh public paint keys at the bounds-to-raster boundary.
/// Backend unit tests separately establish actual image-generation reuse.
fn assert_fresh_paint_handoffs(
    observations: &[GlyphRasterObservation],
    font_size: Pixels,
    scale_factor: f32,
) {
    let mut raster_count = 0;
    for (index, observation) in observations.iter().enumerate() {
        if observation.phase != GlyphRasterPhase::Raster {
            continue;
        }
        raster_count += 1;
        assert_eq!(observation.params.font_size, font_size);
        assert_eq!(observation.params.scale_factor, scale_factor);
        assert!(!observation.params.is_emoji);
        assert!(!observation.params.subpixel_rendering);
        assert_eq!(observation.params.dilation, 0);
        assert!(observation.bounds.size.width.0 > 0);
        assert!(observation.bounds.size.height.0 > 0);
        let bounds = index
            .checked_sub(1)
            .map(|previous| &observations[previous])
            .expect("a fresh public raster must follow its bounds request");
        assert_eq!(bounds.phase, GlyphRasterPhase::Bounds);
        assert_eq!(bounds.params, observation.params);
        assert_eq!(bounds.bounds, observation.bounds);
    }
    assert!(
        raster_count > 0,
        "the public paint must rasterize real glyphs"
    );
}

type RunRecipe = &'static [(usize, usize)];
type SegmentRecipe = (&'static str, RunRecipe);

struct LogicalLineRecipe {
    text: &'static str,
    segments: &'static [SegmentRecipe],
}

/// The middle Sans run crosses the newline; other boundaries cross paragraph interiors.
const SOURCE_RUNS: RunRecipe = &[
    ("אב\u{001c}A".len(), SANS),
    ("é\u{0085}ג".len(), MONO),
    ("ד\u{2029}BC\nde\u{001c}ה".len(), SANS),
    ("ו\u{0085}f".len(), MONO),
    ("ĝ\u{2029}זח".len(), SANS),
];

/// Explicit segment recipes avoid production run clipping and private Core cache fields.
const LOGICAL_LINES: &[LogicalLineRecipe] = &[
    LogicalLineRecipe {
        text: FIRST_LINE,
        segments: &[
            ("אב", &[(4, SANS)]),
            ("\u{001c}", &[(1, SANS)]),
            ("Aé", &[(1, SANS), (2, MONO)]),
            ("\u{0085}", &[(2, MONO)]),
            ("גד", &[(2, MONO), (2, SANS)]),
            ("\u{2029}", &[(3, SANS)]),
            ("BC", &[(2, SANS)]),
        ],
    },
    LogicalLineRecipe {
        text: SECOND_LINE,
        segments: &[
            ("de", &[(2, SANS)]),
            ("\u{001c}", &[(1, SANS)]),
            ("הו", &[(2, SANS), (2, MONO)]),
            ("\u{0085}", &[(2, MONO)]),
            ("fĝ", &[(1, MONO), (2, SANS)]),
            ("\u{2029}", &[(3, SANS)]),
            ("זח", &[(4, SANS)]),
        ],
    },
];

#[derive(Clone, Debug, PartialEq)]
struct GlyphObservation {
    font_id: FontId,
    id: GlyphId,
    position: Point<Pixels>,
    index: usize,
    is_emoji: bool,
}

fn glyphs(layout: &LineLayout) -> Vec<GlyphObservation> {
    layout
        .runs
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

fn assert_same_layout(actual: &LineLayout, expected: &LineLayout) {
    assert_eq!(actual.len, expected.len);
    assert_eq!(actual.font_size, expected.font_size);
    assert_eq!(actual.width, expected.width);
    assert_eq!(actual.ascent, expected.ascent);
    assert_eq!(actual.descent, expected.descent);
    assert_eq!(glyphs(actual), glyphs(expected));
}

fn reference_line(
    platform: &CosmicTextSystem,
    font_ids: &[FontId; 2],
    font_size: Pixels,
    recipe: &LogicalLineRecipe,
) -> LineLayout {
    let mut expected = LineLayout {
        font_size,
        ..Default::default()
    };
    let mut source = String::new();
    for &(segment, runs) in recipe.segments {
        assert_eq!(
            runs.iter().map(|(len, _)| len).sum::<usize>(),
            segment.len()
        );
        let runs = runs
            .iter()
            .map(|&(len, face)| FontRun {
                len,
                font_id: font_ids[face],
            })
            .collect::<Vec<_>>();
        let mut shaped = platform.layout_line(segment, font_size, &runs);
        assert_eq!(shaped.len, segment.len());
        for run in &mut shaped.runs {
            for glyph in &mut run.glyphs {
                glyph.index += source.len();
                glyph.position.x += expected.width;
            }
        }
        expected.runs.extend(shaped.runs);
        expected.width += shaped.width;
        expected.ascent = expected.ascent.max(shaped.ascent);
        expected.descent = expected.descent.max(shaped.descent);
        source.push_str(segment);
    }
    assert_eq!(source, recipe.text);
    expected.len = source.len();
    expected
}

#[derive(Clone)]
struct PaintedLine {
    text: SharedString,
    source_start: usize,
    layout: Arc<LineLayout>,
    wrapped_rows: usize,
}

#[derive(Clone)]
struct PaintedText {
    version: usize,
    bounds: Bounds<Pixels>,
    lines: Vec<PaintedLine>,
}

impl PaintedText {
    fn wrapped_rows(&self) -> usize {
        self.lines.iter().map(|line| line.wrapped_rows).sum()
    }

    fn assert_same_text(&self, other: &Self) {
        assert_eq!(self.lines.len(), other.lines.len());
        for (actual, expected) in self.lines.iter().zip(&other.lines) {
            assert_eq!(actual.text, expected.text);
            assert_eq!(actual.source_start, expected.source_start);
            assert_eq!(actual.wrapped_rows, expected.wrapped_rows);
            assert_same_layout(&actual.layout, &expected.layout);
        }
    }
}

struct ParagraphView {
    platform: Arc<CosmicTextSystem>,
    fonts: [Font; 2],
    paints: Rc<RefCell<Vec<PaintedText>>>,
    content_width: Pixels,
    font_size: Pixels,
    version: usize,
}

impl Render for ParagraphView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let platform = self.platform.clone();
        let fonts = self.fonts.clone();
        let paints = self.paints.clone();
        let font_size = self.font_size;
        let version = self.version;
        div().size_full().child(
            canvas(
                move |bounds, window, cx| {
                    let window_text: &TextSystem = window.text_system();
                    assert!(std::ptr::eq(window_text, cx.text_system().as_ref()));
                    let font_ids = fonts
                        .each_ref()
                        .map(|font| cx.text_system().resolve_font(font));
                    let runs = SOURCE_RUNS
                        .iter()
                        .map(|&(len, face)| TextRun {
                            len,
                            font: fonts[face].clone(),
                            ..Default::default()
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(runs.iter().map(|run| run.len).sum::<usize>(), SOURCE.len());
                    let lines = window
                        .text_system()
                        .shape_text(
                            SOURCE.into(),
                            font_size,
                            &runs,
                            Some(bounds.size.width),
                            None,
                        )
                        .expect("the original mixed-direction paragraphs must shape without panic");
                    assert_eq!(lines.len(), LOGICAL_LINES.len());
                    assert_eq!(
                        lines
                            .iter()
                            .map(|line| line.text.as_ref())
                            .collect::<Vec<_>>()
                            .join("\n"),
                        SOURCE,
                    );
                    for (line, recipe) in lines.iter().zip(LOGICAL_LINES) {
                        assert_eq!(line.text.as_ref(), recipe.text);
                        assert_eq!(line.len(), recipe.text.len());
                        let expected = reference_line(&platform, &font_ids, font_size, recipe);
                        assert_same_layout(&line.unwrapped_layout, &expected);
                    }
                    let control = window.text_system().shape_line(
                        LTR_CONTROL.into(),
                        font_size,
                        &[TextRun {
                            len: LTR_CONTROL.len(),
                            font: fonts[SANS].clone(),
                            ..Default::default()
                        }],
                        None,
                    );
                    assert_ltr_index_hit_and_split(&control);
                    (lines, control)
                },
                move |bounds, (lines, control), window, cx| {
                    let line_height = px(40.);
                    let mut origin = bounds.origin;
                    let mut source_start = 0;
                    let mut painted = Vec::new();
                    for line in lines {
                        assert_eq!(
                            &SOURCE[source_start..source_start + line.len()],
                            line.text.as_ref(),
                        );
                        line.paint(origin, line_height, TextAlign::Left, None, window, cx)
                            .expect("real glyphs must paint through the public window path");
                        let wrapped_rows = line.wrap_boundaries().len() + 1;
                        origin.y += line.size(line_height).height;
                        painted.push(PaintedLine {
                            text: line.text.clone(),
                            source_start,
                            layout: line.unwrapped_layout.clone(),
                            wrapped_rows,
                        });
                        source_start += line.len() + '\n'.len_utf8();
                    }
                    assert_eq!(source_start - 1, SOURCE.len());
                    control
                        .paint(origin, line_height, TextAlign::Left, None, window, cx)
                        .expect("the LTR control must also paint real glyphs");
                    paints.borrow_mut().push(PaintedText {
                        version,
                        bounds,
                        lines: painted,
                    });
                },
            )
            .w(self.content_width)
            .h(px(400.)),
        )
    }
}

fn is_separator(character: char) -> bool {
    matches!(character, '\u{001c}' | '\u{0085}' | '\u{2029}')
}

fn assert_ltr_index_hit_and_split(line: &open_gpui::ShapedLine) {
    assert_eq!(line.text.as_ref(), LTR_CONTROL);
    assert_eq!(line.len(), LTR_CONTROL.len());
    let visible = glyphs(line)
        .into_iter()
        .filter(|glyph| !is_separator(LTR_CONTROL[glyph.index..].chars().next().unwrap()))
        .collect::<Vec<_>>();
    for (index, glyph) in visible.iter().enumerate() {
        let next_x = visible
            .get(index + 1)
            .map_or(line.width(), |next| next.position.x);
        assert!(next_x > glyph.position.x);
        let inside = glyph.position.x + (next_x - glyph.position.x) / 4.;
        assert_eq!(line.x_for_index(glyph.index), glyph.position.x);
        assert_eq!(line.index_for_x(inside), Some(glyph.index));
        assert_eq!(line.closest_index_for_x(inside), glyph.index);
    }
    assert_eq!(line.index_for_x(line.width()), None);
    let split = "éAB\u{001c}".len();
    let (left, right) = line.split_at(split);
    assert_eq!(left.text.as_ref(), &LTR_CONTROL[..split]);
    assert_eq!(right.text.as_ref(), &LTR_CONTROL[split..]);
    assert_eq!(left.len() + right.len(), line.len());
    assert!((left.width() + right.width() - line.width()).abs() < px(0.001));
    let mut reconstructed = glyphs(&left);
    reconstructed.extend(glyphs(&right).into_iter().map(|mut glyph| {
        glyph.index += split;
        glyph.position.x += left.width();
        glyph
    }));
    let original = glyphs(line);
    assert_eq!(reconstructed.len(), original.len());
    for (actual, expected) in reconstructed.iter().zip(&original) {
        assert_eq!(actual.font_id, expected.font_id);
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.index, expected.index);
        assert_eq!(actual.is_emoji, expected.is_emoji);
        assert_eq!(actual.position.y, expected.position.y);
        assert!((actual.position.x - expected.position.x).abs() < px(0.001));
    }
}

fn assert_real_glyphs_and_software_rasters(
    platform: &CosmicTextSystem,
    painted: &PaintedText,
) -> Result<()> {
    let mut hebrew_font = None;
    let mut descending_rtl_indices = false;
    for line in &painted.lines {
        assert!(line.layout.width > px(0.));
        assert!(f32::from(line.layout.width).is_finite());
        let shaped = glyphs(&line.layout);
        descending_rtl_indices |= shaped.windows(2).any(|pair| pair[0].index > pair[1].index);
        for (index, character) in line
            .text
            .char_indices()
            .filter(|(_, ch)| !is_separator(*ch))
        {
            let glyph = shaped
                .iter()
                .find(|glyph| glyph.index == index)
                .expect("every visible source character must retain its original byte index");
            assert_ne!(glyph.id, GlyphId(0));
            assert_eq!(
                platform.glyph_for_char(glyph.font_id, character),
                Some(glyph.id),
            );
            assert!(SOURCE.is_char_boundary(line.source_start + glyph.index));
            assert_eq!(
                SOURCE[line.source_start + glyph.index..].chars().next(),
                Some(character),
            );
            if ('א'..='ת').contains(&character) {
                assert_eq!(*hebrew_font.get_or_insert(glyph.font_id), glyph.font_id);
                assert!(platform.glyph_for_char(glyph.font_id, 'm').is_none());
            }
            for scale_factor in [1., 1.5] {
                let params = RenderGlyphParams {
                    font_id: glyph.font_id,
                    glyph_id: glyph.id,
                    font_size: line.layout.font_size,
                    subpixel_variant: point(0, 0),
                    scale_factor,
                    is_emoji: glyph.is_emoji,
                    subpixel_rendering: false,
                    dilation: 0,
                };
                let bounds = platform.glyph_raster_bounds(&params)?;
                assert!(bounds.size.width.0 > 0 && bounds.size.height.0 > 0);
                let (raster_size, pixels) = platform.rasterize_glyph(&params, bounds)?;
                assert_eq!(raster_size, bounds.size);
                assert_eq!(
                    pixels.len(),
                    usize::try_from(raster_size.width.0)? * usize::try_from(raster_size.height.0)?,
                );
                assert!(pixels.iter().any(|alpha| *alpha != 0));
                let (repeated_size, repeated_pixels) = platform.rasterize_glyph(&params, bounds)?;
                assert_eq!(repeated_size, raster_size);
                assert_eq!(repeated_pixels, pixels);
            }
        }
        for glyph in &shaped {
            assert!(glyph.index < line.text.len());
            assert!(line.text.is_char_boundary(glyph.index));
            assert!(f32::from(glyph.position.x).is_finite());
            assert!(f32::from(glyph.position.y).is_finite());
        }
    }
    let hebrew_font = hebrew_font.expect("the actual Hebrew fallback must be exercised");
    assert_eq!(platform.font_metrics(hebrew_font).units_per_em, 1_000);
    for (character, glyph, advance) in [('א', GlyphId(3), 632.), ('ב', GlyphId(12), 572.)] {
        assert_eq!(platform.glyph_for_char(hebrew_font, character), Some(glyph));
        assert_eq!(platform.advance(hebrew_font, glyph)?.width, advance);
    }
    assert!(
        descending_rtl_indices,
        "the fixture must exercise visual RTL ordering",
    );
    Ok(())
}

fn draw_frame(app: &mut HeadlessAppContext, handle: WindowHandle<ParagraphView>) -> Result<u64> {
    app.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        let revision = window.rendered_frame_revision();
        assert!(revision > 0);
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            Some(revision),
        );
        revision
    })
}

#[test]
fn paragraph_text_survives_headless_notification_layout_resize_and_reconstruction() -> Result<()> {
    let platform = Arc::new(CosmicTextSystem::new_without_system_fonts("IBM Plex Sans"));
    let observed = Arc::new(ObservedTextSystem {
        platform: platform.clone(),
        prewarmed: Mutex::default(),
        rasters: Mutex::default(),
    });
    let mut app = HeadlessAppContext::new(observed.clone());
    app.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        cx.set_text_rendering_mode(TextRenderingMode::Grayscale);
    });
    let text_system = app.update(|cx| cx.text_system().clone());
    assert!(Arc::ptr_eq(app.text_system(), &text_system));
    let shutdowns = Rc::new(RefCell::new(0));
    let shutdown_subscription = app.update(|cx| {
        let shutdowns = shutdowns.clone();
        cx.on_app_quit(move |cx| {
            assert!(cx.windows().is_empty());
            *shutdowns.borrow_mut() += 1;
            std::future::ready(())
        })
    });
    text_system.add_fonts(vec![
        Cow::Borrowed(IBM_PLEX),
        Cow::Borrowed(LILEX),
        Cow::Borrowed(NOTO_HEBREW),
    ])?;
    assert_eq!(
        platform.all_font_names(),
        ["IBM Plex Sans", "Lilex", "Noto Sans Hebrew"],
    );
    assert_eq!(text_system.all_font_names(), platform.all_font_names());
    let fonts = ["IBM Plex Sans", "Lilex"].map(|family| Font {
        features: FontFeatures::disable_ligatures(),
        fallbacks: Some(FontFallbacks::from_fonts(vec!["Noto Sans Hebrew".into()])),
        ..font(family)
    });
    text_system.prewarm_fonts(&fonts);
    let prewarmed_ids = fonts.each_ref().map(|font| text_system.resolve_font(font));
    assert_eq!(
        observed.prewarmed.lock().unwrap().as_slice(),
        &[prewarmed_ids.to_vec()],
    );
    assert!(observed.rasters.lock().unwrap().is_empty());
    for font in &fonts {
        let primary = text_system.resolve_font(font);
        assert!(platform.glyph_for_char(primary, 'א').is_none());
    }
    let paints = Rc::new(RefCell::new(Vec::new()));
    let closed = Rc::new(RefCell::new(Vec::new()));
    let close_subscription = app.update(|cx| {
        let closed = closed.clone();
        cx.on_window_closed(move |_, window| closed.borrow_mut().push(window))
    });
    let handle = app.open_window(size(px(640.), px(440.)), {
        let platform = platform.clone();
        let fonts = fonts.clone();
        let paints = paints.clone();
        move |_, cx| {
            cx.new(|_| ParagraphView {
                platform,
                fonts,
                paints,
                content_width: px(600.),
                font_size: px(24.),
                version: 0,
            })
        }
    })?;
    let root = handle.root(&mut app)?;
    let scale_factor = app.update_window(handle.into(), |_, window, _| {
        assert_eq!(window.viewport_size(), size(px(640.), px(440.)));
        window.scale_factor()
    })?;
    let notifications = Rc::new(RefCell::new(0));
    let notification_subscription = app.update(|cx| {
        let notifications = notifications.clone();
        cx.observe(&root, move |_, _| *notifications.borrow_mut() += 1)
    });
    let initial_revision = draw_frame(&mut app, handle)?;
    let initial = paints
        .borrow()
        .last()
        .expect("initial canvas paint")
        .clone();
    assert_eq!(initial.version, 0);
    assert_eq!(initial.wrapped_rows(), 2);
    let paint_rasters = observed.rasters.lock().unwrap().clone();
    assert_fresh_paint_handoffs(&paint_rasters, px(24.), scale_factor);
    for expected in [GlyphId(3), GlyphId(12)] {
        assert!(paint_rasters.iter().any(|raster| {
            raster.phase == GlyphRasterPhase::Raster
                && raster.params.glyph_id == expected
                && platform
                    .glyph_for_char(raster.params.font_id, 'm')
                    .is_none()
        }));
    }
    assert_real_glyphs_and_software_rasters(&platform, &initial)?;

    let rasters_before_notification = observed.rasters.lock().unwrap().len();
    handle.update(&mut app, |view, _, cx| {
        view.font_size = px(28.);
        view.version = 1;
        cx.notify();
    })?;
    app.run_until_parked();
    assert_eq!(*notifications.borrow(), 1);
    let notified_revision = draw_frame(&mut app, handle)?;
    assert!(notified_revision > initial_revision);
    let notified = paints
        .borrow()
        .last()
        .expect("notified canvas paint")
        .clone();
    assert_eq!(notified.version, 1);
    assert!(notified.lines[0].layout.width > initial.lines[0].layout.width);
    assert_fresh_paint_handoffs(
        &observed.rasters.lock().unwrap()[rasters_before_notification..],
        px(28.),
        scale_factor,
    );
    assert_real_glyphs_and_software_rasters(&platform, &notified)?;

    handle.update(&mut app, |view, _, cx| {
        view.content_width = px(60.);
        view.version = 2;
        cx.notify();
    })?;
    app.run_until_parked();
    assert_eq!(*notifications.borrow(), 2);
    let resized_revision = draw_frame(&mut app, handle)?;
    assert!(resized_revision > notified_revision);
    let resized = paints
        .borrow()
        .last()
        .expect("resized canvas paint")
        .clone();
    assert_eq!(resized.version, 2);
    assert_eq!(resized.bounds.size.width, px(60.));
    assert!(resized.wrapped_rows() > notified.wrapped_rows());
    for (actual, expected) in resized.lines.iter().zip(&notified.lines) {
        assert_same_layout(&actual.layout, &expected.layout);
    }

    let mut revision = resized_revision;
    for _ in 0..2 {
        let paint_count = paints.borrow().len();
        let raster_count = observed.rasters.lock().unwrap().len();
        app.update_window(handle.into(), |_, window, _| window.refresh())?;
        let next_revision = draw_frame(&mut app, handle)?;
        assert!(next_revision > revision);
        assert!(paints.borrow().len() > paint_count);
        paints.borrow().last().unwrap().assert_same_text(&resized);
        assert_eq!(
            observed.rasters.lock().unwrap().len(),
            raster_count,
            "unchanged redraws exercise the existing atlas cache control",
        );
        revision = next_revision;
    }

    drop(notification_subscription);
    drop(root);
    app.update_window(handle.into(), |_, window, cx| window.remove_window(cx))?;
    app.run_until_parked();
    assert_eq!(&*closed.borrow(), &[handle.window_id()]);
    assert!(app.update(|cx| cx.windows().is_empty()));
    assert!(Arc::ptr_eq(app.text_system(), &text_system));
    app.update(|cx| assert!(Arc::ptr_eq(cx.text_system(), &text_system)));

    let rasters_before_reconstruction = observed
        .rasters
        .lock()
        .unwrap()
        .iter()
        .filter(|raster| raster.phase == GlyphRasterPhase::Raster)
        .count();
    let reconstructed = app.open_window(size(px(360.), px(440.)), {
        let platform = platform.clone();
        let paints = paints.clone();
        move |_, cx| {
            cx.new(|_| ParagraphView {
                platform,
                fonts,
                paints,
                content_width: px(60.),
                font_size: px(28.),
                version: 2,
            })
        }
    })?;
    assert_ne!(reconstructed.window_id(), handle.window_id());
    app.update_window(reconstructed.into(), |_, window, _| {
        assert_eq!(window.viewport_size(), size(px(360.), px(440.)));
    })?;
    draw_frame(&mut app, reconstructed)?;
    assert!(
        observed
            .rasters
            .lock()
            .unwrap()
            .iter()
            .filter(|raster| raster.phase == GlyphRasterPhase::Raster)
            .count()
            > rasters_before_reconstruction
    );
    let rebuilt = paints
        .borrow()
        .last()
        .expect("reconstructed canvas paint")
        .clone();
    rebuilt.assert_same_text(&resized);
    assert_eq!(rebuilt.bounds.size.width, resized.bounds.size.width);
    assert_real_glyphs_and_software_rasters(&platform, &rebuilt)?;
    app.update_window(reconstructed.into(), |_, window, cx| {
        window.remove_window(cx)
    })?;
    app.run_until_parked();
    assert_eq!(
        &*closed.borrow(),
        &[handle.window_id(), reconstructed.window_id()],
    );
    assert!(app.update(|cx| cx.windows().is_empty()));
    drop(close_subscription);

    let text_lifetime = Arc::downgrade(&text_system);
    let observed_lifetime = Arc::downgrade(&observed);
    let platform_lifetime = Arc::downgrade(&platform);
    drop(text_system);
    assert_eq!(*shutdowns.borrow(), 0);
    drop(app);
    assert_eq!(*shutdowns.borrow(), 1);
    drop(shutdown_subscription);
    assert!(text_lifetime.upgrade().is_none());
    drop(observed);
    assert!(observed_lifetime.upgrade().is_none());
    drop(platform);
    assert!(platform_lifetime.upgrade().is_none());
    Ok(())
}
