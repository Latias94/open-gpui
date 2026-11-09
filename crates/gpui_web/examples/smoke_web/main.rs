use std::{cell::Cell, rc::Rc, sync::Arc, time::Duration};

use open_gpui::prelude::*;
use open_gpui::{
    App, Application, Bounds, Context, DispatchPhase, FocusHandle, Image, ImageFormat,
    InteractiveElement, KeyDownEvent, MouseButton, MouseDownEvent, PathBuilder, PointerCancelEvent,
    PointerCancelReason, PointerCaptureHandle, RenderImage, SubtreeTransform, SubtreeTransformExt,
    SubtreeTransformOrigin, SvgRenderer, TargetedEvent, Window, WindowActivationPolicy,
    WindowBounds, WindowOptions, canvas, div, fill, img, point, px, rgb, rgba, size,
};
use open_gpui_docking::prelude::{
    DockPanelPlacement, DockSurface, DockSurfacePrimaryWindowOpenOutcome,
    DockSurfaceViewportOpenOutcome, DockSurfaceViewportReadinessStatus,
    DockSurfaceViewportUnavailable, DockSurfaceWindowSessionStatus,
};
use open_gpui_web::{WebPlatform, WebPlatformOptions};
use open_gpui_wgpu::{WebBackendPreference, WgpuBackend};
use wasm_bindgen::JsValue;

mod clipboard;

const GRID_COLUMNS: usize = 128;
const GRID_ROWS: usize = 128;
const GRID_CELL: f32 = 2.0;
const GRID_RED: u32 = 0xe63946;
const GRID_CYAN: u32 = 0x2ec4b6;

#[derive(Clone, Copy)]
struct BrowserConfig {
    requested_backend: &'static str,
    backend_preference: WebBackendPreference,
    force_fallback_adapter: bool,
    renderer: bool,
}

impl BrowserConfig {
    fn from_query() -> Self {
        let params = web_sys::window()
            .and_then(|window| window.location().search().ok())
            .and_then(|search| web_sys::UrlSearchParams::new_with_str(&search).ok());
        let requested = params.as_ref().and_then(|params| params.get("backend"));
        let (requested_backend, backend_preference) = match requested.as_deref() {
            None | Some("auto") => ("auto", WebBackendPreference::Auto),
            Some("webgpu") => ("webgpu", WebBackendPreference::WebGpu),
            Some("webgl") => ("webgl", WebBackendPreference::WebGl),
            Some(value) => panic!("unsupported smoke backend: {value}"),
        };
        Self {
            requested_backend,
            backend_preference,
            force_fallback_adapter: params
                .as_ref()
                .and_then(|params| params.get("software"))
                .as_deref()
                != Some("false"),
            renderer: params
                .as_ref()
                .and_then(|params| params.get("renderer"))
                .as_deref()
                == Some("1"),
        }
    }
}

#[derive(Clone, Copy)]
struct GraphicsProbe {
    config: BrowserConfig,
    actual_backend: Option<WgpuBackend>,
}

impl GraphicsProbe {
    fn publish(&self, probe: &JsValue) {
        for (name, value) in [
            ("requestedBackend", self.config.requested_backend.into()),
            (
                "graphicsBackend",
                match self.actual_backend {
                    Some(WgpuBackend::BrowserWebGpu) => JsValue::from_str("WebGPU"),
                    Some(WgpuBackend::Gl) => JsValue::from_str("Gl"),
                    Some(WgpuBackend::Native(_)) => JsValue::from_str("Native"),
                    None => JsValue::NULL,
                },
            ),
            (
                "forceFallbackAdapter",
                self.config.force_fallback_adapter.into(),
            ),
            ("rendererMode", self.config.renderer.into()),
        ] {
            let _ = js_sys::Reflect::set(probe, &name.into(), &value);
        }
    }
}

#[derive(Default)]
struct FrameProbe {
    render_calls: Cell<u64>,
    paint_calls: Cell<u64>,
    painted_revision: Cell<u64>,
    dense_paint_calls: Cell<u64>,
    dense_quad_requests: Cell<u64>,
    viewport: Cell<[f32; 2]>,
    scale_factor: Cell<f32>,
    paint_bounds: Cell<[f32; 4]>,
}

