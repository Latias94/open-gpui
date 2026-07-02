use jellyflow::{
    NodeGraphStore,
    core::{
        CanvasPoint as JellyPoint, CanvasRect as JellyRect, CanvasSize as JellySize, Edge,
        EdgeId as JellyEdgeId, Graph, GraphOp, GraphTransaction, Node, NodeId as JellyNodeId,
        NodeKindKey, PortDirection, PortId as JellyPortId, PortKey,
    },
    runtime::{
        io::{NodeGraphEditorConfig, NodeGraphViewState},
        rules::EdgeEndpoint,
        runtime::{
            chrome::{
                NodeChromeFactsRequest, NodeChromeLayoutPolicy, NodeChromeState,
                ResolvedNodeChrome, resolve_node_chrome_facts,
            },
            connection::ConnectionHandleRef,
            geometry::{HandleBounds, HandlePosition},
            measurement::{NodeHandleMeasurementSource, NodeMeasurement, NodeMeasurementStatus},
        },
        schema::{
            MenuSurface, NodeChromeKind, NodeKindViewDescriptor, NodeKitRegistry, NodeRegistry,
            NodeSurfaceProjection, NodeSurfaceSlotDescriptor, NodeSurfaceSlotKind,
            NodeSurfaceSlotProjection,
        },
    },
};
#[cfg(test)]
use jellyflow::{
    core::{EdgeKind, GraphId, Port, PortCapacity, PortKind},
    layout::{LayoutPresetBuilder, builtin_layout_engine_registry},
};
#[cfg(test)]
use jellyflow_open_gpui::OpenGpuiNodeRendererResolution;
use jellyflow_open_gpui::{
    OpenGpuiActionPlan, OpenGpuiActionSurface, OpenGpuiAdapter, OpenGpuiAuthoringController,
    OpenGpuiAuthoringOutcome, OpenGpuiAuthoringSkipReason, OpenGpuiBlackboardPlan,
    OpenGpuiBoundsCollector, OpenGpuiConnectionSyncError, OpenGpuiConnectionSyncRequest,
    OpenGpuiControlEditPlan, OpenGpuiControlEventValue, OpenGpuiControlPlan,
    OpenGpuiDroppedWireInsertError, OpenGpuiDynamicPortPolicy, OpenGpuiInspectorPlan,
    OpenGpuiInspectorSurface, OpenGpuiInspectorTargetBounds, OpenGpuiInspectorTargetSource,
    OpenGpuiMeasurementContext, OpenGpuiMeasurementId,
    OpenGpuiMeasurementMode as NodeSurfaceMeasurementSource, OpenGpuiMenuPlan,
    OpenGpuiNodeRendererContext, OpenGpuiNodeRendererOutputSource, OpenGpuiNodeRendererRegistry,
    OpenGpuiNodeRendererState, OpenGpuiNodeRendererTable,
    OpenGpuiNodeSurfaceLayout as NodeSurfaceComponentLayout,
    OpenGpuiNodeSurfaceSlotLayout as NodeSurfaceSlotLayout, OpenGpuiNodeTransformSnapshot,
    OpenGpuiProductSurfacePreset, OpenGpuiRepeatableActionPlan,
    OpenGpuiRepeatableItemLayout as NodeRepeatableItemLayout,
    OpenGpuiRepeatableItemProjection as NodeRepeatableItemProjection,
    OpenGpuiRepeatableSurfaceLayout as NodeRepeatableSurfaceLayout,
    OpenGpuiRepeatableSurfaceProjection as NodeRepeatableSurfaceProjection, OpenGpuiViewPoint,
    apply_dropped_wire_insert, assign_layout_pass_measurement_revision,
    layout_pass_measurement_from_regions, measured_surface_anchors,
    open_gpui_action_summary_element_id, open_gpui_blackboard_item_element_id,
    open_gpui_blackboard_status_element_id, open_gpui_chrome_fallback_button_element_id,
    open_gpui_node_renderer_context, open_gpui_repeatable_add_action_element_id,
    open_gpui_repeatable_collection_element_id, open_gpui_repeatable_item_element_id,
    open_gpui_repeatable_remove_action_element_id, open_gpui_repeatable_reorder_action_element_id,
    open_gpui_slot_action_label_element_id, open_gpui_slot_badge_element_id,
    open_gpui_slot_preview_progress_element_id, open_gpui_slot_status_label_element_id,
    open_gpui_slot_value_element_id, project_actions_for_surface,
    project_blackboards_for_descriptor, project_dropped_wire_menu, project_inspectors_for_surface,
    project_menu, project_node_measurement, project_slot_controls,
    projected_node_surface_graph_layout, repeatable_item_projection, repeatable_surface_projection,
    resolve_inspector_target_bounds,
};
use open_gpui::{
    AnyElement, App, Bounds, Context, FocusHandle, Hsla, KeyDownEvent, Modifiers, MouseButton,
    MouseDownEvent, Pixels, WeakEntity, Window, WindowBounds, WindowOptions, div, point,
    prelude::*, px, rgb, size,
};
use open_gpui_canvas::{
    CanvasDocument, CanvasEditor, CanvasEditorInputHandler, CanvasEvent, CanvasHandle,
    CanvasKeyModifiers, CanvasKindLabel, CanvasKindPaint, CanvasKindRegistry, CanvasNode,
    CanvasNodeKind, CanvasNodeRenderPolicy, CanvasPaintModel, CanvasPaintOptions, CanvasPaintTheme,
    CanvasTool, CanvasToolIntent, CanvasViewport, DocumentError, HandleRole, HitTarget, NodeId,
    PointerButton, canvas_editor_view_with_frame, connection_hit_options,
};
#[cfg(test)]
use open_gpui_canvas::{CanvasTransaction, DocumentCommand};
use open_gpui_platform::application;
use open_gpui_ui_components::gpui_adapter::init_text_input;
use open_gpui_ui_components::prelude::Sizable;
use open_gpui_ui_components::{Badge, BadgeVariant, Button, ButtonVariant, Progress};
use open_gpui_ui_core::Size;
use serde_json::Value;

#[cfg(test)]
mod gallery_screenshot;
mod node_component_kit;
mod product_gallery;
mod product_renderers;
#[cfg(test)]
mod visual_regression;

const REPEATABLE_ITEM_SNAPSHOTS_FIELD: &str = "jellyflow_repeatable_items";
#[cfg(test)]
const INITIAL_SELECTION: u128 = 2;
const CANVAS_WIDTH: f32 = 1140.0;
const CANVAS_HEIGHT: f32 = 650.0;
const SIDEBAR_WIDTH: f32 = 320.0;
const TOOLBAR_HEIGHT: f32 = 46.0;
const INITIAL_VIEWPORT_PADDING: f32 = 36.0;
const INITIAL_VIEWPORT_MIN_ZOOM: f32 = 0.25;
const INITIAL_VIEWPORT_MAX_ZOOM: f32 = 1.0;
const NODE_SURFACE_CHROME_HEIGHT: f32 = 78.0;
const NODE_SURFACE_SLOT_ROW_HEIGHT: f32 = 26.0;
const GPUI_LAYOUT_PASS_MEASUREMENT_STATUS: &str = "canvas-jellyflow consumes open-gpui \
measured_element bounds for visible slots, controls, repeatable rows, and anchors. Projection \
fallback now means initial, dirty, hidden, missing, duplicate, or partial coverage; the remaining \
maturity gap is productized authoring interaction coverage and advanced widget stubs.";

#[derive(Clone)]
pub(crate) struct GpuiNodeRendererServices {
    pub(crate) collector: OpenGpuiBoundsCollector,
    pub(crate) view: WeakEntity<JellyflowCanvasView>,
}

pub(crate) type GpuiNodeRendererTable =
    OpenGpuiNodeRendererTable<GpuiNodeRendererServices, AnyElement>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanvasToolOption {
    id: &'static str,
    label: &'static str,
    tool: CanvasTool,
}

fn canvas_tool_options() -> Vec<CanvasToolOption> {
    vec![
        CanvasToolOption {
            id: "select",
            label: "Select",
            tool: CanvasTool::Select,
        },
        CanvasToolOption {
            id: "pan",
            label: "Pan",
            tool: CanvasTool::Pan,
        },
        CanvasToolOption {
            id: "connect",
            label: "Connect",
            tool: CanvasTool::Connect,
        },
    ]
}

fn product_tool_switcher_visible() -> bool {
    let options = canvas_tool_options();
    options
        .iter()
        .any(|option| option.tool == CanvasTool::Select)
        && options.iter().any(|option| option.tool == CanvasTool::Pan)
        && options
            .iter()
            .any(|option| option.tool == CanvasTool::Connect)
}

fn init_canvas_jellyflow_app(cx: &mut App) {
    init_text_input(cx);
    cx.init_colors();
}

pub(crate) fn node_component_kit_actions(
    view: WeakEntity<JellyflowCanvasView>,
) -> node_component_kit::NodeComponentKitActions {
    node_component_kit::NodeComponentKitActions::new(
        {
            let view = view.clone();
            move |node_id, control, event, cx| {
                view.update(cx, |this, cx| {
                    this.dispatch_control_authoring_event(node_id, &control, event, cx);
                })
                .ok();
            }
        },
        {
            let view = view.clone();
            move |menu, action_key, node_id, cx| {
                view.update(cx, |this, cx| {
                    this.dispatch_menu_action(&menu, &action_key, node_id, cx);
                })
                .ok();
            }
        },
        move |node_id, action, cx| {
            view.update(cx, |this, cx| {
                this.dispatch_repeatable_action(node_id, action, cx);
            })
            .ok();
        },
    )
}

pub(crate) fn dispatch_node_drag_surface_mouse_down(
    view: WeakEntity<JellyflowCanvasView>,
    event: &MouseDownEvent,
    cx: &mut App,
) -> bool {
    view.update(cx, |this, cx| {
        this.handle_node_surface_drag_start(event, cx)
    })
    .unwrap_or(false)
}

struct JellyflowCanvasView {
    editor: CanvasEditor,
    store: NodeGraphStore,
    focus_handle: FocusHandle,
    projection: ProjectionSummary,
    gallery: product_gallery::ProductGalleryState,
    adapter: OpenGpuiAdapter,
    semantic_registry: NodeRegistry,
    node_kit_registry: NodeKitRegistry,
    measured_regions: OpenGpuiBoundsCollector,
    measurement_revision: u64,
    measurement_frame_pending: bool,
    auto_fit_viewport: bool,
    last_canvas_view_size: Option<open_gpui::Size<Pixels>>,
    last_canvas_bounds: Option<Bounds<Pixels>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutPassMeasurementConsume {
    NoRegions,
    Unchanged,
    Changed,
}

#[derive(Clone)]
struct ProjectionSummary {
    graph_nodes: usize,
    graph_ports: usize,
    graph_edges: usize,
    canvas_nodes: usize,
    canvas_edges: usize,
    layout_preset: String,
    last_commit: String,
    source: String,
    adapter: String,
    kit: String,
    capability: GpuiAuthoringCapabilitySummary,
}

#[derive(Clone)]
struct GpuiAuthoringCapabilitySummary {
    controls: &'static str,
    repeatables: &'static str,
    actions: &'static str,
    layout_measurement: NodeSurfaceMeasurementSource,
    layout_status: &'static str,
}

#[derive(Clone)]
struct NodeSurfaceSummary {
    node_kind: String,
    renderer_key: String,
    title: String,
    summary: String,
    slots: Vec<NodeSurfaceSlotProjection>,
    slot_descriptors: Vec<NodeSurfaceSlotDescriptor>,
    chrome: Vec<ResolvedNodeChrome>,
    document_bounds: JellyRect,
    selected: bool,
    zoom: f32,
    projection: NodeSurfaceProjection,
    actions: usize,
    menus: usize,
    action_menus: Vec<OpenGpuiMenuPlan>,
    toolbar_menu: OpenGpuiMenuPlan,
    renderer_context: OpenGpuiNodeRendererContext,
    inspectors: usize,
    blackboards: usize,
    repeatables: Vec<NodeRepeatableSurfaceProjection>,
    repeatable_items: Vec<NodeRepeatableItemProjection>,
    measurement: Option<NodeMeasurement>,
    inspector_target: Option<OpenGpuiInspectorTargetBounds>,
    node_data: Value,
}

#[derive(Clone, Debug, PartialEq)]
struct RepeatableItemSnapshot {
    collection_key: String,
    item_id: String,
    item_index: usize,
    slot_key: String,
    anchor: String,
    label: String,
    port_key: Option<String>,
    port_id: Option<String>,
    port_direction: Option<PortDirection>,
    dynamic_port_policy: OpenGpuiDynamicPortPolicy,
    controls: usize,
    remove_disabled_reason: Option<String>,
}

struct SelectedNodeSummary {
    node_id: JellyNodeId,
    id: String,
    kind: String,
    title: String,
    detail: String,
    ports: String,
    inspectors: Vec<OpenGpuiInspectorPlan>,
    inspector_target: Option<OpenGpuiInspectorTargetBounds>,
    blackboards: Vec<OpenGpuiBlackboardPlan>,
}

struct JellyflowNodeKind;

impl CanvasNodeRenderPolicy for JellyflowNodeKind {
    fn node_paint(&self, node: &CanvasNode) -> Option<CanvasKindPaint> {
        let (fill, stroke) = match data_string(node, "jellyflow_kind") {
            Some("demo.table") => ("#eff6ff", "#3b82f6"),
            Some("demo.topic") => ("#f5f3ff", "#8b5cf6"),
            Some("demo.source") => ("#ecfeff", "#0891b2"),
            Some("demo.decision") => ("#fff7ed", "#f97316"),
            Some("demo.llm") => ("#f8fafc", "#64748b"),
            Some("demo.workflow_output") => ("#f0fdf4", "#16a34a"),
            Some("demo.tool") => ("#fefce8", "#ca8a04"),
            _ => ("#f8fafc", "#64748b"),
        };

        Some(CanvasKindPaint {
            fill: Some(fill.to_string()),
            stroke: Some(stroke.to_string()),
            stroke_width: Some(px(1.5)),
            corner_radius: Some(px(7.0)),
        })
    }

    fn node_label(&self, node: &CanvasNode) -> Option<CanvasKindLabel> {
        Some(
            CanvasKindLabel::new(node_title(node))
                .with_inset(px(12.0))
                .with_color("#0f172a"),
        )
    }
}

impl Render for JellyflowCanvasView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let measurement_consume = self.consume_layout_pass_measurements();
        self.measured_regions.clear();
        if matches!(
            measurement_consume,
            LayoutPassMeasurementConsume::NoRegions | LayoutPassMeasurementConsume::Changed
        ) && !self.measurement_frame_pending
        {
            self.measurement_frame_pending = true;
            cx.on_next_frame(window, |this, window, cx| {
                this.measurement_frame_pending = false;
                window.refresh();
                cx.notify();
            });
        }

        let model = CanvasPaintModel::from(&self.editor);
        let render_model = model.clone();
        let selected = self.selected_node_summary();
        let selection_count = self.editor.selection().selected_nodes().count()
            + self.editor.selection().selected_edges().count();
        let options = CanvasPaintOptions {
            include_handles: true,
            ..CanvasPaintOptions::default()
        };
        let theme = CanvasPaintTheme {
            background: Some(Hsla::from(rgb(0xf8fafc))),
            label_line_clamp: Some(2),
            ..CanvasPaintTheme::default()
        };

        div()
            .size_full()
            .flex()
            .bg(rgb(0xf8fafc))
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                cx.stop_propagation();
                this.handle_key_down(event, cx);
            }))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(self.render_toolbar(selection_count, cx.weak_entity()))
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .overflow_hidden()
                            .child(
                                canvas_editor_view_with_frame(
                                    model,
                                    cx.entity(),
                                    self.focus_handle.clone(),
                                    Self::canvas_input_handler(),
                                    options,
                                    theme,
                                    |this, bounds, _frame, cx| {
                                        this.update_canvas_viewport_from_bounds(bounds, cx);
                                    },
                                )
                                .size_full(),
                            )
                            .children(self.render_node_surfaces(&render_model, cx)),
                    ),
            )
            .child(self.render_sidebar(selected, cx.weak_entity()))
    }
}

impl JellyflowCanvasView {
    fn render_toolbar(
        &self,
        selection_count: usize,
        view: WeakEntity<JellyflowCanvasView>,
    ) -> impl IntoElement {
        let dropped_wire_action =
            self.demo_dropped_wire_insert_menu()
                .and_then(|(menu, source, pointer)| {
                    menu.enabled_actions()
                        .next()
                        .map(|action| (menu.clone(), action.key.clone(), source, pointer))
                });

        div()
            .h(px(46.0))
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .px_4()
            .border_b_1()
            .border_color(rgb(0xdbe3ea))
            .bg(rgb(0xffffff))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .min_w(px(0.0))
                    .child(
                        div()
                            .text_sm()
                            .truncate()
                            .min_w(px(0.0))
                            .text_color(rgb(0x111827))
                            .child(self.gallery.active_case().label),
                    )
                    .child(
                        div()
                            .text_xs()
                            .truncate()
                            .text_color(rgb(self.gallery.active_case().accent))
                            .child(self.gallery.active_case().family_label()),
                    )
                    .child(self.render_gallery_selector(view.clone())),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .flex_shrink_0()
                    .children(
                        canvas_tool_options()
                            .into_iter()
                            .map(|option| self.render_tool_button(option, view.clone())),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .truncate()
                    .min_w(px(0.0))
                    .text_color(rgb(0x64748b))
                    .child(self.gallery.active_case().summary),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .flex_shrink_0()
                    .child(div().text_xs().text_color(rgb(0x475569)).child(format!(
                        "{} graph nodes / {} ports / {} edges / {} selected records",
                        self.projection.graph_nodes,
                        self.projection.graph_ports,
                        self.projection.graph_edges,
                        selection_count
                    )))
                    .when_some(
                        dropped_wire_action,
                        move |this, (menu, action_key, source, pointer)| {
                            this.child(
                                Button::new("jellyflow-toolbar-dropped-wire-insert", "Insert LLM")
                                    .variant(ButtonVariant::Secondary)
                                    .with_size(Size::XSmall)
                                    .on_click(move |event, _window, cx| {
                                        cx.stop_propagation();
                                        let _ = event;
                                        let menu = menu.clone();
                                        let action_key = action_key.clone();
                                        view.update(cx, |this, cx| {
                                            this.dispatch_dropped_wire_insert(
                                                &menu,
                                                &action_key,
                                                source,
                                                pointer,
                                                cx,
                                            );
                                        })
                                        .ok();
                                    }),
                            )
                        },
                    ),
            )
    }

    fn render_tool_button(
        &self,
        option: CanvasToolOption,
        view: WeakEntity<JellyflowCanvasView>,
    ) -> AnyElement {
        let selected = self.editor.tool() == &option.tool;
        let tool = option.tool.clone();
        Button::new(
            format!("jellyflow-toolbar-tool-{}", option.id),
            option.label,
        )
        .variant(if selected {
            ButtonVariant::Default
        } else {
            ButtonVariant::Secondary
        })
        .selected(selected)
        .with_size(Size::XSmall)
        .on_click(move |event, _window, cx| {
            cx.stop_propagation();
            let _ = event;
            let tool = tool.clone();
            view.update(cx, |this, cx| this.set_canvas_tool(tool, cx))
                .ok();
        })
        .into_any_element()
    }

    fn render_gallery_selector(&self, view: WeakEntity<JellyflowCanvasView>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap_1()
            .overflow_hidden()
            .children(self.gallery.cases().iter().map(|case| {
                let id = case.id().to_owned();
                let active = id == self.gallery.active_id();
                let view = view.clone();
                Button::new(
                    format!("jellyflow-product-gallery-case:{id}"),
                    case.family_label(),
                )
                .variant(if active {
                    ButtonVariant::Default
                } else {
                    ButtonVariant::Secondary
                })
                .with_size(Size::XSmall)
                .on_click(move |event, _window, cx| {
                    cx.stop_propagation();
                    let _ = event;
                    let id = id.clone();
                    view.update(cx, |this, cx| {
                        this.switch_product_gallery_fixture(&id, cx);
                    })
                    .ok();
                })
                .into_any_element()
            }))
            .into_any_element()
    }

