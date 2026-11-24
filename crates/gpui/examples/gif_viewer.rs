use open_gpui::{App, Context, Render, Window, WindowOptions, div, img, prelude::*};
use open_gpui_platform::application;
use std::path::PathBuf;

struct GifViewer {
    gif_path: PathBuf,
}

impl GifViewer {
    fn new(gif_path: PathBuf) -> Self {
        Self { gif_path }
    }
}

impl Render for GifViewer {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            img(self.gif_path.clone())
                .size_full()
                .object_fit(open_gpui::ObjectFit::Contain)
                .id("gif"),
        )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        let gif_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/image/black-cat-typing.gif");

        cx.open_window(
            WindowOptions {
                focus_on_appearing: true,
                ..Default::default()
            },
            |_, cx| cx.new(|_| GifViewer::new(gif_path)),
        )
        .unwrap();
        cx.activate(true);
    });
}

fn main() {
    env_logger::init();
    run_example();
}
