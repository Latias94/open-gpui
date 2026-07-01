use std::collections::BTreeMap;

use jellyflow::{
    NodeGraphStore,
    core::{
        CanvasPoint as JellyPoint, CanvasRect as JellyRect, CanvasSize as JellySize, Edge,
        EdgeId as JellyEdgeId, EdgeKind, Graph, GraphId, GraphOp, GraphTransaction, Node,
        NodeId as JellyNodeId, NodeKindKey, Port, PortCapacity, PortDirection,
        PortId as JellyPortId, PortKey, PortKind,
    },
    layout::{LayoutPresetBuilder, builtin_layout_engine_registry},
    runtime::{
        io::{NodeGraphEditorConfig, NodeGraphViewState},
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
use jellyflow_open_gpui::OpenGpuiControlEventValue;
use jellyflow_open_gpui::open_gpui_node_renderer_context;
use jellyflow_open_gpui::{
    OpenGpuiActionPlan, OpenGpuiActionSurface, OpenGpuiAuthoringController,
    OpenGpuiAuthoringOutcome, OpenGpuiAuthoringSkipReason, OpenGpuiBoundsCollector,
    OpenGpuiControlEditPlan, OpenGpuiControlPlan, OpenGpuiControlPrimitive,
    OpenGpuiDynamicPortPolicy, OpenGpuiInspectorPlan, OpenGpuiInspectorSurface,
    OpenGpuiInspectorTargetBounds, OpenGpuiInspectorTargetSource, OpenGpuiMeasurementContext,
    OpenGpuiMeasurementId, OpenGpuiMeasurementMode as NodeSurfaceMeasurementSource,
    OpenGpuiMenuPlan, OpenGpuiNodeRendererContext, OpenGpuiNodeRendererRegistry,
    OpenGpuiNodeRendererResolution, OpenGpuiNodeRendererState,
    OpenGpuiNodeSurfaceLayout as NodeSurfaceComponentLayout,
    OpenGpuiNodeSurfaceSlotLayout as NodeSurfaceSlotLayout,
    OpenGpuiRepeatableItemLayout as NodeRepeatableItemLayout,
    OpenGpuiRepeatableItemProjection as NodeRepeatableItemProjection,
    OpenGpuiRepeatableSurfaceLayout as NodeRepeatableSurfaceLayout,
    OpenGpuiRepeatableSurfaceProjection as NodeRepeatableSurfaceProjection, OpenGpuiViewBounds,
    OpenGpuiViewPoint, OpenGpuiViewSize, control_option_key, control_selected_option_key,
    layout_pass_measurement_from_regions, measured_surface_anchors, project_actions_for_surface,
    project_inspectors_for_surface, project_menu, project_node_measurement, project_slot_controls,
    projected_node_surface_graph_layout, repeatable_item_projection, repeatable_surface_projection,
    resolve_inspector_target_bounds,
};
use open_gpui::{
    AnyElement, App, Bounds, Context, FocusHandle, Hsla, KeyDownEvent, MouseButton, MouseDownEvent,
    Pixels, WeakEntity, Window, WindowBounds, WindowOptions, div, measured_element, point,
    prelude::*, px, rgb, size,
};
use open_gpui_canvas::{
    CanvasDocument, CanvasEditor, CanvasEditorInputHandler, CanvasEvent, CanvasHandle,
    CanvasKindLabel, CanvasKindPaint, CanvasKindRegistry, CanvasNode, CanvasNodeKind,
    CanvasNodeRenderPolicy, CanvasPaintModel, CanvasPaintOptions, CanvasPaintTheme,
    CanvasToolIntent, DocumentError, HandleRole, HitTarget, NodeId, canvas_editor_view,
};
use open_gpui_platform::application;
use open_gpui_ui_components::gpui_adapter::init_text_input;
use open_gpui_ui_components::prelude::Sizable;
use open_gpui_ui_components::{
    Badge, BadgeVariant, Button, ButtonVariant, ListboxOption, Menu, MenuItem, NumberInput,
    Progress, Select, Slider, Switch, TextInput, Textarea,
};
use open_gpui_ui_core::Size;
use serde_json::Value;

const REPEATABLE_ITEM_SNAPSHOTS_FIELD: &str = "jellyflow_repeatable_items";
const INITIAL_SELECTION: u128 = 2;
const CANVAS_WIDTH: f32 = 1140.0;
const CANVAS_HEIGHT: f32 = 650.0;
const NODE_SURFACE_CHROME_HEIGHT: f32 = 78.0;
const NODE_SURFACE_SLOT_ROW_HEIGHT: f32 = 26.0;
const GPUI_LAYOUT_PASS_MEASUREMENT_GAP: &str = "canvas-jellyflow can project Jellyflow \
authoring controls, repeatables, and actions through open-gpui components, but it cannot claim \
full layout-pass measurement until open-gpui exposes a stable element-bounds callback for \
node-local slot/control/anchor regions during layout or prepaint.";

type GpuiNodeRenderer = Box<
    dyn Fn(
        &OpenGpuiNodeRendererContext,
        OpenGpuiBoundsCollector,
        WeakEntity<JellyflowCanvasView>,
    ) -> AnyElement,
>;

struct JellyflowCanvasView {
    editor: CanvasEditor,
    store: NodeGraphStore,
    focus_handle: FocusHandle,
    projection: ProjectionSummary,
    semantic_registry: NodeRegistry,
    node_kit_registry: NodeKitRegistry,
    measured_regions: OpenGpuiBoundsCollector,
    measurement_revision: u64,
    measurement_frame_pending: bool,
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
    layout_gap: &'static str,
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
}

struct SelectedNodeSummary {
    id: String,
    kind: String,
    title: String,
    detail: String,
    ports: String,
    inspectors: Vec<OpenGpuiInspectorPlan>,
    inspector_target: Option<OpenGpuiInspectorTargetBounds>,
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
                    .child(self.render_toolbar(selection_count))
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .overflow_hidden()
                            .child(
                                canvas_editor_view(
                                    model,
                                    cx.entity(),
                                    self.focus_handle.clone(),
                                    Self::canvas_input_handler(),
                                    options,
                                    theme,
                                )
                                .size_full(),
                            )
                            .children(self.render_node_surfaces(&render_model, cx)),
                    ),
            )
            .child(self.render_sidebar(selected))
    }
}