    fn render_sidebar(
        &self,
        selected: Option<SelectedNodeSummary>,
        view: WeakEntity<JellyflowCanvasView>,
    ) -> impl IntoElement {
        let selection = match selected {
            Some(summary) => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x64748b))
                        .child(summary.kind.clone()),
                )
                .child(
                    div()
                        .text_lg()
                        .line_height(px(22.0))
                        .text_color(rgb(0x111827))
                        .child(summary.title.clone()),
                )
                .child(
                    div()
                        .text_sm()
                        .line_height(px(20.0))
                        .text_color(rgb(0x334155))
                        .child(summary.detail.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x64748b))
                        .child(summary.ports.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x64748b))
                        .child(summary.id.clone()),
                )
                .child(render_selected_inspector_panel(&summary, view.clone()))
                .child(render_selected_blackboard_panel(&summary, view.clone())),
            None => div().flex().flex_col().gap_3().child(
                div()
                    .text_sm()
                    .text_color(rgb(0x64748b))
                    .child("No Jellyflow node selected"),
            ),
        };

        div()
            .w(px(320.0))
            .h_full()
            .flex_none()
            .border_l_1()
            .border_color(rgb(0xdbe3ea))
            .bg(rgb(0xffffff))
            .p_4()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x64748b))
                            .child("Canvas projection"),
                    )
                    .child(div().text_sm().text_color(rgb(0x111827)).child(format!(
                        "{} nodes / {} edges",
                        self.projection.canvas_nodes, self.projection.canvas_edges
                    )))
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(18.0))
                            .text_color(rgb(0x475569))
                            .child(format!("layout preset: {}", self.projection.layout_preset)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(18.0))
                            .text_color(rgb(0x475569))
                            .child(format!("source: {}", self.projection.source)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(18.0))
                            .text_color(rgb(0x475569))
                            .child(format!("adapter: {}", self.projection.adapter)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(18.0))
                            .text_color(rgb(0x475569))
                            .child(format!("kit: {}", self.projection.kit)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(18.0))
                            .text_color(rgb(0x64748b))
                            .child(self.projection.last_commit.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x64748b))
                            .child("Authoring capabilities"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(18.0))
                            .text_color(rgb(0x475569))
                            .child(format!(
                                "controls: {} / repeatables: {} / actions: {}",
                                self.projection.capability.controls,
                                self.projection.capability.repeatables,
                                self.projection.capability.actions
                            )),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(18.0))
                            .text_color(rgb(0x475569))
                            .child(format!(
                                "measurement: {:?}",
                                self.projection.capability.layout_measurement
                            )),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(18.0))
                            .text_color(rgb(0x64748b))
                            .child(self.projection.capability.layout_status),
                    ),
            )
            .child(selection)
    }

    fn render_node_surfaces(
        &self,
        model: &CanvasPaintModel,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let zoom = model.viewport().zoom;
        let collector = self.measured_regions.clone();
        self.editor
            .document()
            .nodes()
            .filter_map(|node| {
                let surface = self.node_surface_summary(node, zoom)?;
                let jelly_node = jelly_node_id_from_node(node)?;
                let view = cx.weak_entity();
                Some(
                    render_node_surface(
                        model.viewport().document_bounds_to_view(node.bounds()),
                        jelly_node,
                        surface,
                        collector.clone(),
                        view,
                    )
                    .into_any_element(),
                )
            })
            .collect()
    }

    fn node_surface_summary(&self, node: &CanvasNode, zoom: f32) -> Option<NodeSurfaceSummary> {
        let jelly_node = jelly_node_id_from_node(node)?;
        let jelly_node_record = self.store.graph().nodes().get(&jelly_node)?;
        node_surface_summary_for_node(
            node,
            jelly_node,
            jelly_node_record,
            self.store.graph(),
            zoom,
            self.editor
                .selection()
                .contains_node(&NodeId::from(node.id.as_str())),
            &self.semantic_registry,
            &self.node_kit_registry,
            self.store.node_measurement(jelly_node),
        )
    }

    fn selected_node_summary(&self) -> Option<SelectedNodeSummary> {
        let id = self.editor.selection().selected_nodes().next()?;
        let node = self.editor.document().node(id)?;
        let jelly_node = jelly_node_id_from_node(node)?;
        let jelly_node_record = self.store.graph().nodes().get(&jelly_node)?;
        let descriptor = self
            .semantic_registry
            .view_descriptor(&jelly_node_record.kind)?;
        let node_data = jelly_node_record.data.clone();
        let inspectors = self.inspector_plans_for_node_data(&descriptor, &node_data);
        let blackboards = project_blackboards_for_descriptor(&descriptor, &node_data);
        let measurement = self.store.node_measurement(jelly_node);
        let inspector_target = inspectors.first().map(|inspector| {
            resolve_inspector_target_bounds(inspector, measurement.as_ref(), None)
        });
        Some(SelectedNodeSummary {
            node_id: jelly_node,
            id: node.id.as_str().to_string(),
            kind: data_string(node, "jellyflow_kind")
                .unwrap_or(node.kind.as_str())
                .to_string(),
            title: node_title(node),
            detail: data_string(node, "description")
                .unwrap_or("Jellyflow node projected into open-gpui-canvas")
                .to_string(),
            ports: format!("ports: {}", data_string(node, "ports").unwrap_or("none")),
            inspectors,
            inspector_target,
            blackboards,
        })
    }

    fn inspector_plans_for_node_data(
        &self,
        descriptor: &NodeKindViewDescriptor,
        node_data: &Value,
    ) -> Vec<OpenGpuiInspectorPlan> {
        project_inspectors_for_surface(
            descriptor,
            node_data,
            &OpenGpuiInspectorSurface::Node {
                node_kind: descriptor.kind.0.clone(),
            },
        )
    }

    fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        match self.handle_canvas_shortcut(event) {
            Ok(true) => {
                self.auto_fit_viewport = false;
                self.sync_store_from_canvas_document();
                cx.notify();
            }
            Ok(false) => {
                self.auto_fit_viewport = false;
                Self::canvas_input_handler().dispatch_key_down(self, event, cx);
            }
            Err(error) => {
                eprintln!("canvas shortcut failed: {error}");
            }
        }
    }

    fn set_canvas_tool(&mut self, tool: CanvasTool, cx: &mut Context<Self>) {
        match self.editor.set_tool(tool) {
            Ok(()) => cx.notify(),
            Err(error) => eprintln!("canvas tool switch failed: {error}"),
        }
    }

    fn canvas_input_handler() -> CanvasEditorInputHandler<Self> {
        CanvasEditorInputHandler::new(
            |this: &JellyflowCanvasView| this.is_pointer_interacting(),
            |this, event, cx| this.handle_canvas_event(Some(event), cx),
        )
    }

    fn handle_canvas_shortcut(&mut self, event: &KeyDownEvent) -> Result<bool, DocumentError> {
        let modifiers = event.keystroke.modifiers;
        if !(modifiers.platform || modifiers.control) {
            return Ok(false);
        }

        match event.keystroke.key.as_str() {
            "c" => {
                // CanvasEditor currently manages clipboard, but this fixture stays read-only.
                Ok(true)
            }
            "z" if modifiers.shift => {
                self.editor.redo()?;
                Ok(true)
            }
            "z" => {
                self.editor.undo()?;
                Ok(true)
            }
            "y" => {
                self.editor.redo()?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn handle_canvas_event(&mut self, event: Option<CanvasEvent>, cx: &mut Context<Self>) {
        let Some(event) = event else {
            return;
        };

        self.auto_fit_viewport = false;
        let dropped_wire_intent =
            dropped_wire_intent_from_canvas_event(&self.editor, &self.store, &event);
        match self.editor.handle_event(event) {
            Ok(()) => {
                let synced = self.sync_store_from_canvas_document();
                if !synced && let Some((source, pointer)) = dropped_wire_intent {
                    self.dispatch_first_dropped_wire_insert(source, pointer, cx);
                }
            }
            Err(error) => {
                eprintln!("canvas event failed: {error}");
            }
        }
        cx.notify();
    }

    fn switch_product_gallery_fixture(&mut self, fixture_id: &str, cx: &mut Context<Self>) {
        let Some(case) = self
            .gallery
            .cases()
            .iter()
            .find(|case| case.id() == fixture_id)
            .cloned()
        else {
            eprintln!("product gallery fixture not found: {fixture_id}");
            return;
        };
        if case.id() == self.gallery.active_id() {
            return;
        }

        match project_product_gallery_case(&case).and_then(|(store, document, projection)| {
            let editor = editor_for_document(document)
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
            Ok((store, editor, projection))
        }) {
            Ok((store, editor, projection)) => {
                self.store = store;
                self.editor = editor;
                self.projection = projection;
                self.gallery.set_active(case.id().to_owned());
                self.measured_regions.clear();
                self.measurement_revision = 1;
                self.measurement_frame_pending = false;
                self.auto_fit_viewport = true;
                self.last_canvas_view_size = None;
                self.last_canvas_bounds = None;
                cx.notify();
            }
            Err(error) => {
                eprintln!("product gallery fixture projection failed: {fixture_id}: {error}");
            }
        }
    }

    fn dispatch_control_authoring_plan(
        &mut self,
        outcome: Result<OpenGpuiAuthoringOutcome<OpenGpuiControlEditPlan>, String>,
        cx: &mut Context<Self>,
    ) {
        match outcome {
            Ok(OpenGpuiAuthoringOutcome::Planned(plan)) => {
                if let Err(error) = self.store.dispatch_transaction(&plan.transaction) {
                    eprintln!("control edit dispatch failed: {error}");
                    return;
                }
                self.store.invalidate_node_internals(plan.invalidation);
                self.refresh_editor_from_store();
                cx.notify();
            }
            Ok(OpenGpuiAuthoringOutcome::Skipped(reason)) => {
                report_authoring_skip(reason);
            }
            Err(error) => {
                eprintln!("control edit planning failed: {error}");
            }
        }
    }

    fn dispatch_control_authoring_event(
        &mut self,
        node_id: JellyNodeId,
        control: &OpenGpuiControlPlan,
        event: OpenGpuiControlEventValue,
        cx: &mut Context<Self>,
    ) {
        let outcome = OpenGpuiAuthoringController.plan_store_control_event(
            &self.store,
            &self.semantic_registry,
            node_id,
            control,
            event,
        );
        self.dispatch_control_authoring_plan(outcome, cx);
    }

    fn dispatch_repeatable_action(
        &mut self,
        node_id: JellyNodeId,
        action: OpenGpuiRepeatableActionPlan,
        cx: &mut Context<Self>,
    ) {
        match OpenGpuiAuthoringController.apply_repeatable_action_to_store(
            &mut self.store,
            &self.semantic_registry,
            node_id,
            action,
        ) {
            Ok(Some(_plan)) => {
                self.refresh_editor_from_store();
                cx.notify();
            }
            Ok(None) => {}
            Err(error) => {
                eprintln!("repeatable action dispatch failed: {error}");
            }
        }
    }

    fn dispatch_menu_action(
        &mut self,
        menu: &OpenGpuiMenuPlan,
        action_key: &str,
        node_id: Option<JellyNodeId>,
        cx: &mut Context<Self>,
    ) {
        match OpenGpuiAuthoringController.plan_menu_action_dispatch(menu, action_key) {
            OpenGpuiAuthoringOutcome::Planned(plan) => {
                if menu.surface == MenuSurface::DroppedWire {
                    let Some((source, pointer)) =
                        dropped_wire_source_for_menu(self.store.graph(), menu)
                    else {
                        eprintln!("dropped-wire insert skipped: no compatible source handle");
                        return;
                    };
                    self.dispatch_dropped_wire_insert(menu, &plan.action_key, source, pointer, cx);
                    return;
                }
                if let Some(node_id) = node_id {
                    match OpenGpuiAuthoringController.plan_repeatable_action_dispatch(
                        &self.store,
                        &self.semantic_registry,
                        Some(node_id),
                        &plan,
                        |context| {
                            Some(demo_repeatable_add_item(
                                &context.collection_key,
                                context.item_count,
                            ))
                        },
                    ) {
                        Ok(OpenGpuiAuthoringOutcome::Planned(repeatable)) => {
                            self.dispatch_repeatable_action(node_id, repeatable, cx);
                            return;
                        }
                        Ok(OpenGpuiAuthoringOutcome::Skipped(reason)) => {
                            report_authoring_skip(reason);
                            return;
                        }
                        Err(error) => {
                            eprintln!("semantic action planning failed: {error}");
                            return;
                        }
                    }
                } else if let Ok(OpenGpuiAuthoringOutcome::Skipped(reason)) =
                    OpenGpuiAuthoringController.plan_repeatable_action_dispatch(
                        &self.store,
                        &self.semantic_registry,
                        None,
                        &plan,
                        |_| None,
                    )
                {
                    if matches!(
                        reason,
                        OpenGpuiAuthoringSkipReason::MissingActionNodeTarget { .. }
                            | OpenGpuiAuthoringSkipReason::MissingRepeatableCollection { .. }
                            | OpenGpuiAuthoringSkipReason::MissingRepeatableReorderTarget { .. }
                    ) {
                        report_authoring_skip(reason);
                        return;
                    }
                }
                eprintln!(
                    "semantic action skipped: unsupported action executor for {} {:?} {:?}",
                    plan.action_key, plan.intent, plan.target
                );
            }
            OpenGpuiAuthoringOutcome::Skipped(reason) => {
                report_authoring_skip(reason);
            }
        }
    }

    fn demo_dropped_wire_insert_menu(
        &self,
    ) -> Option<(OpenGpuiMenuPlan, ConnectionHandleRef, JellyPoint)> {
        let source_key = PortKey::new("completion");
        let source = dropped_wire_source_for_port_key(self.store.graph(), &source_key)?;
        let pointer = dropped_wire_insert_pointer(self.store.graph(), source);
        let menu =
            project_dropped_wire_menu(&self.semantic_registry, source, Some(&source_key), pointer);
        Some((menu, source, pointer))
    }

    fn dispatch_dropped_wire_insert(
        &mut self,
        menu: &OpenGpuiMenuPlan,
        action_key: &str,
        source: ConnectionHandleRef,
        pointer: JellyPoint,
        cx: &mut Context<Self>,
    ) {
        match apply_demo_dropped_wire_insert(
            &mut self.store,
            &self.semantic_registry,
            menu,
            action_key,
            source,
            pointer,
        ) {
            Ok(_outcome) => {
                self.refresh_editor_from_store();
                cx.notify();
            }
            Err(error) => report_dropped_wire_insert_error(error),
        }
    }

    fn dispatch_first_dropped_wire_insert(
        &mut self,
        source: ConnectionHandleRef,
        pointer: JellyPoint,
        cx: &mut Context<Self>,
    ) {
        let source_key = self
            .store
            .graph()
            .ports()
            .get(&source.port)
            .map(|port| port.key.clone());
        let menu = project_dropped_wire_menu(
            &self.semantic_registry,
            source,
            source_key.as_ref(),
            pointer,
        );
        let Some(action_key) = menu
            .enabled_actions()
            .next()
            .map(|action| action.key.clone())
        else {
            return;
        };
        self.dispatch_dropped_wire_insert(&menu, &action_key, source, pointer, cx);
    }

    fn is_pointer_interacting(&self) -> bool {
        !self.editor.is_tool_state_idle()
    }

    fn consume_layout_pass_measurements(&mut self) -> LayoutPassMeasurementConsume {
        let regions = self.measured_regions.regions();
        if regions.is_empty() {
            return LayoutPassMeasurementConsume::NoRegions;
        }

        let mut changed = false;
        let node_ids = self
            .store
            .graph()
            .nodes()
            .keys()
            .copied()
            .collect::<Vec<_>>();

        for node_id in node_ids {
            let Some(node) = self.store.graph().nodes().get(&node_id).cloned() else {
                continue;
            };
            let node_regions = regions
                .iter()
                .filter(|region| region.node == Some(node_id))
                .cloned()
                .collect::<Vec<_>>();
            if node_regions.is_empty() {
                continue;
            }

            let Some(canvas_node) = self
                .editor
                .document()
                .node(&NodeId::from(canvas_node_id(&node_id)))
            else {
                continue;
            };
            let Some(descriptor) = self.semantic_registry.view_descriptor(&node.kind) else {
                continue;
            };
            let node_size = node.size.unwrap_or(JellySize {
                width: canvas_node.size.width.as_f32(),
                height: canvas_node.size.height.as_f32(),
            });
            let fallback_layout = projected_node_surface_graph_layout(
                &descriptor,
                &node,
                self.store.graph(),
                &node_id,
                node_size,
            );
            let fallback_anchors = measured_surface_anchors(
                &descriptor,
                self.store.graph(),
                &node_id,
                &fallback_layout,
            );
            let node_view_bounds = self
                .editor
                .viewport()
                .document_bounds_to_view(canvas_node.bounds());
            let context = OpenGpuiMeasurementContext::new(
                node_id,
                OpenGpuiViewPoint::new(
                    node_view_bounds.origin.x.as_f32(),
                    node_view_bounds.origin.y.as_f32(),
                ),
                1.0 / self.editor.viewport().zoom.max(f32::EPSILON),
                node_size,
            )
            .with_revision(0);
            let (mut measurement, _coverage) =
                layout_pass_measurement_from_regions(context, node_regions, fallback_anchors);
            let existing = self.store.node_measurement(node_id);
            assign_layout_pass_measurement_revision(
                self.store.node_measurement_status(node_id),
                existing.as_ref(),
                &mut measurement,
                &mut self.measurement_revision,
            );
            let outcome = self.store.report_node_measurement(measurement);
            if let Ok(outcome) = outcome {
                changed |= outcome.changed();
            }
        }

        if changed {
            self.refresh_editor_from_store();
            LayoutPassMeasurementConsume::Changed
        } else {
            LayoutPassMeasurementConsume::Unchanged
        }
    }

    fn sync_store_from_canvas_document(&mut self) -> bool {
        let mut changed = false;
        let transform_transaction = canvas_document_transform_transaction(
            &self.adapter,
            &self.store,
            self.editor.document(),
        );
        if !transform_transaction.is_empty() {
            match self.store.dispatch_transaction(&transform_transaction) {
                Ok(_) => changed = true,
                Err(error) => {
                    eprintln!("canvas transform sync failed: {error}");
                }
            }
        }

        match canvas_document_connection_sync_transactions(
            &self.adapter,
            &self.store,
            self.editor.document(),
        ) {
            Ok(transactions) => {
                for transaction in transactions {
                    match self.store.dispatch_transaction(&transaction) {
                        Ok(_) => changed = true,
                        Err(error) => {
                            eprintln!("canvas connection sync failed: {error}");
                        }
                    }
                }
            }
            Err(error) => {
                eprintln!("canvas connection sync planning failed: {error}");
            }
        }

        if changed {
            self.refresh_editor_from_store();
        }

        changed
    }

    fn update_canvas_viewport_from_bounds(
        &mut self,
        bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let view_size = bounds.size;
        if view_size.width <= px(0.0) || view_size.height <= px(0.0) {
            return;
        }

        self.last_canvas_bounds = Some(bounds);
        if self.update_canvas_viewport_for_view_size(view_size) {
            cx.notify();
        }
    }

    fn handle_node_surface_drag_start(
        &mut self,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(bounds) = self.last_canvas_bounds else {
            return false;
        };
        if let Some(event) = canvas_drag_start_event_from_bounds(bounds, event) {
            self.handle_canvas_event(Some(event), cx);
            return true;
        }
        false
    }

    fn update_canvas_viewport_for_view_size(&mut self, view_size: open_gpui::Size<Pixels>) -> bool {
        let viewport = self.editor.viewport();
        let update = canvas_viewport_size_update(
            self.editor.document(),
            viewport,
            self.auto_fit_viewport,
            self.last_canvas_view_size,
            view_size,
        );

        self.auto_fit_viewport = update.auto_fit_viewport;
        self.last_canvas_view_size = update.last_canvas_view_size;
        if update.viewport != viewport {
            self.editor.set_viewport(update.viewport);
            return true;
        }
        false
    }

    fn refresh_editor_from_store(&mut self) {
        let selection = self
            .editor
            .selection()
            .selected_nodes()
            .next()
            .map(|id| id.clone());
        let viewport = self.editor.viewport();
        let Ok((document, projection)) = project_store(&self.store) else {
            return;
        };
        let Ok(mut editor) =
            CanvasEditor::try_new_with_kind_registry(document, jellyflow_kind_registry())
        else {
            return;
        };
        editor.set_viewport(viewport);
        if let Some(id) = selection {
            let _ = editor.apply_tool_intent(CanvasToolIntent::ReplaceSelection(HitTarget::Node(
                id.clone(),
            )));
        }
        self.editor = editor;
        self.projection = projection;
    }
}

fn dropped_wire_source_for_menu(
    graph: &Graph,
    menu: &OpenGpuiMenuPlan,
) -> Option<(ConnectionHandleRef, JellyPoint)> {
    let source_key = menu
        .actions
        .iter()
        .find_map(|action| match &action.target {
            jellyflow::runtime::schema::ActionTarget::DroppedWire { source_port_key } => {
                source_port_key.as_deref()
            }
            _ => None,
        })?;
    let source = dropped_wire_source_for_port_key(graph, &PortKey::new(source_key))?;
    Some((source, dropped_wire_insert_pointer(graph, source)))
}

fn dropped_wire_intent_from_canvas_event(
    editor: &CanvasEditor,
    store: &NodeGraphStore,
    event: &CanvasEvent,
) -> Option<(ConnectionHandleRef, JellyPoint)> {
    let CanvasEvent::PointerUp {
        position,
        button: PointerButton::Primary,
        ..
    } = event
    else {
        return None;
    };
    let drag = editor.connection_drag_state()?;
    let document_position = editor.viewport().view_to_document(*position);
    if editor
        .runtime()
        .precise_hit_test_with_kind_registry(
            editor.document(),
            editor.kind_registry(),
            document_position,
            connection_hit_options(),
        )
        .any(|record| matches!(record.target, HitTarget::Handle { .. }))
    {
        return None;
    }
    let source = connection_handle_ref_from_canvas_endpoint(store.graph(), &drag.source)?;
    Some((
        source,
        JellyPoint {
            x: document_position.x.as_f32(),
            y: document_position.y.as_f32(),
        },
    ))
}

fn connection_handle_ref_from_canvas_endpoint(
    graph: &Graph,
    endpoint: &open_gpui_canvas::CanvasEndpoint,
) -> Option<ConnectionHandleRef> {
    let node = jelly_node_id_from_str(endpoint.node_id.as_str())?;
    let port = endpoint
        .handle_id
        .as_ref()
        .and_then(|handle| jelly_port_id_from_str(handle.as_str()))?;
    let direction = graph.ports().get(&port)?.dir;
    Some(ConnectionHandleRef::new(node, port, direction))
}

fn dropped_wire_source_for_port_key(
    graph: &Graph,
    source_key: &PortKey,
) -> Option<ConnectionHandleRef> {
    graph.ports().iter().find_map(|(port_id, port)| {
        (port.key == *source_key && port.dir == PortDirection::Out)
            .then_some(ConnectionHandleRef::new(port.node, *port_id, port.dir))
    })
}

fn dropped_wire_insert_pointer(graph: &Graph, source: ConnectionHandleRef) -> JellyPoint {
    graph
        .nodes()
        .get(&source.node)
        .map(|node| {
            let size = node.size.unwrap_or(JellySize {
                width: 260.0,
                height: 170.0,
            });
            JellyPoint {
                x: node.pos.x + size.width + 80.0,
                y: node.pos.y + size.height * 0.35,
            }
        })
        .unwrap_or(JellyPoint { x: 420.0, y: 180.0 })
}

fn apply_demo_dropped_wire_insert(
    store: &mut NodeGraphStore,
    registry: &NodeRegistry,
    menu: &OpenGpuiMenuPlan,
    action_key: &str,
    source: ConnectionHandleRef,
    pointer: JellyPoint,
) -> Result<jellyflow_open_gpui::OpenGpuiDroppedWireInsertOutcome, OpenGpuiDroppedWireInsertError> {
    let outcome = apply_dropped_wire_insert(
        store,
        registry,
        menu,
        action_key,
        source,
        pointer,
        jellyflow::core::NodeGraphConnectionMode::Strict,
    )?;
    store.invalidate_node_internals(
        jellyflow::runtime::runtime::measurement::NodeInternalsInvalidation::one(
            outcome.plan.node_id,
            jellyflow::runtime::runtime::measurement::NodeInternalsInvalidationReason::DataChanged,
        ),
    );
    Ok(outcome)
}

fn render_selected_inspector_panel(
    summary: &SelectedNodeSummary,
    view: WeakEntity<JellyflowCanvasView>,
) -> AnyElement {
    let inspectors = summary
        .inspectors
        .iter()
        .take(2)
        .enumerate()
        .map(|(index, inspector)| {
            render_inspector_card(summary.node_id, inspector, index, view.clone())
        })
        .collect::<Vec<_>>();

    div()
        .flex()
        .flex_col()
        .gap_2()
        .border_t_1()
        .border_color(rgb(0xe2e8f0))
        .pt_3()
        .child(div().text_xs().text_color(rgb(0x64748b)).child("Inspector"))
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x64748b))
                .truncate()
                .child(inspector_target_status_label(summary.inspector_target)),
        )
        .when(inspectors.is_empty(), |this| {
            this.child(
                div()
                    .text_sm()
                    .text_color(rgb(0x94a3b8))
                    .child("No semantic inspector for selection"),
            )
        })
        .children(inspectors)
        .into_any_element()
}

fn render_inspector_card(
    node_id: JellyNodeId,
    inspector: &OpenGpuiInspectorPlan,
    card_index: usize,
    view: WeakEntity<JellyflowCanvasView>,
) -> AnyElement {
    let controls = inspector
        .controls
        .iter()
        .take(2)
        .enumerate()
        .map(|(index, control)| {
            render_control_plan(
                node_id,
                &format!("inspector:{}", inspector.key),
                control,
                card_index * 10 + index,
                view.clone(),
            )
        })
        .collect::<Vec<_>>();
    let actions = inspector
        .action_menu
        .actions
        .iter()
        .take(2)
        .enumerate()
        .map(|(index, action)| {
            render_dispatch_action_button(
                &inspector.action_menu,
                action,
                index,
                Some(node_id),
                view.clone(),
            )
        })
        .collect::<Vec<_>>();
    let status = inspector
        .read_only_reason
        .as_deref()
        .unwrap_or(if inspector.editable {
            "editable"
        } else {
            "read-only"
        });

    div()
        .flex()
        .flex_col()
        .gap_2()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xdbe3ea))
        .bg(rgb(0xf8fafc))
        .p_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .line_height(px(18.0))
                        .text_color(rgb(0x111827))
                        .truncate()
                        .child(inspector.label.clone()),
                )
                .child(
                    Badge::new(
                        format!("jellyflow-inspector-status:{}", inspector.key),
                        status.to_owned(),
                    )
                    .variant(if inspector.editable {
                        BadgeVariant::Default
                    } else {
                        BadgeVariant::Outline
                    })
                    .with_size(Size::XSmall),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x64748b))
                .truncate()
                .child(format!(
                    "{} controls · {} actions",
                    inspector.controls.len(),
                    inspector.action_menu.actions.len()
                )),
        )
        .children(controls)
        .when(!actions.is_empty(), |this| {
            this.child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .overflow_hidden()
                    .children(actions),
            )
        })
        .into_any_element()
}

fn render_selected_blackboard_panel(
    summary: &SelectedNodeSummary,
    view: WeakEntity<JellyflowCanvasView>,
) -> AnyElement {
    if summary.blackboards.is_empty() {
        return div().into_any_element();
    }
    let blackboards = summary
        .blackboards
        .iter()
        .take(2)
        .enumerate()
        .map(|(index, blackboard)| {
            render_blackboard_card(summary.node_id, blackboard, index, view.clone())
        })
        .collect::<Vec<_>>();

    div()
        .flex()
        .flex_col()
        .gap_2()
        .border_t_1()
        .border_color(rgb(0xe2e8f0))
        .pt_3()
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x64748b))
                .child("Blackboard"),
        )
        .children(blackboards)
        .into_any_element()
}

fn render_blackboard_card(
    node_id: JellyNodeId,
    blackboard: &OpenGpuiBlackboardPlan,
    card_index: usize,
    view: WeakEntity<JellyflowCanvasView>,
) -> AnyElement {
    let items = blackboard
        .items
        .iter()
        .take(3)
        .map(|item| {
            div()
                .h(px(22.0))
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .rounded_sm()
                .bg(rgb(0xffffff))
                .px_2()
                .overflow_hidden()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x334155))
                        .truncate()
                        .min_w(px(0.0))
                        .child(item.label.clone()),
                )
                .child(
                    Badge::new(
                        open_gpui_blackboard_item_element_id(
                            node_id,
                            &blackboard.key,
                            &item.item_id,
                        ),
                        format!("{} controls", item.controls),
                    )
                    .variant(BadgeVariant::Outline)
                    .with_size(Size::XSmall),
                )
                .into_any_element()
        })
        .collect::<Vec<_>>();
    let actions = blackboard
        .action_menu
        .actions
        .iter()
        .take(2)
        .enumerate()
        .map(|(index, action)| {
            render_dispatch_action_button(
                &blackboard.action_menu,
                action,
                index,
                Some(node_id),
                view.clone(),
            )
        })
        .collect::<Vec<_>>();

    div()
        .flex()
        .flex_col()
        .gap_2()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xdbe3ea))
        .bg(rgb(0xf8fafc))
        .p_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .line_height(px(18.0))
                        .text_color(rgb(0x111827))
                        .truncate()
                        .child(blackboard.label.clone()),
                )
                .child(
                    Badge::new(
                        open_gpui_blackboard_status_element_id(node_id, &blackboard.key),
                        format!("{} items", blackboard.item_count),
                    )
                    .variant(BadgeVariant::Secondary)
                    .with_size(Size::XSmall),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x64748b))
                .truncate()
                .child(format!(
                    "{} · {} controls",
                    blackboard.collection_key, blackboard.controls
                )),
        )
        .children(items)
        .when(!actions.is_empty(), |this| {
            this.child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .overflow_hidden()
                    .children(actions)
                    .child(render_action_menu(
                        &blackboard.action_menu,
                        &format!("blackboard-{card_index}"),
                        Some(node_id),
                        view,
                    )),
            )
        })
        .into_any_element()
}

fn render_node_surface(
    bounds: Bounds<Pixels>,
    node_id: JellyNodeId,
    surface: NodeSurfaceSummary,
    collector: OpenGpuiBoundsCollector,
    view: WeakEntity<JellyflowCanvasView>,
) -> impl IntoElement {
    let registry = demo_node_renderer_registry();
    let renderers = demo_custom_node_renderers();
    let services = GpuiNodeRendererServices { collector, view };
    let rendered = registry.render_with_host(
        &surface.renderer_context,
        &services,
        &renderers,
        |host, _fallback| {
            render_descriptor_fallback_node_surface(
                bounds,
                node_id,
                surface.clone(),
                host.services().collector.clone(),
                host.services().view.clone(),
            )
        },
    );

    match rendered.source {
        OpenGpuiNodeRendererOutputSource::Custom(_) => render_node_surface_shell(bounds, &surface)
            .child(rendered.output)
            .into_any_element(),
        OpenGpuiNodeRendererOutputSource::Fallback(_) => rendered.output,
    }
}

