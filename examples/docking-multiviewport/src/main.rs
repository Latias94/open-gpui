use open_gpui::{
    AnyView, App, Bounds, Context, IntoElement, ParentElement, Render, Styled, TitlebarOptions,
    Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use open_gpui_docking::prelude::{
    DockPanelPlacement, DockSurface, DockSurfaceViewportOpenOutcome, DockSurfaceViewportSpec,
    DockSurfaceViewportUnavailable,
};
use open_gpui_platform::application;

const MAIN_SPACE: &str = "main";

struct ExamplePanel {
    title: &'static str,
    accent: u32,
    lines: &'static [&'static str],
}

impl ExamplePanel {
    fn new(title: &'static str, accent: u32, lines: &'static [&'static str]) -> Self {
        Self {
            title,
            accent,
            lines,
        }
    }
}

impl Render for ExamplePanel {
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
                    .items_center()
                    .gap_2()
                    .child(div().w(px(4.0)).h(px(28.0)).bg(rgb(self.accent)))
                    .child(div().text_lg().child(self.title)),
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
                            .bg(rgb(0xf3f4f6))
                            .text_color(rgb(0x253041))
                            .child(*line)
                    })),
            )
    }
}

fn panel(
    title: &'static str,
    accent: u32,
    lines: &'static [&'static str],
) -> impl Fn(&mut App) -> AnyView {
    move |cx| cx.new(|_| ExamplePanel::new(title, accent, lines)).into()
}

fn build_surface(cx: &mut App) -> DockSurface {
    DockSurface::builder(MAIN_SPACE)
        .panel_placements([
            DockPanelPlacement::left_rail("project").fraction(0.25),
            DockPanelPlacement::center("editor").selected(),
            DockPanelPlacement::right_rail("inspector").fraction(0.26),
            DockPanelPlacement::center("preview"),
        ])
        .panel_factory(
            "project",
            "Project",
            panel("Project", 0x2563eb, &["src", "examples", "docs"]),
        )
        .panel_factory(
            "editor",
            "Editor",
            panel(
                "Editor",
                0x7c3aed,
                &["Facade owns host wiring", "Panels use durable ids"],
            ),
        )
        .panel_factory(
            "inspector",
            "Inspector",
            panel(
                "Inspector",
                0x0f766e,
                &[
                    "Policy gates platform viewports",
                    "Unsupported backends stay single-window",
                ],
            ),
        )
        .panel_factory(
            "preview",
            "Preview",
            panel(
                "Preview",
                0xb45309,
                &[
                    "Secondary viewport opens when backend supports it",
                    "The main surface remains usable otherwise",
                ],
            ),
        )
        .allow_floating(true)
        .allow_platform_viewports(true)
        .build(cx)
        .expect("multi-viewport docking surface should validate")
}

fn main_window_options(cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(1040.0), px(680.0)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("Open GPUI Docking Multiviewport".into()),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        ..Default::default()
    }
}

fn secondary_window_options(cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(560.0), px(420.0)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("Open GPUI Docking Secondary".into()),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        ..Default::default()
    }
}

fn main() {
    application().run(|cx: &mut App| {
        let surface = build_surface(cx);
        surface
            .open_primary_window(main_window_options(cx), cx)
            .expect("failed to open primary docking window");

        let secondary_viewport =
            DockSurfaceViewportSpec::new(MAIN_SPACE, secondary_window_options(cx));

        match surface.open_viewport_spec(secondary_viewport, cx) {
            DockSurfaceViewportOpenOutcome::Opened(_) => {}
            DockSurfaceViewportOpenOutcome::Unavailable(
                DockSurfaceViewportUnavailable::BackendUnsupported
                | DockSurfaceViewportUnavailable::PolicyDisabled(_),
            ) => {}
            DockSurfaceViewportOpenOutcome::Unavailable(
                DockSurfaceViewportUnavailable::OpenFailed(error),
            ) => {
                log::warn!("secondary docking viewport did not open: {error}");
            }
        }

        cx.activate(true);
    });
}
