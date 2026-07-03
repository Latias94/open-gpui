use open_gpui::{
    App, Bounds, ClickEvent, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Pixels, Render, Styled, Window, WindowBounds, WindowOptions, div, point, prelude::*, px, rgb,
    size,
};
use open_gpui_docking::{
    DockController, DockItemId, DockLayout, DockLayoutCentralRegion, DockLayoutSpace, DockPanel,
    DockPanelDescriptor, DockSpaceId, DockViewportClosePolicy, DockViewportCoordinateSpaceRecord,
    DockViewportLifecycleRecord, DockViewportPlacement, DockViewportPlacementLayout,
    DockViewportPlatformCapabilityRecord, DockViewportPlatformFlagCapabilityRecord,
    DockViewportPlatformSyncRecord, DockViewportReleaseUnavailableRecord,
    DockViewportRestoreReadinessRecord, DockViewportRuntimeHandle,
    DockViewportTearOffPlacementRecord, DockViewportWindowBounds, DockVisualAffordanceDebugSummary,
    EditorDockLayoutSpec,
};
use open_gpui_platform::application;

const SPACE: &str = "docking-demo";
const SECONDARY_SPACE: &str = "docking-preview";
const CENTRAL_SPACE: &str = "docking-empty-central";
const PRIMARY_DOCK_CLASS: &str = "primary-demo";
const SECONDARY_DOCK_CLASS: &str = "secondary-demo";
const CENTRAL_DOCK_CLASS: &str = "central-demo";
const DOCKING_DEBUG_PREFIX: &str = "[DEBUG-docking-native]";

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
                    outcome.space().as_str(),
                    outcome.status()
                ),
                cx,
            ),
            Err(error) => self.set_operation_log(
                format!("open viewport {} failed: {error}", space_id.as_str()),
                cx,
            ),
        }
    }

    fn check_saved_placement_restore(&mut self, cx: &mut Context<Self>) {
        match self.runtime.check_placement_restore(&self.placement) {
            Ok(readiness) => {
                self.set_operation_log(format!("placement restore readiness: {readiness:?}"), cx)
            }
            Err(error) => self.set_operation_log(format!("check placement failed: {error}"), cx),
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

    fn restore_central_note_panel(&mut self, cx: &mut Context<Self>) {
        let message = self
            .controller
            .update(cx, |controller, _| restore_central_note_panel(controller));
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
            let status = self
                .runtime
                .runtime_status()
                .with_platform_capabilities(cx.viewport_capabilities());
            let flag_capabilities =
                DockViewportPlatformFlagCapabilityRecord::from(cx.viewport_flag_capabilities());
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
            let mut lines = vec![
                format!("close policy: {:?}", self.runtime.close_policy()),
                format!("registered viewports: {}", spaces.len()),
                format!("placement snapshots: {}", placement.viewports.len()),
                format!("spaces: {}", spaces.join(", ")),
                format!(
                    "last route: {}",
                    debug_option(status.last_route.as_ref().map(|record| &record.target))
                ),
                format!(
                    "last route source: {}",
                    debug_option(
                        status
                            .last_route
                            .as_ref()
                            .and_then(|record| record.selection_source.as_ref())
                    )
                ),
                format!(
                    "last route unavailable: {}",
                    route_unavailable_summary(
                        status
                            .last_route
                            .as_ref()
                            .and_then(|record| record.unavailable_reason.as_ref())
                    )
                ),
                format!(
                    "route facts: {}",
                    route_capability_summary(status.platform_capabilities.as_ref())
                ),
                format!(
                    "placement facts: {}",
                    placement_capability_summary(status.platform_capabilities.as_ref())
                ),
                format!(
                    "coordinate facts: {}",
                    coordinate_status_summary(&status.viewport_lifecycle)
                ),
                format!(
                    "viewport flags: {}",
                    viewport_flag_capability_summary(Some(&flag_capabilities))
                ),
                format!(
                    "last platform sync: {}",
                    platform_sync_summary(status.last_platform_sync.as_ref())
                ),
                format!("preview proof: {}", preview_proof_summary()),
                format!("motion proof: {}", motion_runtime_proof_summary()),
                format!(
                    "placement restore: {}",
                    placement_restore_summary(status.placement_restore.as_ref())
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
                format!(
                    "last tear-off placement: {}",
                    tear_off_placement_summary(
                        status
                            .last_tear_off
                            .as_ref()
                            .and_then(|record| record.placement_source.as_ref())
                    )
                ),
            ];
            lines.extend(status.visual_affordances.iter().map(|record| {
                format!(
                    "affordance {}: {}",
                    record.space.as_str(),
                    affordance_debug_summary(&record.summary)
                )
            }));
            lines
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
                                "Check placement",
                                cx.listener(|this, _, _, cx| {
                                    this.check_saved_placement_restore(cx);
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
                            ))
                            .child(control_button(
                                "Restore central note",
                                cx.listener(|this, _, _, cx| {
                                    this.restore_central_note_panel(cx);
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

fn restore_central_note_panel(controller: &mut DockController) -> String {
    let central_space = DockSpaceId::from(CENTRAL_SPACE);
    let note = DockItemId::from("central-note");
    if controller
        .graph()
        .find_item_in_space(&central_space, &note)
        .is_some()
    {
        return "central note already in central".to_string();
    }
    if controller.graph().contains_item(&note) {
        return "central note is open outside central".to_string();
    }

    open_item_result(
        "central note",
        controller.open_item(central_space, None, note, None),
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

fn affordance_debug_summary(summary: &DockVisualAffordanceDebugSummary) -> String {
    let active = summary
        .active
        .as_ref()
        .map(|layer| {
            format!(
                "{} {} state={} node={} zone={} payload={} label={}",
                layer.kind,
                layer.scope,
                layer.state,
                layer
                    .target_node
                    .map(|node| node.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                layer
                    .zone
                    .map(|zone| format!("{zone:?}"))
                    .unwrap_or_else(|| "none".to_string()),
                layer
                    .payload_index
                    .map(|index| index.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                layer.label.as_deref().unwrap_or("none"),
            )
        })
        .unwrap_or_else(|| "none".to_string());
    format!(
        "layers={} active={} motion={} frame={} active={}",
        summary.layer_count,
        summary.active_count,
        summary.motion_state.as_deref().unwrap_or("none"),
        summary
            .frame_generation
            .map(|generation| generation.to_string())
            .unwrap_or_else(|| "none".to_string()),
        active,
    )
}

fn capability_flag(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn route_capability_summary(capabilities: Option<&DockViewportPlatformCapabilityRecord>) -> String {
    capabilities
        .map(|capabilities| {
            format!(
                "bounds={}, stack={}, hover-through-no-input={}",
                capability_flag(capabilities.global_window_bounds),
                capability_flag(capabilities.window_stack),
                capability_flag(capabilities.hovered_window_ignores_no_input),
            )
        })
        .unwrap_or_else(|| "unavailable".to_string())
}

fn placement_capability_summary(
    capabilities: Option<&DockViewportPlatformCapabilityRecord>,
) -> String {
    capabilities
        .map(|capabilities| {
            format!(
                "work-area={}, dpi={}, live-move={}, no-input={}",
                capability_flag(capabilities.display_work_area),
                capability_flag(capabilities.dpi_scale),
                capability_flag(capabilities.live_window_move),
                capability_flag(capabilities.no_input_windows),
            )
        })
        .unwrap_or_else(|| "unavailable".to_string())
}

fn viewport_flag_capability_summary(
    capabilities: Option<&DockViewportPlatformFlagCapabilityRecord>,
) -> String {
    capabilities
        .map(|capabilities| {
            format!(
                "no-focus-appearing={}, no-focus-click={}, alpha={}, topmost={}, no-taskbar={}",
                capability_flag(capabilities.no_focus_on_appearing_windows),
                capability_flag(capabilities.no_focus_on_click_windows),
                capability_flag(capabilities.alpha_windows),
                capability_flag(capabilities.topmost_windows),
                capability_flag(capabilities.no_taskbar_windows),
            )
        })
        .unwrap_or_else(|| "unavailable".to_string())
}

fn route_unavailable_summary(
    reason: Option<&DockViewportReleaseUnavailableRecord>,
) -> &'static str {
    match reason {
        Some(DockViewportReleaseUnavailableRecord::BlockedByViewportWindow) => "blocked-window",
        Some(DockViewportReleaseUnavailableRecord::NoViewportRouteSelection) => {
            "no-route-selection"
        }
        Some(DockViewportReleaseUnavailableRecord::TrustedHoveredNone) => "trusted-hovered-none",
        Some(_) => "unknown",
        None => "none",
    }
}

fn coordinate_status_summary(lifecycle: &[DockViewportLifecycleRecord]) -> String {
    if lifecycle.is_empty() {
        return "none".to_string();
    }
    lifecycle
        .iter()
        .map(|record| {
            let coordinate = record
                .coordinate_status
                .map(|status| {
                    let space = match status.coordinate_space {
                        DockViewportCoordinateSpaceRecord::GlobalScreen => "global",
                        DockViewportCoordinateSpaceRecord::WindowLocal => "local",
                        _ => "unknown",
                    };
                    format!("{space}@gen{}", status.facts_generation)
                })
                .unwrap_or_else(|| "missing".to_string());
            format!("{}={coordinate}", record.space.as_str())
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn platform_sync_summary(sync: Option<&DockViewportPlatformSyncRecord>) -> String {
    sync.map(|sync| {
        format!(
            "applied={}, skipped={}, unsupported={}",
            sync.applied.len(),
            sync.skipped_requests.len(),
            sync.unsupported_requests.len()
        )
    })
    .unwrap_or_else(|| "none".to_string())
}

fn preview_proof_summary() -> &'static str {
    "presentation-scene+real-content-reveal+overlay-motion+tab-insertion+retargeting+splitter-motion+zoom-focus+divider-hit-map+corner-drag+a11y+route-cleanup+reduced-motion"
}

fn motion_runtime_proof_summary() -> &'static str {
    "shared-runtime+run-state+scalar-value+scalar-sample+explicit-models+policy-gates+layout-projection+projection-clips+sampled-progress+retargeted-identity+reduced-motion-final-state+high-frequency-bypass"
}

fn tear_off_placement_summary(source: Option<&DockViewportTearOffPlacementRecord>) -> &'static str {
    match source {
        Some(DockViewportTearOffPlacementRecord::Suggested) => "suggested",
        Some(DockViewportTearOffPlacementRecord::DragGeometry) => "drag-geometry",
        None => "unavailable",
    }
}

fn placement_restore_summary(readiness: Option<&DockViewportRestoreReadinessRecord>) -> String {
    readiness
        .map(|readiness| {
            format!(
                "matched={}, missing={}",
                readiness.matched, readiness.missing
            )
        })
        .unwrap_or_else(|| "unavailable".to_string())
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
            .with_fractions(0.24, 0.68),
        )
        .allow_floating(true)
        .allow_platform_viewports(true)
        .allow_dock_class_in_space(SPACE, PRIMARY_DOCK_CLASS)
        .allow_dock_class_in_space(SPACE, SECONDARY_DOCK_CLASS)
        .allow_dock_class_in_space(SPACE, CENTRAL_DOCK_CLASS)
        .allow_dock_class_in_space(SECONDARY_SPACE, SECONDARY_DOCK_CLASS)
        .allow_dock_class_in_space(CENTRAL_SPACE, CENTRAL_DOCK_CLASS)
        .panel_descriptor(
            "explorer",
            dogfood_descriptor("Explorer", PRIMARY_DOCK_CLASS),
        )
        .panel_descriptor("outline", dogfood_descriptor("Outline", PRIMARY_DOCK_CLASS))
        .panel_descriptor(
            "workspace",
            dogfood_descriptor("Workspace", PRIMARY_DOCK_CLASS),
        )
        .panel_descriptor("editor", dogfood_descriptor("Editor", PRIMARY_DOCK_CLASS))
        .panel_descriptor(
            "preview",
            dogfood_descriptor("Preview", SECONDARY_DOCK_CLASS),
        )
        .panel_descriptor("diff", dogfood_descriptor("Diff", SECONDARY_DOCK_CLASS))
        .panel_descriptor(
            "terminal",
            dogfood_descriptor("Terminal", PRIMARY_DOCK_CLASS),
        )
        .panel_descriptor(
            "problems",
            dogfood_descriptor("Problems", PRIMARY_DOCK_CLASS),
        )
        .panel_descriptor("runtime", dogfood_descriptor("Runtime", PRIMARY_DOCK_CLASS))
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
        .allow_dock_class_in_space(SPACE, PRIMARY_DOCK_CLASS)
        .allow_dock_class_in_space(SPACE, SECONDARY_DOCK_CLASS)
        .allow_dock_class_in_space(SPACE, CENTRAL_DOCK_CLASS)
        .allow_dock_class_in_space(SECONDARY_SPACE, SECONDARY_DOCK_CLASS)
        .allow_dock_class_in_space(CENTRAL_SPACE, CENTRAL_DOCK_CLASS)
        .panel_descriptor("runtime", dogfood_descriptor("Runtime", PRIMARY_DOCK_CLASS))
        .panel(
            "explorer",
            DockPanel::lazy("Explorer", |cx| {
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
            .with_dock_class(PRIMARY_DOCK_CLASS),
        )
        .panel(
            "outline",
            DockPanel::lazy("Outline", |cx| {
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
            .with_dock_class(PRIMARY_DOCK_CLASS),
        )
        .panel(
            "workspace",
            DockPanel::lazy("Workspace", |cx| {
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
            .with_dock_class(PRIMARY_DOCK_CLASS),
        )
        .panel(
            "editor",
            DockPanel::lazy("Editor", |cx| {
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
            .with_dock_class(PRIMARY_DOCK_CLASS),
        )
        .panel(
            "preview",
            DockPanel::lazy("Preview", |cx| {
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
            .with_dock_class(SECONDARY_DOCK_CLASS),
        )
        .panel(
            "diff",
            DockPanel::lazy("Diff", |cx| {
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
            .with_dock_class(SECONDARY_DOCK_CLASS),
        )
        .panel(
            "terminal",
            DockPanel::lazy("Terminal", |cx| {
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
            .with_dock_class(PRIMARY_DOCK_CLASS),
        )
        .panel(
            "problems",
            DockPanel::lazy("Problems", |cx| {
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
            .with_dock_class(PRIMARY_DOCK_CLASS),
        )
        .panel(
            "central-note",
            DockPanel::lazy("Central note", |cx| {
                cx.new(|_| {
                    DemoPanel::new(
                        "Central note",
                        "Central-only dogfood panel",
                        0x4f46e5,
                        &[
                            "This panel is classed for the empty central viewport.",
                            "Secondary-class panels should reject here.",
                            "Opening content recovers the central region identity.",
                        ],
                    )
                })
                .into()
            })
            .with_dock_class(CENTRAL_DOCK_CLASS),
        )
        .try_build()
        .expect("demo controller setup should validate")
}

fn dogfood_descriptor(title: impl Into<String>, dock_class: &str) -> DockPanelDescriptor {
    DockPanelDescriptor::new(title).with_dock_class(dock_class)
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
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,open_gpui_docking=info,open_gpui=info"),
    )
    .format_timestamp_millis()
    .init();
    log::info!("{DOCKING_DEBUG_PREFIX} starting docking native example");
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
                .attach_panel_view("runtime", runtime_panel.clone())
                .expect("runtime panel descriptor should exist");
        });
        log::info!("{DOCKING_DEBUG_PREFIX} opened runtime panel and attached dock layout");

        let primary_options = restored_viewport_options(&placement, SPACE, primary_bounds);
        let primary_opened = runtime
            .open_viewport(SPACE, primary_options, cx)
            .expect("failed to open primary docking viewport");
        log::info!(
            "{DOCKING_DEBUG_PREFIX} opened primary viewport space={} window_id={:?}",
            SPACE,
            primary_opened.window().window_id()
        );
        let primary_window_id = primary_opened.window().window_id();
        cx.on_window_closed(move |cx, window_id| {
            if window_id == primary_window_id {
                log::info!(
                    "{DOCKING_DEBUG_PREFIX} primary window closed, quitting app window_id={:?}",
                    window_id
                );
                cx.quit();
            }
        })
        .detach();

        let secondary_options =
            restored_viewport_options(&placement, SECONDARY_SPACE, secondary_bounds);
        let _secondary_opened = runtime
            .open_viewport(SECONDARY_SPACE, secondary_options, cx)
            .expect("failed to open secondary docking viewport");
        log::info!("{DOCKING_DEBUG_PREFIX} opened secondary viewport space={SECONDARY_SPACE}");

        let central_options = restored_viewport_options(&placement, CENTRAL_SPACE, central_bounds);
        let _central_opened = runtime
            .open_viewport(CENTRAL_SPACE, central_options, cx)
            .expect("failed to open empty central docking viewport");
        log::info!("{DOCKING_DEBUG_PREFIX} opened central viewport space={CENTRAL_SPACE}");

        cx.activate(true);
        log::info!("{DOCKING_DEBUG_PREFIX} application activated");
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext};
    use open_gpui_docking::{
        DockActionApplyError, DockActionOutcome, DockClassId, DockGraph, DockHost, DockNode,
        DockNodeId, DockPolicyError, DockViewportCoordinateStatusRecord, DockViewportInputStatus,
        DockViewportPlatformRequestStatus, DockViewportRouteStatus, DropZone, SplitAxis,
    };

    fn item(id: &str) -> DockItemId {
        DockItemId::from(id)
    }

    fn tabs_items(graph: &DockGraph, tabs: DockNodeId) -> (Vec<DockItemId>, usize) {
        let DockNode::Tabs { items, selected } = graph.node(tabs).expect("tabs node should exist")
        else {
            panic!("node should be tabs");
        };
        let selected_index = selected
            .as_ref()
            .and_then(|selected| items.iter().position(|item| item == selected))
            .expect("tabs node should select one of its items");
        (items.clone(), selected_index)
    }

    fn tab_selector(space: &str, tabs: DockNodeId, item: &str) -> String {
        format!("dock:{space}:tabs:{}:tab:{item}", tabs.as_u64())
    }

    fn tabs_selector(space: &str, tabs: DockNodeId) -> String {
        format!("dock:{space}:tabs:{}", tabs.as_u64())
    }

    fn floating_handle_selector(space: &str, floating: DockNodeId) -> String {
        format!("dock:{space}:floating:{}:handle", floating.as_u64())
    }

    fn drop_preview_selector(space: &str) -> String {
        format!("dock:{space}:drop-preview")
    }

    fn drop_guide_selector(space: &str, tabs: DockNodeId, zone: DropZone) -> String {
        format!("dock:{space}:drop-guide:inner:{}:{zone:?}", tabs.as_u64())
    }

    fn debug_bounds(cx: &mut VisualTestContext, selector: impl Into<String>) -> Bounds<Pixels> {
        let selector: &'static str = Box::leak(selector.into().into_boxed_str());
        cx.debug_bounds(selector)
            .unwrap_or_else(|| panic!("debug selector {selector} should have bounds"))
    }

    fn axis_for_zone(zone: DropZone) -> SplitAxis {
        match zone {
            DropZone::Left | DropZone::Right => SplitAxis::Horizontal,
            DropZone::Top | DropZone::Bottom => SplitAxis::Vertical,
            DropZone::Center => unreachable!("center does not create a split"),
        }
    }

    fn simulate_cross_window_left_drag(
        source: &mut VisualTestContext,
        target: &mut VisualTestContext,
        start: open_gpui::Point<Pixels>,
        end: open_gpui::Point<Pixels>,
    ) {
        let threshold = point(start.x + px(24.0), start.y);
        source.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        source.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
        target.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
        target.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    }

    fn open_dogfood_viewport(
        cx: &mut TestAppContext,
        runtime: &DockViewportRuntimeHandle,
        space: &str,
        bounds: Bounds<Pixels>,
    ) -> (Entity<DockHost>, VisualTestContext) {
        let opened = cx
            .update(|app| runtime.open_viewport(space, viewport_window_options(bounds), app))
            .expect("dogfood viewport should open");
        let window = opened
            .window()
            .downcast::<DockHost>()
            .expect("dogfood viewport should render DockHost");
        let host = window
            .root(cx)
            .expect("dogfood viewport should expose DockHost root");
        cx.run_until_parked();
        let visual = VisualTestContext::from_window(opened.window(), cx);
        (host, visual)
    }

    #[test]
    fn restored_layout_exposes_native_dogfood_spaces() {
        let layout = restored_demo_layout();
        let graph = DockGraph::import_layout(&layout).expect("demo layout should import");
        let primary = DockSpaceId::from(SPACE);
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let central = DockSpaceId::from(CENTRAL_SPACE);

        assert!(graph.root(&primary).is_some());
        assert!(graph.root(&secondary).is_some());
        assert_eq!(graph.root(&central), None);

        let preview = item("preview");
        let diff = item("diff");
        let (secondary_tabs, preview_index) = graph
            .find_item_in_space(&secondary, &preview)
            .expect("preview should start in the secondary space");
        let (diff_tabs, diff_index) = graph
            .find_item_in_space(&secondary, &diff)
            .expect("diff should start in the secondary space");
        assert_eq!(secondary_tabs, diff_tabs);
        assert_eq!(preview_index, 0);
        assert_eq!(diff_index, 1);
        let DockNode::Tabs { items, .. } = graph
            .node(secondary_tabs)
            .expect("secondary stack should exist")
        else {
            panic!("secondary dogfood node should be tabs");
        };
        assert_eq!(items.as_slice(), &[preview, diff]);

        let problems = item("problems");
        let (problem_tabs, _) = graph
            .find_item_in_space(&primary, &problems)
            .expect("problems should start in an in-window floating stack");
        let problem_root = graph
            .root_for_node_in_space(&primary, problem_tabs)
            .expect("problems stack should have a root in the primary space");
        assert!(
            graph
                .floating_containers(&primary)
                .iter()
                .any(|floating| floating.node == problem_root),
            "problems should be reachable through a floating container"
        );

        let central_region = graph
            .central_region(&central)
            .expect("empty central dogfood space should keep central metadata");
        assert_eq!(central_region.node, None);
        assert!(central_region.keep_alive_when_empty);
        assert!(central_region.passthrough_when_empty);
    }

    #[test]
    fn built_controller_preserves_dogfood_panel_policy() {
        let controller = build_controller();
        let workspace = controller
            .panels()
            .descriptor(&item("workspace"))
            .expect("workspace descriptor should be registered");
        assert!(
            workspace.is_closable(),
            "demo default should let the primary window close; Prevent policy remains available in the runtime panel"
        );
        assert_eq!(
            workspace.dock_class(),
            Some(&DockClassId::from(PRIMARY_DOCK_CLASS))
        );

        for id in ["preview", "diff", "runtime", "problems", "central-note"] {
            assert!(
                controller.panels().descriptor(&item(id)).is_some(),
                "{id} descriptor should be registered for native dogfood"
            );
        }
        assert_eq!(
            controller
                .panels()
                .descriptor(&item("preview"))
                .and_then(|descriptor| descriptor.dock_class()),
            Some(&DockClassId::from(SECONDARY_DOCK_CLASS))
        );
        assert!(controller.policy().allows_dock_class_in_space(
            &DockSpaceId::from(SECONDARY_SPACE),
            Some(&DockClassId::from(SECONDARY_DOCK_CLASS)),
        ));
        assert!(!controller.policy().allows_dock_class_in_space(
            &DockSpaceId::from(CENTRAL_SPACE),
            Some(&DockClassId::from(SECONDARY_DOCK_CLASS)),
        ));
    }

    #[open_gpui::test]
    fn default_runtime_policy_allows_primary_dogfood_window_close(cx: &mut TestAppContext) {
        let controller = cx.new(|_| build_controller());
        let runtime = DockViewportRuntimeHandle::new(controller);
        let (_primary_host, mut primary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SPACE,
            Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0))),
        );

        assert_eq!(
            runtime.close_policy(),
            DockViewportClosePolicy::RetainLayout
        );
        assert!(
            primary_visual.simulate_close(),
            "default dogfood close policy should not veto the primary window"
        );
    }

    #[test]
    fn dogfood_restore_controls_reopen_registered_panels() {
        let mut controller = build_controller();
        let primary = DockSpaceId::from(SPACE);
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let preview = item("preview");
        let diff = item("diff");
        let outline = item("outline");

        assert_eq!(
            controller
                .close_item(secondary.clone(), preview.clone())
                .expect("preview should close from secondary dogfood stack"),
            DockActionOutcome::Changed
        );
        assert_eq!(
            controller
                .close_item(secondary.clone(), diff.clone())
                .expect("diff should close from secondary dogfood stack"),
            DockActionOutcome::Changed
        );
        assert!(
            controller
                .graph()
                .find_item_in_space(&secondary, &preview)
                .is_none()
        );
        assert!(
            controller
                .graph()
                .find_item_in_space(&secondary, &diff)
                .is_none()
        );

        assert_eq!(
            restore_secondary_panels(&mut controller),
            "opened preview: Changed; opened diff: Changed"
        );
        let (preview_tabs, preview_index) = controller
            .graph()
            .find_item_in_space(&secondary, &preview)
            .expect("preview should reopen into secondary dogfood stack");
        let (diff_tabs, diff_index) = controller
            .graph()
            .find_item_in_space(&secondary, &diff)
            .expect("diff should reopen into secondary dogfood stack");
        assert_eq!(preview_tabs, diff_tabs);
        assert_eq!(preview_index, 0);
        assert_eq!(diff_index, 1);

        assert_eq!(
            controller
                .close_item(primary.clone(), outline.clone())
                .expect("outline should close while descriptor remains registered"),
            DockActionOutcome::Changed
        );
        assert!(
            controller
                .graph()
                .find_item_in_space(&primary, &outline)
                .is_none()
        );

        assert_eq!(
            restore_outline_panel(&mut controller),
            "opened outline: Changed"
        );
        assert!(
            controller
                .graph()
                .find_item_in_space(&primary, &outline)
                .is_some()
        );
    }

    #[test]
    fn dogfood_class_policy_rejects_secondary_stack_in_central_but_allows_central_note() {
        let mut controller = build_controller();
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let central = DockSpaceId::from(CENTRAL_SPACE);
        let preview = item("preview");
        let (secondary_tabs, _) = controller
            .graph()
            .find_item_in_space(&secondary, &preview)
            .expect("preview should start in secondary dogfood space");

        let err = controller
            .float_tabs_in_window(
                secondary,
                secondary_tabs,
                central.clone(),
                Bounds::new(point(px(80.0), px(40.0)), size(px(260.0), px(180.0))),
            )
            .expect_err("secondary-class stack should reject the central-only dogfood space");

        assert_eq!(
            err,
            DockActionApplyError::Policy(DockPolicyError::DockClassRejected {
                space: central.clone(),
                item: preview,
                dock_class: Some(DockClassId::from(SECONDARY_DOCK_CLASS)),
            })
        );
        assert_eq!(
            restore_central_note_panel(&mut controller),
            "opened central note: Changed"
        );
        assert!(
            controller
                .graph()
                .find_item_in_space(&central, &item("central-note"))
                .is_some()
        );
        let central_region = controller
            .graph()
            .central_region(&central)
            .expect("central dogfood space should keep central metadata");
        assert!(
            central_region.node.is_some(),
            "opening central-note should recover central identity instead of ordinary root-only state"
        );
    }

    #[test]
    fn dogfood_whole_stack_can_float_and_merge_back_without_reordering() {
        let mut controller = build_controller();
        let primary = DockSpaceId::from(SPACE);
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let preview = item("preview");
        let diff = item("diff");
        let editor = item("editor");
        let (secondary_tabs, _) = controller
            .graph()
            .find_item_in_space(&secondary, &preview)
            .expect("secondary dogfood stack should contain preview");
        let (secondary_items, secondary_active) = tabs_items(controller.graph(), secondary_tabs);
        assert_eq!(secondary_items, vec![preview.clone(), diff.clone()]);
        let secondary_active_item = secondary_items[secondary_active].clone();

        assert_eq!(
            controller
                .float_tabs_in_window(
                    secondary.clone(),
                    secondary_tabs,
                    primary.clone(),
                    Bounds::new(point(px(560.0), px(96.0)), size(px(320.0), px(220.0))),
                )
                .expect("secondary stack should float into primary dogfood space"),
            DockActionOutcome::Changed
        );
        assert_eq!(controller.graph().root(&secondary), None);
        assert_eq!(
            controller.graph().floating_containers(&primary).len(),
            2,
            "primary should keep its existing problems floating stack plus the moved secondary stack"
        );
        let moved_floating = controller
            .graph()
            .floating_containers(&primary)
            .iter()
            .find(|floating| {
                controller.graph().collect_items_in_subtree(floating.node)
                    == vec![preview.clone(), diff.clone()]
            })
            .expect("moved secondary stack should be represented as a primary floating container")
            .node;
        let (editor_tabs, _) = controller
            .graph()
            .find_item_in_space(&primary, &editor)
            .expect("editor target stack should stay in primary space");

        assert_eq!(
            controller
                .merge_floating_into(primary.clone(), moved_floating, editor_tabs)
                .expect("moved stack should merge into primary editor tabs"),
            DockActionOutcome::Changed
        );
        assert!(
            controller
                .graph()
                .floating_containers(&primary)
                .iter()
                .all(|floating| floating.node != moved_floating)
        );
        let (items, active) = tabs_items(controller.graph(), editor_tabs);
        let expected_items = vec![editor, preview, diff];
        let expected_active = expected_items
            .iter()
            .position(|item| item == &secondary_active_item)
            .expect("merged stack should keep its active item");
        assert_eq!(items, expected_items);
        assert_eq!(active, expected_active);
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_tab_drag(cx: &mut TestAppContext) {
        let controller = cx.new(|_| build_controller());
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let primary = DockSpaceId::from(SPACE);
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let preview = item("preview");
        let editor = item("editor");
        let (secondary_tabs, _) = controller.read_with(cx, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&secondary, &preview)
                .expect("preview should start in secondary dogfood space")
        });
        let (editor_tabs, _) = controller.read_with(cx, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&primary, &editor)
                .expect("editor should start in primary dogfood space")
        });

        let (_primary_host, mut primary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SPACE,
            Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0))),
        );
        let (_secondary_host, mut secondary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SECONDARY_SPACE,
            Bounds::new(point(px(944.0), px(0.0)), size(px(460.0), px(360.0))),
        );

        let start = debug_bounds(
            &mut secondary_visual,
            tab_selector(SECONDARY_SPACE, secondary_tabs, "preview"),
        )
        .center();
        let end = debug_bounds(&mut primary_visual, tabs_selector(SPACE, editor_tabs)).center();
        let threshold = point(start.x + px(24.0), start.y);

        secondary_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        secondary_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
        primary_visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert!(
            debug_bounds(&mut primary_visual, drop_preview_selector(SPACE))
                .size
                .width
                > px(0.0),
            "primary viewport should render a host-local drop preview during cross-window drag"
        );

        primary_visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();

        controller.read_with(cx, |controller, _| {
            assert!(
                controller
                    .graph()
                    .find_item_in_space(&secondary, &preview)
                    .is_none(),
                "preview should leave the secondary viewport after rendered drop"
            );
            let (preview_tabs, preview_index) = controller
                .graph()
                .find_item_in_space(&primary, &preview)
                .expect("preview should dock into the primary editor stack");
            assert_eq!(preview_tabs, editor_tabs);
            assert_eq!(preview_index, 1);
            let (items, active) = tabs_items(controller.graph(), editor_tabs);
            assert_eq!(items, vec![editor, preview]);
            assert_eq!(active, 1);
        });
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_stack_drag(cx: &mut TestAppContext) {
        let controller = cx.new(|_| build_controller());
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let primary = DockSpaceId::from(SPACE);
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let preview = item("preview");
        let diff = item("diff");
        let editor = item("editor");
        let (secondary_tabs, secondary_items, secondary_active_item) =
            controller.read_with(cx, |controller, _| {
                let (tabs, _) = controller
                    .graph()
                    .find_item_in_space(&secondary, &preview)
                    .expect("preview should start in secondary dogfood space");
                let (items, active) = tabs_items(controller.graph(), tabs);
                let active_item = items[active].clone();
                (tabs, items, active_item)
            });
        assert_eq!(secondary_items, vec![preview.clone(), diff.clone()]);
        let (editor_tabs, _) = controller.read_with(cx, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&primary, &editor)
                .expect("editor should start in primary dogfood space")
        });

        let (_primary_host, mut primary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SPACE,
            Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0))),
        );
        let (_secondary_host, mut secondary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SECONDARY_SPACE,
            Bounds::new(point(px(944.0), px(0.0)), size(px(460.0), px(360.0))),
        );

        let source_bounds = debug_bounds(
            &mut secondary_visual,
            tabs_selector(SECONDARY_SPACE, secondary_tabs),
        );
        let start = point(
            source_bounds.origin.x + source_bounds.size.width - px(8.0),
            source_bounds.origin.y + px(12.0),
        );
        let end = debug_bounds(&mut primary_visual, tabs_selector(SPACE, editor_tabs)).center();

        simulate_cross_window_left_drag(&mut secondary_visual, &mut primary_visual, start, end);
        cx.run_until_parked();

        controller.read_with(cx, |controller, _| {
            assert_eq!(
                controller.graph().root(&secondary),
                None,
                "whole-stack drag should empty the secondary viewport root"
            );
            let (items, active) = tabs_items(controller.graph(), editor_tabs);
            let expected_items = vec![editor, preview, diff];
            let expected_active = expected_items
                .iter()
                .position(|item| item == &secondary_active_item)
                .expect("merged stack should keep its active item");
            assert_eq!(items, expected_items);
            assert_eq!(active, expected_active);
        });
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_tab_drag_into_primary_editor_left_edge(
        cx: &mut TestAppContext,
    ) {
        let controller = cx.new(|_| build_controller());
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let primary = DockSpaceId::from(SPACE);
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let preview = item("preview");
        let diff = item("diff");
        let editor = item("editor");
        let (secondary_tabs, _) = controller.read_with(cx, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&secondary, &preview)
                .expect("preview should start in secondary dogfood space")
        });
        let (editor_tabs, _) = controller.read_with(cx, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&primary, &editor)
                .expect("editor should start in primary dogfood space")
        });

        let (_primary_host, mut primary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SPACE,
            Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0))),
        );
        let (_secondary_host, mut secondary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SECONDARY_SPACE,
            Bounds::new(point(px(944.0), px(0.0)), size(px(460.0), px(360.0))),
        );

        let start = debug_bounds(
            &mut secondary_visual,
            tab_selector(SECONDARY_SPACE, secondary_tabs, "preview"),
        )
        .center();
        let editor_bounds = debug_bounds(&mut primary_visual, tabs_selector(SPACE, editor_tabs));
        let editor_hover = editor_bounds.center();
        let threshold = point(start.x + px(24.0), start.y);

        secondary_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        secondary_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
        primary_visual.simulate_mouse_move(editor_hover, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();

        let left_edge = debug_bounds(
            &mut primary_visual,
            drop_guide_selector(SPACE, editor_tabs, DropZone::Left),
        )
        .center();
        primary_visual.simulate_mouse_move(left_edge, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert!(
            debug_bounds(&mut primary_visual, drop_preview_selector(SPACE))
                .size
                .width
                > px(0.0),
            "primary editor stack should render a left-edge split preview"
        );

        primary_visual.simulate_mouse_up(left_edge, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();

        controller.read_with(cx, |controller, _| {
            assert!(
                controller
                    .graph()
                    .find_item_in_space(&secondary, &preview)
                    .is_none(),
                "preview should leave the secondary viewport after left-edge drop"
            );
            assert!(
                controller
                    .graph()
                    .find_item_in_space(&secondary, &diff)
                    .is_some(),
                "diff should remain in the secondary viewport after moving only preview"
            );
            let (preview_tabs, preview_index) = controller
                .graph()
                .find_item_in_space(&primary, &preview)
                .expect("preview should dock into the primary editor area");
            assert_ne!(
                preview_tabs, editor_tabs,
                "left-edge drop must split the editor stack instead of center-merging"
            );
            assert_eq!(preview_index, 0);
            assert_eq!(
                controller.graph().collect_items_in_subtree(preview_tabs),
                vec![preview.clone()],
                "left split child should contain only the moved preview tab"
            );
            assert_eq!(
                controller.graph().collect_items_in_subtree(editor_tabs),
                vec![editor],
                "original editor stack should remain intact on the right"
            );
        });
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_tab_drag_into_primary_floating_title_bar(
        cx: &mut TestAppContext,
    ) {
        let controller = cx.new(|_| build_controller());
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let primary = DockSpaceId::from(SPACE);
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let preview = item("preview");
        let problems = item("problems");
        let (secondary_tabs, _) = controller.read_with(cx, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&secondary, &preview)
                .expect("preview should start in secondary dogfood space")
        });
        let (problem_tabs, problem_floating) = controller.read_with(cx, |controller, _| {
            let (tabs, _) = controller
                .graph()
                .find_item_in_space(&primary, &problems)
                .expect("problems should start in the primary floating stack");
            let floating = controller
                .graph()
                .root_for_node_in_space(&primary, tabs)
                .expect("primary problems stack should have a floating root");
            (tabs, floating)
        });

        let (_primary_host, mut primary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SPACE,
            Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0))),
        );
        let (_secondary_host, mut secondary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SECONDARY_SPACE,
            Bounds::new(point(px(944.0), px(0.0)), size(px(460.0), px(360.0))),
        );

        let start = debug_bounds(
            &mut secondary_visual,
            tab_selector(SECONDARY_SPACE, secondary_tabs, "preview"),
        )
        .center();
        let end = debug_bounds(
            &mut primary_visual,
            floating_handle_selector(SPACE, problem_floating),
        )
        .center();
        let threshold = point(start.x + px(24.0), start.y);

        secondary_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        secondary_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
        primary_visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();

        assert!(
            debug_bounds(&mut primary_visual, drop_preview_selector(SPACE))
                .size
                .width
                > px(0.0),
            "primary viewport should render a drop preview on the floating title bar"
        );

        primary_visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();

        controller.read_with(cx, |controller, _| {
            assert!(
                controller
                    .graph()
                    .find_item_in_space(&secondary, &preview)
                    .is_none(),
                "preview should leave the secondary viewport after floating-title-bar drop"
            );
            let (preview_tabs, preview_index) = controller
                .graph()
                .find_item_in_space(&primary, &preview)
                .expect("preview should dock into the primary floating stack");
            assert_eq!(preview_tabs, problem_tabs);
            assert_eq!(preview_index, 1);
            let (items, active) = tabs_items(controller.graph(), problem_tabs);
            assert_eq!(items, vec![problems, preview]);
            assert_eq!(active, 1);
            assert_eq!(
                controller.graph().floating_containers(&primary).len(),
                1,
                "primary should keep a single floating container after merging into the title bar"
            );
        });
    }

    fn assert_cross_window_tab_drag_into_primary_floating_stack_guide(
        cx: &mut TestAppContext,
        zone: DropZone,
    ) {
        let controller = cx.new(|_| build_controller());
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let primary = DockSpaceId::from(SPACE);
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let preview = item("preview");
        let diff = item("diff");
        let problems = item("problems");
        let (secondary_tabs, _) = controller.read_with(cx, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&secondary, &preview)
                .expect("preview should start in secondary dogfood space")
        });
        let (problem_tabs, problem_floating) = controller.read_with(cx, |controller, _| {
            let (tabs, _) = controller
                .graph()
                .find_item_in_space(&primary, &problems)
                .expect("problems should start in the primary floating stack");
            let floating = controller
                .graph()
                .root_for_node_in_space(&primary, tabs)
                .expect("primary problems stack should have a floating root");
            (tabs, floating)
        });

        let (_primary_host, mut primary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SPACE,
            Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0))),
        );
        let (_secondary_host, mut secondary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SECONDARY_SPACE,
            Bounds::new(point(px(944.0), px(0.0)), size(px(460.0), px(360.0))),
        );

        let start = debug_bounds(
            &mut secondary_visual,
            tab_selector(SECONDARY_SPACE, secondary_tabs, "preview"),
        )
        .center();
        let target_bounds = debug_bounds(&mut primary_visual, tabs_selector(SPACE, problem_tabs));
        let target_hover = target_bounds.center();
        let threshold = point(start.x + px(24.0), start.y);

        secondary_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        secondary_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
        primary_visual.simulate_mouse_move(target_hover, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        let end = debug_bounds(
            &mut primary_visual,
            drop_guide_selector(SPACE, problem_tabs, zone),
        )
        .center();
        primary_visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();

        assert!(
            debug_bounds(&mut primary_visual, drop_preview_selector(SPACE))
                .size
                .width
                > px(0.0),
            "primary viewport should render a {zone:?} drop preview inside the floating stack"
        );

        primary_visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();

        controller.read_with(cx, |controller, _| {
            assert!(
                controller
                    .graph()
                    .find_item_in_space(&secondary, &preview)
                    .is_none(),
                "preview should leave the secondary viewport after a {zone:?} floating-stack drop"
            );
            assert!(
                controller
                    .graph()
                    .find_item_in_space(&secondary, &diff)
                    .is_some(),
                "diff should remain in the secondary viewport after moving only preview"
            );
            assert_eq!(
                controller.graph().floating_containers(&primary).len(),
                1,
                "primary should keep one floating container after a {zone:?} drop"
            );

            if zone == DropZone::Center {
                let (preview_tabs, preview_index) = controller
                    .graph()
                    .find_item_in_space(&primary, &preview)
                    .expect("preview should merge into the primary floating stack");
                assert_eq!(preview_tabs, problem_tabs);
                assert_eq!(preview_index, 1);
                let (items, active) = tabs_items(controller.graph(), problem_tabs);
                assert_eq!(items, vec![problems.clone(), preview.clone()]);
                assert_eq!(active, 1);
                return;
            }

            let DockNode::Floating { child } = controller
                .graph()
                .node(problem_floating)
                .expect("primary floating root should still exist")
            else {
                panic!("primary floating root should remain floating after a {zone:?} drop");
            };
            let DockNode::Split { axis, children, .. } = controller
                .graph()
                .node(*child)
                .expect("primary floating child should become a split")
            else {
                panic!("primary floating child should be split after a {zone:?} drop");
            };
            assert_eq!(*axis, axis_for_zone(zone), "{zone:?}");
            assert_eq!(children.len(), 2, "{zone:?}");
            let (first_expected, second_expected) = match zone {
                DropZone::Left | DropZone::Top => (vec![preview.clone()], vec![problems.clone()]),
                DropZone::Right | DropZone::Bottom => {
                    (vec![problems.clone()], vec![preview.clone()])
                }
                DropZone::Center => unreachable!("center returned earlier"),
            };
            assert_eq!(
                controller.graph().collect_items_in_subtree(children[0]),
                first_expected,
                "{zone:?} should place the expected items in the first floating child"
            );
            assert_eq!(
                controller.graph().collect_items_in_subtree(children[1]),
                second_expected,
                "{zone:?} should place the expected items in the second floating child"
            );
        });
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_tab_drag_into_primary_floating_stack_center(
        cx: &mut TestAppContext,
    ) {
        assert_cross_window_tab_drag_into_primary_floating_stack_guide(cx, DropZone::Center);
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_tab_drag_into_primary_floating_stack_left(
        cx: &mut TestAppContext,
    ) {
        assert_cross_window_tab_drag_into_primary_floating_stack_guide(cx, DropZone::Left);
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_tab_drag_into_primary_floating_stack_right(
        cx: &mut TestAppContext,
    ) {
        assert_cross_window_tab_drag_into_primary_floating_stack_guide(cx, DropZone::Right);
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_tab_drag_into_primary_floating_stack_top(
        cx: &mut TestAppContext,
    ) {
        assert_cross_window_tab_drag_into_primary_floating_stack_guide(cx, DropZone::Top);
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_tab_drag_into_primary_floating_stack_bottom(
        cx: &mut TestAppContext,
    ) {
        assert_cross_window_tab_drag_into_primary_floating_stack_guide(cx, DropZone::Bottom);
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_tab_drag_then_split_primary_floating_stack(
        cx: &mut TestAppContext,
    ) {
        let controller = cx.new(|_| build_controller());
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let primary = DockSpaceId::from(SPACE);
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let preview = item("preview");
        let diff = item("diff");
        let problems = item("problems");
        let (secondary_tabs, _) = controller.read_with(cx, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&secondary, &preview)
                .expect("preview should start in secondary dogfood space")
        });
        let (_problem_tabs, problem_floating) = controller.read_with(cx, |controller, _| {
            let (tabs, _) = controller
                .graph()
                .find_item_in_space(&primary, &problems)
                .expect("problems should start in the primary floating stack");
            let floating = controller
                .graph()
                .root_for_node_in_space(&primary, tabs)
                .expect("primary problems stack should have a floating root");
            (tabs, floating)
        });

        let (_primary_host, mut primary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SPACE,
            Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0))),
        );
        let (_secondary_host, mut secondary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SECONDARY_SPACE,
            Bounds::new(point(px(944.0), px(0.0)), size(px(460.0), px(360.0))),
        );

        let start = debug_bounds(
            &mut secondary_visual,
            tab_selector(SECONDARY_SPACE, secondary_tabs, "preview"),
        )
        .center();
        let end = debug_bounds(
            &mut primary_visual,
            floating_handle_selector(SPACE, problem_floating),
        )
        .center();
        let threshold = point(start.x + px(24.0), start.y);

        secondary_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        secondary_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
        primary_visual.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        primary_visual.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();

        let merged_problem_tabs = controller.read_with(cx, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&primary, &problems)
                .map(|(tabs, _)| tabs)
                .expect("problems should stay in the primary floating stack after merge")
        });

        let diff_start = debug_bounds(
            &mut secondary_visual,
            tab_selector(SECONDARY_SPACE, secondary_tabs, "diff"),
        )
        .center();
        let diff_end = debug_bounds(
            &mut primary_visual,
            tabs_selector(SPACE, merged_problem_tabs),
        );
        let diff_hover = diff_end.center();
        let diff_threshold = point(diff_start.x + px(24.0), diff_start.y);

        secondary_visual.simulate_mouse_down(diff_start, MouseButton::Left, Modifiers::none());
        secondary_visual.simulate_mouse_move(diff_threshold, MouseButton::Left, Modifiers::none());
        primary_visual.simulate_mouse_move(diff_hover, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        let diff_target = debug_bounds(
            &mut primary_visual,
            drop_guide_selector(SPACE, merged_problem_tabs, DropZone::Bottom),
        )
        .center();
        primary_visual.simulate_mouse_move(diff_target, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();

        assert!(
            debug_bounds(&mut primary_visual, drop_preview_selector(SPACE))
                .size
                .width
                > px(0.0),
            "primary viewport should render a split preview inside the floating stack"
        );

        primary_visual.simulate_mouse_up(diff_target, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();

        controller.read_with(cx, |controller, _| {
            assert!(
                controller
                    .graph()
                    .find_item_in_space(&secondary, &diff)
                    .is_none(),
                "diff should leave the secondary viewport after the nested split"
            );
            assert_eq!(
                controller.graph().root(&secondary),
                None,
                "secondary viewport should become empty after moving its last tab"
            );
            assert_eq!(
                controller.graph().floating_containers(&primary).len(),
                1,
                "primary should still have one floating container after the nested split"
            );
            let DockNode::Floating { child } = controller
                .graph()
                .node(problem_floating)
                .expect("primary floating root should still exist")
            else {
                panic!("primary floating root should remain floating");
            };
            let DockNode::Split { axis, children, .. } = controller
                .graph()
                .node(*child)
                .expect("primary floating child should become a split")
            else {
                panic!("primary floating child should be split after docking diff");
            };
            assert_eq!(*axis, SplitAxis::Vertical);
            assert_eq!(children.len(), 2);
            assert_eq!(
                controller.graph().collect_items_in_subtree(children[0]),
                vec![problems, preview],
                "top child should preserve the original floating stack"
            );
            assert_eq!(
                controller.graph().collect_items_in_subtree(children[1]),
                vec![diff],
                "bottom child should contain the newly docked diff tab"
            );
        });
    }

    #[test]
    fn saved_placement_restores_all_dogfood_viewport_titles() {
        let primary_bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0)));
        let secondary_bounds = Bounds::new(point(px(944.0), px(0.0)), size(px(460.0), px(360.0)));
        let central_bounds = Bounds::new(point(px(944.0), px(384.0)), size(px(460.0), px(220.0)));
        let placement = saved_viewport_placement(primary_bounds, secondary_bounds, central_bounds);
        assert_eq!(placement.viewports.len(), 3);

        assert_viewport_title(&placement, SPACE, primary_bounds, "Docking demo");
        assert_viewport_title(
            &placement,
            SECONDARY_SPACE,
            secondary_bounds,
            "Docking preview",
        );
        assert_viewport_title(
            &placement,
            CENTRAL_SPACE,
            central_bounds,
            "Empty central dogfood",
        );
    }

    #[test]
    fn runtime_status_panel_formats_platform_capabilities() {
        let capabilities = DockViewportPlatformCapabilityRecord {
            global_window_bounds: true,
            window_stack: false,
            display_work_area: true,
            dpi_scale: false,
            live_window_move: true,
            no_input_windows: false,
            hovered_window_ignores_no_input: true,
        };
        let flag_capabilities = DockViewportPlatformFlagCapabilityRecord {
            no_focus_on_appearing_windows: true,
            no_focus_on_click_windows: false,
            alpha_windows: false,
            topmost_windows: true,
            no_taskbar_windows: false,
        };

        assert_eq!(
            route_capability_summary(Some(&capabilities)),
            "bounds=yes, stack=no, hover-through-no-input=yes"
        );
        assert_eq!(
            placement_capability_summary(Some(&capabilities)),
            "work-area=yes, dpi=no, live-move=yes, no-input=no"
        );
        assert_eq!(
            viewport_flag_capability_summary(Some(&flag_capabilities)),
            "no-focus-appearing=yes, no-focus-click=no, alpha=no, topmost=yes, no-taskbar=no"
        );
        assert_eq!(route_capability_summary(None), "unavailable");
        assert_eq!(placement_capability_summary(None), "unavailable");
        assert_eq!(viewport_flag_capability_summary(None), "unavailable");
        assert_eq!(route_unavailable_summary(None), "none");
        assert_eq!(
            route_unavailable_summary(Some(
                &DockViewportReleaseUnavailableRecord::BlockedByViewportWindow
            )),
            "blocked-window"
        );
        assert_eq!(coordinate_status_summary(&[]), "none");
        assert_eq!(
            coordinate_status_summary(&[
                DockViewportLifecycleRecord {
                    space: DockSpaceId::from(SPACE),
                    window_id: open_gpui::WindowId::from(1),
                    route_status: DockViewportRouteStatus::RouteReady,
                    input_status: DockViewportInputStatus::ReceivesInput,
                    platform_request_status: DockViewportPlatformRequestStatus::default(),
                    coordinate_status: Some(DockViewportCoordinateStatusRecord {
                        display_id: None,
                        coordinate_space: DockViewportCoordinateSpaceRecord::GlobalScreen,
                        facts_generation: 7,
                    }),
                    facts_generation: 7,
                },
                DockViewportLifecycleRecord {
                    space: DockSpaceId::from(SECONDARY_SPACE),
                    window_id: open_gpui::WindowId::from(2),
                    route_status: DockViewportRouteStatus::RegisteredNotReady,
                    input_status: DockViewportInputStatus::ReceivesInput,
                    platform_request_status: DockViewportPlatformRequestStatus::default(),
                    coordinate_status: Some(DockViewportCoordinateStatusRecord {
                        display_id: None,
                        coordinate_space: DockViewportCoordinateSpaceRecord::WindowLocal,
                        facts_generation: 8,
                    }),
                    facts_generation: 8,
                },
                DockViewportLifecycleRecord {
                    space: DockSpaceId::from(CENTRAL_SPACE),
                    window_id: open_gpui::WindowId::from(3),
                    route_status: DockViewportRouteStatus::RegisteredNotReady,
                    input_status: DockViewportInputStatus::ReceivesInput,
                    platform_request_status: DockViewportPlatformRequestStatus::default(),
                    coordinate_status: None,
                    facts_generation: 0,
                },
            ]),
            "docking-demo=global@gen7, docking-preview=local@gen8, docking-empty-central=missing"
        );
        assert_eq!(platform_sync_summary(None), "none");
        assert_eq!(
            platform_sync_summary(Some(&DockViewportPlatformSyncRecord {
                window_id: open_gpui::WindowId::from(4),
                applied: Vec::new(),
                skipped_requests: Vec::new(),
                unsupported_requests: Vec::new(),
            })),
            "applied=0, skipped=0, unsupported=0"
        );
        assert_eq!(
            preview_proof_summary(),
            "presentation-scene+real-content-reveal+overlay-motion+tab-insertion+retargeting+splitter-motion+zoom-focus+divider-hit-map+corner-drag+a11y+route-cleanup+reduced-motion"
        );
        assert_eq!(
            motion_runtime_proof_summary(),
            "shared-runtime+run-state+scalar-value+scalar-sample+explicit-models+policy-gates+layout-projection+projection-clips+sampled-progress+retargeted-identity+reduced-motion-final-state+high-frequency-bypass"
        );
        assert_eq!(
            placement_restore_summary(Some(&DockViewportRestoreReadinessRecord {
                matched: 2,
                missing: 1,
            })),
            "matched=2, missing=1"
        );
        assert_eq!(placement_restore_summary(None), "unavailable");
        assert_eq!(
            tear_off_placement_summary(Some(&DockViewportTearOffPlacementRecord::Suggested)),
            "suggested"
        );
        assert_eq!(
            tear_off_placement_summary(Some(&DockViewportTearOffPlacementRecord::DragGeometry)),
            "drag-geometry"
        );
        assert_eq!(tear_off_placement_summary(None), "unavailable");
        assert_eq!(capability_flag(true), "yes");
        assert_eq!(capability_flag(false), "no");
    }

    fn assert_viewport_title(
        placement: &DockViewportPlacementLayout,
        space: &str,
        fallback_bounds: Bounds<Pixels>,
        expected: &str,
    ) {
        let options = restored_viewport_options(placement, space, fallback_bounds);
        let title = options
            .titlebar
            .as_ref()
            .and_then(|titlebar| titlebar.title.as_ref())
            .map(ToString::to_string);
        assert_eq!(title.as_deref(), Some(expected));
    }
}
