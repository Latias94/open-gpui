use std::{borrow::Cow, cell::Cell, rc::Rc, sync::Arc};

use open_gpui::prelude::*;
use open_gpui::{
    App, Bounds, Context, FocusHandle, Font, FontRun, KeyDownEvent, LineLayout, MissingGlyph,
    PlatformTextSystem, PrepaintPublicationId, RenderGlyphParams, ShapedLine, Subscription,
    TextAlign, TextRun, Window, WindowBounds, WindowOptions, canvas, div, font, point, px, rgb,
    size,
};
use wasm_bindgen::JsValue;

const REPEATED: &str = "😀😀😀 🕸🕸🕸";
const CJK: &str = "中文 が か\u{3099} 각 각";
const EMOJI: &str = "🕸 🕸\u{fe0f} 🕸\u{fe0e} 🕸 😀 ❤️ 👍🏽 🇯🇵 1️⃣ 👩‍💻 👨‍👩‍👧‍👦";
const MAX_REPORTS: usize = 128;

#[derive(Clone, Copy)]
enum Phase {
    Bundled,
    Emoji,
    Spider,
    Mixed,
    Size,
    Italic,
    Face,
    RegularInstalled,
    BoldInstalled,
}

impl Phase {
    fn name(self) -> &'static str {
        match self {
            Self::Bundled => "bundled",
            Self::Emoji => "emoji",
            Self::Spider => "spider",
            Self::Mixed => "mixed",
            Self::Size => "size",
            Self::Italic => "italic",
            Self::Face => "face",
            Self::RegularInstalled => "regular-installed",
            Self::BoldInstalled => "bold-installed",
        }
    }

    fn samples(self) -> Vec<Sample> {
        match self {
            Self::Bundled => vec![
                Sample::new("Hello ©", font("Inter Variable"), 24.0),
                Sample::new("Hello ©", font("JetBrains Mono"), 24.0),
            ],
            Self::Emoji => vec![Sample::new("😀😀😀", font("Inter Variable"), 24.0)],
            Self::Spider => vec![Sample::new("🕸🕸🕸", font("Inter Variable"), 24.0)],
            Self::Mixed => vec![
                Sample::new(CJK, font("Inter Variable"), 24.0),
                Sample::new(EMOJI, font("Inter Variable"), 24.0),
                Sample::new("ش か\u{309a} ᄀ", font("JetBrains Mono"), 24.0),
            ],
            Self::Size => vec![Sample::new(REPEATED, font("Inter Variable"), 36.0)],
            Self::Italic => vec![Sample::new(REPEATED, font("Inter Variable").italic(), 24.0)],
            Self::Face | Self::RegularInstalled | Self::BoldInstalled => vec![
                Sample::new(REPEATED, font("JetBrains Mono"), 24.0),
                Sample::new("mmmm", font("Lilex"), 24.0),
                Sample::new("mmmm", font("Lilex").bold(), 24.0),
                Sample::new("ش", font("JetBrains Mono"), 24.0),
            ],
        }
    }
}

struct Sample {
    text: &'static str,
    font: Font,
    size: f32,
}

impl Sample {
    fn new(text: &'static str, font: Font, size: f32) -> Self {
        Self { text, font, size }
    }
}

struct Report {
    installation: u32,
    missing: MissingGlyph,
}

struct FontFallbackSmoke {
    focus: FocusHandle,
    graphics: super::GraphicsProbe,
    text_system: Arc<dyn PlatformTextSystem>,
    publication: PrepaintPublicationId,
    phase: Phase,
    policy_query: String,
    command_revision: u32,
    installations: u32,
    renders: u32,
    reports: Vec<Report>,
    report_overflow: bool,
    callback_reentries: u32,
    failure: Option<String>,
    _missing_subscription: Option<Subscription>,
}

