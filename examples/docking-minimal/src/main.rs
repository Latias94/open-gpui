use open_gpui::{
    AnyView, App, Bounds, Context, IntoElement, ParentElement, Render, Styled, TitlebarOptions,
    Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use open_gpui_docking::prelude::{
    DockController, DockHost, DockViewportRuntimeHandle, EditorDockLayoutSpec,
};
use open_gpui_platform::application;

const SPACE: &str = "main";

struct DemoPanel {
    title: &'static str,
    subtitle: &'static str,
    accent: u32,
    lines: &'static [&'static str],
}

impl DemoPanel {
    fn new(
        title: &'static str,
        subtitle: &'static str,
        accent: u32,
        lines: &'static [&'static str],
    ) -> Self {
        Self {
            title,
            subtitle,
            accent,
            lines,
        }
    }
}

impl Render for DemoPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .gap_3()
            .p_4()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x111827))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(4.0)).h(px(30.0)).bg(rgb(self.accent)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_lg().child(self.title))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x5f6b7a))
                                    .child(self.subtitle),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(self.lines.iter().map(|line| {
                        div()
                            .px_2()
                            .py_1()
                            .bg(rgb(0xf4f6f8))
                            .text_color(rgb(0x253041))
                            .child(*line)
                    })),
            )
    }
}

fn explorer_panel(cx: &mut App) -> AnyView {
    cx.new(|_| {
        DemoPanel::new(
            "Explorer",
            "Registered through DockController::builder",
            0x2563eb,
            &[
                "crates/gpui_docking",
                "examples/docking-minimal",
                "README.md",
            ],
        )
    })
    .into()
}

fn editor_panel(cx: &mut App) -> AnyView {
    cx.new(|_| {
        DemoPanel::new(
            "Editor",
            "The active tab stack is durable graph state",
            0x7c3aed,
            &[
                "DockGraph stores tabs and splits",
                "DockHost renders one logical dock space",
                "Panel views stay outside serialized layouts",
            ],
        )
    })
    .into()
}

fn terminal_panel(cx: &mut App) -> AnyView {
    cx.new(|_| {
        DemoPanel::new(
            "Terminal",
            "Single-window docking keeps platform viewport support optional",
            0x0f766e,
            &[
                "In-window floating is enabled",
                "Platform viewports remain runtime capability facts",
                "No advanced diagnostics are imported",
            ],
        )
    })
    .into()
}

fn build_controller() -> DockController {
    DockController::builder(SPACE)
        .default_editor_layout(
            EditorDockLayoutSpec::new(["explorer"], ["editor"], ["terminal"])
                .with_fractions(0.24, 0.70),
        )
        .panel_factory("explorer", "Explorer", explorer_panel)
        .panel_factory("editor", "Editor", editor_panel)
        .panel_factory("terminal", "Terminal", terminal_panel)
        .allow_floating(true)
        .try_build()
        .expect("minimal docking layout should validate")
}

fn main() {
    application().run(|cx: &mut App| {
        let controller = cx.new(|_| build_controller());
        let viewport_runtime = DockViewportRuntimeHandle::new(controller.clone());
        let bounds = Bounds::centered(None, size(px(960.0), px(640.0)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Open GPUI Docking Minimal".into()),
                        appears_transparent: false,
                        traffic_light_position: None,
                    }),
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|cx| {
                        DockHost::from_controller(
                            controller.clone(),
                            SPACE,
                            viewport_runtime.clone(),
                            cx,
                        )
                    })
                },
            )
            .expect("failed to open minimal docking window");

        let window_id = window.window_id();
        cx.on_window_closed(move |cx, closed_window_id| {
            if closed_window_id == window_id {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}