impl FrameProbe {
    fn publish(&self, probe: &JsValue) {
        let [width, height] = self.viewport.get();
        let [left, top, paint_width, paint_height] = self.paint_bounds.get();
        let stage = if self.paint_calls.get() > 0
            && self.painted_revision.get() == self.render_calls.get()
        {
            "paint-callback"
        } else if self.render_calls.get() > 0 {
            "root-rendered"
        } else {
            "window-created"
        };
        for (name, value) in [
            ("stage", stage.into()),
            ("frameRevision", (self.render_calls.get() as f64).into()),
            (
                "paintedRevision",
                (self.painted_revision.get() as f64).into(),
            ),
            ("renderCalls", (self.render_calls.get() as f64).into()),
            ("paintCalls", (self.paint_calls.get() as f64).into()),
            (
                "densePaintCalls",
                (self.dense_paint_calls.get() as f64).into(),
            ),
            (
                "denseQuadRequests",
                (self.dense_quad_requests.get() as f64).into(),
            ),
            (
                "viewport",
                js_object(&[
                    ("width", width.into()),
                    ("height", height.into()),
                    ("scaleFactor", self.scale_factor.get().into()),
                    (
                        "paintBounds",
                        js_object(&[
                            ("left", left.into()),
                            ("top", top.into()),
                            ("width", paint_width.into()),
                            ("height", paint_height.into()),
                        ]),
                    ),
                ]),
            ),
            (
                "counterScope",
                "Application render and paint callbacks, not GPU submission or presentation".into(),
            ),
        ] {
            let _ = js_sys::Reflect::set(probe, &name.into(), &value);
        }
    }

    fn publish_current(&self) {
        if let Some(window) = web_sys::window()
            && let Ok(probe) =
                js_sys::Reflect::get(window.as_ref(), &"__OPEN_GPUI_WEB_SMOKE__".into())
            && probe.is_object()
        {
            self.publish(&probe);
        }
    }
}

fn js_object(fields: &[(&str, JsValue)]) -> JsValue {
    let object = js_sys::Object::new();
    for (name, value) in fields {
        let _ = js_sys::Reflect::set(&object, &JsValue::from_str(name), value);
    }
    object.into()
}

fn publish_startup_probe(graphics: GraphicsProbe, stage: &str) {
    if let Some(window) = web_sys::window() {
        let probe = js_object(&[("ready", false.into()), ("stage", stage.into())]);
        graphics.publish(&probe);
        let _ = js_sys::Reflect::set(window.as_ref(), &"__OPEN_GPUI_WEB_SMOKE__".into(), &probe);
    }
}

#[derive(Clone, Copy)]
struct DockingProbe {
    readiness: &'static str,
    outcome: &'static str,
    opened: bool,
    window_delta: u64,
    registered_spaces: u64,
    app_window_count: u64,
    session_status: DockSurfaceWindowSessionStatus,
}

#[derive(Clone, Copy)]
struct ActivationProbeConfig {
    mode: &'static str,
    policy: WindowActivationPolicy,
}

struct ActivationProbeState {
    config: ActivationProbeConfig,
    programmatic_attempted: bool,
    creation_focus_on_appearing: Option<bool>,
    observed_accepts_activation: Option<bool>,
    observed_focus_on_click: Option<bool>,
    observed_active: Option<bool>,
}

struct SmokeWeb {
    focus_handle: FocusHandle,
    pointer_capture: Option<PointerCaptureHandle>,
    click_events: u64,
    key_events: u64,
    pointer_capture_requests: u64,
    pointer_move_events: u64,
    pointer_cancel_events: u64,
    platform_capture_lost_events: u64,
    window_deactivated_events: u64,
    shell_interactions: u64,
    platform_viewport_windows: bool,
    docking_probe: Rc<Cell<Option<DockingProbe>>>,
    activation_probe: Option<ActivationProbeState>,
    graphics_probe: GraphicsProbe,
    frame_probe: Rc<FrameProbe>,
    renderer_image: Option<Arc<RenderImage>>,
    renderer_expectations: Option<JsValue>,
}

impl SmokeWeb {
    fn new(
        platform_viewport_windows: bool,
        docking_probe: Rc<Cell<Option<DockingProbe>>>,
        activation_probe: Option<ActivationProbeConfig>,
        graphics_probe: GraphicsProbe,
        cx: &mut Context<Self>,
    ) -> Self {
        let smoke = Self {
            focus_handle: cx.focus_handle(),
            pointer_capture: None,
            click_events: 0,
            key_events: 0,
            pointer_capture_requests: 0,
            pointer_move_events: 0,
            pointer_cancel_events: 0,
            platform_capture_lost_events: 0,
            window_deactivated_events: 0,
            shell_interactions: 0,
            platform_viewport_windows,
            docking_probe,
            activation_probe: activation_probe.map(|config| ActivationProbeState {
                config,
                programmatic_attempted: false,
                creation_focus_on_appearing: None,
                observed_accepts_activation: None,
                observed_focus_on_click: None,
                observed_active: None,
            }),
            graphics_probe,
            frame_probe: Rc::new(FrameProbe::default()),
            renderer_image: graphics_probe.config.renderer.then(renderer_image),
            renderer_expectations: graphics_probe.config.renderer.then(renderer_expectations),
        };
        smoke.publish_probe();
        smoke
    }