impl FontFallbackSmoke {
    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let phase = match event.keystroke.key.as_str() {
            "f1" => Phase::Emoji,
            "f2" => Phase::Spider,
            "f3" => Phase::Mixed,
            "f4" => Phase::Size,
            "f5" => Phase::Italic,
            "f6" => Phase::Face,
            "f7" => {
                if self.installations == 0 {
                    self.install(
                        include_bytes!("../../../../tests/fixtures/fonts/Lilex-Regular.ttf"),
                        cx,
                    );
                }
                Phase::RegularInstalled
            }
            "f8" => {
                if self.installations == 1 {
                    self.install(
                        include_bytes!("../../../../tests/fixtures/fonts/Lilex-Bold.ttf"),
                        cx,
                    );
                } else if self.installations == 0 {
                    self.failure = Some("Install Lilex Regular with F7 before F8".into());
                }
                Phase::BoldInstalled
            }
            "f9" => self.phase,
            _ => return,
        };
        window.prevent_default();
        cx.stop_propagation();
        self.phase = phase;
        self.command_revision += 1;
        cx.notify();
    }

    fn install(&mut self, bytes: &'static [u8], cx: &mut Context<Self>) {
        match cx.text_system().add_fonts(vec![Cow::Borrowed(bytes)]) {
            Ok(()) => self.installations += 1,
            Err(error) => self.failure = Some(format!("Font installation failed: {error:#}")),
        }
    }

    fn observe_missing(&mut self, missing: &[MissingGlyph], cx: &mut Context<Self>) {
        match self.text_system.font_id(&font("Inter Variable")) {
            Ok(font_id) => {
                let text = "Hello ©";
                let layout = self.text_system.layout_line(
                    text,
                    px(24.0),
                    &[FontRun {
                        len: text.len(),
                        font_id,
                    }],
                );
                if layout.runs.is_empty()
                    || layout
                        .runs
                        .iter()
                        .flat_map(|run| &run.glyphs)
                        .any(|glyph| glyph.id.0 == 0)
                {
                    self.failure =
                        Some("Missing-glyph callback reentry lost bundled glyphs".into());
                } else {
                    self.callback_reentries += 1;
                }
            }
            Err(error) => {
                self.failure = Some(format!("Missing-glyph callback reentry failed: {error:#}"));
            }
        }
        let mut changed = false;
        for missing in missing {
            if self.reports.iter().any(|report| {
                report.installation == self.installations && report.missing == *missing
            }) {
                continue;
            }
            if self.reports.len() == MAX_REPORTS {
                self.report_overflow = true;
                break;
            }
            self.reports.push(Report {
                installation: self.installations,
                missing: missing.clone(),
            });
            changed = true;
        }
        if changed || self.report_overflow {
            cx.notify();
        }
    }
}

fn glyphs(layout: &LineLayout) -> Vec<(String, u32, usize, bool, f32, f32)> {
    layout
        .runs
        .iter()
        .flat_map(|run| {
            run.glyphs.iter().map(move |glyph| {
                (
                    run.font_id.0.to_string(),
                    glyph.id.0,
                    glyph.index,
                    glyph.is_emoji,
                    f32::from(glyph.position.x),
                    f32::from(glyph.position.y),
                )
            })
        })
        .collect()
}