impl JellyflowCanvasView {
    fn render_toolbar(&self, selection_count: usize) -> impl IntoElement {
        div()
            .h(px(46.0))
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .border_b_1()
            .border_color(rgb(0xdbe3ea))
            .bg(rgb(0xffffff))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x111827))
                            .child("Jellyflow GPUI adapter fixture"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x64748b))
                            .child(self.projection.last_commit.clone()),
                    ),
            )
            .child(div().text_xs().text_color(rgb(0x475569)).child(format!(
                "{} graph nodes / {} ports / {} edges / {} selected records",
                self.projection.graph_nodes,
                self.projection.graph_ports,
                self.projection.graph_edges,
                selection_count
            )))
    }

    fn render_sidebar(&self, selected: Option<SelectedNodeSummary>) -> impl IntoElement {
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
                .child(render_selected_inspector_panel(&summary)),
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
                            .child(self.projection.capability.layout_gap),
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
        let inspectors = self.inspector_plans_for_canvas_node(node);
        let jelly_node = jelly_node_id_from_node(node)?;
        let measurement = self.store.node_measurement(jelly_node);
        let inspector_target = inspectors.first().map(|inspector| {
            resolve_inspector_target_bounds(inspector, measurement.as_ref(), None)
        });
        Some(SelectedNodeSummary {
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
        })
    }

    fn inspector_plans_for_canvas_node(&self, node: &CanvasNode) -> Vec<OpenGpuiInspectorPlan> {
        let kind =
            NodeKindKey::new(data_string(node, "jellyflow_kind").unwrap_or(node.kind.as_str()));
        let Some(descriptor) = self.semantic_registry.view_descriptor(&kind) else {
            return Vec::new();
        };
        let node_data = Value::Object(node.data.clone());
        project_inspectors_for_surface(
            &descriptor,
            &node_data,
            &OpenGpuiInspectorSurface::Node {
                node_kind: descriptor.kind.0.clone(),
            },
        )
    }

    fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        match self.handle_canvas_shortcut(event) {
            Ok(true) => {
                cx.notify();
            }
            Ok(false) => {
                Self::canvas_input_handler().dispatch_key_down(self, event, cx);
            }
            Err(error) => {
                eprintln!("canvas shortcut failed: {error}");
            }
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

        if let Err(error) = self.editor.handle_event(event) {
            eprintln!("canvas event failed: {error}");
        }
        cx.notify();
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
            assign_layout_pass_revision(
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

    fn refresh_editor_from_store(&mut self) {
        let selection = self
            .editor
            .selection()
            .selected_nodes()
            .next()
            .map(|id| id.clone());
        let Ok((document, projection)) = project_store(&self.store) else {
            return;
        };
        let Ok(mut editor) =
            CanvasEditor::try_new_with_kind_registry(document, jellyflow_kind_registry())
        else {
            return;
        };
        if let Some(id) = selection {
            let _ = editor.apply_tool_intent(CanvasToolIntent::ReplaceSelection(HitTarget::Node(
                id.clone(),
            )));
        }
        self.editor = editor;
        self.projection = projection;
    }
}

fn assign_layout_pass_revision(
    status: NodeMeasurementStatus,
    existing: Option<&NodeMeasurement>,
    measurement: &mut NodeMeasurement,
    next_revision: &mut u64,
) {
    if status.is_fresh()
        && let Some(existing) = existing
        && node_measurement_regions_match(existing, measurement)
    {
        measurement.revision = existing.revision;
        return;
    }

    let floor = existing
        .map(|measurement| measurement.revision)
        .unwrap_or(0);
    *next_revision = (*next_revision).max(floor).saturating_add(1);
    measurement.revision = *next_revision;
}

fn node_measurement_regions_match(left: &NodeMeasurement, right: &NodeMeasurement) -> bool {
    left.node == right.node
        && left.density == right.density
        && left.size == right.size
        && left.handles == right.handles
        && left.slots == right.slots
        && left.anchors == right.anchors
}

fn render_selected_inspector_panel(summary: &SelectedNodeSummary) -> AnyElement {
    let inspectors = summary
        .inspectors
        .iter()
        .take(2)
        .map(render_inspector_card)
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

fn render_inspector_card(inspector: &OpenGpuiInspectorPlan) -> AnyElement {
    let controls = inspector
        .controls
        .iter()
        .take(2)
        .enumerate()
        .map(|(index, control)| render_control_preview(control, index))
        .collect::<Vec<_>>();
    let actions = inspector
        .action_menu
        .actions
        .iter()
        .take(2)
        .enumerate()
        .map(|(index, action)| render_action_button(action, index))
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

fn render_node_surface(
    bounds: Bounds<Pixels>,
    node_id: JellyNodeId,
    surface: NodeSurfaceSummary,
    collector: OpenGpuiBoundsCollector,
    view: WeakEntity<JellyflowCanvasView>,
) -> impl IntoElement {
    let registry = demo_node_renderer_registry();
    match registry.resolve(&surface.renderer_context) {
        OpenGpuiNodeRendererResolution::Custom(registration) => {
            let renderers = demo_custom_node_renderers();
            if let Some(renderer) = renderers.get(&registration.renderer_key) {
                let shell = render_node_surface_shell(bounds, &surface);
                let body = renderer(&surface.renderer_context, collector.clone(), view.clone());
                return shell.child(body).into_any_element();
            }
            render_descriptor_fallback_node_surface(bounds, node_id, surface, collector, view)
        }
        OpenGpuiNodeRendererResolution::Fallback(_) => {
            render_descriptor_fallback_node_surface(bounds, node_id, surface, collector, view)
        }
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
        .relative()
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
        .children(render_surface_chrome(&surface, bounds))
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
    OpenGpuiNodeRendererRegistry::new().with_renderer("decision-card", "Dify LLM decision card")
}

fn demo_custom_node_renderers() -> BTreeMap<String, GpuiNodeRenderer> {
    let mut renderers: BTreeMap<String, GpuiNodeRenderer> = BTreeMap::new();
    renderers.insert(
        "decision-card".to_owned(),
        Box::new(|context, collector, view| {
            render_decision_card_node_surface(context, collector, view)
        }),
    );
    renderers
}

fn render_decision_card_node_surface(
    context: &OpenGpuiNodeRendererContext,
    collector: OpenGpuiBoundsCollector,
    view: WeakEntity<JellyflowCanvasView>,
) -> AnyElement {
    let prompt_control = context.control("control.prompt");
    let model_control = context.control("control.model");
    let temperature_control = context.control("control.temperature");
    let stream_control = context.control("control.stream");
    let primary_action = context
        .toolbar_menu
        .actions
        .iter()
        .find(|action| action.key == "action.llm.run")
        .or_else(|| context.toolbar_menu.actions.first());

    div()
        .size_full()
        .relative()
        .rounded_sm()
        .border_1()
        .border_color(if context.state.selected {
            rgb(0x2563eb)
        } else {
            rgb(0x0f766e)
        })
        .bg(rgb(0xf8fafc))
        .overflow_hidden()
        .shadow_sm()
        .child(
            div()
                .absolute()
                .left(px(8.0))
                .top(px(8.0))
                .right(px(8.0))
                .h(px(24.0))
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .overflow_hidden()
                .child(
                    Badge::new(
                        format!("jellyflow-custom-renderer:{}", context.renderer_key),
                        "custom renderer",
                    )
                    .variant(BadgeVariant::Default)
                    .with_size(Size::XSmall),
                )
                .child(
                    div()
                        .text_xs()
                        .truncate()
                        .min_w(px(0.0))
                        .text_color(rgb(0x64748b))
                        .child(context.renderer_key.clone()),
                ),
        )
        .child(render_measured_region(
            context.slot_measurement_id("field.prompt"),
            collector.clone(),
            div()
                .absolute()
                .left(px(8.0))
                .top(px(38.0))
                .right(px(8.0))
                .h(px(34.0))
                .rounded_sm()
                .bg(rgb(0xecfeff))
                .px_2()
                .py_1()
                .overflow_hidden()
                .child(
                    div()
                        .text_sm()
                        .line_height(px(18.0))
                        .truncate()
                        .text_color(rgb(0x0f172a))
                        .child(context.title.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .line_height(px(14.0))
                        .truncate()
                        .text_color(rgb(0x475569))
                        .child(context.summary.clone().unwrap_or_default()),
                ),
        ))
        .child(render_measured_region(
            context.anchor_measurement_id("field.prompt"),
            collector.clone(),
            div()
                .absolute()
                .left(px(0.0))
                .top(px(46.0))
                .w(px(8.0))
                .h(px(20.0)),
        ))
        .child(render_measured_region(
            context.anchor_measurement_id("field.completion"),
            collector.clone(),
            div()
                .absolute()
                .right(px(0.0))
                .bottom(px(46.0))
                .w(px(8.0))
                .h(px(20.0)),
        ))
        .child(
            div()
                .absolute()
                .left(px(8.0))
                .top(px(78.0))
                .right(px(8.0))
                .h(px(58.0))
                .flex()
                .flex_col()
                .gap_1()
                .overflow_hidden()
                .child(render_custom_control_row(
                    context,
                    "field.prompt",
                    prompt_control.as_ref(),
                    0,
                    collector.clone(),
                    view.clone(),
                ))
                .child(render_custom_control_row(
                    context,
                    "badge.model",
                    model_control.as_ref(),
                    1,
                    collector.clone(),
                    view.clone(),
                )),
        )
        .child(
            div()
                .absolute()
                .left(px(8.0))
                .top(px(142.0))
                .right(px(8.0))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_between()
                .gap_1()
                .overflow_hidden()
                .child(render_custom_control_chip(
                    context,
                    "config.model",
                    temperature_control.as_ref(),
                    2,
                    collector.clone(),
                    view.clone(),
                ))
                .child(render_custom_control_chip(
                    context,
                    "config.model",
                    stream_control.as_ref(),
                    3,
                    collector,
                    view,
                ))
                .child(
                    primary_action
                        .map(|action| render_action_button(action, 0))
                        .unwrap_or_else(|| {
                            Badge::new(
                                format!("jellyflow-custom-action-missing:{}", context.node_id.0),
                                "no action",
                            )
                            .variant(BadgeVariant::Outline)
                            .with_size(Size::XSmall)
                            .into_any_element()
                        }),
                ),
        )
        .child(
            div()
                .absolute()
                .left(px(8.0))
                .bottom(px(8.0))
                .right(px(8.0))
                .h(px(18.0))
                .flex()
                .items_center()
                .gap_1()
                .overflow_hidden()
                .child(
                    Badge::new(
                        format!("jellyflow-custom-slots:{}", context.node_id.0),
                        format!("{} slots", context.surface_layout.slots.len()),
                    )
                    .variant(BadgeVariant::Secondary)
                    .with_size(Size::XSmall),
                )
                .child(
                    Badge::new(
                        format!("jellyflow-custom-repeatables:{}", context.node_id.0),
                        format!("{} repeatables", context.repeatable_items.len()),
                    )
                    .variant(BadgeVariant::Outline)
                    .with_size(Size::XSmall),
                ),
        )
        .into_any_element()
}

fn render_custom_control_row(
    context: &OpenGpuiNodeRendererContext,
    slot_key: &str,
    control: Option<&OpenGpuiControlPlan>,
    index: usize,
    collector: OpenGpuiBoundsCollector,
    view: WeakEntity<JellyflowCanvasView>,
) -> AnyElement {
    let Some(control) = control else {
        return div().into_any_element();
    };
    render_measured_region(
        context.control_measurement_id(slot_key, control.key.clone()),
        collector,
        div()
            .h(px(26.0))
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .rounded_sm()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xcbd5e1))
            .px_2()
            .overflow_hidden()
            .child(
                div()
                    .text_xs()
                    .truncate()
                    .min_w(px(0.0))
                    .text_color(rgb(0x334155))
                    .child(control.label.clone()),
            )
            .child(
                div()
                    .max_w(px(132.0))
                    .overflow_hidden()
                    .child(render_control_plan(
                        context.node_id,
                        context.node_data.clone(),
                        control,
                        index,
                        view,
                    )),
            ),
    )
}

fn render_custom_control_chip(
    context: &OpenGpuiNodeRendererContext,
    slot_key: &str,
    control: Option<&OpenGpuiControlPlan>,
    index: usize,
    collector: OpenGpuiBoundsCollector,
    view: WeakEntity<JellyflowCanvasView>,
) -> AnyElement {
    let Some(control) = control else {
        return div().into_any_element();
    };
    render_measured_region(
        context.control_measurement_id(slot_key, control.key.clone()),
        collector,
        div()
            .h(px(24.0))
            .max_w(px(112.0))
            .overflow_hidden()
            .child(render_control_plan(
                context.node_id,
                context.node_data.clone(),
                control,
                index,
                view,
            )),
    )
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
) -> Vec<AnyElement> {
    surface
        .chrome
        .iter()
        .filter_map(|chrome| render_node_chrome(chrome, surface, view_bounds))
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
                format!("jellyflow-action-summary:{}", action.key),
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

fn render_action_button(action: &OpenGpuiActionPlan, index: usize) -> AnyElement {
    Button::new(
        format!("jellyflow-action-button:{}:{index}", action.key),
        action_button_label(action),
    )
    .variant(action_button_variant(action, index))
    .disabled(!action.dispatchable())
    .with_size(Size::XSmall)
    .into_any_element()
}

fn render_action_menu(menu: &OpenGpuiMenuPlan, id_suffix: &str) -> AnyElement {
    let items = menu
        .actions
        .iter()
        .map(|action| {
            MenuItem::action(action.key.clone(), action_menu_item_label(action))
                .disabled(!action.dispatchable())
        })
        .collect::<Vec<_>>();

    Menu::new(
        format!("jellyflow-action-menu:{}:{id_suffix}", menu.key),
        format!("{} {}", menu.label, menu.actions.len()),
    )
    .items(items)
    .disabled(menu.actions.is_empty())
    .with_size(Size::XSmall)
    .into_any_element()
}

fn render_chrome_action_buttons(
    surface: &NodeSurfaceSummary,
    fallback_label: &str,
) -> Vec<AnyElement> {
    let actions = surface
        .action_menus
        .iter()
        .flat_map(|menu| menu.actions.iter())
        .take(2)
        .enumerate()
        .map(|(index, action)| render_action_button(action, index))
        .collect::<Vec<_>>();

    if actions.is_empty() {
        return vec![
            Button::new(
                format!("jellyflow-chrome-run-fallback:{}", surface.node_kind),
                fallback_label.to_owned(),
            )
            .variant(ButtonVariant::Default)
            .with_size(Size::XSmall)
            .into_any_element(),
        ];
    }

    actions
}

fn action_button_variant(action: &OpenGpuiActionPlan, index: usize) -> ButtonVariant {
    if action.danger {
        ButtonVariant::Destructive
    } else if index == 0 {
        ButtonVariant::Default
    } else {
        ButtonVariant::Secondary
    }
}

fn action_button_label(action: &OpenGpuiActionPlan) -> String {
    action
        .icon_key
        .as_ref()
        .map(|icon| format!("{icon} {}", action.label))
        .unwrap_or_else(|| action.label.clone())
}

fn action_summary_label(action: &OpenGpuiActionPlan) -> String {
    action
        .shortcut
        .as_ref()
        .map(|shortcut| format!("{} {}", action.label, shortcut))
        .unwrap_or_else(|| action.label.clone())
}

fn action_menu_item_label(action: &OpenGpuiActionPlan) -> String {
    match (&action.shortcut, &action.disabled_reason) {
        (Some(shortcut), Some(reason)) => format!("{} · {} · {}", action.label, shortcut, reason),
        (Some(shortcut), None) => format!("{} · {}", action.label, shortcut),
        (None, Some(reason)) => format!("{} · {}", action.label, reason),
        (None, None) => action.label.clone(),
    }
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
            .children(render_chrome_action_buttons(surface, &label))
            .into_any_element(),
        NodeChromeKind::Toolbar => base
            .justify_end()
            .gap_1()
            .child(render_action_menu(&surface.toolbar_menu, &chrome.key))
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
        .child(render_slot_label(&slot, label, status))
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

fn gpui_view_bounds(bounds: Bounds<Pixels>) -> OpenGpuiViewBounds {
    OpenGpuiViewBounds::new(
        OpenGpuiViewPoint::new(bounds.origin.x.as_f32(), bounds.origin.y.as_f32()),
        OpenGpuiViewSize::new(bounds.size.width.as_f32(), bounds.size.height.as_f32()),
    )
}

fn render_measured_region(
    id: OpenGpuiMeasurementId,
    collector: OpenGpuiBoundsCollector,
    child: impl IntoElement,
) -> AnyElement {
    let element_id = id.element_id();
    measured_element(element_id, child, move |_, bounds, global_id, _, _| {
        collector.record_id(id.clone(), gpui_view_bounds(bounds), global_id);
    })
    .into_any_element()
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
    slot: &NodeSurfaceSlotProjection,
    label: String,
    status: &'static str,
) -> AnyElement {
    match slot.kind {
        NodeSurfaceSlotKind::Badge | NodeSurfaceSlotKind::MetricBadge => {
            Badge::new(format!("jellyflow-slot-badge:{}", slot.key), label)
                .variant(BadgeVariant::Secondary)
                .with_size(Size::XSmall)
                .into_any_element()
        }
        NodeSurfaceSlotKind::StatusBanner => {
            Badge::new(format!("jellyflow-status-label:{}", slot.key), label)
                .variant(BadgeVariant::Default)
                .with_size(Size::XSmall)
                .into_any_element()
        }
        NodeSurfaceSlotKind::ActionRow => {
            Badge::new(format!("jellyflow-action-label:{}", slot.key), label)
                .variant(BadgeVariant::Outline)
                .with_size(Size::XSmall)
                .into_any_element()
        }
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
        NodeSurfaceSlotKind::ActionRow => render_action_buttons(slot, &value).into_any_element(),
        NodeSurfaceSlotKind::Preview => div()
            .w(px(72.0))
            .flex_shrink_0()
            .child(
                Progress::new(format!("jellyflow-preview-progress:{}", slot.key), value)
                    .value(64.0)
                    .with_size(Size::XSmall),
            )
            .into_any_element(),
        NodeSurfaceSlotKind::Badge
        | NodeSurfaceSlotKind::MetricBadge
        | NodeSurfaceSlotKind::StatusBanner => {
            Badge::new(format!("jellyflow-slot-value:{}", slot.key), value)
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
                node_data.clone(),
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
    node_data: Value,
    view: WeakEntity<JellyflowCanvasView>,
) -> AnyElement {
    render_measured_region(
        OpenGpuiMeasurementId::control_in_slot(node_id, slot_key, control.key.clone()),
        collector,
        render_control_plan(node_id, node_data, control, index, view),
    )
}

fn render_control_plan(
    node_id: JellyNodeId,
    node_data: Value,
    control: &OpenGpuiControlPlan,
    index: usize,
    view: WeakEntity<JellyflowCanvasView>,
) -> AnyElement {
    let id = format!("jellyflow-control:{}:{index}", control.key);
    let disabled = control.disabled_reason.is_some();
    let read_only = control.read_only || !control.is_editable();
    let label = control.label.clone();
    let value = control_value_label(control);
    let control_plan = control.clone();

    let element = match control.primitive {
        OpenGpuiControlPrimitive::TextInput => TextInput::new(id, label)
            .value(value)
            .placeholder(control.placeholder.clone().unwrap_or_default())
            .disabled(disabled)
            .read_only(read_only)
            .on_change(control_text_change_handler(
                node_id,
                node_data.clone(),
                control_plan.clone(),
                view.clone(),
            ))
            .with_size(Size::XSmall)
            .into_any_element(),
        OpenGpuiControlPrimitive::TextArea => Textarea::new(id, label)
            .value(value)
            .placeholder(control.placeholder.clone().unwrap_or_default())
            .rows(2)
            .disabled(disabled)
            .read_only(read_only)
            .on_change(control_text_change_handler(
                node_id,
                node_data.clone(),
                control_plan.clone(),
                view.clone(),
            ))
            .with_size(Size::XSmall)
            .into_any_element(),
        OpenGpuiControlPrimitive::NumberInput => NumberInput::new(id, label)
            .value(control_number_value(control))
            .disabled(disabled)
            .read_only(read_only)
            .on_change(control_number_change_handler(
                node_id,
                node_data.clone(),
                control_plan.clone(),
                view.clone(),
            ))
            .with_size(Size::XSmall)
            .into_any_element(),
        OpenGpuiControlPrimitive::Select | OpenGpuiControlPrimitive::MultiSelect => {
            let selected = control_selected_option_key(control).unwrap_or_default();
            let select = Select::new(id, label)
                .options(control_options(control))
                .placeholder(
                    control
                        .placeholder
                        .clone()
                        .unwrap_or_else(|| "Select".to_string()),
                )
                .selected(selected)
                .disabled(disabled || control.options.is_empty())
                .on_select(control_select_change_handler(
                    node_id,
                    node_data.clone(),
                    control_plan.clone(),
                    view.clone(),
                ))
                .with_size(Size::XSmall);
            select.into_any_element()
        }
        OpenGpuiControlPrimitive::Switch => Switch::new(id)
            .label(label)
            .checked(control_bool_value(control))
            .disabled(disabled)
            .on_change(control_bool_change_handler(
                node_id,
                node_data.clone(),
                control_plan.clone(),
                view.clone(),
            ))
            .with_size(Size::XSmall)
            .into_any_element(),
        OpenGpuiControlPrimitive::Slider => Slider::new(id, label)
            .value(control_number_value(control))
            .disabled(disabled)
            .on_change(control_slider_change_handler(
                node_id,
                node_data.clone(),
                control_plan.clone(),
                view.clone(),
            ))
            .with_size(Size::XSmall)
            .into_any_element(),
        OpenGpuiControlPrimitive::CodeEditor | OpenGpuiControlPrimitive::ColorSwatch => {
            Badge::new(id, format!("{}: {}", control.label, value))
                .variant(BadgeVariant::Default)
                .with_size(Size::XSmall)
                .into_any_element()
        }
        OpenGpuiControlPrimitive::AssetPickerStub
        | OpenGpuiControlPrimitive::VariablePickerStub
        | OpenGpuiControlPrimitive::PortBindingDisplay => {
            Button::new(id, format!("{}*", control.label))
                .variant(ButtonVariant::Secondary)
                .with_size(Size::XSmall)
                .into_any_element()
        }
    };

    render_node_internal_interaction_region(element)
}

fn render_control_preview(control: &OpenGpuiControlPlan, index: usize) -> AnyElement {
    let id = format!("jellyflow-control-preview:{}:{index}", control.key);
    let disabled = control.disabled_reason.is_some();
    let read_only = control.read_only || !control.is_editable();
    let label = control.label.clone();
    let value = control_value_label(control);

    match control.primitive {
        OpenGpuiControlPrimitive::TextInput => TextInput::new(id, label)
            .value(value)
            .placeholder(control.placeholder.clone().unwrap_or_default())
            .disabled(disabled)
            .read_only(read_only)
            .with_size(Size::XSmall)
            .into_any_element(),
        OpenGpuiControlPrimitive::TextArea => Textarea::new(id, label)
            .value(value)
            .placeholder(control.placeholder.clone().unwrap_or_default())
            .rows(2)
            .disabled(disabled)
            .read_only(read_only)
            .with_size(Size::XSmall)
            .into_any_element(),
        OpenGpuiControlPrimitive::NumberInput => NumberInput::new(id, label)
            .value(control_number_value(control))
            .disabled(disabled)
            .read_only(read_only)
            .with_size(Size::XSmall)
            .into_any_element(),
        OpenGpuiControlPrimitive::Select | OpenGpuiControlPrimitive::MultiSelect => {
            Select::new(id, label)
                .options(control_options(control))
                .placeholder(
                    control
                        .placeholder
                        .clone()
                        .unwrap_or_else(|| "Select".to_string()),
                )
                .selected(control_selected_option_key(control).unwrap_or_default())
                .disabled(disabled || control.options.is_empty())
                .with_size(Size::XSmall)
                .into_any_element()
        }
        OpenGpuiControlPrimitive::Switch => Switch::new(id)
            .label(label)
            .checked(control_bool_value(control))
            .disabled(disabled)
            .with_size(Size::XSmall)
            .into_any_element(),
        OpenGpuiControlPrimitive::Slider => Slider::new(id, label)
            .value(control_number_value(control))
            .disabled(disabled)
            .with_size(Size::XSmall)
            .into_any_element(),
        OpenGpuiControlPrimitive::CodeEditor | OpenGpuiControlPrimitive::ColorSwatch => {
            Badge::new(id, format!("{}: {}", control.label, value))
                .variant(BadgeVariant::Default)
                .with_size(Size::XSmall)
                .into_any_element()
        }
        OpenGpuiControlPrimitive::AssetPickerStub
        | OpenGpuiControlPrimitive::VariablePickerStub
        | OpenGpuiControlPrimitive::PortBindingDisplay => {
            Button::new(id, format!("{}*", control.label))
                .variant(ButtonVariant::Secondary)
                .with_size(Size::XSmall)
                .into_any_element()
        }
    }
}

fn control_text_change_handler(
    node_id: JellyNodeId,
    node_data: Value,
    control: OpenGpuiControlPlan,
    view: WeakEntity<JellyflowCanvasView>,
) -> impl Fn(String, &mut Window, &mut App) + 'static {
    move |value, _window, cx| {
        let node = authoring_node_from_control_data(node_data.clone());
        let outcome =
            OpenGpuiAuthoringController.plan_control_text_edit(node_id, &node, &control, value);
        view.update(cx, |this, cx| {
            this.dispatch_control_authoring_plan(outcome, cx);
        })
        .ok();
    }
}

fn control_number_change_handler(
    node_id: JellyNodeId,
    node_data: Value,
    control: OpenGpuiControlPlan,
    view: WeakEntity<JellyflowCanvasView>,
) -> impl Fn(open_gpui_ui_components::NumberInputChange, &mut Window, &mut App) + 'static {
    move |change, _window, cx| {
        if !change.changed() {
            return;
        }
        let node = authoring_node_from_control_data(node_data.clone());
        let outcome = OpenGpuiAuthoringController.plan_control_number_edit(
            node_id,
            &node,
            &control,
            change.value() as f64,
        );
        view.update(cx, |this, cx| {
            this.dispatch_control_authoring_plan(outcome, cx);
        })
        .ok();
    }
}

fn control_slider_change_handler(
    node_id: JellyNodeId,
    node_data: Value,
    control: OpenGpuiControlPlan,
    view: WeakEntity<JellyflowCanvasView>,
) -> impl Fn(open_gpui_ui_components::SliderChange, &mut Window, &mut App) + 'static {
    move |change, _window, cx| {
        if !change.changed() {
            return;
        }
        let node = authoring_node_from_control_data(node_data.clone());
        let outcome = OpenGpuiAuthoringController.plan_control_number_edit(
            node_id,
            &node,
            &control,
            change.value() as f64,
        );
        view.update(cx, |this, cx| {
            this.dispatch_control_authoring_plan(outcome, cx);
        })
        .ok();
    }
}

fn control_bool_change_handler(
    node_id: JellyNodeId,
    node_data: Value,
    control: OpenGpuiControlPlan,
    view: WeakEntity<JellyflowCanvasView>,
) -> impl Fn(bool, &open_gpui::ClickEvent, &mut Window, &mut App) + 'static {
    move |checked, _event, _window, cx| {
        let node = authoring_node_from_control_data(node_data.clone());
        let outcome =
            OpenGpuiAuthoringController.plan_control_bool_edit(node_id, &node, &control, checked);
        view.update(cx, |this, cx| {
            this.dispatch_control_authoring_plan(outcome, cx);
        })
        .ok();
    }
}

fn control_select_change_handler(
    node_id: JellyNodeId,
    node_data: Value,
    control: OpenGpuiControlPlan,
    view: WeakEntity<JellyflowCanvasView>,
) -> impl Fn(open_gpui_ui_components::SelectSelection, &mut Window, &mut App) + 'static {
    move |selection, _window, cx| {
        let node = authoring_node_from_control_data(node_data.clone());
        let outcome = OpenGpuiAuthoringController.plan_control_select_edit(
            node_id,
            &node,
            &control,
            selection.value(),
        );
        view.update(cx, |this, cx| {
            this.dispatch_control_authoring_plan(outcome, cx);
        })
        .ok();
    }
}

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

fn render_node_internal_interaction_region(child: AnyElement) -> AnyElement {
    div()
        .block_mouse_except_scroll()
        .on_mouse_down(MouseButton::Left, |event: &MouseDownEvent, _window, cx| {
            cx.stop_propagation();
            let _ = event;
        })
        .on_key_down(|_: &KeyDownEvent, _window, cx| {
            cx.stop_propagation();
        })
        .child(child)
        .into_any_element()
}

fn report_authoring_skip(reason: OpenGpuiAuthoringSkipReason) {
    match reason {
        OpenGpuiAuthoringSkipReason::UnchangedControl { .. } => {}
        other => eprintln!("control authoring skipped: {other:?}"),
    }
}

fn control_options(control: &OpenGpuiControlPlan) -> Vec<ListboxOption> {
    control
        .options
        .iter()
        .map(|option| {
            ListboxOption::new(control_option_key(option), option.label.clone())
                .disabled(option.disabled)
        })
        .collect()
}

fn control_value_label(control: &OpenGpuiControlPlan) -> String {
    control
        .value
        .as_ref()
        .map(json_value_label)
        .unwrap_or_default()
}

fn control_number_value(control: &OpenGpuiControlPlan) -> f32 {
    control
        .value
        .as_ref()
        .and_then(|value| match value {
            Value::Number(number) => number.as_f64(),
            Value::String(text) => text.parse::<f64>().ok(),
            _ => None,
        })
        .unwrap_or_default() as f32
}

fn control_bool_value(control: &OpenGpuiControlPlan) -> bool {
    control
        .value
        .as_ref()
        .and_then(|value| match value {
            Value::Bool(value) => Some(*value),
            Value::String(text) => match text.as_str() {
                "true" | "yes" | "on" | "1" => Some(true),
                "false" | "no" | "off" | "0" => Some(false),
                _ => None,
            },
            _ => None,
        })
        .unwrap_or_default()
}

fn json_value_label(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(values) => values
            .iter()
            .map(json_value_label)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        Value::Object(_) => value.to_string(),
    }
}

fn render_action_buttons(slot: &NodeSurfaceSlotProjection, value: &str) -> impl IntoElement {
    let actions = value
        .split(['·', ',', '[', ']'])
        .filter(|action| !action.trim().is_empty() && *action != "-")
        .take(2)
        .enumerate()
        .map(|(index, action)| {
            Button::new(
                format!("jellyflow-action:{}:{index}", slot.key),
                action.trim().to_owned(),
            )
            .variant(if index == 0 {
                ButtonVariant::Default
            } else {
                ButtonVariant::Secondary
            })
            .with_size(Size::XSmall)
            .into_any_element()
        })
        .collect::<Vec<_>>();

    div()
        .flex()
        .items_center()
        .justify_end()
        .gap_1()
        .min_w(px(0.0))
        .overflow_hidden()
        .children(actions)
}

fn render_repeatable_row(
    node_id: JellyNodeId,
    repeatable: NodeRepeatableSurfaceLayout,
    document_bounds: JellyRect,
    view_width: Pixels,
    view_height: Pixels,
    collector: OpenGpuiBoundsCollector,
) -> impl IntoElement {
    let rect = slot_view_rect(repeatable.rect, document_bounds, view_width, view_height);
    let key = repeatable.projection.key.clone();
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
            Badge::new(
                format!("jellyflow-repeatable:{}", repeatable.projection.key),
                format!("{} controls", repeatable.projection.controls),
            )
            .variant(BadgeVariant::Outline)
            .with_size(Size::XSmall),
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
    let anchor = repeatable.projection.anchor.clone();
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
            Badge::new(
                format!(
                    "jellyflow-repeatable-item:{}:{}",
                    repeatable.projection.collection_key, repeatable.projection.item_id
                ),
                badge,
            )
            .variant(BadgeVariant::Outline)
            .with_size(Size::XSmall),
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
            controls: "partial/local",
            repeatables: "projection",
            actions: "partial/local",
            layout_measurement: NodeSurfaceMeasurementSource::ProjectionFallback,
            layout_gap: GPUI_LAYOUT_PASS_MEASUREMENT_GAP,
        },
    };

    Ok((document, projection))
}