fn render_node_surface_shell(
    bounds: Bounds<Pixels>,
    surface: &NodeSurfaceSummary,
) -> open_gpui::Div {
    let zoom = surface.zoom;
    let pad = if zoom >= 1.0 { px(10.0) } else { px(8.0) };
    let top = bounds.top() + pad;
    let left = bounds.left() + pad;
    let inner_width = (bounds.size.width - pad * 2.0).max(px(0.0));
    let inner_height = (bounds.size.height - pad * 2.0).max(px(0.0));

    div()
        .absolute()
        .left(left)
        .top(top)
        .w(inner_width)
        .h(inner_height)
        .flex_shrink_0()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .overflow_hidden()
}

fn render_descriptor_fallback_node_surface(
    bounds: Bounds<Pixels>,
    node_id: JellyNodeId,
    surface: NodeSurfaceSummary,
    collector: OpenGpuiBoundsCollector,
    view: WeakEntity<JellyflowCanvasView>,
) -> AnyElement {
    let zoom = surface.zoom;
    let pad = if zoom >= 1.0 { px(10.0) } else { px(8.0) };
    let inner_width = (bounds.size.width - pad * 2.0).max(px(0.0));
    let inner_height = (bounds.size.height - pad * 2.0).max(px(0.0));
    let slot_limit = adapter_slot_limit_for_height(inner_height, surface.projection.slot_limit);
    let component_layout = NodeSurfaceComponentLayout::with_repeatable_items(
        surface
            .slots
            .iter()
            .cloned()
            .map(|slot| {
                let descriptor = surface_slot_descriptor_for_projection(&surface, &slot);
                (slot, descriptor)
            })
            .collect(),
        surface.repeatables.clone(),
        surface.repeatable_items.clone(),
        JellySize {
            width: surface.document_bounds.size.width,
            height: surface.document_bounds.size.height,
        },
        slot_limit,
    );
    let accent = if surface.selected {
        rgb(0x2563eb)
    } else {
        rgb(0x475569)
    };

    render_node_surface_shell(bounds, &surface)
        .rounded_sm()
        .border_1()
        .border_color(accent)
        .bg(rgb(0xffffff))
        .overflow_hidden()
        .shadow_sm()
        .child(
            div()
                .absolute()
                .left(px(8.0))
                .top(px(8.0))
                .right(px(8.0))
                .flex()
                .flex_shrink_0()
                .items_center()
                .justify_between()
                .gap_2()
                .min_w(px(0.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(accent)
                        .child(surface.node_kind.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x94a3b8))
                        .truncate()
                        .min_w(px(0.0))
                        .child(surface.renderer_key.clone()),
                ),
        )
        .child(
            div()
                .absolute()
                .left(px(8.0))
                .top(px(30.0))
                .right(px(8.0))
                .text_sm()
                .line_height(px(20.0))
                .text_color(rgb(0x111827))
                .overflow_hidden()
                .line_clamp(2)
                .flex_shrink_1()
                .min_w(px(0.0))
                .child(surface.title.clone()),
        )
        .child(
            div()
                .absolute()
                .left(px(8.0))
                .top(px(52.0))
                .right(px(8.0))
                .text_xs()
                .text_color(rgb(0x64748b))
                .truncate()
                .flex_shrink_1()
                .min_w(px(0.0))
                .child(surface.summary.clone()),
        )
        .child(render_surface_action_summary(&surface))
        .child(
            div()
                .absolute()
                .left(px(8.0))
                .top(px(64.0))
                .right(px(150.0))
                .text_xs()
                .text_color(rgb(0x94a3b8))
                .truncate()
                .flex_shrink_1()
                .min_w(px(0.0))
                .child(format!(
                    "a{} m{} i{} b{} · {} plans · {}",
                    surface.actions,
                    surface.menus,
                    surface.inspectors,
                    surface.blackboards,
                    surface_action_plan_count(&surface),
                    surface_measurement_summary(&surface)
                )),
        )
        .children(render_surface_chrome(&surface, bounds, view.clone()))
        .child(render_inspector_target_highlight(
            &surface,
            inner_width,
            inner_height,
        ))
        .children(render_surface_slots(
            node_id,
            component_layout,
            surface.document_bounds,
            surface.node_data.clone(),
            inner_width,
            inner_height,
            collector,
            view,
        ))
        .into_any_element()
}

fn demo_node_renderer_registry() -> OpenGpuiNodeRendererRegistry {
    product_renderers::demo_node_renderer_registry()
}

fn demo_custom_node_renderers() -> GpuiNodeRendererTable {
    product_renderers::demo_custom_node_renderers()
}

fn render_inspector_target_highlight(
    surface: &NodeSurfaceSummary,
    view_width: Pixels,
    view_height: Pixels,
) -> AnyElement {
    let Some(target) = surface.inspector_target else {
        return div().into_any_element();
    };
    if target.source != OpenGpuiInspectorTargetSource::Measured {
        return div().into_any_element();
    }
    let Some(rect) = target.rect else {
        return div().into_any_element();
    };
    let rect = slot_view_rect(rect, surface.document_bounds, view_width, view_height);

    div()
        .absolute()
        .left(rect.origin.x)
        .top(rect.origin.y)
        .w(rect.size.width)
        .h(rect.size.height)
        .rounded_sm()
        .border_1()
        .border_color(rgb(0x2563eb))
        .bg(rgb(0xdbeafe))
        .into_any_element()
}

fn node_surface_summary_for_node(
    node: &CanvasNode,
    jelly_node: JellyNodeId,
    jelly_node_record: &Node,
    graph: &Graph,
    zoom: f32,
    selected: bool,
    semantic_registry: &NodeRegistry,
    node_kit_registry: &NodeKitRegistry,
    measurement: Option<NodeMeasurement>,
) -> Option<NodeSurfaceSummary> {
    let kind = NodeKindKey::new(data_string(node, "jellyflow_kind").unwrap_or(node.kind.as_str()));
    let descriptor = semantic_registry.view_descriptor(&kind)?;
    let title = node_title(node);
    let data = Value::Object(node.data.clone());
    let layout_hints = node_kit_registry.layout_hints_for_kind(&kind)?;
    let projection = NodeSurfaceProjection::from_layout_hints(layout_hints, zoom);
    let slots = descriptor.surface_slots_projection(&data, Some(layout_hints), zoom);
    let repeatables = repeatable_surface_projection(&descriptor, &data);
    let repeatable_items = repeatable_item_snapshots_from_node(node)
        .into_iter()
        .map(repeatable_item_projection_from_snapshot)
        .collect();
    let document_bounds = jelly_rect_from_bounds(node.bounds());
    let chrome = resolve_node_chrome_facts(
        NodeChromeFactsRequest::new(jelly_node, document_bounds, &descriptor.chrome)
            .with_state(NodeChromeState {
                selected,
                hovered: false,
                focused: false,
            })
            .with_policy(NodeChromeLayoutPolicy::default().with_zoom(zoom)),
    )
    .map(|facts| facts.chrome)
    .unwrap_or_default();
    let summary = data_string(node, "summary")
        .or_else(|| data_string(node, "description"))
        .unwrap_or("Jellyflow node projected into open-gpui-canvas")
        .to_string();
    let action_menus = node_action_menus(&descriptor);
    let toolbar_menu = node_toolbar_menu(&descriptor);
    let renderer_context = open_gpui_node_renderer_context(
        jelly_node,
        jelly_node_record,
        graph,
        &descriptor,
        OpenGpuiNodeRendererState {
            selected,
            hidden: node.hidden,
            ..OpenGpuiNodeRendererState::default()
        },
        projection.clone(),
        slots.clone(),
    );
    let inspector_target = if selected {
        project_inspectors_for_surface(
            &descriptor,
            &data,
            &OpenGpuiInspectorSurface::Node {
                node_kind: descriptor.kind.0.clone(),
            },
        )
        .first()
        .map(|inspector| resolve_inspector_target_bounds(inspector, measurement.as_ref(), None))
    } else {
        None
    };

    Some(NodeSurfaceSummary {
        node_kind: descriptor.kind.0.clone(),
        renderer_key: descriptor.renderer_key.clone(),
        title,
        summary,
        slots,
        slot_descriptors: descriptor.surface_slots.clone(),
        chrome,
        document_bounds,
        selected,
        zoom,
        projection,
        actions: descriptor.actions.len(),
        menus: descriptor.menus.len(),
        action_menus,
        toolbar_menu,
        renderer_context,
        inspectors: descriptor.inspectors.len(),
        blackboards: descriptor.blackboards.len(),
        repeatables,
        repeatable_items,
        measurement,
        inspector_target,
        node_data: data,
    })
}

fn render_surface_chrome(
    surface: &NodeSurfaceSummary,
    view_bounds: Bounds<Pixels>,
    view: WeakEntity<JellyflowCanvasView>,
) -> Vec<AnyElement> {
    surface
        .chrome
        .iter()
        .filter_map(|chrome| render_node_chrome(chrome, surface, view_bounds, view.clone()))
        .collect()
}

fn surface_measurement_summary(surface: &NodeSurfaceSummary) -> String {
    surface
        .measurement
        .as_ref()
        .map(|measurement| {
            format!(
                "measured s{} a{}",
                measurement.slots.len(),
                measurement.anchors.len()
            )
        })
        .unwrap_or_else(|| "projection fallback".to_string())
}

fn inspector_target_status_label(target: Option<OpenGpuiInspectorTargetBounds>) -> &'static str {
    match target.map(|target| target.source) {
        Some(OpenGpuiInspectorTargetSource::Measured) => "target: measured layout-pass bounds",
        Some(OpenGpuiInspectorTargetSource::Fallback) => "target: projection fallback bounds",
        Some(OpenGpuiInspectorTargetSource::Missing) | None => "target: missing bounds",
    }
}

fn render_surface_action_summary(surface: &NodeSurfaceSummary) -> AnyElement {
    let visible = surface
        .action_menus
        .iter()
        .flat_map(|menu| menu.actions.iter())
        .chain(surface.toolbar_menu.actions.iter())
        .take(2)
        .map(|action| {
            Badge::new(
                open_gpui_action_summary_element_id(surface.renderer_context.node_id, &action.key),
                action_summary_label(action),
            )
            .variant(if action.dispatchable() {
                BadgeVariant::Default
            } else {
                BadgeVariant::Outline
            })
            .with_size(Size::XSmall)
            .into_any_element()
        })
        .collect::<Vec<_>>();

    div()
        .absolute()
        .top(px(62.0))
        .right(px(8.0))
        .max_w(px(138.0))
        .flex()
        .items_center()
        .justify_end()
        .gap_1()
        .overflow_hidden()
        .children(visible)
        .into_any_element()
}

fn surface_action_plan_count(surface: &NodeSurfaceSummary) -> usize {
    surface
        .action_menus
        .iter()
        .map(|menu| menu.actions.len())
        .sum::<usize>()
        + surface.toolbar_menu.actions.len()
}

fn render_dispatch_action_button(
    menu: &OpenGpuiMenuPlan,
    action: &OpenGpuiActionPlan,
    index: usize,
    node_id: Option<JellyNodeId>,
    view: WeakEntity<JellyflowCanvasView>,
) -> AnyElement {
    node_component_kit::render_dispatch_action_button(
        menu,
        action,
        index,
        node_id,
        &node_component_kit_actions(view),
    )
}

fn render_action_menu(
    menu: &OpenGpuiMenuPlan,
    id_suffix: &str,
    node_id: Option<JellyNodeId>,
    view: WeakEntity<JellyflowCanvasView>,
) -> AnyElement {
    node_component_kit::render_action_menu(
        menu,
        id_suffix,
        node_id,
        &node_component_kit_actions(view),
    )
}

fn render_chrome_action_buttons(
    surface: &NodeSurfaceSummary,
    fallback_label: &str,
    view: WeakEntity<JellyflowCanvasView>,
) -> Vec<AnyElement> {
    let actions = surface
        .action_menus
        .iter()
        .flat_map(|menu| menu.actions.iter().map(move |action| (menu, action)))
        .take(2)
        .enumerate()
        .map(|(index, (menu, action))| {
            render_dispatch_action_button(
                menu,
                action,
                index,
                Some(surface.renderer_context.node_id),
                view.clone(),
            )
        })
        .collect::<Vec<_>>();

    if actions.is_empty() {
        return vec![
            Button::new(
                open_gpui_chrome_fallback_button_element_id(
                    surface.renderer_context.node_id,
                    &surface.node_kind,
                ),
                fallback_label.to_owned(),
            )
            .variant(ButtonVariant::Default)
            .with_size(Size::XSmall)
            .into_any_element(),
        ];
    }

    actions
}

fn action_summary_label(action: &OpenGpuiActionPlan) -> String {
    action
        .shortcut
        .as_ref()
        .map(|shortcut| format!("{} {}", action.label, shortcut))
        .unwrap_or_else(|| action.label.clone())
}

fn surface_slot_descriptor_for_projection(
    surface: &NodeSurfaceSummary,
    slot: &NodeSurfaceSlotProjection,
) -> Option<NodeSurfaceSlotDescriptor> {
    // The GPUI fixture keeps descriptor lookup local to the projected surface summary.
    // The cloned slot descriptor lets local components render controls without carrying runtime
    // widget types across the headless boundary.
    surface
        .slot_descriptors
        .iter()
        .find(|candidate| candidate.key == slot.key)
        .cloned()
}

fn node_action_menus(descriptor: &NodeKindViewDescriptor) -> Vec<OpenGpuiMenuPlan> {
    let surface = OpenGpuiActionSurface::Node {
        node_kind: descriptor.kind.0.clone(),
    };
    let mut menus = descriptor
        .menus
        .iter()
        .filter(|menu| menu.surface == MenuSurface::Node)
        .map(|menu| project_menu(descriptor, menu, &surface))
        .filter(|menu| !menu.actions.is_empty())
        .collect::<Vec<_>>();

    if menus.is_empty() {
        let synthetic = project_actions_for_surface(descriptor, &surface);
        if !synthetic.actions.is_empty() {
            menus.push(synthetic);
        }
    }

    menus
}

fn node_toolbar_menu(descriptor: &NodeKindViewDescriptor) -> OpenGpuiMenuPlan {
    let surface = OpenGpuiActionSurface::Toolbar {
        node_kind: Some(descriptor.kind.0.clone()),
    };
    let explicit = descriptor
        .menus
        .iter()
        .find(|menu| menu.surface == MenuSurface::Toolbar)
        .map(|menu| project_menu(descriptor, menu, &surface))
        .filter(|menu| !menu.actions.is_empty());

    explicit.unwrap_or_else(|| project_actions_for_surface(descriptor, &surface))
}

fn render_node_chrome(
    chrome: &ResolvedNodeChrome,
    surface: &NodeSurfaceSummary,
    view_bounds: Bounds<Pixels>,
    view: WeakEntity<JellyflowCanvasView>,
) -> Option<AnyElement> {
    let bounds = chrome_view_bounds(chrome, surface, view_bounds)?;
    let base = div()
        .absolute()
        .left(bounds.origin.x)
        .top(bounds.origin.y)
        .w(bounds.size.width)
        .h(bounds.size.height)
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden();

    let label = chrome.label.clone().unwrap_or_else(|| chrome.key.clone());
    Some(match chrome.kind {
        NodeChromeKind::StatusStrip => base
            .px_2()
            .rounded_sm()
            .bg(rgb(0xecfdf5))
            .border_1()
            .border_color(rgb(0x86efac))
            .child(
                div()
                    .text_xs()
                    .truncate()
                    .text_color(rgb(0x166534))
                    .child(label),
            )
            .into_any_element(),
        NodeChromeKind::RunActionStrip => base
            .justify_start()
            .gap_1()
            .children(render_chrome_action_buttons(surface, &label, view))
            .into_any_element(),
        NodeChromeKind::Toolbar => base
            .justify_end()
            .gap_1()
            .child(render_action_menu(
                &surface.toolbar_menu,
                &chrome.key,
                Some(surface.renderer_context.node_id),
                view,
            ))
            .into_any_element(),
        NodeChromeKind::Resizer => base
            .rounded_sm()
            .bg(rgb(0x2563eb))
            .border_1()
            .border_color(rgb(0xffffff))
            .into_any_element(),
        NodeChromeKind::ValidationBanner => base
            .px_2()
            .rounded_sm()
            .bg(rgb(0xfffbeb))
            .border_1()
            .border_color(rgb(0xf59e0b))
            .child(
                div()
                    .text_xs()
                    .truncate()
                    .text_color(rgb(0x92400e))
                    .child(label),
            )
            .into_any_element(),
        NodeChromeKind::InspectorAnchor => base
            .rounded_sm()
            .border_1()
            .border_color(rgb(0x94a3b8))
            .bg(rgb(0xffffff))
            .into_any_element(),
    })
}

fn chrome_view_bounds(
    chrome: &ResolvedNodeChrome,
    surface: &NodeSurfaceSummary,
    view_bounds: Bounds<Pixels>,
) -> Option<Bounds<Pixels>> {
    let document = surface.document_bounds;
    if !document.is_positive_finite() {
        return None;
    }
    let scale_x = view_bounds.size.width.as_f32() / document.size.width;
    let scale_y = view_bounds.size.height.as_f32() / document.size.height;
    let x = view_bounds.origin.x.as_f32() + (chrome.rect.origin.x - document.origin.x) * scale_x;
    let y = view_bounds.origin.y.as_f32() + (chrome.rect.origin.y - document.origin.y) * scale_y;
    let width = chrome.rect.size.width * scale_x;
    let height = chrome.rect.size.height * scale_y;
    (x.is_finite()
        && y.is_finite()
        && width.is_finite()
        && height.is_finite()
        && width > 0.0
        && height > 0.0)
        .then(|| Bounds::new(point(px(x), px(y)), size(px(width), px(height))))
}

fn render_surface_slots(
    node_id: JellyNodeId,
    layout: NodeSurfaceComponentLayout,
    document_bounds: JellyRect,
    node_data: Value,
    view_width: Pixels,
    view_height: Pixels,
    collector: OpenGpuiBoundsCollector,
    view: WeakEntity<JellyflowCanvasView>,
) -> Vec<AnyElement> {
    let mut elements = layout
        .slots
        .into_iter()
        .map(|slot| {
            render_node_slot(
                node_id,
                slot,
                document_bounds,
                &node_data,
                view_width,
                view_height,
                collector.clone(),
                view.clone(),
            )
        })
        .map(|slot| slot.into_any_element())
        .collect::<Vec<_>>();
    elements.extend(layout.repeatable_items.into_iter().map(|repeatable| {
        render_repeatable_item_row(
            node_id,
            repeatable,
            document_bounds,
            view_width,
            view_height,
            collector.clone(),
            view.clone(),
        )
        .into_any_element()
    }));
    elements.extend(layout.repeatables.into_iter().map(|repeatable| {
        render_repeatable_row(
            node_id,
            repeatable,
            document_bounds,
            view_width,
            view_height,
            collector.clone(),
            view.clone(),
        )
        .into_any_element()
    }));
    elements
}

fn render_node_slot(
    node_id: JellyNodeId,
    slot_layout: NodeSurfaceSlotLayout,
    document_bounds: JellyRect,
    node_data: &Value,
    view_width: Pixels,
    view_height: Pixels,
    collector: OpenGpuiBoundsCollector,
    view: WeakEntity<JellyflowCanvasView>,
) -> impl IntoElement {
    let slot = slot_layout.slot;
    let rect = slot_view_rect(slot_layout.rect, document_bounds, view_width, view_height);
    let slot_key = slot.key.clone();
    let anchor_key = slot_layout
        .descriptor
        .as_ref()
        .and_then(|descriptor| descriptor.anchor.clone())
        .unwrap_or_else(|| slot_key.clone());
    let fill = match slot.kind {
        NodeSurfaceSlotKind::Header => rgb(0xe0f2fe),
        NodeSurfaceSlotKind::Body => rgb(0xf1f5f9),
        NodeSurfaceSlotKind::Footer => rgb(0xe2e8f0),
        NodeSurfaceSlotKind::Badge => rgb(0xfef3c7),
        NodeSurfaceSlotKind::MetricBadge => rgb(0xe0f2fe),
        NodeSurfaceSlotKind::StatusBanner => rgb(0xdcfce7),
        NodeSurfaceSlotKind::Icon => rgb(0xe0e7ff),
        NodeSurfaceSlotKind::FieldRow => rgb(0xecfeff),
        NodeSurfaceSlotKind::ActionRow => rgb(0xfce7f3),
        NodeSurfaceSlotKind::ConfigGroup => rgb(0xf1f5f9),
        NodeSurfaceSlotKind::PortRail => rgb(0xe5e7eb),
        NodeSurfaceSlotKind::Preview => rgb(0xd1fae5),
        NodeSurfaceSlotKind::NestedRegion => rgb(0xf3e8ff),
    };
    let value = if slot.value.is_empty() {
        "-".to_string()
    } else {
        slot.value.clone()
    };
    let label = if slot.label.is_empty() {
        "slot".to_string()
    } else {
        slot.label.clone()
    };
    let status = if slot.visible { "visible" } else { "hidden" };

    let row = div()
        .absolute()
        .left(rect.origin.x)
        .top(rect.origin.y)
        .w(rect.size.width)
        .h(rect.size.height)
        .flex()
        .flex_shrink_1()
        .items_center()
        .justify_between()
        .gap_2()
        .min_w(px(0.0))
        .px_2()
        .py_1()
        .rounded_sm()
        .bg(fill)
        .overflow_hidden()
        .child(render_slot_label(node_id, &slot, label, status))
        .child(render_slot_value(
            node_id,
            &slot,
            slot_layout.descriptor.as_ref(),
            node_data,
            value,
            collector.clone(),
            view,
        ))
        .child(render_slot_anchor_measurement(
            node_id,
            anchor_key,
            slot_anchor_view_rect(
                slot_layout.anchor_rect,
                document_bounds,
                view_width,
                view_height,
            ),
            collector.clone(),
        ));

    render_measured_region(
        OpenGpuiMeasurementId::slot(node_id, slot_key),
        collector,
        row,
    )
}

fn slot_view_rect(
    rect: JellyRect,
    document_bounds: JellyRect,
    view_width: Pixels,
    view_height: Pixels,
) -> Bounds<Pixels> {
    let width = document_bounds.size.width.max(1.0);
    let height = document_bounds.size.height.max(1.0);
    Bounds::new(
        point(
            px(rect.origin.x / width * view_width.as_f32()),
            px(rect.origin.y / height * view_height.as_f32()),
        ),
        size(
            px((rect.size.width / width * view_width.as_f32()).max(1.0)),
            px((rect.size.height / height * view_height.as_f32()).max(1.0)),
        ),
    )
}

fn slot_anchor_view_rect(
    rect: JellyRect,
    document_bounds: JellyRect,
    view_width: Pixels,
    view_height: Pixels,
) -> Bounds<Pixels> {
    slot_view_rect(rect, document_bounds, view_width, view_height)
}

fn render_measured_region(
    id: OpenGpuiMeasurementId,
    collector: OpenGpuiBoundsCollector,
    child: impl IntoElement,
) -> AnyElement {
    node_component_kit::render_measured_region(id, collector, child)
}

fn render_slot_anchor_measurement(
    node_id: JellyNodeId,
    anchor_key: String,
    rect: Bounds<Pixels>,
    collector: OpenGpuiBoundsCollector,
) -> AnyElement {
    render_measured_region(
        OpenGpuiMeasurementId::anchor(node_id, anchor_key),
        collector,
        div()
            .absolute()
            .left(rect.origin.x)
            .top(rect.origin.y)
            .w(rect.size.width)
            .h(rect.size.height),
    )
}

fn render_slot_label(
    node_id: JellyNodeId,
    slot: &NodeSurfaceSlotProjection,
    label: String,
    status: &'static str,
) -> AnyElement {
    match slot.kind {
        NodeSurfaceSlotKind::Badge | NodeSurfaceSlotKind::MetricBadge => {
            Badge::new(open_gpui_slot_badge_element_id(node_id, &slot.key), label)
                .variant(BadgeVariant::Secondary)
                .with_size(Size::XSmall)
                .into_any_element()
        }
        NodeSurfaceSlotKind::StatusBanner => Badge::new(
            open_gpui_slot_status_label_element_id(node_id, &slot.key),
            label,
        )
        .variant(BadgeVariant::Default)
        .with_size(Size::XSmall)
        .into_any_element(),
        NodeSurfaceSlotKind::ActionRow => Badge::new(
            open_gpui_slot_action_label_element_id(node_id, &slot.key),
            label,
        )
        .variant(BadgeVariant::Outline)
        .with_size(Size::XSmall)
        .into_any_element(),
        _ => div()
            .text_xs()
            .text_color(rgb(0x334155))
            .truncate()
            .flex_shrink_1()
            .min_w(px(0.0))
            .child(format!("{label} · {status}"))
            .into_any_element(),
    }
}

fn render_slot_value(
    node_id: JellyNodeId,
    slot: &NodeSurfaceSlotProjection,
    descriptor: Option<&NodeSurfaceSlotDescriptor>,
    node_data: &Value,
    value: String,
    collector: OpenGpuiBoundsCollector,
    view: WeakEntity<JellyflowCanvasView>,
) -> AnyElement {
    if let Some(descriptor) = descriptor
        && !descriptor.controls.is_empty()
    {
        return render_slot_controls(node_id, descriptor, node_data, &value, collector, view)
            .into_any_element();
    }

    match slot.kind {
        NodeSurfaceSlotKind::ActionRow => {
            render_action_buttons(node_id, slot, &value).into_any_element()
        }
        NodeSurfaceSlotKind::Preview => div()
            .w(px(72.0))
            .flex_shrink_0()
            .child(
                Progress::new(
                    open_gpui_slot_preview_progress_element_id(node_id, &slot.key),
                    value,
                )
                .value(64.0)
                .with_size(Size::XSmall),
            )
            .into_any_element(),
        NodeSurfaceSlotKind::Badge
        | NodeSurfaceSlotKind::MetricBadge
        | NodeSurfaceSlotKind::StatusBanner => {
            Badge::new(open_gpui_slot_value_element_id(node_id, &slot.key), value)
                .variant(BadgeVariant::Default)
                .with_size(Size::XSmall)
                .into_any_element()
        }
        _ => div()
            .text_xs()
            .text_color(rgb(0x475569))
            .truncate()
            .flex_shrink_1()
            .min_w(px(0.0))
            .child(value)
            .into_any_element(),
    }
}

