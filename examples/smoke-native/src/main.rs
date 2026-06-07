use open_gpui::{
    App, Bounds, Context, SharedString, Window, WindowBounds, WindowOptions, div, prelude::*, px,
    rgb, size,
};
use open_gpui_platform::application;

struct SmokeView {
    message: SharedString,
}

impl Render for SmokeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .justify_center()
            .items_center()
            .bg(rgb(0x202124))
            .text_color(rgb(0xf8f8f2))
            .text_xl()
            .child(format!("{}", self.message))
    }
}

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(520.0), px(320.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| SmokeView {
                    message: "Open GPUI native smoke example".into(),
                })
            },
        )
        .expect("failed to open native smoke window");

        cx.activate(true);
    });
}
