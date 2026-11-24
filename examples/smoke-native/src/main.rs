use open_gpui::{
    App, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use open_gpui_platform::application;

struct SmokeView {
    clicks: usize,
}

impl Render for SmokeView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(0xf8fafc))
            .child(
                div()
                    .id("smoke-counter")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0xe2e8f0))
                    .cursor_pointer()
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.clicks += 1;
                        cx.notify();
                    }))
                    .child(format!("Open GPUI smoke · {} clicks", self.clicks)),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(760.0), px(520.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| SmokeView { clicks: 0 }),
        )
        .expect("failed to open native smoke window");
        cx.activate(true);
    });
}