#[cfg(test)]
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
    let node_size = node.size.unwrap_or(JellySize {
        width: 228.0,
        height: 168.0,
    });
    let mut canvas_node = CanvasNode::new(
        canvas_node_id(id),
        point(px(node.pos.x), px(node.pos.y)),
        size(px(node_size.width), px(node_size.height)),
    );
    let descriptor = semantic_registry
        .view_descriptor(&node.kind)
        .expect("demo graph should resolve a builtin node descriptor");
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
        | NodeHandleMeasurementSource::MeasuredAnchor { .. } => {
            resolution.bounds.map(handle_position_from_bounds)
        }
        NodeHandleMeasurementSource::Fallback { .. } => Some(JellyPoint {
            x: match direction {
                PortDirection::In => 0.0,
                PortDirection::Out => node_size.width,
            },
            y: port_y(fallback_index, fallback_count, node_size.height),
        }),
    }
}

fn handle_position_from_bounds(bounds: HandleBounds) -> JellyPoint {
    match bounds.position {
        HandlePosition::Left => JellyPoint {
            x: bounds.rect.origin.x,
            y: bounds.rect.origin.y + bounds.rect.size.height * 0.5,
        },
        HandlePosition::Right => JellyPoint {
            x: bounds.rect.origin.x + bounds.rect.size.width,
            y: bounds.rect.origin.y + bounds.rect.size.height * 0.5,
        },
        HandlePosition::Top => JellyPoint {
            x: bounds.rect.origin.x + bounds.rect.size.width * 0.5,
            y: bounds.rect.origin.y,
        },
        HandlePosition::Bottom => JellyPoint {
            x: bounds.rect.origin.x + bounds.rect.size.width * 0.5,
            y: bounds.rect.origin.y + bounds.rect.size.height,
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

fn demo_state() -> (NodeGraphStore, CanvasEditor, ProjectionSummary) {
    let store = make_demo_store();
    let (document, projection) = project_store(&store).expect("demo graph should project");
    let mut editor = CanvasEditor::try_new_with_kind_registry(document, jellyflow_kind_registry())
        .expect("canvas editor should accept projected Jellyflow graph");
    editor
        .apply_tool_intent(CanvasToolIntent::ReplaceSelection(HitTarget::Node(
            NodeId::from(canvas_node_id(&JellyNodeId::from_u128(INITIAL_SELECTION))),
        )))
        .expect("initial selection should exist");
    (store, editor, projection)
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
        init_text_input(cx);

        let bounds = Bounds::centered(None, size(px(CANVAS_WIDTH), px(CANVAS_HEIGHT)), cx);
        let (store, editor, projection) = demo_state();
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
                    semantic_registry,
                    node_kit_registry,
                    measured_regions: OpenGpuiBoundsCollector::new(),
                    measurement_revision: 1,
                    measurement_frame_pending: false,
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
    use jellyflow::runtime::{
        runtime::measurement::{
            MeasuredSurfaceAnchor, NodeInternalsInvalidation, NodeInternalsInvalidationReason,
            NodeMeasurement,
        },
        schema::NodeControlKind,
    };
    use jellyflow_open_gpui::{
        plan_action_dispatch, plan_dropped_wire_insert, project_dropped_wire_menu,
        projected_node_surface_component_layout,
        testing::{
            assert_authoring_interaction_regression_gates, assert_product_fixture_regression_gates,
        },
    };
    use open_gpui_canvas::{
        CanvasConnectionEndpointRole, CanvasGeometryFacts, CanvasRuntime, connection_hit_options,
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
            OpenGpuiNodeRendererResolution::Fallback(_)
        ));
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
            NodeSurfaceMeasurementSource::ProjectionFallback
        );
        assert_eq!(projection.capability.controls, "partial/local");
        assert!(
            projection
                .capability
                .layout_gap
                .contains("element-bounds callback")
        );
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
    fn canvas_example_consumes_adapter_product_fixture_gates() {
        assert_product_fixture_regression_gates();
        assert_authoring_interaction_regression_gates();
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
        assert_eq!(completion_handle.position, point(px(268.0), px(150.0)));
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
        let expected = projection_store
            .resolve_node_handle_measurement(ConnectionHandleRef::new(
                transform,
                prompt,
                PortDirection::In,
            ))
            .bounds
            .map(handle_position_from_bounds)
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

        assign_layout_pass_revision(
            NodeMeasurementStatus::Fresh { revision: 7 },
            Some(&existing),
            &mut measurement,
            &mut next_revision,
        );

        assert_eq!(measurement.revision, 7);
        assert_eq!(next_revision, 7);

        assign_layout_pass_revision(
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
        let measured_completion_point = node.position + point(px(268.0), px(150.0));
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

    #[test]
    fn adapter_slot_limit_scales_with_available_height() {
        assert_eq!(adapter_slot_limit_for_height(px(148.0), usize::MAX), 2);
        assert_eq!(adapter_slot_limit_for_height(px(220.0), 3), 3);
        assert_eq!(adapter_slot_limit_for_height(px(88.0), 4), 0);
    }
}
