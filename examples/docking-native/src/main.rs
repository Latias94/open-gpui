use open_gpui::{
    App, Bounds, ClickEvent, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Pixels, Render, Styled, Window, WindowBounds, WindowOptions, div, point, prelude::*, px, rgb,
    size,
};
use open_gpui_docking::{
    DockController, DockItemId, DockLayout, DockLayoutCentralRegion, DockLayoutSpace,
    DockPanelDescriptor, DockSpaceId, DockViewportClosePolicy, DockViewportPlacement,
    DockViewportPlacementLayout, DockViewportRuntimeHandle, DockViewportWindowBounds,
    EditorDockLayoutSpec,
};
use open_gpui_platform::application;

const SPACE: &str = "docking-demo";
const SECONDARY_SPACE: &str = "docking-preview";
const CENTRAL_SPACE: &str = "docking-empty-central";

struct DemoPanel {
    title: &'static str,
    subtitle: &'static str,
    accent: u32,
    lines: &'static [&'static str],
}

struct RuntimeStatusPanel {
    runtime: DockViewportRuntimeHandle,
    controller: Entity<DockController>,
    placement: DockViewportPlacementLayout,
    primary_bounds: Bounds<Pixels>,
    secondary_bounds: Bounds<Pixels>,
    central_bounds: Bounds<Pixels>,
    last_operation: Option<String>,
}

impl RuntimeStatusPanel {
    fn new(
        runtime: DockViewportRuntimeHandle,
        controller: Entity<DockController>,
        placement: DockViewportPlacementLayout,
        primary_bounds: Bounds<Pixels>,
        secondary_bounds: Bounds<Pixels>,
        central_bounds: Bounds<Pixels>,
    ) -> Self {
        Self {
            runtime,
            controller,
            placement,
            primary_bounds,
            secondary_bounds,
            central_bounds,
            last_operation: None,
        }
    }

    fn set_operation_log(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.last_operation = Some(message.into());
        cx.notify();
    }

    fn set_close_policy(&mut self, policy: DockViewportClosePolicy, cx: &mut Context<Self>) {
        self.runtime.set_close_policy(policy.clone());
        self.set_operation_log(format!("set close policy: {policy:?}"), cx);
    }

    fn open_demo_viewport(&mut self, space: &str, cx: &mut Context<Self>) {
        let space_id = DockSpaceId::from(space);
        let fallback_bounds = self.fallback_bounds(&space_id);
        let options = restored_viewport_options(&self.placement, space_id.clone(), fallback_bounds);
        match self.runtime.open_viewport(space_id.clone(), options, cx) {
            Ok(outcome) => self.set_operation_log(
                format!(
                    "opened viewport {}: {:?}",
                    outcome.space.as_str(),
                    outcome.status
                ),
                cx,
            ),
            Err(error) => self.set_operation_log(
                format!("open viewport {} failed: {error}", space_id.as_str()),
                cx,
            ),
        }
    }

    fn apply_saved_placement(&mut self, cx: &mut Context<Self>) {
        match self.runtime.apply_placement(&self.placement) {
            Ok(outcome) => {
                self.set_operation_log(format!("applied saved placement: {outcome:?}"), cx)
            }
            Err(error) => self.set_operation_log(format!("apply placement failed: {error}"), cx),
        }
    }

    fn restore_secondary_panels(&mut self, cx: &mut Context<Self>) {
        let message = self
            .controller
            .update(cx, |controller, _| restore_secondary_panels(controller));
        self.set_operation_log(message, cx);
    }

    fn restore_outline_panel(&mut self, cx: &mut Context<Self>) {
        let message = self
            .controller
            .update(cx, |controller, _| restore_outline_panel(controller));
        self.set_operation_log(message, cx);
    }

    fn fallback_bounds(&self, space: &DockSpaceId) -> Bounds<Pixels> {
        if space.as_str() == SECONDARY_SPACE {
            self.secondary_bounds
        } else if space.as_str() == CENTRAL_SPACE {
            self.central_bounds
        } else {
            self.primary_bounds
        }
    }
}