    fn bind_window(&mut self, window: &mut Window, cx: &mut Context<Self>) -> PointerCaptureHandle {
        if let Some(capture) = self.pointer_capture {
            return capture;
        }
        let capture = window.new_pointer_capture_handle();
        self.pointer_capture = Some(capture);
        if self.activation_probe.is_some() {
            cx.spawn_in(window, async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                this.update_in(cx, |this, window, cx| {
                    if let Some(probe) = this.activation_probe.as_mut() {
                        probe.programmatic_attempted = true;
                    }
                    let _ = window.activate_window();
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        capture
    }

    fn sync_activation_probe(&mut self, window: &Window) {
        let Some(probe) = self.activation_probe.as_mut() else {
            return;
        };
        probe.creation_focus_on_appearing = Some(window.creation_facts().focus_on_appearing);
        let facts = window.platform_facts();
        probe.observed_accepts_activation = Some(facts.accepts_activation);
        probe.observed_focus_on_click = Some(facts.focus_on_click);
        probe.observed_active = Some(facts.is_active);
    }

    fn record_click(&mut self) {
        self.click_events += 1;
        self.shell_interactions += 1;
        self.publish_probe();
    }

    fn record_key(&mut self, event: &KeyDownEvent) {
        if event.keystroke.key == "s" {
            self.shell_interactions += 1;
        }
        self.key_events += 1;
        self.publish_probe();
    }

    fn record_pointer_capture_request(&mut self) {
        self.pointer_capture_requests += 1;
        self.publish_probe();
    }

    fn record_pointer_move(&mut self) {
        self.pointer_move_events += 1;
        self.publish_probe();
    }

    fn record_pointer_cancel(&mut self, reason: PointerCancelReason) {
        self.pointer_cancel_events += 1;
        if reason == PointerCancelReason::PlatformCaptureLost {
            self.platform_capture_lost_events += 1;
        }
        if reason == PointerCancelReason::WindowDeactivated {
            self.window_deactivated_events += 1;
        }
        self.publish_probe();
    }

    fn publish_probe(&self) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let probe = js_sys::Object::new();
        let _ = js_sys::Reflect::set(
            &probe,
            &"ready".into(),
            &JsValue::from_bool(
                self.pointer_capture.is_some() && self.docking_probe.get().is_some(),
            ),
        );
        self.graphics_probe.publish(probe.as_ref());
        self.frame_probe.publish(probe.as_ref());
        if let Some(expectations) = &self.renderer_expectations {
            let _ = js_sys::Reflect::set(&probe, &"rendererExpectations".into(), expectations);
        }
        let _ = js_sys::Reflect::set(
            &probe,
            &"clickEvents".into(),
            &JsValue::from_f64(self.click_events as f64),
        );
        let _ = js_sys::Reflect::set(
            &probe,
            &"keyEvents".into(),
            &JsValue::from_f64(self.key_events as f64),
        );
        let _ = js_sys::Reflect::set(
            &probe,
            &"pointerCaptureRequests".into(),
            &JsValue::from_f64(self.pointer_capture_requests as f64),
        );
        let _ = js_sys::Reflect::set(
            &probe,
            &"pointerMoveEvents".into(),
            &JsValue::from_f64(self.pointer_move_events as f64),
        );
        let _ = js_sys::Reflect::set(
            &probe,
            &"pointerCancelEvents".into(),
            &JsValue::from_f64(self.pointer_cancel_events as f64),
        );
        let _ = js_sys::Reflect::set(
            &probe,
            &"platformCaptureLostEvents".into(),
            &JsValue::from_f64(self.platform_capture_lost_events as f64),
        );
        let _ = js_sys::Reflect::set(
            &probe,
            &"windowDeactivatedEvents".into(),
            &JsValue::from_f64(self.window_deactivated_events as f64),
        );
        let _ = js_sys::Reflect::set(
            &probe,
            &"shellInteractions".into(),
            &JsValue::from_f64(self.shell_interactions as f64),
        );
        let _ = js_sys::Reflect::set(
            &probe,
            &"platformViewportWindows".into(),
            &JsValue::from_bool(self.platform_viewport_windows),
        );
        if let Some(docking) = self.docking_probe.get() {
            let _ = js_sys::Reflect::set(
                &probe,
                &"dockingViewportReadiness".into(),
                &JsValue::from_str(docking.readiness),
            );
            let _ = js_sys::Reflect::set(
                &probe,
                &"dockingViewportOutcome".into(),
                &JsValue::from_str(docking.outcome),
            );
            let _ = js_sys::Reflect::set(
                &probe,
                &"dockingViewportOpened".into(),
                &JsValue::from_bool(docking.opened),
            );
            let _ = js_sys::Reflect::set(
                &probe,
                &"dockingViewportWindowDelta".into(),
                &JsValue::from_f64(docking.window_delta as f64),
            );
            let _ = js_sys::Reflect::set(
                &probe,
                &"dockingViewportRegisteredSpaces".into(),
                &JsValue::from_f64(docking.registered_spaces as f64),
            );
            for (name, value) in [
                ("appWindowCount", (docking.app_window_count as f64).into()),
                (
                    "dockingSessionPhase",
                    format!("{:?}", docking.session_status.phase()).into(),
                ),
                (
                    "dockingSessionGeneration",
                    (docking.session_status.generation() as f64).into(),
                ),
            ] {
                let _ = js_sys::Reflect::set(&probe, &name.into(), &value);
            }
        }
        if let Some(activation) = &self.activation_probe {
            let _ = js_sys::Reflect::set(
                &probe,
                &"activationMode".into(),
                &JsValue::from_str(activation.config.mode),
            );
            let _ = js_sys::Reflect::set(
                &probe,
                &"requestedAcceptsActivation".into(),
                &JsValue::from_bool(activation.config.policy.accepts_activation),
            );
            let _ = js_sys::Reflect::set(
                &probe,
                &"requestedFocusOnClick".into(),
                &JsValue::from_bool(activation.config.policy.focus_on_click),
            );
            let _ = js_sys::Reflect::set(
                &probe,
                &"programmaticActivationAttempted".into(),
                &JsValue::from_bool(activation.programmatic_attempted),
            );
            for (name, value) in [
                (
                    "creationFocusOnAppearing",
                    activation.creation_focus_on_appearing,
                ),
                (
                    "observedAcceptsActivation",
                    activation.observed_accepts_activation,
                ),
                ("observedFocusOnClick", activation.observed_focus_on_click),
                ("observedActive", activation.observed_active),
            ] {
                if let Some(value) = value {
                    let _ = js_sys::Reflect::set(
                        &probe,
                        &JsValue::from_str(name),
                        &JsValue::from_bool(value),
                    );
                }
            }
        }
        let _ = js_sys::Reflect::set(
            window.as_ref(),
            &"__OPEN_GPUI_WEB_SMOKE__".into(),
            probe.as_ref(),
        );

        if let Some(document) = window.document() {
            if let Some(body) = document.body() {
                body.set_attribute("data-open-gpui-web-smoke-ready", "true")
                    .ok();
            }
        }
    }
}

impl Render for SmokeWeb {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pointer_capture = self.bind_window(window, cx);
        let revision = self.frame_probe.render_calls.get() + 1;
        self.frame_probe.render_calls.set(revision);
        let viewport = window.viewport_size();
        self.frame_probe
            .viewport
            .set([f32::from(viewport.width), f32::from(viewport.height)]);
        self.frame_probe.scale_factor.set(window.scale_factor());
        self.sync_activation_probe(window);
        self.publish_probe();
        let focus_handle = self.focus_handle.clone();
        let cancel_probe = cx.entity().downgrade();
        let frame_probe = self.frame_probe.clone();
        let cancel_listener = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                frame_probe
                    .paint_calls
                    .set(frame_probe.paint_calls.get() + 1);
                frame_probe.painted_revision.set(revision);
                let viewport = window.viewport_size();
                frame_probe
                    .viewport
                    .set([f32::from(viewport.width), f32::from(viewport.height)]);
                frame_probe.scale_factor.set(window.scale_factor());
                frame_probe.paint_bounds.set([
                    f32::from(bounds.origin.x),
                    f32::from(bounds.origin.y),
                    f32::from(bounds.size.width),
                    f32::from(bounds.size.height),
                ]);
                frame_probe.publish_current();
                let cancel_probe = cancel_probe.clone();
                window.on_pointer_cancel(move |event: &PointerCancelEvent, phase, _, cx| {
                    if phase != DispatchPhase::Bubble {
                        return;
                    }
                    cancel_probe
                        .update(cx, |this, cx| {
                            this.record_pointer_cancel(event.reason);
                            cx.notify();
                        })
                        .ok();
                });
            },
        )
        .absolute()
        .size_full();

        div()
            .id("smoke-root")
            .relative()
            .size_full()
            .bg(rgb(0x101318))
            .text_color(rgb(0xe5edf5))
            .flex()
            .items_center()
            .justify_center()
            .track_focus(&focus_handle)
            .track_pointer_capture(&pointer_capture)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &TargetedEvent<MouseDownEvent>, window, cx| {
                    if this.activation_probe.is_none() {
                        window
                            .capture_pointer(&pointer_capture, MouseButton::Left)
                            .expect("web smoke pointer owner must capture after pointer down");
                        this.record_pointer_capture_request();
                        cx.notify();
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, _, _, _| this.record_pointer_move()))
            .on_click(cx.listener(move |this, _event, window, cx| {
                this.record_click();
                if this.activation_probe.is_none() {
                    this.focus_handle.focus(window, cx);
                }
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                this.record_key(event);
                cx.notify();
            }))
            .child(cancel_listener)
            .when_some(self.renderer_image.clone(), |root, image| {
                root.child(renderer_scene(image, self.frame_probe.clone()))
            })
            .child(
                div()
                    .id("smoke-shell")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_4()
                    .rounded(px(6.0))
                    .bg(rgb(0x1b222c))
                    .when(self.graphics_probe.config.renderer, |shell| {
                        shell.absolute().left(px(16.0)).top(px(320.0))
                    })
                    .child("Open GPUI Web Smoke")
                    .child(format!(
                        "clicks={} keys={} shell={} platform_viewports={} docking={}",
                        self.click_events,
                        self.key_events,
                        self.shell_interactions,
                        if self.platform_viewport_windows {
                            "supported"
                        } else {
                            "unsupported"
                        },
                        self.docking_probe
                            .get()
                            .map(|probe| probe.outcome)
                            .unwrap_or("opening")
                    )),
            )
    }
}