fn render_slot_controls(
    node_id: JellyNodeId,
    descriptor: &NodeSurfaceSlotDescriptor,
    node_data: &Value,
    value: &str,
    collector: OpenGpuiBoundsCollector,
    view: WeakEntity<JellyflowCanvasView>,
) -> impl IntoElement {
    let plans = project_slot_controls(node_data, descriptor);
    let controls = plans
        .iter()
        .take(2)
        .enumerate()
        .map(|(index, control)| {
            render_node_control_plan(
                node_id,
                descriptor.key.as_str(),
                control,
                index,
                collector.clone(),
                view.clone(),
            )
        })
        .collect::<Vec<_>>();

    div()
        .flex()
        .items_center()
        .justify_end()
        .gap_1()
        .min_w(px(0.0))
        .overflow_hidden()
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x475569))
                .truncate()
                .min_w(px(0.0))
                .child(format!("{value} · {}", plans.len())),
        )
        .children(controls)
}

fn render_node_control_plan(
    node_id: JellyNodeId,
    slot_key: &str,
    control: &OpenGpuiControlPlan,
    index: usize,
    collector: OpenGpuiBoundsCollector,
    view: WeakEntity<JellyflowCanvasView>,
) -> AnyElement {
    render_measured_region(
        OpenGpuiMeasurementId::control_in_slot(node_id, slot_key, control.key.clone()),
        collector,
        render_control_plan(node_id, slot_key, control, index, view),
    )
}

fn render_control_plan(
    node_id: JellyNodeId,
    control_scope: &str,
    control: &OpenGpuiControlPlan,
    index: usize,
    view: WeakEntity<JellyflowCanvasView>,
) -> AnyElement {
    node_component_kit::render_control_plan(
        node_id,
        control_scope,
        control,
        index,
        &node_component_kit_actions(view),
    )
}

#[cfg(test)]
fn authoring_node_from_control_data(data: Value) -> Node {
    Node {
        kind: NodeKindKey::new("open-gpui.authoring.control"),
        kind_version: 1,
        pos: JellyPoint::default(),
        origin: None,
        selectable: None,
        focusable: None,
        draggable: None,
        connectable: None,
        deletable: None,
        parent: None,
        extent: None,
        expand_parent: None,
        size: None,
        hidden: false,
        collapsed: false,
        ports: Vec::new(),
        data,
    }
}

fn report_authoring_skip(reason: OpenGpuiAuthoringSkipReason) {
    match reason {
        OpenGpuiAuthoringSkipReason::UnchangedControl { .. } => {}
        other => eprintln!("control authoring skipped: {other:?}"),
    }
}

fn report_dropped_wire_insert_error(error: OpenGpuiDroppedWireInsertError) {
    eprintln!("dropped-wire insert failed: {error}");
}

fn render_action_buttons(
    node_id: JellyNodeId,
    slot: &NodeSurfaceSlotProjection,
    value: &str,
) -> impl IntoElement {
    node_component_kit::render_action_buttons(node_id, slot, value)
}

pub(crate) fn demo_repeatable_add_item(collection_key: &str, item_count: usize) -> Value {
    let next = item_count + 1;
    match collection_key {
        "shader.inputs" => serde_json::json!({
            "name": format!("Input {next}"),
            "ty": "vec4",
            "port": format!("input_{next}")
        }),
        "table.columns" => serde_json::json!({
            "name": format!("column_{next}"),
            "ty": "text",
            "port": format!("field_column_{next}")
        }),
        "shader.properties" => serde_json::json!({
            "name": format!("property_{next}")
        }),
        "llm.params" => {
            let name = format!("param_{next}");
            serde_json::json!({
                "name": name,
                "value": format!("{{{{ input.{name} }}}}")
            })
        }
        _ => serde_json::json!({
            "name": format!("item_{next}")
        }),
    }
}

fn repeatable_action_button(
    node_id: JellyNodeId,
    id: String,
    label: &'static str,
    variant: ButtonVariant,
    disabled: bool,
    action: OpenGpuiRepeatableActionPlan,
    view: WeakEntity<JellyflowCanvasView>,
) -> AnyElement {
    node_component_kit::repeatable_action_button(
        node_id,
        id,
        label,
        variant,
        disabled,
        action,
        &node_component_kit_actions(view),
    )
}

fn render_repeatable_row(
    node_id: JellyNodeId,
    repeatable: NodeRepeatableSurfaceLayout,
    document_bounds: JellyRect,
    view_width: Pixels,
    view_height: Pixels,
    collector: OpenGpuiBoundsCollector,
    view: WeakEntity<JellyflowCanvasView>,
) -> impl IntoElement {
    let rect = slot_view_rect(repeatable.rect, document_bounds, view_width, view_height);
    let key = repeatable.projection.key.clone();
    let collection_key = repeatable.projection.key.clone();
    let add_item = demo_repeatable_add_item(&collection_key, repeatable.projection.item_count);
    let add_disabled = repeatable.projection.add_disabled_reason.is_some();
    let add_action = OpenGpuiRepeatableActionPlan::Add {
        collection_key: collection_key.clone(),
        item: add_item,
    };
    let row = div()
        .absolute()
        .left(rect.origin.x)
        .top(rect.origin.y)
        .w(rect.size.width)
        .h(rect.size.height)
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .min_w(px(0.0))
        .px_2()
        .py_1()
        .rounded_sm()
        .bg(rgb(0xf8fafc))
        .border_1()
        .border_color(rgb(0xcbd5e1))
        .overflow_hidden()
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x334155))
                .truncate()
                .min_w(px(0.0))
                .child(format!(
                    "{} · {} items",
                    repeatable.projection.label, repeatable.projection.item_count
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    Badge::new(
                        open_gpui_repeatable_collection_element_id(
                            node_id,
                            &repeatable.projection.key,
                        ),
                        format!("{} controls", repeatable.projection.controls),
                    )
                    .variant(BadgeVariant::Outline)
                    .with_size(Size::XSmall),
                )
                .child(repeatable_action_button(
                    node_id,
                    open_gpui_repeatable_add_action_element_id(node_id, &collection_key),
                    "Add",
                    ButtonVariant::Secondary,
                    add_disabled,
                    add_action,
                    view,
                )),
        );

    render_measured_region(OpenGpuiMeasurementId::slot(node_id, key), collector, row)
}

fn render_repeatable_item_row(
    node_id: JellyNodeId,
    repeatable: NodeRepeatableItemLayout,
    document_bounds: JellyRect,
    view_width: Pixels,
    view_height: Pixels,
    collector: OpenGpuiBoundsCollector,
    view: WeakEntity<JellyflowCanvasView>,
) -> impl IntoElement {
    let rect = slot_view_rect(repeatable.rect, document_bounds, view_width, view_height);
    let anchor_rect = slot_anchor_view_rect(
        repeatable.anchor_rect,
        document_bounds,
        view_width,
        view_height,
    );
    let slot_key = repeatable.projection.slot_key.clone();
    let item_id = repeatable.projection.item_id.clone();
    let collection_key = repeatable.projection.collection_key.clone();
    let item_index = repeatable.projection.item_index;
    let anchor = repeatable.projection.anchor.clone();
    let remove_disabled = repeatable.projection.remove_disabled_reason.is_some();
    let fill = if repeatable.projection.has_graph_port() {
        rgb(0xecfeff)
    } else {
        rgb(0xfffbeb)
    };
    let stroke = if repeatable.projection.has_graph_port() {
        rgb(0x67e8f9)
    } else {
        rgb(0xfbbf24)
    };
    let badge = repeatable
        .projection
        .port_key
        .as_ref()
        .map(|port| format!("port {}", port.0))
        .unwrap_or_else(|| "display".to_string());

    let row = div()
        .absolute()
        .left(rect.origin.x)
        .top(rect.origin.y)
        .w(rect.size.width)
        .h(rect.size.height)
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .min_w(px(0.0))
        .px_2()
        .py_1()
        .rounded_sm()
        .bg(fill)
        .border_1()
        .border_color(stroke)
        .overflow_hidden()
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x334155))
                .truncate()
                .min_w(px(0.0))
                .child(format!(
                    "{} · {}",
                    repeatable.projection.label, repeatable.projection.item_id
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    Badge::new(
                        open_gpui_repeatable_item_element_id(
                            node_id,
                            &repeatable.projection.collection_key,
                            &repeatable.projection.item_id,
                        ),
                        badge,
                    )
                    .variant(BadgeVariant::Outline)
                    .with_size(Size::XSmall),
                )
                .child(repeatable_action_button(
                    node_id,
                    open_gpui_repeatable_reorder_action_element_id(
                        node_id,
                        &collection_key,
                        &item_id,
                    ),
                    "Up",
                    ButtonVariant::Secondary,
                    item_index == 0,
                    OpenGpuiRepeatableActionPlan::Reorder {
                        collection_key: collection_key.clone(),
                        item_id: item_id.clone(),
                        to_index: item_index.saturating_sub(1),
                    },
                    view.clone(),
                ))
                .child(repeatable_action_button(
                    node_id,
                    open_gpui_repeatable_remove_action_element_id(
                        node_id,
                        &collection_key,
                        &item_id,
                    ),
                    "Del",
                    ButtonVariant::Destructive,
                    remove_disabled,
                    OpenGpuiRepeatableActionPlan::Remove {
                        collection_key,
                        item_id: item_id.clone(),
                    },
                    view,
                )),
        )
        .child(render_slot_anchor_measurement(
            node_id,
            anchor,
            anchor_rect,
            collector.clone(),
        ));

    render_measured_region(
        OpenGpuiMeasurementId::repeatable_item(node_id, slot_key, item_id),
        collector,
        row,
    )
}

fn adapter_slot_limit_for_height(inner_height: Pixels, semantic_slot_limit: usize) -> usize {
    let available = (inner_height.as_f32() - NODE_SURFACE_CHROME_HEIGHT).max(0.0);
    let height_limit = (available / NODE_SURFACE_SLOT_ROW_HEIGHT).floor() as usize;
    semantic_slot_limit.min(height_limit)
}

#[cfg(test)]
fn make_demo_store() -> NodeGraphStore {
    let graph = make_demo_graph().expect("demo graph transaction should apply");
    let mut store = NodeGraphStore::new(
        graph,
        NodeGraphViewState::default(),
        NodeGraphEditorConfig::default(),
    );
    let layout_request = LayoutPresetBuilder::tree().build();
    let layout_registry = builtin_layout_engine_registry();
    store
        .apply_layout(&layout_request, &layout_registry)
        .expect("tree layout should apply");

    let node_id = JellyNodeId::from_u128(3);
    let mut move_node = GraphTransaction::new().with_label("move transform node through store");
    move_node.push(GraphOp::SetNodePos {
        id: node_id,
        from: JellyPoint { x: 300.0, y: 110.0 },
        to: JellyPoint { x: 316.0, y: 136.0 },
    });

    store
        .dispatch_transaction(&move_node)
        .expect("demo graph store dispatch should succeed");
    store
}

#[cfg(test)]
fn make_demo_graph() -> Result<Graph, Box<dyn std::error::Error>> {
    let source = JellyNodeId::from_u128(2);
    let transform = JellyNodeId::from_u128(3);
    let sink = JellyNodeId::from_u128(4);

    let source_out = JellyPortId::from_u128(20);
    let transform_in = JellyPortId::from_u128(30);
    let transform_out = JellyPortId::from_u128(31);
    let sink_in = JellyPortId::from_u128(40);

    let mut graph = Graph::new(GraphId::from_u128(1));
    let mut tx = GraphTransaction::new().with_label("build demo jellyflow graph");
    tx.extend([
        GraphOp::AddNode {
            id: source,
            node: make_node(
                "demo.source",
                "Load CSV",
                "Reads orders.csv and emits typed rows.",
                40.0,
                90.0,
            ),
        },
        GraphOp::AddNode {
            id: transform,
            node: make_node(
                "demo.llm",
                "Normalize Rows",
                "Maps raw rows into a clean order stream.",
                300.0,
                110.0,
            ),
        },
        GraphOp::AddNode {
            id: sink,
            node: make_node(
                "demo.workflow_output",
                "Publish Report",
                "Writes the summarized result to the reporting channel.",
                580.0,
                90.0,
            ),
        },
        GraphOp::AddPort {
            id: source_out,
            port: make_port(source, "out", PortDirection::Out),
        },
        GraphOp::AddPort {
            id: transform_in,
            port: make_port(transform, "prompt", PortDirection::In),
        },
        GraphOp::AddPort {
            id: transform_out,
            port: make_port(transform, "completion", PortDirection::Out),
        },
        GraphOp::AddPort {
            id: sink_in,
            port: make_port(sink, "result", PortDirection::In),
        },
        GraphOp::SetNodePorts {
            id: source,
            from: Vec::new(),
            to: vec![source_out],
        },
        GraphOp::SetNodePorts {
            id: transform,
            from: Vec::new(),
            to: vec![transform_in, transform_out],
        },
        GraphOp::SetNodePorts {
            id: sink,
            from: Vec::new(),
            to: vec![sink_in],
        },
        GraphOp::AddEdge {
            id: JellyEdgeId::from_u128(200),
            edge: make_edge(source_out, transform_in),
        },
        GraphOp::AddEdge {
            id: JellyEdgeId::from_u128(201),
            edge: make_edge(transform_out, sink_in),
        },
    ]);
    tx.apply_to(&mut graph)?;
    Ok(graph)
}

#[cfg(test)]
fn make_node(kind: &str, label: &str, description: &str, x: f32, y: f32) -> Node {
    Node {
        kind: NodeKindKey::new(kind),
        kind_version: 1,
        pos: JellyPoint { x, y },
        origin: None,
        selectable: None,
        focusable: None,
        draggable: None,
        connectable: None,
        deletable: None,
        parent: None,
        extent: None,
        expand_parent: None,
        size: Some(JellySize {
            width: if kind == "demo.llm" { 268.0 } else { 236.0 },
            height: if kind == "demo.llm" { 228.0 } else { 188.0 },
        }),
        hidden: false,
        collapsed: false,
        ports: Vec::new(),
        data: serde_json::json!({
            "label": label,
            "title": label,
            "summary": description,
            "description": description,
            "fields": {
                "prompt": "Customer intake + policy",
                "completion": "Priority and route"
            },
            "meta": {
                "model": "gpt-4.1-mini",
                "cardinality": "1:N",
                "branch": "yes"
            },
            "nested": {
                "policy": {
                    "guardrails": "Block PII",
                    "response": "Return structured route"
                }
            },
            "actions": {
                "primary": ["Test prompt", "Open trace", "Copy config"],
                "table": ["Add column", "Inspect relation"]
            },
            "preview": "Evidence card"
        }),
    }
}

#[cfg(test)]
fn make_port(node: JellyNodeId, key: &str, dir: PortDirection) -> Port {
    Port {
        node,
        key: PortKey::new(key),
        dir,
        kind: PortKind::Data,
        capacity: PortCapacity::Multi,
        connectable: None,
        connectable_start: None,
        connectable_end: None,
        ty: None,
        data: Value::Null,
    }
}

#[cfg(test)]
fn make_edge(from: JellyPortId, to: JellyPortId) -> Edge {
    Edge {
        kind: EdgeKind::Data,
        from,
        to,
        hidden: false,
        selectable: None,
        focusable: None,
        interaction_width: Some(14.0),
        deletable: None,
        reconnectable: None,
        data: Value::Null,
        view: Default::default(),
    }
}

fn project_store(
    store: &NodeGraphStore,
) -> Result<(CanvasDocument, ProjectionSummary), DocumentError> {
    let graph = store.graph();
    let kit_registry = NodeKitRegistry::builtin();
    let semantic_registry = kit_registry.node_registry();
    let measured_store = measurement_store_with_projection_fallback(store, &semantic_registry);
    let mut builder = CanvasDocument::builder();

    for (id, node) in graph.nodes().iter() {
        builder.add_node(project_node(
            id,
            node,
            measured_store.graph(),
            &measured_store,
            &semantic_registry,
        ))?;
    }

    for (id, edge) in graph.edges().iter() {
        let Some(from) = graph.ports().get(&edge.from) else {
            continue;
        };
        let Some(to) = graph.ports().get(&edge.to) else {
            continue;
        };

        let mut canvas_edge = open_gpui_canvas::CanvasEdge::new(
            canvas_edge_id(id),
            open_gpui_canvas::CanvasEndpoint::new(
                canvas_node_id(&from.node),
                Some(canvas_port_id(&edge.from)),
            ),
            open_gpui_canvas::CanvasEndpoint::new(
                canvas_node_id(&to.node),
                Some(canvas_port_id(&edge.to)),
            ),
        );
        canvas_edge.kind = "jellyflow.edge.data".to_string();
        canvas_edge.route = project_edge_route(edge);
        canvas_edge.route.interaction_width = px(edge.interaction_width.unwrap_or(14.0));
        canvas_edge.data.insert(
            "jellyflow_edge_id".to_string(),
            serde_json::json!(canvas_edge_id(id)),
        );
        builder.add_edge(canvas_edge)?;
    }

    let document = builder.build()?;
    let projection = ProjectionSummary {
        graph_nodes: graph.nodes().len(),
        graph_ports: graph.ports().len(),
        graph_edges: graph.edges().len(),
        canvas_nodes: document.nodes().count(),
        canvas_edges: document.edges().count(),
        layout_preset: "tree -> tidy_tree".to_string(),
        last_commit: "jellyflow-open-gpui owns reusable adapter gates".to_string(),
        source: "jellyflow graph v1".to_string(),
        adapter: "open-gpui-canvas consumer of jellyflow-open-gpui".to_string(),
        kit: "workflow.automation / erd.table / shader.blueprint / mind-map.knowledge-canvas"
            .to_string(),
        capability: GpuiAuthoringCapabilitySummary {
            controls: "live/partial",
            repeatables: "live/partial",
            actions: "live/partial",
            layout_measurement: NodeSurfaceMeasurementSource::LayoutPass,
            layout_status: GPUI_LAYOUT_PASS_MEASUREMENT_STATUS,
        },
    };

    Ok((document, projection))
}

fn project_kit_fixture(
    kit_key: &str,
    fixture_key: &str,
) -> Result<(NodeGraphStore, CanvasDocument, ProjectionSummary), Box<dyn std::error::Error>> {
    use jellyflow::runtime::schema::NodeKitKey;

    let graph =
        NodeKitRegistry::builtin().fixture_graph(&NodeKitKey::from(kit_key), fixture_key)?;
    let store = NodeGraphStore::new(
        graph,
        NodeGraphViewState::default(),
        NodeGraphEditorConfig::default(),
    );
    let (document, projection) =
        project_store(&store).map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
    Ok((store, document, projection))
}

fn project_product_gallery_case(
    case: &product_gallery::ProductGalleryCase,
) -> Result<(NodeGraphStore, CanvasDocument, ProjectionSummary), Box<dyn std::error::Error>> {
    project_kit_fixture(case.kit_key(), case.fixture_key())
}

#[cfg(test)]
fn project_schema_node(
    kind: &str,
) -> Result<
    (
        NodeGraphStore,
        CanvasDocument,
        ProjectionSummary,
        JellyNodeId,
    ),
    Box<dyn std::error::Error>,
> {
    use jellyflow::runtime::runtime::create_node::CreateNodeRequest;

    let registry = NodeKitRegistry::builtin().node_registry();
    let mut store = NodeGraphStore::new(
        Graph::new(GraphId::from_u128(900)),
        NodeGraphViewState::default(),
        NodeGraphEditorConfig::default(),
    );
    let outcome = store.apply_create_node_from_schema(
        &registry,
        CreateNodeRequest::new(NodeKindKey::new(kind), JellyPoint::default()),
    )?;
    let node_id = outcome.node_id();
    let (document, projection) = project_store(&store)?;
    Ok((store, document, projection, node_id))
}

fn project_edge_route(edge: &Edge) -> open_gpui_canvas::CanvasEdgeRoute {
    match edge.view.route_kind {
        Some(jellyflow::core::EdgeRouteKind::Straight) => {
            open_gpui_canvas::CanvasEdgeRoute::straight()
        }
        Some(jellyflow::core::EdgeRouteKind::Bezier) => open_gpui_canvas::CanvasEdgeRoute::new(
            open_gpui_canvas::CanvasEdgeRouteKind::CUBIC_BEZIER,
        ),
        Some(jellyflow::core::EdgeRouteKind::Orthogonal)
        | Some(jellyflow::core::EdgeRouteKind::SmoothStep)
        | None => open_gpui_canvas::CanvasEdgeRoute::orthogonal(),
    }
}

fn project_node(
    id: &JellyNodeId,
    node: &Node,
    graph: &Graph,
    measurement_store: &NodeGraphStore,
    semantic_registry: &NodeRegistry,
) -> CanvasNode {
    let descriptor = semantic_registry
        .view_descriptor(&node.kind)
        .expect("demo graph should resolve a builtin node descriptor");
    let preset = OpenGpuiProductSurfacePreset::from_descriptor(&descriptor);
    let requested_size = node
        .size
        .unwrap_or_else(|| preset.initial_size_or(default_adapter_node_size()));
    let node_size = preset.readable_size_for_request(requested_size);
    let mut canvas_node = CanvasNode::new(
        canvas_node_id(id),
        point(px(node.pos.x), px(node.pos.y)),
        size(px(node_size.width), px(node_size.height)),
    );
    canvas_node.kind = descriptor.renderer_key.clone();
    canvas_node.hidden = node.hidden;
    canvas_node.data = canvas_value_from_json(node.data.clone());
    canvas_node.data.insert(
        "jellyflow_kind".to_string(),
        serde_json::json!(node.kind.0.as_str().to_string()),
    );
    canvas_node.data.insert(
        "jellyflow_node_id".to_string(),
        serde_json::json!(canvas_node_id(id)),
    );
    canvas_node.data.insert(
        "ports".to_string(),
        serde_json::json!(port_summary(node, graph)),
    );
    canvas_node.data.insert(
        REPEATABLE_ITEM_SNAPSHOTS_FIELD.to_string(),
        Value::Array(
            repeatable_item_projection(&descriptor, node, graph, id)
                .into_iter()
                .map(repeatable_item_projection_to_snapshot_value)
                .collect(),
        ),
    );

    let input_ports = node
        .ports
        .iter()
        .filter(|id| {
            graph
                .ports()
                .get(id)
                .is_some_and(|port| port.dir == PortDirection::In)
        })
        .copied()
        .collect::<Vec<_>>();
    let output_ports = node
        .ports
        .iter()
        .filter(|id| {
            graph
                .ports()
                .get(id)
                .is_some_and(|port| port.dir == PortDirection::Out)
        })
        .copied()
        .collect::<Vec<_>>();

    for (index, port_id) in input_ports.iter().enumerate() {
        let position = graph
            .ports()
            .get(port_id)
            .and_then(|port| {
                measured_handle_position(
                    measurement_store,
                    *id,
                    *port_id,
                    port.dir,
                    node_size,
                    index,
                    input_ports.len(),
                )
            })
            .unwrap_or_else(|| JellyPoint {
                x: 0.0,
                y: port_y(index, input_ports.len(), node_size.height),
            });
        canvas_node.handles.push(project_handle(
            *port_id,
            HandleRole::Target,
            position.x,
            position.y,
            graph.ports().get(port_id).and_then(|port| port.connectable),
        ));
    }

    for (index, port_id) in output_ports.iter().enumerate() {
        let position = graph
            .ports()
            .get(port_id)
            .and_then(|port| {
                measured_handle_position(
                    measurement_store,
                    *id,
                    *port_id,
                    port.dir,
                    node_size,
                    index,
                    output_ports.len(),
                )
            })
            .unwrap_or_else(|| JellyPoint {
                x: node_size.width,
                y: port_y(index, output_ports.len(), node_size.height),
            });
        canvas_node.handles.push(project_handle(
            *port_id,
            HandleRole::Source,
            position.x,
            position.y,
            graph.ports().get(port_id).and_then(|port| port.connectable),
        ));
    }

    canvas_node
}

fn default_adapter_node_size() -> JellySize {
    JellySize {
        width: 236.0,
        height: 188.0,
    }
}

fn measurement_store_with_projection_fallback(
    store: &NodeGraphStore,
    semantic_registry: &NodeRegistry,
) -> NodeGraphStore {
    let mut measured_store = NodeGraphStore::new(
        store.graph().clone(),
        store.view_state().clone(),
        NodeGraphEditorConfig::default(),
    );

    let existing_measurements = store
        .graph()
        .nodes()
        .keys()
        .filter_map(|id| match store.node_measurement_status(*id) {
            NodeMeasurementStatus::Fresh { .. } => store.node_measurement(*id),
            NodeMeasurementStatus::Missing | NodeMeasurementStatus::Dirty { .. } => None,
        })
        .collect::<Vec<_>>();

    for measurement in existing_measurements {
        measured_store
            .report_node_measurement(measurement)
            .expect("live GPUI measurement should match the graph");
    }

    let measurements = measured_store
        .graph()
        .nodes()
        .iter()
        .filter_map(|(id, node)| {
            if measured_store.node_measurement(*id).is_some() {
                return None;
            }
            let descriptor = semantic_registry.view_descriptor(&node.kind)?;
            Some(project_node_measurement(
                id,
                node,
                measured_store.graph(),
                &descriptor,
            ))
        })
        .collect::<Vec<_>>();

    for measurement in measurements {
        measured_store
            .report_node_measurement(measurement)
            .expect("projected GPUI measurement should match the graph");
    }

    measured_store
}

