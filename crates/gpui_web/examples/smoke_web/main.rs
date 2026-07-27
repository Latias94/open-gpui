use open_gpui::prelude::*;
use open_gpui::{
    App, Bounds, Context, DispatchPhase, FocusHandle, InteractiveElement, KeyDownEvent,
    MouseButton, MouseDownEvent, PointerCancelEvent, PointerCancelReason, PointerCaptureHandle,
    TargetedEvent, Window, WindowBounds, WindowOptions, canvas, div, px, rgb, size,
};
use open_gpui_docking::prelude::{
    DockPanelPlacement, DockSurface, DockSurfaceViewportOpenOutcome,
    DockSurfaceViewportReadinessStatus, DockSurfaceViewportUnavailable,
};
use wasm_bindgen::JsValue;

struct DockingProbe {
    readiness: &'static str,
    outcome: &'static str,
    opened: bool,
    window_delta: u64,
    registered_spaces: u64,
}

struct SmokeWeb {
    focus_handle: FocusHandle,
    pointer_capture: PointerCaptureHandle,
    click_events: u64,
    key_events: u64,
    pointer_capture_requests: u64,
    pointer_move_events: u64,
    pointer_cancel_events: u64,
    platform_capture_lost_events: u64,
    window_deactivated_events: u64,
    shell_interactions: u64,
    platform_viewport_windows: bool,
    docking_probe: DockingProbe,
}

impl SmokeWeb {
    fn new(
        platform_viewport_windows: bool,
        docking_probe: DockingProbe,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let smoke = Self {
            focus_handle: cx.focus_handle(),
            pointer_capture: window.new_pointer_capture_handle(),
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
        };
        smoke.publish_probe();
        smoke
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
        let _ = js_sys::Reflect::set(&probe, &"ready".into(), &JsValue::from_bool(true));
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
        let _ = js_sys::Reflect::set(
            &probe,
            &"dockingViewportReadiness".into(),
            &JsValue::from_str(self.docking_probe.readiness),
        );
        let _ = js_sys::Reflect::set(
            &probe,
            &"dockingViewportOutcome".into(),
            &JsValue::from_str(self.docking_probe.outcome),
        );
        let _ = js_sys::Reflect::set(
            &probe,
            &"dockingViewportOpened".into(),
            &JsValue::from_bool(self.docking_probe.opened),
        );
        let _ = js_sys::Reflect::set(
            &probe,
            &"dockingViewportWindowDelta".into(),
            &JsValue::from_f64(self.docking_probe.window_delta as f64),
        );
        let _ = js_sys::Reflect::set(
            &probe,
            &"dockingViewportRegisteredSpaces".into(),
            &JsValue::from_f64(self.docking_probe.registered_spaces as f64),
        );
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.publish_probe();
        let focus_handle = self.focus_handle.clone();
        let pointer_capture = self.pointer_capture;
        let cancel_probe = cx.entity().downgrade();
        let cancel_listener = canvas(
            |_, _, _| (),
            move |_, _, window, _| {
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
                    window
                        .capture_pointer(&pointer_capture, MouseButton::Left)
                        .expect("web smoke pointer owner must capture after pointer down");
                    this.record_pointer_capture_request();
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, _, _, _| this.record_pointer_move()))
            .on_click(cx.listener(move |this, _event, window, cx| {
                this.record_click();
                this.focus_handle.focus(window, cx);
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                this.record_key(event);
                cx.notify();
            }))
            .child(cancel_listener)
            .child(
                div()
                    .id("smoke-shell")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_4()
                    .rounded(px(6.0))
                    .bg(rgb(0x1b222c))
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
                        self.docking_probe.outcome
                    )),
            )
    }
}

struct SmokeDockPanel;

impl Render for SmokeDockPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn smoke_dock_panel(cx: &mut App) -> open_gpui::AnyView {
    cx.new(|_| SmokeDockPanel).into()
}

fn docking_probe(cx: &mut App) -> DockingProbe {
    let surface = DockSurface::builder("main")
        .panel_placements([DockPanelPlacement::center("editor")])
        .panel_factory("editor", "Editor", smoke_dock_panel)
        .allow_platform_viewports(true)
        .build(cx)
        .expect("smoke docking surface should validate");
    let spec = open_gpui_docking::prelude::DockSurfaceViewportSpec::new(
        "main",
        WindowOptions::default(),
    );
    let readiness = surface.viewports().readiness(&spec, cx);
    let readiness = match readiness.status() {
        DockSurfaceViewportReadinessStatus::Openable => "openable",
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
        DockSurfaceViewportOpenOutcome::Unavailable(DockSurfaceViewportUnavailable::OpenFailed(
            _,
        )) => "open_failed",
    };

    DockingProbe {
        readiness,
        outcome,
        opened,
        window_delta: after_windows.saturating_sub(before_windows) as u64,
        registered_spaces: surface.registered_viewport_spaces(cx).len() as u64,
    }
}

fn main() {
    open_gpui_platform::web_init();
    open_gpui_platform::single_threaded_web_with_options(
        open_gpui_platform::WebPlatformOptions {
            force_fallback_adapter: true,
            ..Default::default()
        },
    )
    .run(|cx: &mut App| {
        let platform_viewport_windows = cx.viewport_capabilities().platform_viewport_windows;
        let docking_probe = docking_probe(cx);
        let bounds = Bounds::centered(None, size(px(640.0), px(420.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| SmokeWeb::new(platform_viewport_windows, docking_probe, window, cx))
            },
        )
        .expect("failed to open smoke window");
        cx.activate(true);
    });
}