fn renderer_image() -> Arc<RenderImage> {
    let mut bytes = b"P6\n4 4\n255\n".to_vec();
    for row in 0..4 {
        for column in 0..4 {
            let pixel = match (row < 2, column < 2) {
                (true, true) => [255, 0, 0],
                (true, false) => [0, 255, 0],
                (false, true) => [0, 0, 255],
                (false, false) => [255, 255, 255],
            };
            bytes.extend_from_slice(&pixel);
        }
    }
    Image::from_bytes(ImageFormat::Pnm, bytes)
        .to_image_data(SvgRenderer::new(Arc::new(())))
        .expect("deterministic smoke PNM must decode")
}

fn renderer_scene(image: Arc<RenderImage>, frame_probe: Rc<FrameProbe>) -> impl IntoElement {
    let grid = canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            for row in 0..GRID_ROWS {
                for column in 0..GRID_COLUMNS {
                    let origin = point(
                        bounds.origin.x + px(column as f32 * GRID_CELL),
                        bounds.origin.y + px(row as f32 * GRID_CELL),
                    );
                    let color = if (row + column) % 2 == 0 {
                        GRID_RED
                    } else {
                        GRID_CYAN
                    };
                    window.paint_quad(fill(
                        Bounds::new(origin, size(px(GRID_CELL), px(GRID_CELL))),
                        rgb(color),
                    ));
                }
            }
            frame_probe
                .dense_paint_calls
                .set(frame_probe.dense_paint_calls.get() + 1);
            frame_probe
                .dense_quad_requests
                .set(frame_probe.dense_quad_requests.get() + (GRID_COLUMNS * GRID_ROWS) as u64);
            frame_probe.publish_current();
        },
    )
    .absolute()
    .left(px(16.0))
    .top(px(16.0))
    .w(px(GRID_COLUMNS as f32 * GRID_CELL))
    .h(px(GRID_ROWS as f32 * GRID_CELL));
    let nested_clip = div()
        .absolute()
        .left(px(304.0))
        .top(px(16.0))
        .w(px(144.0))
        .h(px(112.0))
        .rounded_tl(px(28.0))
        .rounded_tr(px(8.0))
        .rounded_br(px(24.0))
        .rounded_bl(px(4.0))
        .overflow_hidden()
        .bg(rgb(0x26354a))
        .child(
            div()
                .absolute()
                .left(px(12.0))
                .top(px(10.0))
                .w(px(128.0))
                .h(px(96.0))
                .rounded_tl(px(4.0))
                .rounded_tr(px(24.0))
                .rounded_br(px(6.0))
                .rounded_bl(px(20.0))
                .overflow_hidden()
                .child(
                    div()
                        .absolute()
                        .left(px(-12.0))
                        .top(px(-10.0))
                        .w(px(160.0))
                        .h(px(120.0))
                        .bg(rgb(0xf4b942)),
                ),
        );
    let transformed = div()
        .absolute()
        .left(px(304.0))
        .top(px(152.0))
        .w(px(96.0))
        .h(px(64.0))
        .rounded_tl(px(16.0))
        .rounded_tr(px(4.0))
        .rounded_br(px(12.0))
        .rounded_bl(px(2.0))
        .overflow_hidden()
        .bg(rgb(0x8f5bd7))
        .child(
            div()
                .absolute()
                .left(px(16.0))
                .top(px(16.0))
                .w(px(24.0))
                .h(px(24.0))
                .bg(rgb(0x4ade80)),
        )
        .with_subtree_transform(
            SubtreeTransform::try_new(
                size(1.5, 0.75),
                point(px(8.0), px(4.0)),
                SubtreeTransformOrigin::TOP_LEFT,
            )
            .expect("fixed smoke transform must be finite and invertible"),
        );
    let path = canvas(
        |_, _, _| (),
        |bounds, _, window, _| {
            let mut path = PathBuilder::fill();
            path.move_to(bounds.origin);
            path.line_to(point(bounds.origin.x + px(64.0), bounds.origin.y));
            path.line_to(point(
                bounds.origin.x + px(32.0),
                bounds.origin.y + px(48.0),
            ));
            path.close();
            window.paint_path(
                path.build().expect("fixed smoke triangle must tessellate"),
                rgb(0xf472b6),
            );
        },
    )
    .absolute()
    .left(px(480.0))
    .top(px(184.0))
    .w(px(64.0))
    .h(px(48.0));

    div()
        .absolute()
        .left_0()
        .top_0()
        .w(px(640.0))
        .h(px(304.0))
        .child(grid)
        .child(nested_clip)
        .child(transformed)
        .child(
            div()
                .absolute()
                .left(px(480.0))
                .top(px(16.0))
                .w(px(64.0))
                .h(px(48.0))
                .bg(rgb(0x0000ff))
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .size_full()
                        .bg(rgba(0xff000080)),
                ),
        )
        .child(
            img(image)
                .absolute()
                .left(px(480.0))
                .top(px(88.0))
                .w(px(64.0))
                .h(px(64.0)),
        )
        .child(path)
        .child(
            div()
                .absolute()
                .left(px(304.0))
                .top(px(240.0))
                .w(px(256.0))
                .h(px(28.0))
                .font_family("Inter")
                .text_size(px(18.0))
                .line_height(px(24.0))
                .text_color(rgb(0xffffff))
                .child("GPUI packed data"),
        )
        .children(
            [(288.0, 0xffffff), (320.0, 0x4ade80), (352.0, 0xf472b6)].map(|(left, color)| {
                div()
                    .absolute()
                    .left(px(left))
                    .top(px(280.0))
                    .w(px(24.0))
                    .h(px(8.0))
                    .bg(rgb(color))
            }),
        )
}