fn measured_handle_position(
    store: &NodeGraphStore,
    node: JellyNodeId,
    port: JellyPortId,
    direction: PortDirection,
    node_size: JellySize,
    fallback_index: usize,
    fallback_count: usize,
) -> Option<JellyPoint> {
    let resolution =
        store.resolve_node_handle_measurement(ConnectionHandleRef::new(node, port, direction));
    match resolution.source {
        NodeHandleMeasurementSource::MeasuredHandle
        | NodeHandleMeasurementSource::MeasuredAnchor { .. } => resolution
            .bounds
            .map(|bounds| handle_position_from_bounds(bounds, node_size)),
        NodeHandleMeasurementSource::Fallback { .. } => Some(JellyPoint {
            x: match direction {
                PortDirection::In => 0.0,
                PortDirection::Out => node_size.width,
            },
            y: port_y(fallback_index, fallback_count, node_size.height),
        }),
    }
}

fn handle_position_from_bounds(bounds: HandleBounds, node_size: JellySize) -> JellyPoint {
    match bounds.position {
        HandlePosition::Left => JellyPoint {
            x: 0.0,
            y: bounds.rect.origin.y + bounds.rect.size.height * 0.5,
        },
        HandlePosition::Right => JellyPoint {
            x: node_size.width,
            y: bounds.rect.origin.y + bounds.rect.size.height * 0.5,
        },
        HandlePosition::Top => JellyPoint {
            x: bounds.rect.origin.x + bounds.rect.size.width * 0.5,
            y: 0.0,
        },
        HandlePosition::Bottom => JellyPoint {
            x: bounds.rect.origin.x + bounds.rect.size.width * 0.5,
            y: node_size.height,
        },
    }
}

fn jellyflow_kind_registry() -> CanvasKindRegistry {
    let mut registry = CanvasKindRegistry::open();
    for kind in [
        "data-card",
        "task-card",
        "decision-card",
        "output-card",
        "table-card",
        "shader-card",
        "topic-card",
        "idea-card",
        "source-card",
    ] {
        registry.register_node_kind(
            kind,
            CanvasNodeKind::new().with_render_policy(JellyflowNodeKind),
        );
    }
    registry
}

#[cfg(test)]
fn demo_state() -> (NodeGraphStore, CanvasEditor, ProjectionSummary) {
    let store = make_demo_store();
    let (document, projection) = project_store(&store).expect("demo graph should project");
    let mut editor = editor_for_document(document)
        .expect("canvas editor should accept projected Jellyflow graph");
    editor
        .apply_tool_intent(CanvasToolIntent::ReplaceSelection(HitTarget::Node(
            NodeId::from(canvas_node_id(&JellyNodeId::from_u128(INITIAL_SELECTION))),
        )))
        .expect("initial selection should exist");
    (store, editor, projection)
}

fn product_gallery_state() -> (
    product_gallery::ProductGalleryState,
    NodeGraphStore,
    CanvasEditor,
    ProjectionSummary,
) {
    let gallery = product_gallery::ProductGalleryState::default();
    let (store, document, projection) = project_product_gallery_case(gallery.active_case())
        .expect("default product gallery fixture should project");
    let editor =
        editor_for_document(document).expect("canvas editor should accept product gallery fixture");
    (gallery, store, editor, projection)
}

fn editor_for_document(document: CanvasDocument) -> Result<CanvasEditor, DocumentError> {
    let initial_selection = document
        .nodes()
        .next()
        .map(|node| NodeId::from(node.id.as_str()));
    let mut editor = CanvasEditor::try_new_with_kind_registry(document, jellyflow_kind_registry())?;
    editor.set_viewport(initial_viewport_for_document(editor.document()));
    if let Some(id) = initial_selection {
        editor.apply_tool_intent(CanvasToolIntent::ReplaceSelection(HitTarget::Node(id)))?;
    }
    Ok(editor)
}

fn initial_viewport_for_document(document: &CanvasDocument) -> CanvasViewport {
    fit_viewport_for_document(document, default_canvas_view_size())
}

fn default_canvas_view_size() -> open_gpui::Size<Pixels> {
    canvas_view_size_from_window_size(size(px(CANVAS_WIDTH), px(CANVAS_HEIGHT)))
}

fn canvas_view_size_from_window_size(
    window_size: open_gpui::Size<Pixels>,
) -> open_gpui::Size<Pixels> {
    size(
        (window_size.width - px(SIDEBAR_WIDTH)).max(px(320.0)),
        (window_size.height - px(TOOLBAR_HEIGHT)).max(px(240.0)),
    )
}

fn fit_viewport_for_document(
    document: &CanvasDocument,
    view_size: open_gpui::Size<Pixels>,
) -> CanvasViewport {
    let Some(bounds) = document_content_bounds(document) else {
        return CanvasViewport::default();
    };
    let content_width = bounds.size.width.as_f32().max(1.0);
    let content_height = bounds.size.height.as_f32().max(1.0);
    let view_width = view_size.width.as_f32().max(1.0);
    let view_height = view_size.height.as_f32().max(1.0);
    let available_width = (view_width - INITIAL_VIEWPORT_PADDING * 2.0).max(1.0);
    let available_height = (view_height - INITIAL_VIEWPORT_PADDING * 2.0).max(1.0);
    let zoom = (available_width / content_width)
        .min(available_height / content_height)
        .clamp(INITIAL_VIEWPORT_MIN_ZOOM, INITIAL_VIEWPORT_MAX_ZOOM);
    let visible_width = view_width / zoom;
    let visible_height = view_height / zoom;
    let content_center_x = bounds.origin.x.as_f32() + content_width / 2.0;
    let content_center_y = bounds.origin.y.as_f32() + content_height / 2.0;

    CanvasViewport::new(
        point(
            px(content_center_x - visible_width / 2.0),
            px(content_center_y - visible_height / 2.0),
        ),
        zoom,
    )
    .unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CanvasViewportSizeUpdate {
    viewport: CanvasViewport,
    auto_fit_viewport: bool,
    last_canvas_view_size: Option<open_gpui::Size<Pixels>>,
}

fn canvas_viewport_size_update(
    document: &CanvasDocument,
    viewport: CanvasViewport,
    auto_fit_viewport: bool,
    previous_size: Option<open_gpui::Size<Pixels>>,
    view_size: open_gpui::Size<Pixels>,
) -> CanvasViewportSizeUpdate {
    let viewport = if auto_fit_viewport {
        fit_viewport_for_document(document, view_size)
    } else if let Some(previous_size) = previous_size {
        if previous_size == view_size {
            viewport
        } else {
            preserve_viewport_center_for_resize(viewport, previous_size, view_size)
        }
    } else {
        viewport
    };

    CanvasViewportSizeUpdate {
        viewport,
        auto_fit_viewport: false,
        last_canvas_view_size: Some(view_size),
    }
}

fn canvas_drag_start_event_from_bounds(
    bounds: Bounds<Pixels>,
    event: &MouseDownEvent,
) -> Option<CanvasEvent> {
    if event.button != MouseButton::Left {
        return None;
    }

    bounds
        .contains(&event.position)
        .then(|| CanvasEvent::PointerDown {
            position: event.position - bounds.origin,
            button: PointerButton::Primary,
            modifiers: canvas_key_modifiers(event.modifiers),
        })
}

fn canvas_key_modifiers(modifiers: Modifiers) -> CanvasKeyModifiers {
    CanvasKeyModifiers {
        shift: modifiers.shift,
        alt: modifiers.alt,
        control: modifiers.control,
        platform: modifiers.platform,
        function: modifiers.function,
    }
}

fn preserve_viewport_center_for_resize(
    viewport: CanvasViewport,
    previous_size: open_gpui::Size<Pixels>,
    next_size: open_gpui::Size<Pixels>,
) -> CanvasViewport {
    let previous_center = point(
        previous_size.width * (0.5 / viewport.zoom),
        previous_size.height * (0.5 / viewport.zoom),
    );
    let next_center = point(
        next_size.width * (0.5 / viewport.zoom),
        next_size.height * (0.5 / viewport.zoom),
    );

    CanvasViewport::new(
        viewport.origin + previous_center - next_center,
        viewport.zoom,
    )
    .unwrap_or(viewport)
}

fn document_content_bounds(document: &CanvasDocument) -> Option<Bounds<Pixels>> {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for node in document.nodes().filter(|node| !node.hidden) {
        let bounds = node.bounds();
        min_x = min_x.min(bounds.origin.x.as_f32());
        min_y = min_y.min(bounds.origin.y.as_f32());
        max_x = max_x.max(bounds.origin.x.as_f32() + bounds.size.width.as_f32());
        max_y = max_y.max(bounds.origin.y.as_f32() + bounds.size.height.as_f32());
    }

    if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
        Some(Bounds::new(
            point(px(min_x), px(min_y)),
            size(px((max_x - min_x).max(1.0)), px((max_y - min_y).max(1.0))),
        ))
    } else {
        None
    }
}

fn canvas_document_transform_transaction(
    adapter: &OpenGpuiAdapter,
    store: &NodeGraphStore,
    document: &CanvasDocument,
) -> GraphTransaction {
    adapter.plan_transform_sync_transaction(
        store.graph(),
        document.nodes().filter_map(canvas_node_transform_snapshot),
    )
}

fn canvas_node_transform_snapshot(
    canvas_node: &CanvasNode,
) -> Option<OpenGpuiNodeTransformSnapshot> {
    let node = jelly_node_id_from_node(canvas_node)?;
    Some(
        OpenGpuiNodeTransformSnapshot::new(
            node,
            JellyPoint {
                x: canvas_node.position.x.as_f32(),
                y: canvas_node.position.y.as_f32(),
            },
        )
        .with_size(JellySize {
            width: canvas_node.size.width.as_f32(),
            height: canvas_node.size.height.as_f32(),
        }),
    )
}

fn canvas_document_connection_sync_transactions(
    adapter: &OpenGpuiAdapter,
    store: &NodeGraphStore,
    document: &CanvasDocument,
) -> Result<Vec<GraphTransaction>, OpenGpuiConnectionSyncError> {
    let interaction = store.resolved_interaction_state();
    let mut requests = Vec::new();
    let mut seen_projected_edges = Vec::new();

    for canvas_edge in document.edges() {
        let Some(source_port) = jelly_port_id_from_canvas_endpoint(&canvas_edge.source) else {
            continue;
        };
        let Some(target_port) = jelly_port_id_from_canvas_endpoint(&canvas_edge.target) else {
            continue;
        };

        let request = if let Some(edge_id) = jelly_edge_id_from_canvas_edge(canvas_edge) {
            seen_projected_edges.push(edge_id);
            let Some(edge) = store.graph().edges().get(&edge_id) else {
                continue;
            };
            if edge.from != source_port {
                Some(OpenGpuiConnectionSyncRequest::Reconnect {
                    edge: edge_id,
                    endpoint: EdgeEndpoint::From,
                    new_port: source_port,
                })
            } else if edge.to != target_port {
                Some(OpenGpuiConnectionSyncRequest::Reconnect {
                    edge: edge_id,
                    endpoint: EdgeEndpoint::To,
                    new_port: target_port,
                })
            } else {
                None
            }
        } else {
            Some(OpenGpuiConnectionSyncRequest::Connect {
                source: source_port,
                target: target_port,
                edge: None,
            })
        };

        if let Some(request) = request {
            requests.push(request);
        }
    }

    for edge_id in store.graph().edges().keys() {
        if seen_projected_edges.contains(edge_id) {
            continue;
        }
        requests.push(OpenGpuiConnectionSyncRequest::Delete { edge: *edge_id });
    }

    adapter.plan_connection_sync_transactions(
        store.graph(),
        requests,
        jellyflow::core::NodeGraphConnectionMode::Strict,
        &interaction,
    )
}

fn canvas_value_from_json(value: Value) -> open_gpui_canvas::CanvasValue {
    match value {
        Value::Object(map) => map,
        other => {
            let mut data = open_gpui_canvas::CanvasValue::new();
            data.insert("value".to_string(), other);
            data
        }
    }
}

fn canvas_node_id(id: &JellyNodeId) -> String {
    id.0.to_string()
}

fn canvas_port_id(id: &JellyPortId) -> String {
    id.0.to_string()
}

fn canvas_edge_id(id: &JellyEdgeId) -> String {
    id.0.to_string()
}

fn jelly_edge_id_from_canvas_edge(edge: &open_gpui_canvas::CanvasEdge) -> Option<JellyEdgeId> {
    edge.data
        .get("jellyflow_edge_id")
        .and_then(Value::as_str)
        .and_then(jelly_edge_id_from_str)
        .or_else(|| jelly_edge_id_from_str(edge.id.as_str()))
}

fn jelly_port_id_from_canvas_endpoint(
    endpoint: &open_gpui_canvas::CanvasEndpoint,
) -> Option<JellyPortId> {
    endpoint
        .handle_id
        .as_ref()
        .and_then(|handle| jelly_port_id_from_str(handle.as_str()))
}

fn jelly_node_id_from_node(node: &CanvasNode) -> Option<JellyNodeId> {
    data_string(node, "jellyflow_node_id")
        .and_then(jelly_node_id_from_str)
        .or_else(|| jelly_node_id_from_str(node.id.as_str()))
}

fn jelly_node_id_from_str(id: &str) -> Option<JellyNodeId> {
    id.parse::<u128>()
        .ok()
        .map(JellyNodeId::from_u128)
        .or_else(|| serde_json::from_value(Value::String(id.to_string())).ok())
}

fn jelly_port_id_from_str(id: &str) -> Option<JellyPortId> {
    id.parse::<u128>()
        .ok()
        .map(JellyPortId::from_u128)
        .or_else(|| serde_json::from_value(Value::String(id.to_string())).ok())
}

fn jelly_edge_id_from_str(id: &str) -> Option<JellyEdgeId> {
    id.parse::<u128>()
        .ok()
        .map(JellyEdgeId::from_u128)
        .or_else(|| serde_json::from_value(Value::String(id.to_string())).ok())
}

fn jelly_rect_from_bounds(bounds: Bounds<Pixels>) -> JellyRect {
    JellyRect {
        origin: JellyPoint {
            x: bounds.origin.x.as_f32(),
            y: bounds.origin.y.as_f32(),
        },
        size: JellySize {
            width: bounds.size.width.as_f32(),
            height: bounds.size.height.as_f32(),
        },
    }
}

fn repeatable_item_projection_to_snapshot_value(item: NodeRepeatableItemProjection) -> Value {
    serde_json::json!({
        "collection_key": item.collection_key,
        "item_id": item.item_id,
        "item_index": item.item_index,
        "slot_key": item.slot_key,
        "anchor": item.anchor,
        "label": item.label,
        "port_key": item.port_key.map(|port| port.0),
        "port_id": item.port_id.map(|port| port.0.to_string()),
        "port_direction": item.port_direction.map(port_direction_snapshot_value),
        "dynamic_port_policy": dynamic_port_policy_snapshot_value(item.dynamic_port_policy),
        "controls": item.controls,
        "remove_disabled_reason": item.remove_disabled_reason,
    })
}

fn repeatable_item_snapshots_from_node(node: &CanvasNode) -> Vec<RepeatableItemSnapshot> {
    node.data
        .get(REPEATABLE_ITEM_SNAPSHOTS_FIELD)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(repeatable_item_snapshot_from_value)
                .collect()
        })
        .unwrap_or_default()
}

fn repeatable_item_snapshot_from_value(value: &Value) -> Option<RepeatableItemSnapshot> {
    let object = value.as_object()?;
    Some(RepeatableItemSnapshot {
        collection_key: object.get("collection_key")?.as_str()?.to_string(),
        item_id: object.get("item_id")?.as_str()?.to_string(),
        item_index: object
            .get("item_index")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize,
        slot_key: object.get("slot_key")?.as_str()?.to_string(),
        anchor: object.get("anchor")?.as_str()?.to_string(),
        label: object.get("label")?.as_str()?.to_string(),
        port_key: object
            .get("port_key")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        port_id: object
            .get("port_id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        port_direction: object
            .get("port_direction")
            .and_then(Value::as_str)
            .and_then(port_direction_from_snapshot_value),
        dynamic_port_policy: object
            .get("dynamic_port_policy")
            .and_then(Value::as_str)
            .and_then(dynamic_port_policy_from_snapshot_value)
            .unwrap_or(OpenGpuiDynamicPortPolicy::DisplayOnly),
        controls: object
            .get("controls")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize,
        remove_disabled_reason: object
            .get("remove_disabled_reason")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    })
}

fn repeatable_item_projection_from_snapshot(
    snapshot: RepeatableItemSnapshot,
) -> NodeRepeatableItemProjection {
    NodeRepeatableItemProjection {
        collection_key: snapshot.collection_key,
        item_id: snapshot.item_id,
        item_index: snapshot.item_index,
        slot_key: snapshot.slot_key,
        anchor: snapshot.anchor,
        label: snapshot.label,
        port_key: snapshot.port_key.map(PortKey::new),
        port_id: snapshot.port_id.as_deref().and_then(jelly_port_id_from_str),
        port_direction: snapshot.port_direction,
        dynamic_port_policy: snapshot.dynamic_port_policy,
        controls: snapshot.controls,
        remove_disabled_reason: snapshot.remove_disabled_reason,
        item_data: Value::Null,
    }
}

fn port_direction_snapshot_value(direction: PortDirection) -> &'static str {
    match direction {
        PortDirection::In => "in",
        PortDirection::Out => "out",
    }
}

fn port_direction_from_snapshot_value(value: &str) -> Option<PortDirection> {
    match value {
        "in" => Some(PortDirection::In),
        "out" => Some(PortDirection::Out),
        _ => None,
    }
}

fn dynamic_port_policy_snapshot_value(policy: OpenGpuiDynamicPortPolicy) -> &'static str {
    match policy {
        OpenGpuiDynamicPortPolicy::DisplayOnly => "display_only",
        OpenGpuiDynamicPortPolicy::BoundToGraphPort => "bound_to_graph_port",
        OpenGpuiDynamicPortPolicy::MissingGraphPort => "missing_graph_port",
    }
}

fn dynamic_port_policy_from_snapshot_value(value: &str) -> Option<OpenGpuiDynamicPortPolicy> {
    match value {
        "display_only" => Some(OpenGpuiDynamicPortPolicy::DisplayOnly),
        "bound_to_graph_port" => Some(OpenGpuiDynamicPortPolicy::BoundToGraphPort),
        "missing_graph_port" => Some(OpenGpuiDynamicPortPolicy::MissingGraphPort),
        _ => None,
    }
}

fn node_title(node: &CanvasNode) -> String {
    data_string(node, "label")
        .or_else(|| data_string(node, "title"))
        .or_else(|| data_string(node, "summary"))
        .or_else(|| data_string(node, "description"))
        .unwrap_or_else(|| node.id.as_str())
        .to_string()
}

fn data_string<'a>(node: &'a CanvasNode, field: &str) -> Option<&'a str> {
    node.data.get(field).and_then(|value| value.as_str())
}