fn shape_sample(
    sample: Sample,
    platform: &dyn PlatformTextSystem,
    window: &mut Window,
) -> (ShapedLine, JsValue) {
    let resolved = window.text_system().resolve_font(&sample.font);
    let direct = platform.font_id(&sample.font).ok();
    let raw = platform.layout_line(
        sample.text,
        px(sample.size),
        &[FontRun {
            len: sample.text.len(),
            font_id: resolved,
        }],
    );
    let shaped = window.text_system().shape_line(
        sample.text.into(),
        px(sample.size),
        &[TextRun {
            len: sample.text.len(),
            font: sample.font.clone(),
            color: rgb(0xf0f4fa).into(),
            ..Default::default()
        }],
        None,
    );
    let shaped_glyphs = glyphs(&shaped);
    let matches = shaped_glyphs == glyphs(&raw)
        && raw.len == shaped.len()
        && raw.width == shaped.width()
        && raw.ascent == shaped.ascent
        && raw.descent == shaped.descent;
    let glyph_values: js_sys::Array = shaped_glyphs
        .into_iter()
        .map(|(font, id, index, color, x, y)| {
            super::js_object(&[
                ("font", font.into()),
                ("id", id.into()),
                ("index", (index as f64).into()),
                ("color", color.into()),
                ("x", x.into()),
                ("y", y.into()),
            ])
        })
        .collect();
    let m_raster = platform
        .glyph_for_char(resolved, 'm')
        .and_then(|glyph| {
            let params = RenderGlyphParams {
                font_id: resolved,
                glyph_id: glyph,
                font_size: px(sample.size),
                subpixel_variant: point(0, 0),
                scale_factor: 1.0,
                is_emoji: false,
                subpixel_rendering: false,
                dilation: 0,
            };
            let bounds = platform.glyph_raster_bounds(&params).ok()?;
            let (size, bytes) = platform.rasterize_glyph(&params, bounds).ok()?;
            let nonzero = bytes.iter().filter(|&&byte| byte != 0).count();
            if nonzero == 0 {
                return None;
            }
            let fingerprint = bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
            });
            Some(super::js_object(&[
                ("x", bounds.origin.x.0.into()),
                ("y", bounds.origin.y.0.into()),
                ("width", size.width.0.into()),
                ("height", size.height.0.into()),
                ("nonzeroBytes", (nonzero as f64).into()),
                ("fingerprint", format!("{fingerprint:016x}").into()),
            ]))
        })
        .unwrap_or(JsValue::NULL);
    let coverage: js_sys::Array = ['H', '©', '😀', '🕸', '中', 'ش']
        .into_iter()
        .map(|ch| {
            super::js_object(&[
                ("text", ch.to_string().into()),
                (
                    "present",
                    platform.glyph_for_char(resolved, ch).is_some().into(),
                ),
            ])
        })
        .collect();
    let snapshot = super::js_object(&[
        ("text", sample.text.into()),
        ("family", sample.font.family.as_str().into()),
        ("style", format!("{:?}", sample.font.style).into()),
        ("weight", sample.font.weight.0.into()),
        ("size", sample.size.into()),
        ("resolvedFont", resolved.0.to_string().into()),
        (
            "directFont",
            direct
                .map(|id| JsValue::from_str(&id.0.to_string()))
                .unwrap_or(JsValue::NULL),
        ),
        ("rawMatchesShaped", matches.into()),
        ("width", f32::from(shaped.width()).into()),
        ("glyphs", glyph_values.into()),
        ("mRaster", m_raster),
        ("coverage", coverage.into()),
    ]);
    (shaped, snapshot)
}

fn publish(probe: &JsValue) {
    if let Some(window) = web_sys::window() {
        for name in ["__OPEN_GPUI_WEB_FONT_FALLBACK__", "__OPEN_GPUI_WEB_SMOKE__"] {
            js_sys::Reflect::set(window.as_ref(), &name.into(), probe)
                .expect("font fallback observations must be writable");
        }
    }
}

