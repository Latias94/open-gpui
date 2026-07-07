use open_gpui::prelude::*;
use open_gpui::{
    App, Bounds, Context, FocusHandle, InteractiveElement, KeyDownEvent, Window, WindowBounds,
    WindowOptions, div, px, rgb, size,
};
use wasm_bindgen::JsValue;

struct SmokeWeb {
    focus_handle: FocusHandle,
    click_events: u64,
    key_events: u64,
    shell_interactions: u64,
    platform_viewport_windows: bool,
}

impl SmokeWeb {
    fn new(platform_viewport_windows: bool, cx: &mut Context<Self>) -> Self {
        let smoke = Self {
            focus_handle: cx.focus_handle(),
            click_events: 0,
            key_events: 0,
            shell_interactions: 0,
            platform_viewport_windows,
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
            &"shellInteractions".into(),
            &JsValue::from_f64(self.shell_interactions as f64),
        );
        let _ = js_sys::Reflect::set(
            &probe,
            &"platformViewportWindows".into(),
            &JsValue::from_bool(self.platform_viewport_windows),
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

        div()
            .id("smoke-root")
            .size_full()
            .bg(rgb(0x101318))
            .text_color(rgb(0xe5edf5))
            .flex()
            .items_center()
            .justify_center()
            .track_focus(&focus_handle)
            .on_click(cx.listener(move |this, _event, window, cx| {
                this.record_click();
                this.focus_handle.focus(window, cx);
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                this.record_key(event);
                cx.notify();
            }))
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
                        "clicks={} keys={} shell={} platform_viewports={}",
                        self.click_events,
                        self.key_events,
                        self.shell_interactions,
                        if self.platform_viewport_windows {
                            "supported"
                        } else {
                            "unsupported"
                        }
                    )),
            )
    }
}

fn main() {
    open_gpui_platform::web_init();
    open_gpui_platform::single_threaded_web().run(|cx: &mut App| {
        let platform_viewport_windows = cx.viewport_capabilities().platform_viewport_windows;
        let bounds = Bounds::centered(None, size(px(640.0), px(420.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| SmokeWeb::new(platform_viewport_windows, cx)),
        )
        .expect("failed to open smoke window");
        cx.activate(true);
    });
}
