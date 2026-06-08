use open_gpui::{
    App, Bounds, Context, IntoElement, ParentElement, Render, Styled, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size,
};
use open_gpui_docking::{DockGraph, DockHost, DockWorkspace, EditorDockLayoutSpec};
use open_gpui_platform::application;

const SPACE: &str = "docking-demo";

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
        let accent = rgb(self.accent);

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
                    .child(div().w(px(4.0)).h(px(28.0)).bg(accent))
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

fn build_host(cx: &mut Context<DockHost>) -> DockHost {
    let graph = DockGraph::default_editor_layout(
        SPACE,
        EditorDockLayoutSpec::new(
            ["explorer", "outline"],
            ["editor", "preview"],
            ["terminal", "problems"],
        )
        .with_fractions(0.24, 0.68)
        .with_active_indexes(0, 0, 0),
    );

    let mut workspace = DockWorkspace::new(SPACE, graph);
    workspace.register_panel_view(
        "explorer",
        "Explorer",
        cx.new(|_| {
            DemoPanel::new(
                "Explorer",
                "Project structure",
                0x2563eb,
                &[
                    "crates/gpui_docking",
                    "examples/docking-native",
                    "docs/plans",
                    "target/doc",
                ],
            )
        }),
    );
    workspace.register_panel_view(
        "outline",
        "Outline",
        cx.new(|_| {
            DemoPanel::new(
                "Outline",
                "Symbols in the active file",
                0x0891b2,
                &[
                    "DockHost",
                    "DockPanelRegistry",
                    "DockGraph::default_editor_layout",
                    "Render for DockHost",
                ],
            )
        }),
    );
    workspace.register_panel_view(
        "editor",
        "Editor",
        cx.new(|_| {
            DemoPanel::new(
                "Editor",
                "Active document",
                0x16a34a,
                &[
                    "Workspace-backed rendering is active.",
                    "Tabs route through DockAction.",
                    "Splits use normalized graph fractions.",
                    "Registered panel views stay outside the graph.",
                ],
            )
        }),
    );
    workspace.register_panel_view(
        "preview",
        "Preview",
        cx.new(|_| {
            DemoPanel::new(
                "Preview",
                "Rendered layout notes",
                0x9333ea,
                &[
                    "DockHost adapts DockWorkspace into GPUI.",
                    "Tab selection updates graph state.",
                    "Floating overlays are deferred.",
                    "Drag/drop starts in the next phase.",
                ],
            )
        }),
    );
    workspace.register_panel_view(
        "terminal",
        "Terminal",
        cx.new(|_| {
            DemoPanel::new(
                "Terminal",
                "Command output",
                0xea580c,
                &[
                    "$ cargo nextest run -p open-gpui-docking",
                    "Docking owner seam tests passed",
                    "$ cargo doc -p open-gpui-docking --no-deps",
                ],
            )
        }),
    );
    workspace.register_panel_view(
        "problems",
        "Problems",
        cx.new(|_| {
            DemoPanel::new(
                "Problems",
                "Diagnostics",
                0xdc2626,
                &[
                    "No active diagnostics.",
                    "Missing panels render placeholders.",
                    "OS windows remain out of scope.",
                ],
            )
        }),
    );

    DockHost::from_workspace(workspace)
}

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(980.0), px(680.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(build_host),
        )
        .expect("failed to open docking smoke window");

        cx.activate(true);
    });
}