fn port_summary(node: &Node, graph: &Graph) -> String {
    node.ports
        .iter()
        .filter_map(|id| graph.ports().get(id))
        .map(|port| {
            let dir = match port.dir {
                PortDirection::In => "in",
                PortDirection::Out => "out",
            };
            format!("{dir}:{}", port.key.0)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn project_handle(
    port_id: JellyPortId,
    role: HandleRole,
    x: f32,
    y: f32,
    connectable: Option<bool>,
) -> CanvasHandle {
    let mut handle = CanvasHandle::new(canvas_port_id(&port_id), point(px(x), px(y)));
    handle.role = role;
    handle.connectable = connectable.unwrap_or(true);
    handle
}

fn port_y(index: usize, count: usize, height: f32) -> f32 {
    let step = height / (count + 1) as f32;
    step * (index + 1) as f32
}

fn main() {
    application().run(|cx: &mut App| {
        init_canvas_jellyflow_app(cx);

        let bounds = Bounds::centered(None, size(px(CANVAS_WIDTH), px(CANVAS_HEIGHT)), cx);
        let (gallery, store, editor, projection) = product_gallery_state();
        let node_kit_registry = NodeKitRegistry::builtin();
        let semantic_registry = node_kit_registry.node_registry();

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|cx| JellyflowCanvasView {
                    editor,
                    store,
                    focus_handle: cx.focus_handle(),
                    projection,
                    gallery,
                    adapter: OpenGpuiAdapter::default(),
                    semantic_registry,
                    node_kit_registry,
                    measured_regions: OpenGpuiBoundsCollector::new(),
                    measurement_revision: 1,
                    measurement_frame_pending: false,
                    auto_fit_viewport: true,
                    last_canvas_view_size: None,
                    last_canvas_bounds: None,
                })
            },
        )
        .expect("failed to open Jellyflow canvas window");

        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_component_kit::{
        control_component_disabled, control_component_interaction_disabled,
        control_component_read_only,
    };
    use jellyflow::runtime::{
        runtime::measurement::{
            MeasuredSurfaceAnchor, NodeInternalsInvalidation, NodeInternalsInvalidationReason,
            NodeMeasurement,
        },
        schema::NodeControlKind,
    };
    use jellyflow_open_gpui::{
        control_option_key, open_gpui_action_button_element_id, open_gpui_action_menu_element_id,
        open_gpui_control_element_id, plan_action_dispatch, plan_dropped_wire_insert,
        project_dropped_wire_menu, projected_node_surface_component_layout,
        testing::{
            OpenGpuiHostCapabilityGap, OpenGpuiHostProductInteractionGap,
            OpenGpuiHostProductInteractionReport, OpenGpuiHostRendererSource,
            OpenGpuiHostSurfaceReport, OpenGpuiHostSurfaceReportRow,
            assert_authoring_interaction_regression_gates, assert_host_surface_report_contract,
            assert_host_visual_interaction_report_gates, assert_product_fixture_regression_gates,
            assert_product_gallery_host_report_gates,
            assert_product_interaction_characterization_report_contract, product_fixture_catalog,
        },
    };
    use open_gpui::{MouseMoveEvent, MouseUpEvent};
    use open_gpui_canvas::{
        CanvasConnectionEndpointRole, CanvasEditorInputMapper, CanvasGeometryFacts, CanvasRuntime,
        connection_hit_options,
    };

    #[test]
    fn projects_jellyflow_store_into_canvas_document() {
        let store = make_demo_store();
        let (document, projection) = project_store(&store).unwrap();

        assert_eq!(projection.graph_nodes, 3);
        assert_eq!(projection.graph_ports, 4);
        assert_eq!(projection.graph_edges, 2);
        assert_eq!(projection.canvas_nodes, 3);
        assert_eq!(projection.canvas_edges, 2);
        assert_eq!(projection.layout_preset, "tree -> tidy_tree");
        assert!(projection.kit.contains("workflow.automation"));

        let transform_id = NodeId::from(canvas_node_id(&JellyNodeId::from_u128(3)));
        let transform = document.node(&transform_id).unwrap();
        assert_eq!(transform.position, point(px(316.0), px(136.0)));
        assert_eq!(transform.handles.len(), 2);
        assert!(
            transform
                .handles
                .iter()
                .any(|handle| handle.role == HandleRole::Target)
        );
        assert!(
            transform
                .handles
                .iter()
                .any(|handle| handle.role == HandleRole::Source)
        );

        let edge_id = open_gpui_canvas::EdgeId::from(canvas_edge_id(&JellyEdgeId::from_u128(200)));
        let edge = document.edge(&edge_id).unwrap();
        assert_eq!(
            edge.source.handle_id.as_ref().unwrap().as_str(),
            canvas_port_id(&JellyPortId::from_u128(20))
        );
        assert_eq!(
            edge.target.handle_id.as_ref().unwrap().as_str(),
            canvas_port_id(&JellyPortId::from_u128(30))
        );
    }

    #[test]
    fn initial_viewport_centers_canvas_document_content() {
        let mut builder = CanvasDocument::builder();
        builder
            .add_node(CanvasNode::new(
                "node-a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .unwrap();
        let document = builder.build().unwrap();
        let viewport = initial_viewport_for_document(&document);
        let node = document.node(&NodeId::from("node-a")).unwrap();
        let view_bounds = viewport.document_bounds_to_view(node.bounds());

        assert!(
            view_bounds.left().as_f32() > INITIAL_VIEWPORT_PADDING,
            "initial viewport should leave horizontal breathing room, got {view_bounds:?}"
        );
        assert!(
            view_bounds.top().as_f32() > INITIAL_VIEWPORT_PADDING,
            "initial viewport should leave vertical breathing room, got {view_bounds:?}"
        );
    }

    #[test]
    fn product_gallery_initial_viewport_fits_default_canvas_area() {
        let (_, document, _) = project_product_gallery_case(
            product_gallery::ProductGalleryState::default().active_case(),
        )
        .unwrap();
        let viewport = initial_viewport_for_document(&document);
        let view_size = default_canvas_view_size();
        let content_bounds = document_content_bounds(&document).unwrap();
        let view_bounds = viewport.document_bounds_to_view(content_bounds);
        let tolerance = px(0.5);

        assert!(
            view_bounds.left() >= px(INITIAL_VIEWPORT_PADDING) - tolerance,
            "initial viewport should keep product graph away from the left edge: {view_bounds:?}"
        );
        assert!(
            view_bounds.right() <= view_size.width - px(INITIAL_VIEWPORT_PADDING) + tolerance,
            "initial viewport should keep product graph inside the right edge: {view_bounds:?} in {view_size:?}"
        );
        assert!(
            view_bounds.top() >= px(INITIAL_VIEWPORT_PADDING) - tolerance,
            "initial viewport should keep product graph away from the top edge: {view_bounds:?}"
        );
        assert!(
            view_bounds.bottom() <= view_size.height - px(INITIAL_VIEWPORT_PADDING) + tolerance,
            "initial viewport should keep product graph inside the bottom edge: {view_bounds:?} in {view_size:?}"
        );
    }

    #[test]
    fn viewport_resize_preserves_document_center_after_user_interaction() {
        let viewport = CanvasViewport::new(point(px(100.0), px(50.0)), 2.0).unwrap();
        let previous_size = size(px(400.0), px(200.0));
        let next_size = size(px(400.0), px(400.0));
        let previous_center =
            viewport.view_to_document(point(previous_size.width / 2.0, previous_size.height / 2.0));

        let resized = preserve_viewport_center_for_resize(viewport, previous_size, next_size);
        let next_center =
            resized.view_to_document(point(next_size.width / 2.0, next_size.height / 2.0));

        assert_eq!(next_center, previous_center);
    }

    #[test]
    fn canvas_viewport_size_update_consumes_auto_fit_once_then_preserves_center() {
        let (_, document, _) = project_product_gallery_case(
            product_gallery::ProductGalleryState::default().active_case(),
        )
        .unwrap();
        let initial_viewport = CanvasViewport::new(point(px(100.0), px(50.0)), 1.0).unwrap();
        let first_size = size(px(640.0), px(420.0));

        let first =
            canvas_viewport_size_update(&document, initial_viewport, true, None, first_size);

        assert!(!first.auto_fit_viewport);
        assert_eq!(first.last_canvas_view_size, Some(first_size));
        assert_ne!(first.viewport, initial_viewport);

        let first_center = first
            .viewport
            .view_to_document(point(first_size.width / 2.0, first_size.height / 2.0));
        let resized_size = size(px(640.0), px(520.0));

        let resized = canvas_viewport_size_update(
            &document,
            first.viewport,
            first.auto_fit_viewport,
            first.last_canvas_view_size,
            resized_size,
        );

        let resized_center = resized
            .viewport
            .view_to_document(point(resized_size.width / 2.0, resized_size.height / 2.0));
        assert_eq!(resized_center, first_center);
        assert!(!resized.auto_fit_viewport);
        assert_eq!(resized.last_canvas_view_size, Some(resized_size));
    }

    #[test]
    fn product_surface_drag_start_uses_actual_canvas_bounds_origin() {
        let bounds = Bounds::new(point(px(24.0), px(46.0)), size(px(640.0), px(420.0)));
        let event = MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(124.0), px(146.0)),
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
            ..MouseDownEvent::default()
        };

        assert_eq!(
            canvas_drag_start_event_from_bounds(bounds, &event),
            Some(CanvasEvent::PointerDown {
                position: point(px(100.0), px(100.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers {
                    shift: true,
                    ..CanvasKeyModifiers::default()
                },
            })
        );
    }

    #[test]
    fn product_toolbar_exposes_select_pan_and_connect_tools() {
        assert!(product_tool_switcher_visible());
        let options = canvas_tool_options();
        assert_eq!(
            options
                .iter()
                .filter(|option| matches!(option.tool, CanvasTool::Connect))
                .count(),
            1
        );
    }

    #[test]
    fn product_surface_drag_sequence_moves_shader_node_and_commits_outside_canvas_bounds() {
        assert!(
            product_surface_drag_sequence_probe(ProductSurfaceDragProbeEnd::CommitOutsideCanvas),
            "product node overlay drag should move the shader node and commit on pointer-up outside canvas bounds"
        );
    }

    #[test]
    fn product_surface_drag_sequence_cancel_restores_shader_node() {
        assert!(
            product_surface_drag_sequence_probe(ProductSurfaceDragProbeEnd::Cancel),
            "product node overlay drag should restore the shader node when the gesture is cancelled"
        );
    }

    #[derive(Clone, Copy, Debug)]
    enum ProductSurfaceDragProbeEnd {
        CommitOutsideCanvas,
        Cancel,
    }

    fn product_surface_drag_sequence_probe(end: ProductSurfaceDragProbeEnd) -> bool {
        let Ok((_store, document, _projection)) =
            project_kit_fixture("shader.blueprint", "shader.material_mix")
        else {
            return false;
        };
        let Ok(mut editor) = editor_for_document(document) else {
            return false;
        };
        let Some(shader_node_id) = editor
            .document()
            .nodes()
            .find(|node| node.kind == "shader-card")
            .map(|node| node.id.clone())
        else {
            return false;
        };
        let Some(initial_node) = editor.document().node(&shader_node_id).cloned() else {
            return false;
        };
        let canvas_bounds = Bounds::new(point(px(24.0), px(46.0)), default_canvas_view_size());
        let node_view_bounds = editor
            .viewport()
            .document_bounds_to_view(initial_node.bounds());
        let down_local = point(
            node_view_bounds.origin.x + node_view_bounds.size.width * 0.5,
            node_view_bounds.origin.y + px(24.0),
        );
        if !Bounds::new(point(px(0.0), px(0.0)), canvas_bounds.size).contains(&down_local) {
            return false;
        }
        let down_position = canvas_bounds.origin + down_local;
        let down_event = MouseDownEvent {
            button: MouseButton::Left,
            position: down_position,
            ..MouseDownEvent::default()
        };
        let Some(down) = canvas_drag_start_event_from_bounds(canvas_bounds, &down_event) else {
            return false;
        };
        if editor.handle_event(down).is_err() || editor.is_tool_state_idle() {
            return false;
        }

        let drag_delta = point(px(46.0), px(22.0));
        let move_event = MouseMoveEvent {
            position: down_position + drag_delta,
            ..MouseMoveEvent::default()
        };
        let mapper = CanvasEditorInputMapper::new(canvas_bounds)
            .with_pointer_interacting(!editor.is_tool_state_idle());
        let Some(move_event) = mapper.mouse_move(&move_event) else {
            return false;
        };
        if editor.handle_event(move_event).is_err() {
            return false;
        }
        let Some(moved_position) = editor
            .document()
            .node(&shader_node_id)
            .map(|node| node.position)
        else {
            return false;
        };
        if moved_position == initial_node.position {
            return false;
        }

        match end {
            ProductSurfaceDragProbeEnd::CommitOutsideCanvas => {
                let up_event = MouseUpEvent {
                    button: MouseButton::Left,
                    position: point(canvas_bounds.origin.x - px(32.0), canvas_bounds.origin.y),
                    ..MouseUpEvent::default()
                };
                let mapper = CanvasEditorInputMapper::new(canvas_bounds)
                    .with_pointer_interacting(!editor.is_tool_state_idle());
                let Some(CanvasEvent::PointerUp { position, .. }) = mapper.mouse_up(&up_event)
                else {
                    return false;
                };
                if position.x >= px(0.0) {
                    return false;
                }
                if editor
                    .handle_event(CanvasEvent::PointerUp {
                        position,
                        button: PointerButton::Primary,
                        modifiers: CanvasKeyModifiers::default(),
                    })
                    .is_err()
                    || !editor.is_tool_state_idle()
                {
                    return false;
                }
                editor
                    .document()
                    .node(&shader_node_id)
                    .is_some_and(|node| node.position == moved_position)
            }
            ProductSurfaceDragProbeEnd::Cancel => {
                if editor.handle_event(CanvasEvent::Cancel).is_err() || !editor.is_tool_state_idle()
                {
                    return false;
                }
                editor
                    .document()
                    .node(&shader_node_id)
                    .is_some_and(|node| node.position == initial_node.position)
            }
        }
    }

    #[test]
    fn canvas_node_transform_sync_survives_store_reprojection() {
        let (mut store, document, _) = project_product_gallery_case(
            product_gallery::ProductGalleryState::default().active_case(),
        )
        .unwrap();
        let mut editor = editor_for_document(document).unwrap();
        let node_id = editor.document().nodes().next().unwrap().id.clone();
        let before = editor.document().node(&node_id).unwrap().clone();
        let mut moved = before.clone();
        moved.position += point(px(47.0), px(23.0));
        moved.size.width += px(31.0);

        editor
            .apply_transaction(CanvasTransaction::single(DocumentCommand::UpdateNode(
                moved.clone(),
            )))
            .unwrap();
        let adapter = OpenGpuiAdapter::default();
        let transaction =
            canvas_document_transform_transaction(&adapter, &store, editor.document());
        assert!(
            !transaction.is_empty(),
            "moved canvas node should produce a Jellyflow graph transaction"
        );

        store
            .dispatch_transaction(&transaction)
            .expect("canvas transform sync should dispatch");
        let (projected, _) = project_store(&store).unwrap();
        let projected_node = projected.node(&node_id).unwrap();

        assert_eq!(projected_node.position, moved.position);
        assert_eq!(projected_node.size, moved.size);
        assert_ne!(projected_node.position, before.position);
    }

    #[test]
    fn gpui_node_internal_element_ids_are_scoped_by_node() {
        let first = JellyNodeId::from_u128(1);
        let second = JellyNodeId::from_u128(2);

        assert_ne!(
            open_gpui_control_element_id(first, "field.prompt", "control.prompt", 0),
            open_gpui_control_element_id(second, "field.prompt", "control.prompt", 0)
        );
        assert_ne!(
            open_gpui_action_button_element_id(Some(first), "synthetic.Node", "action.llm.run", 0),
            open_gpui_action_button_element_id(Some(second), "synthetic.Node", "action.llm.run", 0)
        );
        assert_ne!(
            open_gpui_action_menu_element_id(Some(first), "synthetic.Node", "toolbar"),
            open_gpui_action_menu_element_id(Some(second), "synthetic.Node", "toolbar")
        );
        assert_ne!(
            open_gpui_chrome_fallback_button_element_id(first, "demo.llm"),
            open_gpui_chrome_fallback_button_element_id(second, "demo.llm")
        );
        assert!(
            open_gpui_action_button_element_id(None, "synthetic.Graph", "action.graph", 0)
                .contains(":graph:")
        );
    }

    #[test]
    fn semantic_descriptor_extracts_builtin_node_surface_slots() {
        let store = make_demo_store();
        let (document, _) = project_store(&store).unwrap();
        let node_kit_registry = NodeKitRegistry::builtin();
        let semantic_registry = node_kit_registry.node_registry();
        let node = document
            .node(&NodeId::from(canvas_node_id(&JellyNodeId::from_u128(3))))
            .unwrap();
        let descriptor = semantic_registry
            .view_descriptor(&NodeKindKey::new("demo.llm"))
            .unwrap();
        let data = Value::Object(node.data.clone());
        let surface = descriptor.surface_slots_projection(
            &data,
            node_kit_registry.layout_hints_for_kind(&NodeKindKey::new("demo.llm")),
            1.0,
        );

        assert!(surface.iter().any(|slot| slot.label == "Prompt"));
        assert!(surface.iter().any(|slot| slot.label == "Policy"));
        assert!(surface.iter().any(|slot| slot.label == "Actions"));
    }

    #[test]
    fn semantic_chrome_projects_into_gpui_node_surface_summary() {
        let (store, editor, _) = demo_state();
        let node_kit_registry = NodeKitRegistry::builtin();
        let semantic_registry = node_kit_registry.node_registry();
        let model = CanvasPaintModel::from(&editor);
        let node = editor
            .document()
            .node(&NodeId::from(canvas_node_id(&JellyNodeId::from_u128(3))))
            .expect("llm node exists");
        let kind = data_string(node, "jellyflow_kind").expect("projected jellyflow kind");
        assert_eq!(kind, "demo.llm");
        assert!(
            semantic_registry
                .view_descriptor(&NodeKindKey::new(kind))
                .is_some(),
            "projected kind should resolve through Jellyflow node registry"
        );
        assert!(
            node_kit_registry
                .layout_hints_for_kind(&NodeKindKey::new(kind))
                .is_some(),
            "projected kind should resolve kit layout hints"
        );
        assert_eq!(
            jelly_node_id_from_node(node),
            Some(JellyNodeId::from_u128(3))
        );
        let surface = node_surface_summary_for_node(
            node,
            JellyNodeId::from_u128(3),
            store
                .graph()
                .nodes()
                .get(&JellyNodeId::from_u128(3))
                .expect("llm graph node exists"),
            store.graph(),
            model.viewport().zoom,
            true,
            &semantic_registry,
            &node_kit_registry,
            None,
        )
        .expect("llm surface summary");

        let kinds = surface
            .chrome
            .iter()
            .map(|chrome| chrome.kind)
            .collect::<Vec<_>>();
        assert!(kinds.contains(&NodeChromeKind::StatusStrip));
        assert!(kinds.contains(&NodeChromeKind::RunActionStrip));
        assert!(kinds.contains(&NodeChromeKind::Toolbar));
        assert!(kinds.contains(&NodeChromeKind::Resizer));
        assert!(
            surface
                .chrome
                .iter()
                .any(|chrome| chrome.key == "actions.run" && chrome.interactive)
        );
        assert!(surface.action_menus.iter().any(|menu| {
            menu.key == "menu.node.llm"
                && menu
                    .actions
                    .iter()
                    .any(|action| action.key == "action.llm.run")
        }));
        assert!(
            surface
                .toolbar_menu
                .actions
                .iter()
                .any(|action| action.key == "action.llm.run")
        );
        assert_eq!(
            plan_action_dispatch(&surface.toolbar_menu, "action.llm.run")
                .expect("toolbar action dispatch")
                .target,
            jellyflow::runtime::schema::ActionTarget::Node {
                node_kind: "demo.llm".to_owned(),
            }
        );

        let inspectors = project_inspectors_for_surface(
            &semantic_registry
                .view_descriptor(&NodeKindKey::new("demo.llm"))
                .expect("llm descriptor"),
            &Value::Object(node.data.clone()),
            &OpenGpuiInspectorSurface::Node {
                node_kind: "demo.llm".to_owned(),
            },
        );
        assert!(inspectors.iter().any(|inspector| {
            inspector.key == "inspector.llm"
                && inspector
                    .controls
                    .iter()
                    .any(|control| control.key == "inspector.model" && control.is_editable())
        }));
    }

    #[test]
    fn gpui_blackboard_panel_projects_actions_for_local_dispatch() {
        let (mut store, _document, _projection, node_id) =
            project_schema_node("demo.shader.mix").expect("shader schema node projects");
        let semantic_registry = NodeKitRegistry::builtin().node_registry();
        let shader = store
            .graph()
            .nodes()
            .get(&node_id)
            .expect("shader graph node exists");
        let descriptor = semantic_registry
            .view_descriptor(&shader.kind)
            .expect("shader descriptor");

        let blackboards = project_blackboards_for_descriptor(&descriptor, &shader.data);

        let blackboard = blackboards
            .iter()
            .find(|blackboard| blackboard.key == "blackboard.shader.properties")
            .expect("shader properties blackboard");
        assert_eq!(blackboard.item_count, 2);
        assert!(blackboard.items.iter().any(|item| {
            item.item_id == "base_color" && item.label == "Base Color" && item.controls == 1
        }));

        let outcome = OpenGpuiAuthoringController
            .plan_menu_action_dispatch(&blackboard.action_menu, "action.shader_property.add");
        let dispatch = outcome
            .into_plan()
            .expect("blackboard add action dispatch plan");
        assert_eq!(
            dispatch.target,
            jellyflow::runtime::schema::ActionTarget::Blackboard {
                blackboard_key: "blackboard.shader.properties".to_owned(),
            }
        );
        let repeatable = OpenGpuiAuthoringController
            .plan_repeatable_action_dispatch(
                &store,
                &semantic_registry,
                Some(node_id),
                &dispatch,
                |context| {
                    Some(demo_repeatable_add_item(
                        &context.collection_key,
                        context.item_count,
                    ))
                },
            )
            .expect("blackboard add action should map")
            .into_plan()
            .expect("blackboard add action should map to repeatable add");
        OpenGpuiAuthoringController
            .apply_repeatable_action_to_store(&mut store, &semantic_registry, node_id, repeatable)
            .expect("blackboard add repeatable mutation")
            .expect("blackboard add edit plan");
        let updated_shader = store
            .graph()
            .nodes()
            .get(&node_id)
            .expect("updated shader graph node");
        let updated_blackboards =
            project_blackboards_for_descriptor(&descriptor, &updated_shader.data);
        let updated = updated_blackboards
            .iter()
            .find(|blackboard| blackboard.key == "blackboard.shader.properties")
            .expect("updated shader properties blackboard");
        assert_eq!(updated.item_count, 3);
    }

    #[test]
    fn gpui_custom_renderer_registry_routes_known_and_fallback_surfaces() {
        let (store, editor, _) = demo_state();
        let node_kit_registry = NodeKitRegistry::builtin();
        let semantic_registry = node_kit_registry.node_registry();
        let model = CanvasPaintModel::from(&editor);
        let registry = demo_node_renderer_registry();

        let llm_canvas_node = editor
            .document()
            .node(&NodeId::from(canvas_node_id(&JellyNodeId::from_u128(3))))
            .expect("llm node exists");
        let llm_record = store
            .graph()
            .nodes()
            .get(&JellyNodeId::from_u128(3))
            .expect("llm graph node exists");
        let llm_surface = node_surface_summary_for_node(
            llm_canvas_node,
            JellyNodeId::from_u128(3),
            llm_record,
            store.graph(),
            model.viewport().zoom,
            true,
            &semantic_registry,
            &node_kit_registry,
            None,
        )
        .expect("llm surface summary");

        assert!(matches!(
            registry.resolve(&llm_surface.renderer_context),
            OpenGpuiNodeRendererResolution::Custom(_)
        ));
        assert!(
            demo_custom_node_renderers().contains_key(&llm_surface.renderer_context.renderer_key)
        );
        assert_eq!(llm_surface.renderer_context.renderer_key, "decision-card");
        assert!(
            llm_surface
                .renderer_context
                .control("control.prompt")
                .is_some()
        );
        assert!(
            llm_surface
                .renderer_context
                .plan_control_event(
                    "control.prompt",
                    OpenGpuiControlEventValue::Text("Route with a custom renderer".to_owned()),
                )
                .expect("custom renderer control helper should plan")
                .is_planned()
        );
        assert!(
            llm_surface
                .renderer_context
                .plan_menu_action_dispatch("synthetic.Toolbar", "action.llm.run")
                .expect("toolbar menu exists")
                .is_planned()
        );
        assert!(
            llm_surface
                .renderer_context
                .slot_measurement_id("field.prompt")
                .element_id()
                .contains(":slot:field.prompt")
        );
        assert!(
            llm_surface
                .renderer_context
                .control_measurement_id("field.prompt", "control.prompt")
                .element_id()
                .contains(":control:field.prompt:control.prompt")
        );
        assert!(
            llm_surface
                .renderer_context
                .anchor_measurement_id("field.completion")
                .element_id()
                .contains(":anchor:field.completion")
        );

        let source_canvas_node = editor
            .document()
            .node(&NodeId::from(canvas_node_id(&JellyNodeId::from_u128(2))))
            .expect("source node exists");
        let source_record = store
            .graph()
            .nodes()
            .get(&JellyNodeId::from_u128(2))
            .expect("source graph node exists");
        let source_surface = node_surface_summary_for_node(
            source_canvas_node,
            JellyNodeId::from_u128(2),
            source_record,
            store.graph(),
            model.viewport().zoom,
            false,
            &semantic_registry,
            &node_kit_registry,
            None,
        )
        .expect("source surface summary");

        assert_eq!(source_surface.renderer_context.renderer_key, "source-card");
        assert!(matches!(
            registry.resolve(&source_surface.renderer_context),
            OpenGpuiNodeRendererResolution::Custom(_)
        ));
        assert!(
            demo_custom_node_renderers()
                .contains_key(&source_surface.renderer_context.renderer_key)
        );
    }

    #[test]
    fn live_control_authoring_plans_update_dify_node_data_with_typed_values() {
        let store = make_demo_store();
        let node_id = JellyNodeId::from_u128(3);
        let node = store.graph().nodes().get(&node_id).expect("llm node");
        let registry = NodeKitRegistry::builtin().node_registry();
        let descriptor = registry
            .view_descriptor(&NodeKindKey::new("demo.llm"))
            .expect("llm descriptor");
        let node_data = node.data.clone();
        let prompt_slot = descriptor
            .surface_slot("field.prompt")
            .expect("prompt slot");
        let model_slot = descriptor.surface_slot("badge.model").expect("model slot");
        let config_slot = descriptor
            .surface_slot("config.model")
            .expect("config slot");
        let prompt = project_slot_controls(&node_data, prompt_slot)
            .into_iter()
            .find(|control| control.key == "control.prompt")
            .expect("prompt control");
        let model = project_slot_controls(&node_data, model_slot)
            .into_iter()
            .find(|control| control.key == "control.model")
            .expect("model control");
        let config_controls = project_slot_controls(&node_data, config_slot);
        let temperature = config_controls
            .iter()
            .find(|control| control.key == "control.temperature")
            .expect("temperature control");
        let stream = config_controls
            .iter()
            .find(|control| control.key == "control.stream")
            .expect("stream control");
        let controller = OpenGpuiAuthoringController;

        let prompt_plan = controller
            .plan_control_text_edit(
                node_id,
                &authoring_node_from_control_data(node_data.clone()),
                &prompt,
                "Write a normalized JSON row",
            )
            .expect("prompt edit")
            .into_plan()
            .expect("prompt edit plan");
        assert_node_data_path_value(
            &prompt_plan,
            ["fields", "prompt"],
            serde_json::json!("Write a normalized JSON row"),
        );

        let option = model
            .options
            .iter()
            .find(|option| option.label == "GPT 4.1")
            .expect("model option");
        let select_plan = controller
            .plan_control_select_edit(
                node_id,
                &authoring_node_from_control_data(node_data),
                &model,
                control_option_key(option),
            )
            .expect("select edit")
            .into_plan()
            .expect("select edit plan");
        assert_node_data_path_value(
            &select_plan,
            ["meta", "model"],
            serde_json::json!("gpt-4.1"),
        );
        assert_eq!(
            select_plan.invalidation.reason,
            NodeInternalsInvalidationReason::DataChanged
        );

        let number_plan = controller
            .plan_control_number_edit(
                node_id,
                &authoring_node_from_control_data(node.data.clone()),
                temperature,
                1.5,
            )
            .expect("temperature edit")
            .into_plan()
            .expect("temperature edit plan");
        assert_node_data_path_value(
            &number_plan,
            ["config", "model", "temperature"],
            serde_json::json!(1.5),
        );

        let switch_plan = controller
            .plan_control_bool_edit(
                node_id,
                &authoring_node_from_control_data(node.data.clone()),
                stream,
                true,
            )
            .expect("stream edit")
            .into_plan()
            .expect("stream edit plan");
        assert_node_data_path_value(
            &switch_plan,
            ["config", "model", "stream"],
            serde_json::json!(true),
        );
    }

    #[test]
    fn live_control_authoring_reads_current_store_before_planning_each_edit() {
        let mut store = make_demo_store();
        let node_id = JellyNodeId::from_u128(3);
        let registry = NodeKitRegistry::builtin().node_registry();
        let descriptor = registry
            .view_descriptor(&NodeKindKey::new("demo.llm"))
            .expect("llm descriptor");
        let initial_node = store.graph().nodes().get(&node_id).expect("llm node");
        let prompt_slot = descriptor
            .surface_slot("field.prompt")
            .expect("prompt slot");
        let model_slot = descriptor.surface_slot("badge.model").expect("model slot");
        let prompt = project_slot_controls(&initial_node.data, prompt_slot)
            .into_iter()
            .find(|control| control.key == "control.prompt")
            .expect("prompt control");
        let model = project_slot_controls(&initial_node.data, model_slot)
            .into_iter()
            .find(|control| control.key == "control.model")
            .expect("model control");

        let prompt_plan = OpenGpuiAuthoringController
            .plan_store_control_event(
                &store,
                &registry,
                node_id,
                &prompt,
                OpenGpuiControlEventValue::Text("Keep this prompt".to_owned()),
            )
            .expect("prompt edit")
            .into_plan()
            .expect("prompt plan");
        store
            .dispatch_transaction(&prompt_plan.transaction)
            .expect("prompt transaction");

        let option = model
            .options
            .iter()
            .find(|option| option.label == "GPT 4.1")
            .expect("model option");
        let model_plan = OpenGpuiAuthoringController
            .plan_store_control_event(
                &store,
                &registry,
                node_id,
                &model,
                OpenGpuiControlEventValue::SelectOptionKey(control_option_key(option)),
            )
            .expect("model edit")
            .into_plan()
            .expect("model plan");
        store
            .dispatch_transaction(&model_plan.transaction)
            .expect("model transaction");

        let data = &store.graph().nodes().get(&node_id).expect("llm node").data;
        assert_eq!(
            data.pointer("/fields/prompt"),
            Some(&serde_json::json!("Keep this prompt"))
        );
        assert_eq!(
            data.pointer("/meta/model"),
            Some(&serde_json::json!("gpt-4.1"))
        );
    }

    #[test]
    fn unavailable_controls_render_with_disabled_or_readonly_interaction_state() {
        let store = make_demo_store();
        let node = store
            .graph()
            .nodes()
            .get(&JellyNodeId::from_u128(3))
            .expect("llm node");
        let registry = NodeKitRegistry::builtin().node_registry();
        let descriptor = registry
            .view_descriptor(&NodeKindKey::new("demo.llm"))
            .expect("llm descriptor");
        let prompt_slot = descriptor
            .surface_slot("field.prompt")
            .expect("prompt slot");
        let model_slot = descriptor.surface_slot("badge.model").expect("model slot");
        let prompt_controls = project_slot_controls(&node.data, prompt_slot);
        let prompt = prompt_controls
            .iter()
            .find(|control| control.key == "control.prompt")
            .expect("prompt control");
        let stub = prompt_controls
            .iter()
            .find(|control| control.is_partial_stub())
            .expect("stub control");
        let model = project_slot_controls(&node.data, model_slot)
            .into_iter()
            .find(|control| control.key == "control.model")
            .expect("model control");
        let mut read_only_model = model.clone();
        read_only_model.read_only = true;

        assert!(!control_component_disabled(prompt));
        assert!(!control_component_read_only(prompt));
        assert!(!control_component_interaction_disabled(prompt));

        assert!(control_component_disabled(stub));
        assert!(control_component_read_only(stub));
        assert!(control_component_interaction_disabled(stub));

        assert!(!control_component_disabled(&read_only_model));
        assert!(control_component_read_only(&read_only_model));
        assert!(control_component_interaction_disabled(&read_only_model));
    }

    #[test]
    fn shader_fixture_projects_typed_ports_into_gpui_surface_summary() {
        let (store, document, projection) =
            project_kit_fixture("shader.blueprint", "shader.material_mix")
                .expect("shader fixture projects");
        let node_kit_registry = NodeKitRegistry::builtin();
        let semantic_registry = node_kit_registry.node_registry();

        assert_eq!(projection.graph_nodes, 2);
        assert_eq!(projection.graph_edges, 1);
        assert_eq!(projection.canvas_nodes, 2);
        assert_eq!(projection.canvas_edges, 1);

        let shader_node = document
            .nodes()
            .find(|node| node.kind == "shader-card")
            .expect("shader-card canvas node exists");
        let shader_node_id = jelly_node_id_from_node(shader_node).expect("shader jelly node id");
        let shader_record = store
            .graph()
            .nodes()
            .get(&shader_node_id)
            .expect("shader graph node exists");
        let surface = node_surface_summary_for_node(
            shader_node,
            shader_node_id,
            shader_record,
            store.graph(),
            1.0,
            false,
            &semantic_registry,
            &node_kit_registry,
            None,
        )
        .expect("shader surface summary");

        assert_eq!(surface.renderer_key, "shader-card");
        assert!(
            surface
                .slots
                .iter()
                .any(|slot| slot.kind == NodeSurfaceSlotKind::PortRail)
        );
        assert!(
            surface
                .slots
                .iter()
                .any(|slot| slot.kind == NodeSurfaceSlotKind::Preview)
        );
        assert!(
            surface
                .repeatables
                .iter()
                .any(|repeatable| repeatable.key == "shader.inputs")
        );
        assert!(
            shader_node
                .handles
                .iter()
                .any(|handle| handle.role == HandleRole::Source)
        );
    }

    #[test]
    fn shader_default_node_projects_dynamic_repeatable_items_into_surface_summary() {
        let (store, document, projection, node_id) =
            project_schema_node("demo.shader.mix").expect("shader schema node projects");
        let node_kit_registry = NodeKitRegistry::builtin();
        let semantic_registry = node_kit_registry.node_registry();
        let shader_node = document
            .node(&NodeId::from(canvas_node_id(&node_id)))
            .expect("shader-card canvas node exists");
        let shader_record = store
            .graph()
            .nodes()
            .get(&node_id)
            .expect("shader graph node exists");
        let surface = node_surface_summary_for_node(
            shader_node,
            node_id,
            shader_record,
            store.graph(),
            1.0,
            false,
            &semantic_registry,
            &node_kit_registry,
            None,
        )
        .expect("shader surface summary");

        assert_eq!(projection.canvas_nodes, 1);
        let factor = surface
            .repeatable_items
            .iter()
            .find(|item| item.collection_key == "shader.inputs" && item.item_id == "factor")
            .expect("factor repeatable item projects");
        let factor_port = factor
            .port_id
            .expect("factor repeatable item binds a graph port");
        assert!(
            shader_node
                .handles
                .iter()
                .any(|handle| handle.id.as_str() == canvas_port_id(&factor_port))
        );
    }

    #[test]
    fn product_gallery_cases_build_canvas_editors_and_switch_fixture_state() {
        let (mut gallery, _, _, _) = product_gallery_state();
        assert_eq!(gallery.active_id(), "workflow.review");

        let cases = gallery.cases().to_vec();
        for case in cases {
            let (store, document, projection) =
                project_product_gallery_case(&case).expect("product fixture projects");
            let editor = editor_for_document(document).expect("product fixture editor");

            assert_eq!(projection.graph_nodes, store.graph().nodes().len());
            assert_eq!(projection.graph_edges, store.graph().edges().len());
            assert!(editor.document().nodes().next().is_some());
        }

        gallery.set_active("shader.material_mix");
        assert_eq!(gallery.active_case().fixture_key(), "shader.material_mix");
    }

    #[test]
    fn product_gallery_fixtures_project_non_overlapping_node_bounds() {
        for case in product_gallery::product_gallery_cases() {
            let (_, document, _) =
                project_product_gallery_case(&case).expect("product fixture projects");

            assert_canvas_nodes_do_not_overlap(case.id(), &document);
        }
    }

    #[test]
    fn projected_handles_follow_semantic_slot_anchors_after_node_resize() {
        let mut store = make_demo_store();
        let transform = JellyNodeId::from_u128(3);
        let before_size = store.graph().nodes().get(&transform).unwrap().size;
        store
            .dispatch_transaction(&GraphTransaction::from_ops([GraphOp::SetNodeSize {
                id: transform,
                from: before_size,
                to: Some(JellySize {
                    width: 328.0,
                    height: 268.0,
                }),
            }]))
            .expect("resize transform node");

        let (document, _) = project_store(&store).unwrap();
        let node = document
            .node(&NodeId::from(canvas_node_id(&JellyNodeId::from_u128(3))))
            .unwrap();

        let prompt = node
            .handles
            .iter()
            .find(|handle| handle.id.as_str() == canvas_port_id(&JellyPortId::from_u128(30)))
            .unwrap();
        let completion = node
            .handles
            .iter()
            .find(|handle| handle.id.as_str() == canvas_port_id(&JellyPortId::from_u128(31)))
            .unwrap();
        let semantic_registry = NodeKitRegistry::builtin().node_registry();
        let jelly_node = store.graph().nodes().get(&transform).unwrap();
        let descriptor = semantic_registry.view_descriptor(&jelly_node.kind).unwrap();
        let layout = projected_node_surface_component_layout(
            &descriptor,
            jelly_node,
            jelly_node.size.unwrap(),
        );
        let prompt_anchor = layout
            .anchor_rect("field.prompt")
            .expect("prompt component anchor");
        let completion_anchor = layout
            .anchor_rect("field.completion")
            .expect("completion component anchor");

        assert_eq!(node.size.width, px(328.0));
        assert_eq!(node.size.height, px(268.0));
        assert_eq!(prompt.position.x, px(0.0));
        assert_eq!(completion.position.x, px(328.0));
        assert_eq!(
            prompt.position.y,
            px(prompt_anchor.origin.y + prompt_anchor.size.height * 0.5)
        );
        assert_eq!(
            completion.position.y,
            px(completion_anchor.origin.y + completion_anchor.size.height * 0.5)
        );
        assert_eq!(
            completion.position.y.as_f32() - prompt.position.y.as_f32(),
            (completion_anchor.origin.y + completion_anchor.size.height * 0.5)
                - (prompt_anchor.origin.y + prompt_anchor.size.height * 0.5)
        );
    }

    #[test]
    fn gpui_measurements_are_derived_from_component_layout_slots() {
        let store = make_demo_store();
        let semantic_registry = NodeKitRegistry::builtin().node_registry();
        let transform = JellyNodeId::from_u128(3);
        let node = store.graph().nodes().get(&transform).unwrap();
        let descriptor = semantic_registry.view_descriptor(&node.kind).unwrap();
        let node_size = node.size.unwrap();
        let layout = projected_node_surface_component_layout(&descriptor, node, node_size);
        let measurement = project_node_measurement(&transform, node, store.graph(), &descriptor);

        let prompt_layout_rect = layout
            .slot_rect("field.prompt")
            .expect("prompt slot layout rect");
        let completion_layout_anchor = layout
            .anchor_rect("field.completion")
            .expect("completion anchor layout rect");
        let prompt_measurement = measurement
            .slots
            .iter()
            .find(|slot| slot.key == "field.prompt")
            .expect("prompt measured slot");
        let completion_anchor = measurement
            .anchors
            .iter()
            .find(|anchor| anchor.anchor == "field.completion")
            .expect("completion measured anchor");

        assert_eq!(prompt_measurement.rect, prompt_layout_rect);
        assert_eq!(completion_anchor.rect, completion_layout_anchor);
        assert_eq!(completion_anchor.port_key, Some(PortKey::new("completion")));
        assert_eq!(
            layout.measurement_mode,
            NodeSurfaceMeasurementSource::ProjectionFallback
        );
    }

    #[test]
    fn gpui_surface_consumes_controls_repeatables_and_actions_as_local_projection() {
        let (store, document, projection) =
            project_kit_fixture("erd.table", "erd.customer_orders").expect("erd projects");
        let node_kit_registry = NodeKitRegistry::builtin();
        let semantic_registry = node_kit_registry.node_registry();
        let table_node = document
            .nodes()
            .find(|node| node.kind == "table-card")
            .expect("table-card canvas node exists");
        let table_node_id = jelly_node_id_from_node(table_node).expect("table jelly node id");
        let table_record = store
            .graph()
            .nodes()
            .get(&table_node_id)
            .expect("table graph node exists");
        let surface = node_surface_summary_for_node(
            table_node,
            table_node_id,
            table_record,
            store.graph(),
            1.0,
            true,
            &semantic_registry,
            &node_kit_registry,
            None,
        )
        .expect("table surface summary");

        assert!(surface.slot_descriptors.iter().any(|slot| {
            slot.controls
                .iter()
                .any(|control| control.kind == NodeControlKind::TextInput)
        }));
        assert!(
            surface
                .repeatables
                .iter()
                .any(|repeatable| repeatable.key == "table.columns"
                    && repeatable.item_count >= 1
                    && repeatable.controls >= 2)
        );
        assert!(
            surface
                .repeatable_items
                .iter()
                .any(|item| item.collection_key == "table.columns"
                    && item.item_id == "qty"
                    && item.port_key == Some(PortKey::new("field_qty"))
                    && item.dynamic_port_policy == OpenGpuiDynamicPortPolicy::MissingGraphPort)
        );
        assert!(surface.actions >= 3);
        assert!(surface.menus >= 1);
        assert!(surface.action_menus.iter().any(|menu| {
            menu.actions
                .iter()
                .any(|action| action.key == "action.column.add")
        }));
        assert_eq!(
            projection.capability.layout_measurement,
            NodeSurfaceMeasurementSource::LayoutPass
        );
        assert_eq!(projection.capability.controls, "live/partial");
        assert!(
            projection
                .capability
                .layout_status
                .contains("measured_element bounds")
        );
        assert!(
            !projection
                .capability
                .layout_status
                .contains(&["element-bounds", "callback"].join(" "))
        );
    }

    #[test]
    fn gpui_repeatable_actions_commit_store_mutations_and_projection_updates() {
        let (mut store, _document, _projection, node_id) =
            project_schema_node("demo.shader.mix").expect("shader schema node projects");
        let registry = NodeKitRegistry::builtin().node_registry();

        let add = OpenGpuiAuthoringController
            .apply_repeatable_action_to_store(
                &mut store,
                &registry,
                node_id,
                OpenGpuiRepeatableActionPlan::Add {
                    collection_key: "shader.inputs".to_owned(),
                    item: serde_json::json!({
                        "name": "Input 4",
                        "ty": "vec4",
                        "port": "input_4"
                    }),
                },
            )
            .expect("add repeatable")
            .expect("changed add");
        assert!(add.diagnostics.iter().any(|diagnostic| {
            diagnostic.collection_key == "shader.inputs"
                && diagnostic.item_id == "input_4"
                && diagnostic.port_key == PortKey::new("input_4")
                && diagnostic.policy == OpenGpuiDynamicPortPolicy::MissingGraphPort
        }));
        let node = store.graph().nodes().get(&node_id).expect("node after add");
        assert_eq!(
            node.data["dynamic_inputs"][3]["id"],
            serde_json::json!("input_4")
        );

        OpenGpuiAuthoringController
            .apply_repeatable_action_to_store(
                &mut store,
                &registry,
                node_id,
                OpenGpuiRepeatableActionPlan::Reorder {
                    collection_key: "shader.inputs".to_owned(),
                    item_id: "factor".to_owned(),
                    to_index: 0,
                },
            )
            .expect("reorder repeatable")
            .expect("changed reorder");
        let node = store
            .graph()
            .nodes()
            .get(&node_id)
            .expect("node after reorder");
        let ids = node.data["dynamic_inputs"]
            .as_array()
            .expect("dynamic inputs array")
            .iter()
            .map(|item| item["id"].as_str().expect("id"))
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["factor", "a", "b", "input_4"]);

        let factor_port = graph_port_id_for_key(store.graph(), node_id, "factor");
        let result_port = graph_port_id_for_key(store.graph(), node_id, "result");
        let edge_id = JellyEdgeId::from_u128(0xbeef);
        store
            .dispatch_transaction(&GraphTransaction::from_ops([GraphOp::AddEdge {
                id: edge_id,
                edge: make_edge(result_port, factor_port),
            }]))
            .expect("seed incident edge");

        OpenGpuiAuthoringController
            .apply_repeatable_action_to_store(
                &mut store,
                &registry,
                node_id,
                OpenGpuiRepeatableActionPlan::Remove {
                    collection_key: "shader.inputs".to_owned(),
                    item_id: "factor".to_owned(),
                },
            )
            .expect("remove repeatable")
            .expect("changed remove");
        let node = store
            .graph()
            .nodes()
            .get(&node_id)
            .expect("node after remove");
        assert!(
            node.data["dynamic_inputs"]
                .as_array()
                .expect("dynamic inputs array")
                .iter()
                .all(|item| item["id"] != "factor")
        );
        assert!(!node.ports.contains(&factor_port));
        assert!(!store.graph().ports().contains_key(&factor_port));
        assert!(!store.graph().edges().contains_key(&edge_id));

        let descriptor = registry.view_descriptor(&node.kind).expect("descriptor");
        let repeatable_items =
            repeatable_item_projection(&descriptor, node, store.graph(), &node_id);
        assert!(repeatable_items.iter().all(|item| item.item_id != "factor"));
        assert!(repeatable_items.iter().any(|item| {
            item.item_id == "input_4"
                && item.dynamic_port_policy == OpenGpuiDynamicPortPolicy::MissingGraphPort
        }));
    }

    #[test]
    fn gpui_dropped_wire_insert_menu_dispatches_semantic_insert_plan() {
        let registry = NodeKitRegistry::builtin().node_registry();
        let source = ConnectionHandleRef::new(
            JellyNodeId::from_u128(3),
            JellyPortId::from_u128(31),
            PortDirection::Out,
        );
        let pointer = JellyPoint { x: 420.0, y: 180.0 };
        let menu = project_dropped_wire_menu(
            &registry,
            source,
            Some(&PortKey::new("completion")),
            pointer,
        );

        assert_eq!(menu.surface, MenuSurface::DroppedWire);
        assert!(
            menu.actions
                .iter()
                .any(|action| action.key == "action.insert.llm" && action.dispatchable())
        );
        let insert = plan_dropped_wire_insert(&menu, "action.insert.llm", source, pointer)
            .expect("enabled dropped-wire action should dispatch");

        assert_eq!(insert.node_kind, "demo.llm");
        assert_eq!(insert.source, source);
        assert_eq!(insert.pointer, pointer);
    }

    #[test]
    fn gpui_dropped_wire_insert_action_commits_graph_mutation() {
        let mut store = make_demo_store();
        let registry = NodeKitRegistry::builtin().node_registry();
        let source_key = PortKey::new("completion");
        let source = dropped_wire_source_for_port_key(store.graph(), &source_key)
            .expect("demo graph should expose a completion output");
        let pointer = dropped_wire_insert_pointer(store.graph(), source);
        let menu = project_dropped_wire_menu(&registry, source, Some(&source_key), pointer);
        let (resolved_source, resolved_pointer) =
            dropped_wire_source_for_menu(store.graph(), &menu)
                .expect("dropped-wire menu should resolve its source handle");
        let before_nodes = store.graph().nodes().len();
        let before_edges = store.graph().edges().len();

        let outcome = apply_demo_dropped_wire_insert(
            &mut store,
            &registry,
            &menu,
            "action.insert.llm",
            resolved_source,
            resolved_pointer,
        )
        .expect("enabled insert action should mutate graph");

        assert_eq!(resolved_source, source);
        assert_eq!(resolved_pointer, pointer);
        assert_eq!(store.graph().nodes().len(), before_nodes + 1);
        assert_eq!(store.graph().edges().len(), before_edges + 1);
        assert!(store.graph().nodes().contains_key(&outcome.plan.node_id));
        assert!(
            store
                .graph()
                .ports()
                .contains_key(&outcome.plan.target_port)
        );
        assert!(
            store
                .node_measurement_status(outcome.plan.node_id)
                .is_dirty()
        );
    }

    #[test]
    fn canvas_example_consumes_adapter_product_fixture_gates() {
        assert_product_fixture_regression_gates();
        assert_authoring_interaction_regression_gates();
    }

    #[test]
    fn canvas_example_collects_host_product_surface_report() {
        let report = canvas_host_surface_report();
        assert_host_surface_report_contract(&report);
        assert_product_gallery_host_report_gates(&report);
        let visual_report = visual_regression::canvas_host_visual_interaction_report();
        assert_host_visual_interaction_report_gates(&visual_report);
        assert!(report.rows.iter().any(|row| {
            row.fixture_id == "workflow.review"
                && row.renderer_key == "decision-card"
                && row.source == OpenGpuiHostRendererSource::ProductRenderer
        }));
        assert!(report.rows.iter().any(|row| {
            row.fixture_id == "shader.material_mix"
                && row.renderer_key == "shader-card"
                && row.source == OpenGpuiHostRendererSource::ProductRenderer
        }));
        assert!(report.rows.iter().any(|row| {
            row.fixture_id == "erd.customer_orders"
                && row.renderer_key == "table-card"
                && row.source == OpenGpuiHostRendererSource::ProductRenderer
        }));
        assert!(report.rows.iter().any(|row| {
            row.fixture_id == "mind-map.strategy"
                && matches!(row.renderer_key.as_str(), "topic-card" | "source-card")
                && row.source == OpenGpuiHostRendererSource::ProductRenderer
        }));
        assert!(report.rows.iter().any(|row| {
            row.capability_gaps
                .contains(&OpenGpuiHostCapabilityGap::AdvancedControlStub)
        }));
        assert!(
            report
                .rows
                .iter()
                .filter(|row| row.source == OpenGpuiHostRendererSource::ProductRenderer)
                .all(|row| row
                    .style_budget
                    .is_some_and(|style| style.handle_hit_width >= 22
                        && style.edge_hit_width >= style.edge_stroke_width)),
            "{report:?}"
        );
    }

    #[test]
    fn canvas_example_characterizes_current_product_interaction_gaps() {
        let report = canvas_host_product_interaction_report();
        assert_product_interaction_characterization_report_contract(&report);
        assert!(report.product_drag_surface_count >= 4, "{report:?}");
        assert!(report.hidden_repeatable_overflow_count > 0, "{report:?}");
        assert!(report.repeatable_overflow_indicator_count > 0, "{report:?}");
        assert!(
            !report.gaps.contains(
                &OpenGpuiHostProductInteractionGap::DragSurfaceMissingFullPointerSequence
            ),
            "{report:?}"
        );
        assert!(
            !report
                .gaps
                .contains(&OpenGpuiHostProductInteractionGap::ToolSwitcherMissing),
            "{report:?}"
        );
        assert!(
            !report
                .gaps
                .contains(&OpenGpuiHostProductInteractionGap::ConnectFlowNotStoreSynced),
            "{report:?}"
        );
        assert!(
            !report
                .gaps
                .contains(&OpenGpuiHostProductInteractionGap::ReconnectAffordanceMissing),
            "{report:?}"
        );
        assert!(
            !report
                .gaps
                .contains(&OpenGpuiHostProductInteractionGap::PortHotspotPathMissing),
            "{report:?}"
        );
        assert!(
            !report
                .gaps
                .contains(&OpenGpuiHostProductInteractionGap::DroppedWireGestureDetached),
            "{report:?}"
        );
        assert!(
            !report
                .gaps
                .contains(&OpenGpuiHostProductInteractionGap::RepeatableOverflowIndicatorMissing),
            "{report:?}"
        );
    }

    #[test]
    fn canvas_edge_remove_plans_jellyflow_store_delete_transaction() {
        let (mut store, document, _) =
            project_kit_fixture("shader.blueprint", "shader.material_mix")
                .expect("shader fixture projects");
        let mut editor = editor_for_document(document).expect("shader fixture editor");
        let edge_id = editor
            .document()
            .edges()
            .next()
            .expect("fixture edge")
            .id
            .clone();
        let before_edges = store.graph().edges().len();

        editor
            .apply_transaction(CanvasTransaction::single(DocumentCommand::RemoveEdge(
                edge_id,
            )))
            .expect("remove canvas edge");

        let adapter = OpenGpuiAdapter::default();
        let transactions =
            canvas_document_connection_sync_transactions(&adapter, &store, editor.document())
                .expect("edge remove should plan connection sync");
        assert!(
            transactions
                .iter()
                .flat_map(GraphTransaction::ops)
                .any(|op| matches!(op, GraphOp::RemoveEdge { .. })),
            "{transactions:?}"
        );
        for transaction in transactions {
            store
                .dispatch_transaction(&transaction)
                .expect("edge remove sync should dispatch");
        }
        assert_eq!(store.graph().edges().len(), before_edges - 1);
    }

    #[test]
    fn canvas_edge_insert_plans_jellyflow_store_connect_transaction() {
        assert!(
            product_connection_store_sync_probe(),
            "canvas edge insert should dispatch through Jellyflow connection rules"
        );
    }

    #[test]
    fn product_port_hotspot_path_resolves_measured_handle_endpoint() {
        assert!(
            product_port_hotspot_path_probe(),
            "measured product handle should be hit-testable as a connection endpoint"
        );
    }

    #[test]
    fn canvas_edge_update_plans_jellyflow_reconnect_transaction() {
        assert!(
            product_reconnect_store_sync_probe(),
            "canvas edge endpoint update should dispatch through Jellyflow reconnect rules"
        );
    }

    #[test]
    fn selected_product_edge_exposes_reconnect_affordances() {
        assert!(
            product_reconnect_affordance_probe(),
            "selected product edge should expose reconnect affordance handles"
        );
    }

    #[test]
    fn dropped_wire_gesture_commits_insert_from_connect_release() {
        assert!(
            product_dropped_wire_gesture_probe(),
            "connect release away from a port should dispatch dropped-wire insert"
        );
    }

    fn product_connection_store_sync_probe() -> bool {
        let mut store = make_demo_store();
        let Ok((document, _projection)) = project_store(&store) else {
            return false;
        };
        let Ok(mut editor) = editor_for_document(document) else {
            return false;
        };
        let source_port = JellyPortId::from_u128(20);
        let target_port = JellyPortId::from_u128(40);
        let edge = open_gpui_canvas::CanvasEdge::new(
            "canvas-temp-connect",
            open_gpui_canvas::CanvasEndpoint::new(
                canvas_node_id(&JellyNodeId::from_u128(2)),
                Some(canvas_port_id(&source_port)),
            ),
            open_gpui_canvas::CanvasEndpoint::new(
                canvas_node_id(&JellyNodeId::from_u128(4)),
                Some(canvas_port_id(&target_port)),
            ),
        );
        if editor
            .apply_transaction(CanvasTransaction::single(DocumentCommand::InsertEdge(edge)))
            .is_err()
        {
            return false;
        }
        let adapter = OpenGpuiAdapter::default();
        let Ok(transactions) =
            canvas_document_connection_sync_transactions(&adapter, &store, editor.document())
        else {
            return false;
        };
        if !transactions
            .iter()
            .flat_map(GraphTransaction::ops)
            .any(|op| matches!(op, GraphOp::AddEdge { edge, .. } if edge.from == source_port && edge.to == target_port))
        {
            return false;
        }
        for transaction in transactions {
            if store.dispatch_transaction(&transaction).is_err() {
                return false;
            }
        }
        store
            .graph()
            .edges()
            .values()
            .any(|edge| edge.from == source_port && edge.to == target_port)
    }

    fn product_reconnect_store_sync_probe() -> bool {
        let mut store = make_demo_store();
        let Ok((document, _projection)) = project_store(&store) else {
            return false;
        };
        let Ok(mut editor) = editor_for_document(document) else {
            return false;
        };
        let edge_id = JellyEdgeId::from_u128(200);
        let target_port = JellyPortId::from_u128(40);
        let Some(mut canvas_edge) = editor
            .document()
            .edge(&open_gpui_canvas::EdgeId::from(canvas_edge_id(&edge_id)))
            .cloned()
        else {
            return false;
        };
        canvas_edge.target = open_gpui_canvas::CanvasEndpoint::new(
            canvas_node_id(&JellyNodeId::from_u128(4)),
            Some(canvas_port_id(&target_port)),
        );
        if editor
            .apply_transaction(CanvasTransaction::single(DocumentCommand::UpdateEdge(
                canvas_edge,
            )))
            .is_err()
        {
            return false;
        }
        let adapter = OpenGpuiAdapter::default();
        let Ok(transactions) =
            canvas_document_connection_sync_transactions(&adapter, &store, editor.document())
        else {
            return false;
        };
        if !transactions
            .iter()
            .flat_map(GraphTransaction::ops)
            .any(|op| matches!(op, GraphOp::SetEdgeEndpoints { id, to, .. } if *id == edge_id && to.to == target_port))
        {
            return false;
        }
        for transaction in transactions {
            if store.dispatch_transaction(&transaction).is_err() {
                return false;
            }
        }
        store
            .graph()
            .edges()
            .get(&edge_id)
            .is_some_and(|edge| edge.to == target_port)
    }

    fn product_reconnect_affordance_probe() -> bool {
        let store = make_demo_store();
        let Ok((document, _projection)) = project_store(&store) else {
            return false;
        };
        let Ok(mut editor) = editor_for_document(document) else {
            return false;
        };
        let edge_id = open_gpui_canvas::EdgeId::from(canvas_edge_id(&JellyEdgeId::from_u128(200)));
        if editor
            .apply_tool_intent(CanvasToolIntent::ReplaceSelection(HitTarget::Edge(
                edge_id.clone(),
            )))
            .is_err()
        {
            return false;
        }
        let model = CanvasPaintModel::from(&editor);
        let frame = open_gpui_canvas::collect_visible_records(
            &model,
            Bounds::new(point(px(0.0), px(0.0)), default_canvas_view_size()),
            CanvasPaintOptions::default(),
        );
        let handles = &frame.interaction.reconnect_handles;
        handles.len() >= 2
            && handles
                .iter()
                .filter(|handle| handle.edge_id == edge_id)
                .count()
                == 2
    }

    fn product_dropped_wire_gesture_probe() -> bool {
        let mut store = make_demo_store();
        let registry = NodeKitRegistry::builtin().node_registry();
        let source = ConnectionHandleRef::new(
            JellyNodeId::from_u128(3),
            JellyPortId::from_u128(31),
            PortDirection::Out,
        );
        let Ok((document, _projection)) = project_store(&store) else {
            return false;
        };
        let Ok(mut editor) = editor_for_document(document) else {
            return false;
        };
        if editor.set_tool(CanvasTool::Connect).is_err() {
            return false;
        }
        let Some(source_node) = editor
            .document()
            .node(&NodeId::from(canvas_node_id(&source.node)))
        else {
            return false;
        };
        let Some(source_handle) = source_node.handle(Some(&open_gpui_canvas::HandleId::from(
            canvas_port_id(&source.port),
        ))) else {
            return false;
        };
        let source_view = editor
            .viewport()
            .document_to_view(source_node.position + source_handle.position);
        if editor
            .handle_event(CanvasEvent::PointerDown {
                position: source_view,
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .is_err()
            || editor.connection_drag_state().is_none()
        {
            return false;
        }

        let pointer = JellyPoint { x: 760.0, y: 430.0 };
        let release_view = editor
            .viewport()
            .document_to_view(point(px(pointer.x), px(pointer.y)));
        let release = CanvasEvent::PointerUp {
            position: release_view,
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        };
        let Some((resolved_source, resolved_pointer)) =
            dropped_wire_intent_from_canvas_event(&editor, &store, &release)
        else {
            return false;
        };
        if resolved_source != source {
            return false;
        }
        if editor.handle_event(release).is_err() {
            return false;
        }
        let source_key = PortKey::new("completion");
        let menu = project_dropped_wire_menu(
            &registry,
            resolved_source,
            Some(&source_key),
            resolved_pointer,
        );
        let Some(action_key) = menu
            .enabled_actions()
            .next()
            .map(|action| action.key.clone())
        else {
            return false;
        };
        let before_nodes = store.graph().nodes().len();
        let before_edges = store.graph().edges().len();
        if apply_demo_dropped_wire_insert(
            &mut store,
            &registry,
            &menu,
            &action_key,
            resolved_source,
            resolved_pointer,
        )
        .is_err()
        {
            return false;
        }
        store.graph().nodes().len() == before_nodes + 1
            && store.graph().edges().len() == before_edges + 1
    }

    fn product_port_hotspot_path_probe() -> bool {
        let (measured_store, transform, prompt, _) = measured_transform_store();
        let Ok((document, _projection)) = project_store(&measured_store) else {
            return false;
        };
        let kind_registry = jellyflow_kind_registry();
        let runtime = CanvasRuntime::rebuild_with_kind_registry(&document, &kind_registry);
        let Some(node) = document.node(&NodeId::from(canvas_node_id(&transform))) else {
            return false;
        };
        let measured_prompt_point = node.position + point(px(0.0), px(51.0));
        let hit_records = runtime
            .precise_hit_test_with_kind_registry(
                &document,
                &kind_registry,
                measured_prompt_point,
                connection_hit_options(),
            )
            .collect::<Vec<_>>();
        let facts = CanvasGeometryFacts::with_kind_registry(&document, &kind_registry);
        facts
            .connection_endpoint_at(hit_records, CanvasConnectionEndpointRole::Target)
            .is_some_and(|endpoint| {
                endpoint.node_id == NodeId::from(canvas_node_id(&transform))
                    && endpoint
                        .handle_id
                        .as_ref()
                        .is_some_and(|handle| handle.as_str() == canvas_port_id(&prompt))
            })
    }

    fn canvas_host_product_interaction_report() -> OpenGpuiHostProductInteractionReport {
        let catalog = product_fixture_catalog();
        let renderer_registry = demo_node_renderer_registry();
        let renderers = demo_custom_node_renderers();
        let node_kit_registry = NodeKitRegistry::builtin();
        let semantic_registry = node_kit_registry.node_registry();
        let mut product_drag_surfaces = 0;
        let mut hidden_repeatable_overflow = 0;
        let mut repeatable_overflow_indicators = 0;

        for fixture in catalog {
            let (store, document, _projection) =
                project_kit_fixture(&fixture.kit_key, &fixture.fixture_key)
                    .expect("product fixture projects into canvas document");
            let measured_store =
                measurement_store_with_projection_fallback(&store, &semantic_registry);

            for canvas_node in document.nodes() {
                let Some(node_id) = jelly_node_id_from_node(canvas_node) else {
                    continue;
                };
                let Some(graph_node) = store.graph().nodes().get(&node_id) else {
                    continue;
                };
                let Some(surface) = node_surface_summary_for_node(
                    canvas_node,
                    node_id,
                    graph_node,
                    store.graph(),
                    1.0,
                    false,
                    &semantic_registry,
                    &node_kit_registry,
                    measured_store.node_measurement(node_id),
                ) else {
                    continue;
                };
                if host_renderer_source(&renderer_registry, &renderers, &surface.renderer_context)
                    == OpenGpuiHostRendererSource::ProductRenderer
                {
                    product_drag_surfaces += 1;
                    hidden_repeatable_overflow += hidden_repeatable_overflow_for_surface(&surface);
                    repeatable_overflow_indicators +=
                        repeatable_overflow_indicator_count_for_surface(&surface);
                }
            }
        }
        let augmented_hidden_repeatables = augmented_shader_repeatable_overflow_probe();
        hidden_repeatable_overflow += augmented_hidden_repeatables;
        repeatable_overflow_indicators += usize::from(augmented_hidden_repeatables > 0);
        let drag_sequence_checked =
            product_surface_drag_sequence_probe(ProductSurfaceDragProbeEnd::CommitOutsideCanvas)
                && product_surface_drag_sequence_probe(ProductSurfaceDragProbeEnd::Cancel);
        let connect_flow_store_synced = product_connection_store_sync_probe();
        let port_hotspot_path_checked = product_port_hotspot_path_probe();
        let reconnect_affordance_visible = product_reconnect_affordance_probe();
        let dropped_wire_gesture_connected = product_dropped_wire_gesture_probe();

        let mut report = OpenGpuiHostProductInteractionReport::default();
        report.mark_drag_surface_coverage(product_drag_surfaces, drag_sequence_checked);
        report.mark_control_event_shielding_checked(true);
        report.mark_port_hotspot_path_checked(port_hotspot_path_checked);
        report.mark_tool_switcher_visible(product_tool_switcher_visible());
        report.mark_connect_flow_store_synced(connect_flow_store_synced);
        report.mark_reconnect_affordance_visible(reconnect_affordance_visible);
        report.mark_dropped_wire_gesture_connected(dropped_wire_gesture_connected);
        report.mark_repeatable_overflow(hidden_repeatable_overflow, repeatable_overflow_indicators);
        report
    }

    fn augmented_shader_repeatable_overflow_probe() -> usize {
        let Ok((mut store, _document, _projection, node_id)) =
            project_schema_node("demo.shader.mix")
        else {
            return 0;
        };
        let registry = NodeKitRegistry::builtin().node_registry();
        if !matches!(
            OpenGpuiAuthoringController.apply_repeatable_action_to_store(
                &mut store,
                &registry,
                node_id,
                OpenGpuiRepeatableActionPlan::Add {
                    collection_key: "shader.inputs".to_owned(),
                    item: serde_json::json!({
                        "name": "Input 4",
                        "ty": "vec4",
                        "port": "input_4"
                    }),
                },
            ),
            Ok(Some(_))
        ) {
            return 0;
        }

        let Ok((document, _projection)) = project_store(&store) else {
            return 0;
        };
        let node_kit_registry = NodeKitRegistry::builtin();
        let semantic_registry = node_kit_registry.node_registry();
        let Some(shader_node) = document.node(&NodeId::from(canvas_node_id(&node_id))) else {
            return 0;
        };
        let Some(shader_record) = store.graph().nodes().get(&node_id) else {
            return 0;
        };
        let Some(surface) = node_surface_summary_for_node(
            shader_node,
            node_id,
            shader_record,
            store.graph(),
            1.0,
            false,
            &semantic_registry,
            &node_kit_registry,
            None,
        ) else {
            return 0;
        };

        hidden_repeatable_overflow_for_surface(&surface)
    }

    fn hidden_repeatable_overflow_for_surface(surface: &NodeSurfaceSummary) -> usize {
        let visible_items = surface
            .renderer_context
            .surface_preset
            .repeatable_visible_items_or(usize::MAX);
        surface.repeatable_items.len().saturating_sub(visible_items)
    }

    fn repeatable_overflow_indicator_count_for_surface(surface: &NodeSurfaceSummary) -> usize {
        usize::from(
            hidden_repeatable_overflow_for_surface(surface) > 0
                && surface
                    .renderer_context
                    .surface_preset
                    .overflow_indicator
                    .is_some(),
        )
    }

    fn canvas_host_surface_report() -> OpenGpuiHostSurfaceReport {
        let catalog = product_fixture_catalog();
        let renderer_registry = demo_node_renderer_registry();
        let renderers = demo_custom_node_renderers();
        let node_kit_registry = NodeKitRegistry::builtin();
        let semantic_registry = node_kit_registry.node_registry();
        let mut report = OpenGpuiHostSurfaceReport::default();

        for fixture in catalog {
            let (store, document, _projection) =
                project_kit_fixture(&fixture.kit_key, &fixture.fixture_key)
                    .expect("product fixture projects into canvas document");
            let measured_store =
                measurement_store_with_projection_fallback(&store, &semantic_registry);

            for canvas_node in document.nodes() {
                let Some(node_id) = jelly_node_id_from_node(canvas_node) else {
                    continue;
                };
                let Some(graph_node) = store.graph().nodes().get(&node_id) else {
                    continue;
                };
                let Some(surface) = node_surface_summary_for_node(
                    canvas_node,
                    node_id,
                    graph_node,
                    store.graph(),
                    1.0,
                    false,
                    &semantic_registry,
                    &node_kit_registry,
                    measured_store.node_measurement(node_id),
                ) else {
                    continue;
                };

                report.push(host_surface_report_row(
                    &fixture,
                    &renderer_registry,
                    &renderers,
                    &surface,
                ));
            }
        }

        report
    }

    fn host_surface_report_row(
        fixture: &jellyflow_open_gpui::testing::OpenGpuiProductFixtureCase,
        registry: &OpenGpuiNodeRendererRegistry,
        renderers: &GpuiNodeRendererTable,
        surface: &NodeSurfaceSummary,
    ) -> OpenGpuiHostSurfaceReportRow {
        let measurement = surface.measurement.as_ref();
        let mut row = OpenGpuiHostSurfaceReportRow::new(
            fixture,
            surface.node_kind.clone(),
            surface.renderer_key.clone(),
            host_renderer_source(registry, renderers, &surface.renderer_context),
        )
        .with_measurement(
            measurement
                .map(|measurement| measurement.slots.len())
                .unwrap_or(0),
            measurement
                .map(|measurement| measurement.anchors.len())
                .unwrap_or(0),
        )
        .with_style_budget(surface.renderer_context.surface_preset.style.evidence());

        for slot in &surface.slot_descriptors {
            for control in project_slot_controls(&surface.node_data, slot) {
                if control.is_partial_stub() {
                    row.capability_gaps
                        .insert(OpenGpuiHostCapabilityGap::AdvancedControlStub);
                }
            }
        }
        if surface
            .repeatable_items
            .iter()
            .any(|item| item.dynamic_port_policy == OpenGpuiDynamicPortPolicy::MissingGraphPort)
        {
            row.capability_gaps
                .insert(OpenGpuiHostCapabilityGap::MissingDynamicPort);
        }
        if surface.slots.iter().any(|slot| !slot.visible)
            || surface.projection.slot_limit < surface.slots.len()
        {
            row.capability_gaps
                .insert(OpenGpuiHostCapabilityGap::PartialOrHiddenRegion);
        }

        row
    }

    fn host_renderer_source(
        registry: &OpenGpuiNodeRendererRegistry,
        renderers: &GpuiNodeRendererTable,
        context: &OpenGpuiNodeRendererContext,
    ) -> OpenGpuiHostRendererSource {
        match registry.resolve(context) {
            OpenGpuiNodeRendererResolution::Custom(registration) => {
                if renderers.contains_key(&registration.renderer_key) {
                    OpenGpuiHostRendererSource::ProductRenderer
                } else {
                    OpenGpuiHostRendererSource::MissingHostRenderer
                }
            }
            OpenGpuiNodeRendererResolution::Fallback(fallback) => match fallback.reason {
                jellyflow_open_gpui::OpenGpuiNodeRendererFallbackReason::MissingHostRenderer => {
                    OpenGpuiHostRendererSource::MissingHostRenderer
                }
                jellyflow_open_gpui::OpenGpuiNodeRendererFallbackReason::UnregisteredRenderer => {
                    OpenGpuiHostRendererSource::UnregisteredRenderer
                }
            },
        }
    }

    fn assert_canvas_nodes_do_not_overlap(fixture_id: &str, document: &CanvasDocument) {
        let nodes = document.nodes().collect::<Vec<_>>();
        for (index, left) in nodes.iter().enumerate() {
            for right in nodes.iter().skip(index + 1) {
                assert!(
                    !canvas_bounds_overlap(left.bounds(), right.bounds()),
                    "fixture `{fixture_id}` nodes `{}` and `{}` overlap: {:?} vs {:?}",
                    left.id.as_str(),
                    right.id.as_str(),
                    left.bounds(),
                    right.bounds()
                );
            }
        }
    }

    fn canvas_bounds_overlap(left: Bounds<Pixels>, right: Bounds<Pixels>) -> bool {
        let left_max_x = left.origin.x + left.size.width;
        let left_max_y = left.origin.y + left.size.height;
        let right_max_x = right.origin.x + right.size.width;
        let right_max_y = right.origin.y + right.size.height;

        left.origin.x < right_max_x
            && left_max_x > right.origin.x
            && left.origin.y < right_max_y
            && left_max_y > right.origin.y
    }

    #[test]
    fn projects_jellyflow_edge_route_hints_into_canvas_routes() {
        let mut edge = Edge::new(
            EdgeKind::Data,
            JellyPortId::from_u128(1),
            JellyPortId::from_u128(2),
        );

        assert_eq!(
            project_edge_route(&edge).kind.as_str(),
            open_gpui_canvas::CanvasEdgeRouteKind::ORTHOGONAL
        );

        edge.view = jellyflow::core::EdgeViewDescriptor::new()
            .with_route_kind(jellyflow::core::EdgeRouteKind::Bezier);
        assert_eq!(
            project_edge_route(&edge).kind.as_str(),
            open_gpui_canvas::CanvasEdgeRouteKind::CUBIC_BEZIER
        );

        edge.view = jellyflow::core::EdgeViewDescriptor::new()
            .with_route_kind(jellyflow::core::EdgeRouteKind::Straight);
        assert_eq!(
            project_edge_route(&edge).kind.as_str(),
            open_gpui_canvas::CanvasEdgeRouteKind::STRAIGHT
        );
    }

    #[test]
    fn projected_handles_use_runtime_measurement_facts() {
        let (measured_store, transform, prompt, completion) = measured_transform_store();
        let semantic_registry = NodeKitRegistry::builtin().node_registry();

        let node = measured_store.graph().nodes().get(&transform).unwrap();
        let canvas_node = project_node(
            &transform,
            node,
            measured_store.graph(),
            &measured_store,
            &semantic_registry,
        );
        let prompt_handle = canvas_node
            .handles
            .iter()
            .find(|handle| handle.id.as_str() == canvas_port_id(&prompt))
            .unwrap();
        let completion_handle = canvas_node
            .handles
            .iter()
            .find(|handle| handle.id.as_str() == canvas_port_id(&completion))
            .unwrap();

        assert_eq!(prompt_handle.position, point(px(0.0), px(51.0)));
        assert_eq!(
            completion_handle.position,
            point(canvas_node.size.width, px(150.0))
        );
        let resolution = measured_store.resolve_node_handle_measurement(ConnectionHandleRef::new(
            transform,
            prompt,
            PortDirection::In,
        ));
        assert!(matches!(
            resolution.source,
            NodeHandleMeasurementSource::MeasuredAnchor { .. }
        ));
    }

    #[test]
    fn canvas_hit_testing_uses_measured_handle_positions_for_connection_targets() {
        let (measured_store, transform, prompt, _) = measured_transform_store();
        let (document, _) = project_store(&measured_store).expect("measured graph projects");
        let runtime =
            CanvasRuntime::rebuild_with_kind_registry(&document, &jellyflow_kind_registry());
        let node = document
            .node(&NodeId::from(canvas_node_id(&transform)))
            .expect("transform canvas node");
        let measured_prompt_point = node.position + point(px(0.0), px(51.0));

        let hits = runtime
            .precise_hit_test_with_kind_registry(
                &document,
                &jellyflow_kind_registry(),
                measured_prompt_point,
                connection_hit_options(),
            )
            .map(|record| record.target.clone())
            .collect::<Vec<_>>();

        assert!(hits.contains(&HitTarget::Handle {
            node_id: NodeId::from(canvas_node_id(&transform)),
            handle_id: open_gpui_canvas::HandleId::from(canvas_port_id(&prompt)),
        }));
    }

    #[test]
    fn dirty_live_measurements_downgrade_to_projection_until_next_layout_pass() {
        let (mut measured_store, transform, prompt, _) = measured_transform_store();
        assert_eq!(
            measured_store.node_measurement_status(transform),
            NodeMeasurementStatus::Fresh { revision: 7 }
        );
        let semantic_registry = NodeKitRegistry::builtin().node_registry();
        let projection_store =
            measurement_store_with_projection_fallback(&make_demo_store(), &semantic_registry);
        let (projection_document, _) =
            project_store(&projection_store).expect("projection fallback graph projects");
        let projection_node = projection_document
            .node(&NodeId::from(canvas_node_id(&transform)))
            .expect("projection fallback transform canvas node");
        let projection_node_size = JellySize {
            width: projection_node.size.width.as_f32(),
            height: projection_node.size.height.as_f32(),
        };
        let expected = projection_store
            .resolve_node_handle_measurement(ConnectionHandleRef::new(
                transform,
                prompt,
                PortDirection::In,
            ))
            .bounds
            .map(|bounds| handle_position_from_bounds(bounds, projection_node_size))
            .expect("projection fallback prompt handle");
        assert_eq!(
            measured_store.invalidate_node_internals(NodeInternalsInvalidation::one(
                transform,
                NodeInternalsInvalidationReason::DataChanged
            )),
            jellyflow::runtime::runtime::measurement::NodeMeasurementOutcome::Changed
        );
        let (document, _) = project_store(&measured_store).expect("dirty graph projects");
        let node = document
            .node(&NodeId::from(canvas_node_id(&transform)))
            .expect("transform canvas node");
        let prompt_handle = node
            .handle(Some(&open_gpui_canvas::HandleId::from(canvas_port_id(
                &prompt,
            ))))
            .expect("prompt handle");

        assert_eq!(
            prompt_handle.position,
            point(px(expected.x), px(expected.y)),
            "dirty measured anchor should not override projection fallback"
        );
    }

    #[test]
    fn unchanged_layout_pass_measurements_reuse_revision() {
        let node = JellyNodeId::from_u128(3);
        let mut next_revision = 7;
        let mut measurement = NodeMeasurement::new(node)
            .with_revision(0)
            .with_size(Some(JellySize {
                width: 120.0,
                height: 80.0,
            }))
            .with_anchors([MeasuredSurfaceAnchor::new(
                "prompt.measured",
                JellyRect {
                    origin: JellyPoint { x: 0.0, y: 24.0 },
                    size: JellySize {
                        width: 16.0,
                        height: 18.0,
                    },
                },
                HandlePosition::Left,
            )
            .with_port_key(PortKey::new("prompt"))]);
        let existing = measurement.clone().with_revision(7);

        assign_layout_pass_measurement_revision(
            NodeMeasurementStatus::Fresh { revision: 7 },
            Some(&existing),
            &mut measurement,
            &mut next_revision,
        );

        assert_eq!(measurement.revision, 7);
        assert_eq!(next_revision, 7);

        assign_layout_pass_measurement_revision(
            NodeMeasurementStatus::Dirty {
                revision: 7,
                reason: NodeInternalsInvalidationReason::DataChanged,
            },
            Some(&existing),
            &mut measurement,
            &mut next_revision,
        );

        assert_eq!(measurement.revision, 8);
        assert_eq!(next_revision, 8);
    }

    #[test]
    fn invalid_connection_feedback_uses_measured_handle_positions() {
        let (measured_store, transform, _, completion) = measured_transform_store();
        let (document, _) = project_store(&measured_store).expect("measured graph projects");
        let registry = jellyflow_kind_registry();
        let runtime = CanvasRuntime::rebuild_with_kind_registry(&document, &registry);
        let facts = CanvasGeometryFacts::with_kind_registry(&document, &registry);
        let node = document
            .node(&NodeId::from(canvas_node_id(&transform)))
            .expect("transform canvas node");
        let completion_handle = node
            .handle(Some(&open_gpui_canvas::HandleId::from(canvas_port_id(
                &completion,
            ))))
            .expect("completion handle");
        let measured_completion_point = node.position + completion_handle.position;
        let records = runtime
            .precise_hit_test_with_kind_registry(
                &document,
                &registry,
                measured_completion_point,
                connection_hit_options(),
            )
            .collect::<Vec<_>>();

        assert!(records.iter().any(|record| {
            record.target
                == HitTarget::Handle {
                    node_id: NodeId::from(canvas_node_id(&transform)),
                    handle_id: open_gpui_canvas::HandleId::from(canvas_port_id(&completion)),
                }
        }));
        assert!(
            facts
                .connection_endpoint_at(
                    records.iter().copied(),
                    CanvasConnectionEndpointRole::Target
                )
                .is_none(),
            "the measured source handle should be visible for hover but rejected as an invalid target"
        );
    }

    fn measured_transform_store() -> (NodeGraphStore, JellyNodeId, JellyPortId, JellyPortId) {
        let store = make_demo_store();
        let transform = JellyNodeId::from_u128(3);
        let prompt = JellyPortId::from_u128(30);
        let completion = JellyPortId::from_u128(31);
        let mut measured_store = NodeGraphStore::new(
            store.graph().clone(),
            store.view_state().clone(),
            NodeGraphEditorConfig::default(),
        );
        measured_store
            .report_node_measurement(
                NodeMeasurement::new(transform)
                    .with_revision(7)
                    .with_size(Some(JellySize {
                        width: 268.0,
                        height: 228.0,
                    }))
                    .with_anchors([
                        MeasuredSurfaceAnchor::new(
                            "prompt.measured",
                            JellyRect {
                                origin: JellyPoint { x: 0.0, y: 42.0 },
                                size: JellySize {
                                    width: 16.0,
                                    height: 18.0,
                                },
                            },
                            HandlePosition::Left,
                        )
                        .with_port(prompt)
                        .with_port_key(PortKey::new("prompt")),
                        MeasuredSurfaceAnchor::new(
                            "completion.measured",
                            JellyRect {
                                origin: JellyPoint { x: 252.0, y: 138.0 },
                                size: JellySize {
                                    width: 16.0,
                                    height: 24.0,
                                },
                            },
                            HandlePosition::Right,
                        )
                        .with_port(completion)
                        .with_port_key(PortKey::new("completion")),
                    ]),
            )
            .unwrap();

        (measured_store, transform, prompt, completion)
    }

    fn assert_node_data_path_value<const N: usize>(
        plan: &OpenGpuiControlEditPlan,
        path: [&str; N],
        expected: Value,
    ) {
        let [GraphOp::SetNodeData { to, .. }] = plan.transaction.ops() else {
            panic!("expected one SetNodeData op");
        };
        let mut value = to;
        for segment in path {
            value = &value[segment];
        }
        assert_eq!(*value, expected);
    }

    fn graph_port_id_for_key(graph: &Graph, node_id: JellyNodeId, key: &str) -> JellyPortId {
        graph
            .ports()
            .iter()
            .find_map(|(port_id, port)| {
                (port.node == node_id && port.key == PortKey::new(key)).then_some(*port_id)
            })
            .expect("graph port exists")
    }

    #[test]
    fn adapter_slot_limit_scales_with_available_height() {
        assert_eq!(adapter_slot_limit_for_height(px(148.0), usize::MAX), 2);
        assert_eq!(adapter_slot_limit_for_height(px(220.0), 3), 3);
        assert_eq!(adapter_slot_limit_for_height(px(88.0), 4), 0);
    }
}