impl Render for FontFallbackSmoke {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        let samples = self.phase.samples();
        let phase = self.phase.name();
        let text_system = self.text_system.clone();
        let graphics = self.graphics;
        let publication = self.publication;
        let reports: js_sys::Array = self
            .reports
            .iter()
            .map(|report| {
                super::js_object(&[
                    ("installation", report.installation.into()),
                    ("text", report.missing.grapheme().into()),
                    (
                        "fontClass",
                        format!("{:?}", report.missing.font_class()).into(),
                    ),
                ])
            })
            .collect();
        let snapshot = super::js_object(&[
            ("phase", phase.into()),
            ("policyQuery", self.policy_query.as_str().into()),
            (
                "policy",
                format!("{:?}", self.graphics.config.canvas_font_fallback).into(),
            ),
            ("commandRevision", self.command_revision.into()),
            ("installations", self.installations.into()),
            ("renderCalls", self.renders.into()),
            ("missing", reports.into()),
            ("reportOverflow", self.report_overflow.into()),
            ("callbackReentries", self.callback_reentries.into()),
            (
                "failure",
                self.failure
                    .as_deref()
                    .map(JsValue::from_str)
                    .unwrap_or(JsValue::NULL),
            ),
        ]);
        div()
            .id("font-fallback-smoke")
            .size_full()
            .flex()
            .flex_col()
            .p_3()
            .gap_2()
            .font_family("Inter Variable")
            .bg(rgb(0x101318))
            .text_color(rgb(0xf0f4fa))
            .text_size(px(14.0))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key))
            .child("Public browser font fallback")
            .child("F1 emoji | F2 spider | F3 mixed | F4 size | F5 italic | F6 mono")
            .child("F7 install Regular | F8 install Bold | F9 redraw")
            .child(format!("Phase: {phase} | Policy query: {}", self.policy_query))
            .child(
                canvas(
                    move |_, window, _| {
                        let mut lines = Vec::with_capacity(samples.len());
                        let rows = js_sys::Array::new();
                        for sample in samples {
                            let (line, row) = shape_sample(sample, text_system.as_ref(), window);
                            lines.push(line);
                            rows.push(&row);
                        }
                        let painted = Rc::new(Cell::new(0_u32));
                        let committed_painted = painted.clone();
                        let row_count = lines.len() as u32;
                        window.record_prepaint_window_transaction(
                            publication,
                            move |fence, _, _| {
                                let probe = super::js_object(&[
                                    ("ready", (committed_painted.get() == row_count).into()),
                                    ("observation", "accepted-app-model".into()),
                                    ("acceptedGeneration", (fence.generation() as f64).into()),
                                    ("paintedLines", committed_painted.get().into()),
                                    ("rows", rows.clone().into()),
                                    ("model", snapshot.clone()),
                                    (
                                        "evidence",
                                        "Public platform layout and ShapedLine atlas paint in an accepted frame; no display pixel or performance assertion".into(),
                                    ),
                                ]);
                                graphics.publish(&probe);
                                publish(&probe);
                            },
                            move |fence, _, _| {
                                publish(&super::js_object(&[
                                    ("ready", false.into()),
                                    ("observation", "discarded-app-publication".into()),
                                    ("acceptedGeneration", (fence.generation() as f64).into()),
                                ]));
                            },
                        );
                        (lines, painted)
                    },
                    move |bounds, (lines, painted), window, cx| {
                        for (row, line) in lines.iter().enumerate() {
                            line.paint(
                                point(bounds.left(), bounds.top() + px(row as f32 * 68.0)),
                                px(56.0),
                                TextAlign::Left,
                                None,
                                window,
                                cx,
                            )
                            .expect("font fallback sample must paint through the glyph atlas");
                            painted.set(painted.get() + 1);
                        }
                    },
                )
                .w_full()
                .h(px(300.0)),
            )
    }
}

pub(super) fn requested() -> bool {
    web_sys::window()
        .and_then(|window| window.location().search().ok())
        .and_then(|query| web_sys::UrlSearchParams::new_with_str(&query).ok())
        .is_some_and(|params| params.get("font_fallback_case").as_deref() == Some("1"))
}

pub(super) fn open(
    cx: &mut App,
    graphics: super::GraphicsProbe,
    text_system: Arc<dyn PlatformTextSystem>,
) {
    let policy_query = web_sys::window()
        .and_then(|window| window.location().search().ok())
        .and_then(|query| web_sys::UrlSearchParams::new_with_str(&query).ok())
        .and_then(|params| params.get("font_fallback"))
        .unwrap_or_else(|| "default".into());
    let bounds = Bounds::centered(None, size(px(800.0), px(600.0)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        move |window, cx| {
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            let view = cx.new(|_| FontFallbackSmoke {
                focus,
                graphics,
                text_system,
                publication: PrepaintPublicationId::new(),
                phase: Phase::Bundled,
                policy_query,
                command_revision: 0,
                installations: 0,
                renders: 0,
                reports: Vec::new(),
                report_overflow: false,
                callback_reentries: 0,
                failure: None,
                _missing_subscription: None,
            });
            let weak_view = view.downgrade();
            let subscription = cx.on_missing_glyphs(move |missing, cx| {
                let _ = weak_view.update(cx, |view, cx| view.observe_missing(missing, cx));
            });
            view.update(cx, |view, _| {
                view._missing_subscription = Some(subscription)
            });
            view
        },
    )
    .expect("font fallback smoke window must open");
    cx.activate(true);
}
