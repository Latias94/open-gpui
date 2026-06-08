use open_gpui::{
    App, Bounds, Context, IntoElement, ParentElement, Render, Styled, Window, WindowBounds,
    WindowOptions, div, point, prelude::*, px, rgb, size,
};
use open_gpui_docking::{
    DockAction, DockController, DockGraph, DockPolicy, DockSpaceId, DockViewportPlacement,
    DockViewportPlacementLayout, DockViewportRuntimeHandle, DockViewportWindowBounds,
    DockWorkspace, EditorDockLayoutSpec,
};
use open_gpui_platform::application;

const SPACE: &str = "docking-demo";
const SECONDARY_SPACE: &str = "docking-preview";

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

fn default_graph() -> DockGraph {
    DockGraph::default_editor_layout(
        SPACE,
        EditorDockLayoutSpec::new(
            ["explorer", "outline"],
            ["editor", "preview"],
            ["terminal", "problems"],
        )
        .with_fractions(0.24, 0.68)
        .with_active_indexes(0, 0, 0),
    )
}

fn restored_demo_graph() -> DockGraph {
    let mut workspace = DockWorkspace::new(SPACE, default_graph());
    workspace.policy_mut().set_allow_floating(true);
    workspace.policy_mut().set_allow_platform_viewports(true);
    workspace
        .apply_action(&DockAction::MoveItemToEmptyDockSpace {
            source_space: SPACE.into(),
            item: "preview".into(),
            target_space: SECONDARY_SPACE.into(),
        })
        .expect("preview panel should move into the secondary demo dock space");
    workspace
        .apply_action(&DockAction::FloatItemInWindow {
            source_space: SPACE.into(),
            item: "problems".into(),
            target_space: SPACE.into(),
            bounds: Bounds::new(point(px(620.0), px(72.0)), size(px(300.0), px(220.0))),
        })
        .expect("problems panel should float inside the demo dock space");

    let layout = workspace.graph().export_layout();
    DockGraph::import_layout(&layout).expect("demo dock layout should restore")
}

fn build_controller() -> DockController {
    let mut policy = DockPolicy::default();
    policy.set_allow_floating(true);
    policy.set_allow_platform_viewports(true);

    DockController::builder(SPACE)
        .graph(restored_demo_graph())
        .policy(policy)
        .panel_factory("explorer", "Explorer", |cx| {
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
            })
            .into()
        })
        .panel_factory("outline", "Outline", |cx| {
            cx.new(|_| {
                DemoPanel::new(
                    "Outline",
                    "Symbols in the active file",
                    0x0891b2,
                    &[
                        "DockHost",
                        "DockController::builder",
                        "DockGraph::default_editor_layout",
                        "Render for DockHost",
                    ],
                )
            })
            .into()
        })
        .panel_factory("editor", "Editor", |cx| {
            cx.new(|_| {
                DemoPanel::new(
                    "Editor",
                    "Active document",
                    0x16a34a,
                    &[
                        "Controller-backed rendering is active.",
                        "Tabs route through DockAction.",
                        "Splits use normalized graph fractions.",
                        "Registered panel factories stay outside the graph.",
                    ],
                )
            })
            .into()
        })
        .panel_factory("preview", "Preview", |cx| {
            cx.new(|_| {
                DemoPanel::new(
                    "Preview",
                    "Rendered layout notes",
                    0x9333ea,
                    &[
                        "DockHost observes DockController.",
                        "Tab selection updates graph state.",
                        "Layout round-trips through DockLayout.",
                        "Splitter handles resize panes.",
                        "Tabs can drag/drop between stacks.",
                        "Secondary viewport placement lives in the adapter.",
                    ],
                )
            })
            .into()
        })
        .panel_factory("terminal", "Terminal", |cx| {
            cx.new(|_| {
                DemoPanel::new(
                    "Terminal",
                    "Command output",
                    0xea580c,
                    &[
                        "$ cargo nextest run -p open-gpui-docking",
                        "Docking public API tests passed",
                        "$ cargo doc -p open-gpui-docking --no-deps",
                    ],
                )
            })
            .into()
        })
        .panel_factory("problems", "Problems", |cx| {
            cx.new(|_| {
                DemoPanel::new(
                    "Problems",
                    "Diagnostics",
                    0xdc2626,
                    &[
                        "No active diagnostics.",
                        "Missing panels render placeholders.",
                        "OS windows remain adapter state.",
                    ],
                )
            })
            .into()
        })
        .build()
}

fn viewport_window_options(bounds: Bounds<open_gpui::Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    }
}

fn saved_viewport_placement(
    primary_bounds: Bounds<open_gpui::Pixels>,
    secondary_bounds: Bounds<open_gpui::Pixels>,
) -> DockViewportPlacementLayout {
    DockViewportPlacementLayout::new(vec![
        DockViewportPlacement {
            space: SPACE.into(),
            display_id: None,
            window_bounds: Some(DockViewportWindowBounds::from_window_bounds(
                WindowBounds::Windowed(primary_bounds),
            )),
            host_bounds: None,
        },
        DockViewportPlacement {
            space: SECONDARY_SPACE.into(),
            display_id: None,
            window_bounds: Some(DockViewportWindowBounds::from_window_bounds(
                WindowBounds::Windowed(secondary_bounds),
            )),
            host_bounds: None,
        },
    ])
}

fn restored_viewport_options(
    placement: &DockViewportPlacementLayout,
    space: impl Into<DockSpaceId>,
    fallback_bounds: Bounds<open_gpui::Pixels>,
) -> WindowOptions {
    let space = space.into();
    placement
        .window_options_for_space(&space, viewport_window_options(fallback_bounds))
        .expect("demo viewport placement should produce window options")
}

fn main() {
    application().run(|cx: &mut App| {
        let controller = cx.new(|_| build_controller());
        let runtime = DockViewportRuntimeHandle::new(controller);
        runtime.observe_window_closed(cx).detach();

        let primary_bounds = Bounds::centered(None, size(px(920.0), px(640.0)), cx);
        let secondary_bounds = Bounds::new(
            point(
                primary_bounds.origin.x + primary_bounds.size.width + px(24.0),
                primary_bounds.origin.y,
            ),
            size(px(460.0), px(360.0)),
        );
        let placement = saved_viewport_placement(primary_bounds, secondary_bounds);

        let primary_options = restored_viewport_options(&placement, SPACE, primary_bounds);
        runtime
            .open_viewport(SPACE, primary_options, cx)
            .expect("failed to open primary docking viewport");

        let secondary_options =
            restored_viewport_options(&placement, SECONDARY_SPACE, secondary_bounds);
        runtime
            .open_viewport(SECONDARY_SPACE, secondary_options, cx)
            .expect("failed to open secondary docking viewport");

        cx.activate(true);
    });
}
