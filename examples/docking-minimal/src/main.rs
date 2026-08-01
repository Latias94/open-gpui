use open_gpui::{
    AnyView, App, Bounds, Context, IntoElement, ParentElement, Render, Styled, TitlebarOptions,
    Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use open_gpui_docking::prelude::{
    DockPanelPlacement, DockSurface, DockSurfacePrimaryWindowOpenOutcome,
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
            "Registered through DockSurface::builder",
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
                "DockSurface owns controller wiring",
                "Dock hosts render logical dock spaces",
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

fn build_surface(cx: &mut App) -> DockSurface {
    DockSurface::builder(SPACE)
        .panel_placements([
            DockPanelPlacement::left_rail("explorer").fraction(0.24),
            DockPanelPlacement::center("editor").selected(),
            DockPanelPlacement::bottom_rail("terminal").fraction(0.30),
        ])
        .panel_factory("explorer", "Explorer", explorer_panel)
        .panel_factory("editor", "Editor", editor_panel)
        .panel_factory("terminal", "Terminal", terminal_panel)
        .allow_floating(true)
        .build(cx)
        .expect("minimal docking surface should validate")
}

fn main() {
    application().run(|cx: &mut App| {
        let surface = build_surface(cx);
        let bounds = Bounds::centered(None, size(px(960.0), px(640.0)), cx);
        match surface.open_primary_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Open GPUI Docking Minimal".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            cx,
        ) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(_) => {}
            outcome => panic!("failed to open minimal docking window: {outcome:?}"),
        }
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::{QuitMode, TestAppContext};
    use open_gpui_docking::prelude::DockSurfaceWindowSessionPhase;

    #[open_gpui::test]
    fn primary_close_converges_without_app_quit(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_quit_mode(QuitMode::Explicit));
        let (surface, anchor) = cx.update(|cx| {
            let surface = build_surface(cx);
            let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
                DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
                outcome => panic!("minimal primary should open, got {outcome:?}"),
            };
            (surface, anchor)
        });

        let close = cx.simulate_window_close_request(anchor);
        assert!(!close.native_close_allowed());
        assert!(close.logical_window_removed());
        cx.run_until_parked();

        assert!(!cx.windows().contains(&anchor));
        assert!(
            !cx.did_quit(),
            "DockSurface teardown must not call App::quit"
        );
        assert_eq!(
            cx.update(|cx| surface.window_session_status(cx).phase()),
            DockSurfaceWindowSessionPhase::Closed
        );
    }
}