impl Render for RuntimeStatusPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let lines = {
            let status = self.runtime.runtime_status();
            let spaces = self
                .runtime
                .registered_viewport_spaces()
                .into_iter()
                .map(|space| {
                    let status = if self.runtime.is_viewport_open(&space) {
                        "open"
                    } else {
                        "missing"
                    };
                    format!("{}: {}", space.as_str(), status)
                })
                .collect::<Vec<_>>();
            let placement = self.runtime.export_placement();
            vec![
                format!("close policy: {:?}", self.runtime.close_policy()),
                format!("registered viewports: {}", spaces.len()),
                format!("placement snapshots: {}", placement.viewports.len()),
                format!("spaces: {}", spaces.join(", ")),
                format!(
                    "last route: {}",
                    debug_option(status.last_route.as_ref().map(|record| &record.target))
                ),
                format!(
                    "last drop: {}",
                    debug_option(status.last_drop_outcome.as_ref().map(|record| &record.kind))
                ),
                format!(
                    "last activation: {}",
                    debug_option(status.last_activation.as_ref())
                ),
                format!("last close: {}", debug_option(status.last_close.as_ref())),
                format!(
                    "last should-close: {}",
                    debug_option(status.last_should_close.as_ref())
                ),
                format!(
                    "last tear-off: {}",
                    debug_option(status.last_tear_off.as_ref().map(|record| &record.kind))
                ),
            ]
        };
        let last_operation = self.last_operation.clone();

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
                    .child(div().w(px(4.0)).h(px(28.0)).bg(rgb(0x0f766e)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_lg().child("Runtime"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x5f6b7a))
                                    .child("Viewport dogfood state"),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(lines.into_iter().map(|line| {
                        div()
                            .px_2()
                            .py_1()
                            .bg(rgb(0xf4f6f8))
                            .text_color(rgb(0x253041))
                            .child(line)
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(control_button(
                                "Prevent",
                                cx.listener(|this, _, _, cx| {
                                    this.set_close_policy(DockViewportClosePolicy::Prevent, cx);
                                }),
                            ))
                            .child(control_button(
                                "Retain",
                                cx.listener(|this, _, _, cx| {
                                    this.set_close_policy(
                                        DockViewportClosePolicy::RetainLayout,
                                        cx,
                                    );
                                }),
                            ))
                            .child(control_button(
                                "Merge back",
                                cx.listener(|this, _, _, cx| {
                                    this.set_close_policy(
                                        DockViewportClosePolicy::MergeBack {
                                            target_space: SPACE.into(),
                                        },
                                        cx,
                                    );
                                }),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(control_button(
                                "Open primary",
                                cx.listener(|this, _, _, cx| {
                                    this.open_demo_viewport(SPACE, cx);
                                }),
                            ))
                            .child(control_button(
                                "Open secondary",
                                cx.listener(|this, _, _, cx| {
                                    this.open_demo_viewport(SECONDARY_SPACE, cx);
                                }),
                            ))
                            .child(control_button(
                                "Open central",
                                cx.listener(|this, _, _, cx| {
                                    this.open_demo_viewport(CENTRAL_SPACE, cx);
                                }),
                            ))
                            .child(control_button(
                                "Apply placement",
                                cx.listener(|this, _, _, cx| {
                                    this.apply_saved_placement(cx);
                                }),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(control_button(
                                "Restore secondary tabs",
                                cx.listener(|this, _, _, cx| {
                                    this.restore_secondary_panels(cx);
                                }),
                            ))
                            .child(control_button(
                                "Restore outline",
                                cx.listener(|this, _, _, cx| {
                                    this.restore_outline_panel(cx);
                                }),
                            )),
                    ),
            )
            .when_some(last_operation, |element, operation| {
                element.child(
                    div()
                        .px_2()
                        .py_1()
                        .bg(rgb(0xecfdf5))
                        .text_color(rgb(0x065f46))
                        .child(operation),
                )
            })
    }
}

fn control_button(
    label: &str,
    listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(format!("runtime-control:{label}"))
        .px_2()
        .py_1()
        .border_1()
        .border_color(rgb(0xcbd5e1))
        .bg(rgb(0xf8fafc))
        .hover(|style| style.bg(rgb(0xe2e8f0)))
        .active(|style| style.opacity(0.78))
        .cursor_pointer()
        .text_color(rgb(0x1f2937))
        .on_click(listener)
        .child(label.to_string())
}

fn restore_secondary_panels(controller: &mut DockController) -> String {
    let secondary_space = DockSpaceId::from(SECONDARY_SPACE);
    let preview = DockItemId::from("preview");
    let diff = DockItemId::from("diff");
    let mut results = Vec::new();

    if controller
        .graph()
        .find_item_in_space(&secondary_space, &preview)
        .is_some()
    {
        results.push("preview already in secondary".to_string());
    } else if controller.graph().contains_item(&preview) {
        results.push("preview is open outside secondary".to_string());
    } else {
        results.push(open_item_result(
            "preview",
            controller.open_item(secondary_space.clone(), None, preview.clone(), None),
        ));
    }

    let secondary_tabs = controller
        .graph()
        .find_item_in_space(&secondary_space, &preview)
        .or_else(|| {
            controller
                .graph()
                .find_item_in_space(&secondary_space, &diff)
        })
        .map(|(tabs, _)| tabs);

    if controller
        .graph()
        .find_item_in_space(&secondary_space, &diff)
        .is_some()
    {
        results.push("diff already in secondary".to_string());
    } else if controller.graph().contains_item(&diff) {
        results.push("diff is open outside secondary".to_string());
    } else {
        results.push(open_item_result(
            "diff",
            controller.open_item(secondary_space, secondary_tabs, diff, None),
        ));
    }

    results.join("; ")
}

fn restore_outline_panel(controller: &mut DockController) -> String {
    let main_space = DockSpaceId::from(SPACE);
    let outline = DockItemId::from("outline");
    if controller
        .graph()
        .find_item_in_space(&main_space, &outline)
        .is_some()
    {
        return "outline already in primary".to_string();
    }
    if controller.graph().contains_item(&outline) {
        return "outline is open outside primary".to_string();
    }

    let target_tabs = controller
        .graph()
        .find_item_in_space(&main_space, &DockItemId::from("explorer"))
        .or_else(|| {
            controller
                .graph()
                .find_item_in_space(&main_space, &DockItemId::from("workspace"))
        })
        .map(|(tabs, _)| tabs);

    open_item_result(
        "outline",
        controller.open_item(main_space, target_tabs, outline, Some(1)),
    )
}

fn open_item_result(
    label: &str,
    result: std::result::Result<
        open_gpui_docking::DockActionOutcome,
        open_gpui_docking::DockActionApplyError,
    >,
) -> String {
    match result {
        Ok(outcome) => format!("opened {label}: {outcome:?}"),
        Err(error) => format!("open {label} failed: {error}"),
    }
}

fn debug_option<T: std::fmt::Debug>(value: Option<T>) -> String {
    value
        .map(|value| format!("{value:?}"))
        .unwrap_or_else(|| "none".to_string())
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

fn restored_demo_layout() -> DockLayout {
    let mut controller = DockController::builder(SPACE)
        .default_editor_layout(
            EditorDockLayoutSpec::new(
                ["explorer", "outline", "workspace"],
                ["editor", "preview"],
                ["terminal", "problems", "runtime"],
            )
            .with_fractions(0.24, 0.68)
            .with_active_indexes(0, 0, 0),
        )
        .allow_floating(true)
        .allow_platform_viewports(true)
        .panel_descriptor("explorer", DockPanelDescriptor::new("Explorer"))
        .panel_descriptor("outline", DockPanelDescriptor::new("Outline"))
        .panel_descriptor(
            "workspace",
            DockPanelDescriptor::new("Workspace").closable(false),
        )
        .panel_descriptor("editor", DockPanelDescriptor::new("Editor"))
        .panel_descriptor("preview", DockPanelDescriptor::new("Preview"))
        .panel_descriptor("diff", DockPanelDescriptor::new("Diff"))
        .panel_descriptor("terminal", DockPanelDescriptor::new("Terminal"))
        .panel_descriptor("problems", DockPanelDescriptor::new("Problems"))
        .panel_descriptor("runtime", DockPanelDescriptor::new("Runtime"))
        .try_build()
        .expect("demo controller setup should validate");

    let main_space = DockSpaceId::from(SPACE);
    let preview_item: open_gpui_docking::DockItemId = "preview".into();
    controller
        .close_item(main_space.clone(), preview_item.clone())
        .expect("preview panel should close before reopening into secondary space");
    controller
        .open_item(SECONDARY_SPACE, None, preview_item.clone(), None)
        .expect("preview panel should reopen into the secondary demo dock space");
    let secondary_space = DockSpaceId::from(SECONDARY_SPACE);
    let diff_item: open_gpui_docking::DockItemId = "diff".into();
    let (secondary_tabs, _) = controller
        .graph()
        .find_item_in_space(&secondary_space, &preview_item)
        .expect("preview panel should create secondary tabs");
    controller
        .open_item(secondary_space, Some(secondary_tabs), diff_item, Some(1))
        .expect("diff panel should join the secondary demo tab stack");
    controller
        .float_item_in_window(
            SPACE,
            "problems",
            SPACE,
            Bounds::new(point(px(620.0), px(72.0)), size(px(300.0), px(220.0))),
        )
        .expect("problems panel should float inside the demo dock space");

    let outline_item: open_gpui_docking::DockItemId = "outline".into();
    let (outline_tabs, _) = controller
        .graph()
        .find_item_in_space(&main_space, &outline_item)
        .expect("outline panel should be in the restored demo layout");
    controller
        .close_item(main_space.clone(), outline_item.clone())
        .expect("outline panel should close while its registration remains available");
    controller
        .open_item(main_space, Some(outline_tabs), outline_item, Some(1))
        .expect("outline panel should reopen into its original tab stack");

    let mut layout = controller.graph().export_layout();
    layout.spaces.push(DockLayoutSpace {
        id: CENTRAL_SPACE.into(),
        root: None,
        floatings: Vec::new(),
        central: Some(DockLayoutCentralRegion {
            node: None,
            keep_alive_when_empty: true,
            passthrough_when_empty: true,
        }),
    });
    layout
}

fn build_controller() -> DockController {
    DockController::builder(SPACE)
        .try_layout(&restored_demo_layout())
        .expect("demo dock layout should restore")
        .allow_floating(true)
        .allow_platform_viewports(true)
        .panel_descriptor("runtime", DockPanelDescriptor::new("Runtime"))
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
        .panel_factory("workspace", "Workspace", |cx| {
            cx.new(|_| {
                DemoPanel::new(
                    "Workspace",
                    "Pinned overview",
                    0x0f766e,
                    &[
                        "open-gpui",
                        "gpui_docking",
                        "runtime viewports",
                        "retained panels",
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
                        "Tabs route through resolved drop transactions.",
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
        .panel_factory("diff", "Diff", |cx| {
            cx.new(|_| {
                DemoPanel::new(
                    "Diff",
                    "Secondary stack",
                    0x7c3aed,
                    &[
                        "drop_runtime.rs",
                        "viewport_runtime.rs",
                        "render_tabs.rs",
                        "host_interactions.rs",
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
                        "DockController::builder restores DockLayout.",
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
        .try_build()
        .expect("demo controller setup should validate")
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
    central_bounds: Bounds<open_gpui::Pixels>,
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
        DockViewportPlacement {
            space: CENTRAL_SPACE.into(),
            display_id: None,
            window_bounds: Some(DockViewportWindowBounds::from_window_bounds(
                WindowBounds::Windowed(central_bounds),
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
    let mut options = placement
        .window_options_for_space(&space, viewport_window_options(fallback_bounds))
        .expect("demo viewport placement should produce window options");
    if let Some(titlebar) = options.titlebar.as_mut() {
        titlebar.title = Some(viewport_title(&space).into());
    }
    options
}

fn viewport_title(space: &DockSpaceId) -> &'static str {
    match space.as_str() {
        SPACE => "Docking demo",
        SECONDARY_SPACE => "Docking preview",
        CENTRAL_SPACE => "Empty central dogfood",
        _ => "Docking viewport",
    }
}

fn main() {
    application().run(|cx: &mut App| {
        let controller = cx.new(|_| build_controller());
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let primary_bounds = Bounds::centered(None, size(px(920.0), px(640.0)), cx);
        let secondary_bounds = Bounds::new(
            point(
                primary_bounds.origin.x + primary_bounds.size.width + px(24.0),
                primary_bounds.origin.y,
            ),
            size(px(460.0), px(360.0)),
        );
        let central_bounds = Bounds::new(
            point(
                primary_bounds.origin.x + primary_bounds.size.width + px(24.0),
                primary_bounds.origin.y + secondary_bounds.size.height + px(24.0),
            ),
            size(px(460.0), px(220.0)),
        );
        let placement = saved_viewport_placement(primary_bounds, secondary_bounds, central_bounds);
        let runtime_panel = cx.new(|_| {
            RuntimeStatusPanel::new(
                runtime.clone(),
                controller.clone(),
                placement.clone(),
                primary_bounds,
                secondary_bounds,
                central_bounds,
            )
        });
        controller.update(cx, |controller, _| {
            controller
                .attach_panel_view("runtime", runtime_panel)
                .expect("runtime panel descriptor should exist");
        });
        runtime.observe_window_closed(cx).detach();

        let primary_options = restored_viewport_options(&placement, SPACE, primary_bounds);
        runtime
            .open_viewport(SPACE, primary_options, cx)
            .expect("failed to open primary docking viewport");

        let secondary_options =
            restored_viewport_options(&placement, SECONDARY_SPACE, secondary_bounds);
        runtime
            .open_viewport(SECONDARY_SPACE, secondary_options, cx)
            .expect("failed to open secondary docking viewport");

        let central_options = restored_viewport_options(&placement, CENTRAL_SPACE, central_bounds);
        runtime
            .open_viewport(CENTRAL_SPACE, central_options, cx)
            .expect("failed to open empty central docking viewport");

        cx.activate(true);
    });
}