fn renderer_expectations() -> JsValue {
    js_sys::JSON::parse(
        r##"{
            "schemaVersion": 1,
            "coordinateSpace": "Smoke-panel-local logical pixels; add viewport.paintBounds.left/top for canvas coordinates, then multiply by viewport.scaleFactor for device pixels",
            "sceneSize": [640, 304],
            "recommendedViewport": [640, 420],
            "background": "#101318",
            "denseGrid": {
                "bounds": [16, 16, 256, 256],
                "columns": 128,
                "rows": 128,
                "cellSize": 2,
                "quadCount": 16384,
                "evenColor": "#e63946",
                "oddColor": "#2ec4b6"
            },
            "nestedClip": {
                "outerBounds": [304, 16, 144, 112],
                "outerRadiiTopLeftClockwise": [28, 8, 24, 4],
                "outerColor": "#26354a",
                "innerBounds": [316, 26, 128, 96],
                "innerRadiiTopLeftClockwise": [4, 24, 6, 20],
                "childBounds": [304, 16, 160, 120],
                "childColor": "#f4b942"
            },
            "transform": {
                "sourceBounds": [304, 152, 96, 64],
                "scale": [1.5, 0.75],
                "translation": [8, 4],
                "origin": "top-left",
                "displayedBounds": [312, 156, 144, 48],
                "displayedChildBounds": [336, 168, 36, 18]
            },
            "alphaLayers": {
                "bounds": [480, 16, 64, 48],
                "backgroundRgba": [0, 0, 255, 255],
                "foregroundRgba": [255, 0, 0, 128]
            },
            "image": {
                "bounds": [480, 88, 64, 64],
                "sourceSize": [4, 4],
                "quadrants": ["#ff0000", "#00ff00", "#0000ff", "#ffffff"]
            },
            "path": {
                "vertices": [[480, 184], [544, 184], [512, 232]],
                "color": "#f472b6"
            },
            "text": {
                "bounds": [304, 240, 256, 28],
                "fontFamily": "Inter",
                "fontSize": 18,
                "text": "GPUI packed data",
                "color": "#ffffff",
                "background": "#101318"
            },
            "pixelSamples": [
                {"name": "grid-first", "at": [17, 17], "rgba": [230, 57, 70, 255]},
                {"name": "grid-next", "at": [19, 17], "rgba": [46, 196, 182, 255]},
                {"name": "grid-row", "at": [17, 19], "rgba": [46, 196, 182, 255]},
                {"name": "grid-middle", "at": [145, 145], "rgba": [230, 57, 70, 255]},
                {"name": "grid-last", "at": [271, 271], "rgba": [230, 57, 70, 255]},
                {"name": "outer-rounded-cut", "at": [306, 18], "rgba": [16, 19, 24, 255]},
                {"name": "outer-only", "at": [310, 80], "rgba": [38, 53, 74, 255]},
                {"name": "inner-fill", "at": [340, 70], "rgba": [244, 185, 66, 255]},
                {"name": "inner-rounded-cut", "at": [440, 28], "rgba": [38, 53, 74, 255]},
                {"name": "transform-outside", "at": [310, 180], "rgba": [16, 19, 24, 255]},
                {"name": "transform-background", "at": [320, 180], "rgba": [143, 91, 215, 255]},
                {"name": "transform-child", "at": [350, 176], "rgba": [74, 222, 128, 255]},
                {"name": "alpha-over", "at": [512, 40], "rgba": [128, 0, 127, 255]},
                {"name": "image-red", "at": [496, 104], "rgba": [255, 0, 0, 255]},
                {"name": "image-green", "at": [528, 104], "rgba": [0, 255, 0, 255]},
                {"name": "image-blue", "at": [496, 136], "rgba": [0, 0, 255, 255]},
                {"name": "image-white", "at": [528, 136], "rgba": [255, 255, 255, 255]},
                {"name": "path-fill", "at": [512, 200], "rgba": [244, 114, 182, 255]},
                {"name": "path-outside", "at": [482, 228], "rgba": [16, 19, 24, 255]},
                {"name": "white-marker", "at": [300, 284], "rgba": [255, 255, 255, 255]},
                {"name": "green-marker", "at": [332, 284], "rgba": [74, 222, 128, 255]},
                {"name": "pink-marker", "at": [364, 284], "rgba": [244, 114, 182, 255]}
            ]
        }"##,
    )
    .expect("fixed smoke renderer expectations must be valid JSON")
}

fn docking_probe(surface: &DockSurface, cx: &mut App) -> DockingProbe {
    let spec =
        open_gpui_docking::prelude::DockSurfaceViewportSpec::new("main", WindowOptions::default());
    let readiness = surface.viewports().readiness(&spec, cx);
    let readiness = match readiness.status() {
        DockSurfaceViewportReadinessStatus::Openable => "openable",
        DockSurfaceViewportReadinessStatus::SessionInactive { .. } => "session_inactive",
        DockSurfaceViewportReadinessStatus::PolicyDisabled(_) => "policy_disabled",
        DockSurfaceViewportReadinessStatus::BackendUnsupported => "backend_unsupported",
        DockSurfaceViewportReadinessStatus::FlagUnsupported { .. } => "flag_unsupported",
        DockSurfaceViewportReadinessStatus::InvalidPlacement { .. } => "invalid_placement",
    };

    let before_windows = cx.windows().len();
    let outcome = surface.viewports().open_spec(spec, cx);
    let after_windows = cx.windows().len();
    let opened = outcome.opened();
    let outcome = match outcome {
        DockSurfaceViewportOpenOutcome::Opened(_) => "opened",
        DockSurfaceViewportOpenOutcome::Unavailable(
            DockSurfaceViewportUnavailable::SessionInactive { .. },
        ) => "session_inactive",
        DockSurfaceViewportOpenOutcome::Unavailable(
            DockSurfaceViewportUnavailable::PolicyDisabled(_),
        ) => "policy_disabled",
        DockSurfaceViewportOpenOutcome::Unavailable(
            DockSurfaceViewportUnavailable::BackendUnsupported,
        ) => "backend_unsupported",
        DockSurfaceViewportOpenOutcome::Unavailable(
            DockSurfaceViewportUnavailable::FlagUnsupported { .. },
        ) => "flag_unsupported",
        DockSurfaceViewportOpenOutcome::Unavailable(
            DockSurfaceViewportUnavailable::InvalidPlacement { .. },
        ) => "invalid_placement",
        DockSurfaceViewportOpenOutcome::Unavailable(
            DockSurfaceViewportUnavailable::OpenFailed(_),
        ) => "open_failed",
    };

    DockingProbe {
        readiness,
        outcome,
        opened,
        window_delta: after_windows.saturating_sub(before_windows) as u64,
        registered_spaces: surface.registered_viewport_spaces(cx).len() as u64,
        app_window_count: after_windows as u64,
        session_status: surface.window_session_status(cx),
    }
}

fn activation_probe_config() -> Option<ActivationProbeConfig> {
    let search = web_sys::window()?.location().search().ok()?;
    let params = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
    match params.get("activation").as_deref() {
        Some("programmatic-only") => Some(ActivationProbeConfig {
            mode: "programmatic-only",
            policy: WindowActivationPolicy {
                accepts_activation: true,
                focus_on_click: false,
            },
        }),
        Some("click-only") => Some(ActivationProbeConfig {
            mode: "click-only",
            policy: WindowActivationPolicy {
                accepts_activation: false,
                focus_on_click: true,
            },
        }),
        _ => None,
    }
}

fn main() {
    open_gpui_platform::web_init();
    let config = BrowserConfig::from_query();
    publish_startup_probe(
        GraphicsProbe {
            config,
            actual_backend: None,
        },
        "graphics-initializing",
    );
    let platform = Rc::new(WebPlatform::new_with_options(WebPlatformOptions {
        allow_multi_threading: false,
        backend_preference: config.backend_preference,
        force_fallback_adapter: config.force_fallback_adapter,
    }));
    let observed_platform = platform.clone();
    Application::with_platform(platform).run(move |cx: &mut App| {
        if clipboard::requested() {
            clipboard::open(cx);
            return;
        }
        let graphics_probe = GraphicsProbe {
            config,
            actual_backend: observed_platform.graphics_backend(),
        };
        publish_startup_probe(graphics_probe, "platform-ready");
        let platform_viewport_windows = cx.viewport_capabilities().platform_viewport_windows;
        let docking_result = Rc::new(Cell::new(None));
        let activation_probe = activation_probe_config();
        let smoke = cx.new(|cx| {
            SmokeWeb::new(
                platform_viewport_windows,
                docking_result.clone(),
                activation_probe,
                graphics_probe,
                cx,
            )
        });
        let panel = smoke.clone();
        let surface = DockSurface::builder("main")
            .panel_placements([DockPanelPlacement::center("editor")])
            .panel_factory("editor", "Editor", move |_| panel.clone().into())
            .allow_platform_viewports(true)
            .build(cx)
            .expect("smoke docking surface should validate");
        let bounds = Bounds::centered(None, size(px(640.0), px(420.0)), cx);
        let _primary = match surface.open_primary_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus_on_appearing: activation_probe.is_none(),
                activation_policy: activation_probe
                    .map(|probe| probe.policy)
                    .unwrap_or_default(),
                ..Default::default()
            },
            cx,
        ) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened,
            outcome => panic!("smoke docking primary should open: {outcome:?}"),
        };
        docking_result.set(Some(docking_probe(&surface, cx)));
        smoke.update(cx, |smoke, cx| {
            smoke.publish_probe();
            cx.notify();
        });
        cx.activate(true);
    });
}
