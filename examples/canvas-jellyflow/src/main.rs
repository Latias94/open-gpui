use jellyflow::{
    NodeGraphStore,
    core::{
        CanvasPoint as JellyPoint, CanvasSize as JellySize, Edge, EdgeId as JellyEdgeId, EdgeKind,
        Graph, GraphId, GraphOp, GraphTransaction, Node, NodeId as JellyNodeId, NodeKindKey, Port,
        PortCapacity, PortDirection, PortId as JellyPortId, PortKey, PortKind,
    },
    layout::{LayoutPresetBuilder, builtin_layout_engine_registry},
    runtime::{
        io::{NodeGraphEditorConfig, NodeGraphViewState},
        schema::{
            NodeKitRegistry, NodeRegistry, NodeSurfaceProjection, NodeSurfaceSlotKind,
            NodeSurfaceSlotProjection,
        },
    },
};
use open_gpui::{
    AnyElement, App, Bounds, Context, FocusHandle, Hsla, KeyDownEvent, Pixels, Window,
    WindowBounds, WindowOptions, div, point, prelude::*, px, rgb, size,
};
use open_gpui_canvas::{
    CanvasDocument, CanvasEditor, CanvasEditorInputHandler, CanvasEvent, CanvasHandle,
    CanvasKindLabel, CanvasKindPaint, CanvasKindRegistry, CanvasNode, CanvasNodeKind,
    CanvasNodeRenderPolicy, CanvasPaintModel, CanvasPaintOptions, CanvasPaintTheme,
    CanvasToolIntent, DocumentError, HandleRole, HitTarget, NodeId, canvas_editor_view,
};
use open_gpui_platform::application;
use serde_json::Value;

const INITIAL_SELECTION: u128 = 2;
const CANVAS_WIDTH: f32 = 1140.0;
const CANVAS_HEIGHT: f32 = 650.0;
const NODE_SURFACE_CHROME_HEIGHT: f32 = 92.0;
const NODE_SURFACE_SLOT_ROW_HEIGHT: f32 = 28.0;

struct JellyflowCanvasView {
    editor: CanvasEditor,
    focus_handle: FocusHandle,
    projection: ProjectionSummary,
    semantic_registry: NodeRegistry,
    node_kit_registry: NodeKitRegistry,
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
}

#[derive(Clone)]
struct NodeSurfaceSummary {
    node_kind: String,
    renderer_key: String,
    title: String,
    summary: String,
    slots: Vec<NodeSurfaceSlotProjection>,
    selected: bool,
    zoom: f32,
    projection: NodeSurfaceProjection,
}

struct NodeSummary {
    id: String,
    kind: String,
    title: String,
    detail: String,
    ports: String,
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                            .children(self.render_node_surfaces(&render_model)),
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
                            .child("Jellyflow gpui proof"),
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

    fn render_sidebar(&self, selected: Option<NodeSummary>) -> impl IntoElement {
        let selection = match selected {
            Some(summary) => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x64748b))
                        .child(summary.kind),
                )
                .child(
                    div()
                        .text_lg()
                        .line_height(px(22.0))
                        .text_color(rgb(0x111827))
                        .child(summary.title),
                )
                .child(
                    div()
                        .text_sm()
                        .line_height(px(20.0))
                        .text_color(rgb(0x334155))
                        .child(summary.detail),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x64748b))
                        .child(summary.ports),
                )
                .child(div().text_xs().text_color(rgb(0x64748b)).child(summary.id)),
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
            .child(selection)
    }

    fn render_node_surfaces(&self, model: &CanvasPaintModel) -> Vec<AnyElement> {
        let zoom = model.viewport().zoom;
        self.editor
            .document()
            .nodes()
            .filter_map(|node| {
                let surface = self.node_surface_summary(node, zoom)?;
                Some(
                    render_node_surface(
                        model.viewport().document_bounds_to_view(node.bounds()),
                        surface,
                    )
                    .into_any_element(),
                )
            })
            .collect()
    }

    fn node_surface_summary(&self, node: &CanvasNode, zoom: f32) -> Option<NodeSurfaceSummary> {
        let kind =
            NodeKindKey::new(data_string(node, "jellyflow_kind").unwrap_or(node.kind.as_str()));
        let descriptor = self.semantic_registry.view_descriptor(&kind)?;
        let title = node_title(node);
        let data = Value::Object(node.data.clone());
        let layout_hints = self.node_kit_registry.layout_hints_for_kind(&kind)?;
        let projection = NodeSurfaceProjection::from_layout_hints(layout_hints, zoom);
        let slots = descriptor.surface_slots_projection(&data, Some(layout_hints), zoom);
        let summary = data_string(node, "summary")
            .or_else(|| data_string(node, "description"))
            .unwrap_or("Jellyflow node projected into open-gpui-canvas")
            .to_string();
        Some(NodeSurfaceSummary {
            node_kind: descriptor.kind.0.clone(),
            renderer_key: descriptor.renderer_key.clone(),
            title,
            summary,
            slots,
            selected: self
                .editor
                .selection()
                .contains_node(&NodeId::from(node.id.as_str())),
            zoom,
            projection,
        })
    }

    fn selected_node_summary(&self) -> Option<NodeSummary> {
        let id = self.editor.selection().selected_nodes().next()?;
        let node = self.editor.document().node(id)?;
        Some(NodeSummary {
            id: node.id.as_str().to_string(),
            kind: data_string(node, "jellyflow_kind")
                .unwrap_or(node.kind.as_str())
                .to_string(),
            title: node_title(node),
            detail: data_string(node, "description")
                .unwrap_or("Jellyflow node projected into open-gpui-canvas")
                .to_string(),
            ports: format!("ports: {}", data_string(node, "ports").unwrap_or("none")),
        })
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
                // CanvasEditor currently manages clipboard, but this proof stays read-only.
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

    fn is_pointer_interacting(&self) -> bool {
        !self.editor.is_tool_state_idle()
    }
}

fn render_node_surface(bounds: Bounds<Pixels>, surface: NodeSurfaceSummary) -> impl IntoElement {
    let zoom = surface.zoom;
    let pad = if zoom >= 1.0 { px(10.0) } else { px(8.0) };
    let top = bounds.top() + pad;
    let left = bounds.left() + pad;
    let inner_width = (bounds.size.width - pad * 2.0).max(px(0.0));
    let inner_height = (bounds.size.height - pad * 2.0).max(px(0.0));
    let slot_limit = adapter_slot_limit_for_height(inner_height, surface.projection.slot_limit);
    let accent = if surface.selected {
        rgb(0x2563eb)
    } else {
        rgb(0x475569)
    };

    div()
        .absolute()
        .left(left)
        .top(top)
        .w(inner_width)
        .h(inner_height)
        .flex()
        .flex_col()
        .gap_1()
        .flex_shrink_0()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .p_2()
        .rounded_sm()
        .border_1()
        .border_color(accent)
        .bg(rgb(0xffffff))
        .overflow_hidden()
        .shadow_sm()
        .child(
            div()
                .flex()
                .flex_shrink_0()
                .items_center()
                .justify_between()
                .gap_2()
                .min_w(px(0.0))
                .child(div().text_xs().text_color(accent).child(surface.node_kind))
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x94a3b8))
                        .truncate()
                        .min_w(px(0.0))
                        .child(surface.renderer_key),
                ),
        )
        .child(
            div()
                .text_sm()
                .line_height(px(20.0))
                .text_color(rgb(0x111827))
                .overflow_hidden()
                .line_clamp(2)
                .flex_shrink_1()
                .min_w(px(0.0))
                .child(surface.title),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x64748b))
                .truncate()
                .flex_shrink_1()
                .min_w(px(0.0))
                .child(surface.summary),
        )
        .children(render_surface_slots(surface.slots, slot_limit))
}

fn render_surface_slots(
    slots: Vec<NodeSurfaceSlotProjection>,
    slot_limit: usize,
) -> Vec<AnyElement> {
    slots
        .into_iter()
        .filter(|slot| slot.visible)
        .take(slot_limit)
        .map(render_node_slot)
        .map(|slot| slot.into_any_element())
        .collect()
}

fn render_node_slot(slot: NodeSurfaceSlotProjection) -> impl IntoElement {
    let fill = match slot.kind {
        NodeSurfaceSlotKind::Header => rgb(0xe0f2fe),
        NodeSurfaceSlotKind::Body => rgb(0xf1f5f9),
        NodeSurfaceSlotKind::Footer => rgb(0xe2e8f0),
        NodeSurfaceSlotKind::Badge => rgb(0xfef3c7),
        NodeSurfaceSlotKind::Icon => rgb(0xe0e7ff),
        NodeSurfaceSlotKind::FieldRow => rgb(0xecfeff),
        NodeSurfaceSlotKind::ActionRow => rgb(0xfce7f3),
        NodeSurfaceSlotKind::Preview => rgb(0xd1fae5),
        NodeSurfaceSlotKind::NestedRegion => rgb(0xf3e8ff),
    };
    let value = if slot.value.is_empty() {
        "-".to_string()
    } else {
        slot.value
    };
    let label = if slot.label.is_empty() {
        "slot".to_string()
    } else {
        slot.label
    };
    let status = if slot.visible { "visible" } else { "hidden" };

    div()
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
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x334155))
                .truncate()
                .flex_shrink_1()
                .min_w(px(0.0))
                .child(format!("{} · {}", label, status)),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x475569))
                .truncate()
                .flex_shrink_1()
                .min_w(px(0.0))
                .child(value),
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
            port: make_port(source, "rows", PortDirection::Out),
        },
        GraphOp::AddPort {
            id: transform_in,
            port: make_port(transform, "rows", PortDirection::In),
        },
        GraphOp::AddPort {
            id: transform_out,
            port: make_port(transform, "orders", PortDirection::Out),
        },
        GraphOp::AddPort {
            id: sink_in,
            port: make_port(sink, "orders", PortDirection::In),
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
            width: 228.0,
            height: 168.0,
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
    let mut builder = CanvasDocument::builder();

    for (id, node) in graph.nodes().iter() {
        builder.add_node(project_node(id, node, graph, &semantic_registry))?;
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
        canvas_edge.route = open_gpui_canvas::CanvasEdgeRoute::orthogonal();
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
        last_commit: "node-kit gpui proof now uses builtin semantic overlays".to_string(),
        source: "jellyflow graph v1".to_string(),
        adapter: "open-gpui-canvas overlay example".to_string(),
        kit: "workflow.automation / erd.table / mind-map.knowledge-canvas".to_string(),
    };

    Ok((document, projection))
}

fn project_node(
    id: &JellyNodeId,
    node: &Node,
    graph: &Graph,
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
        canvas_node.handles.push(project_handle(
            *port_id,
            HandleRole::Target,
            0.0,
            port_y(index, input_ports.len(), node_size.height),
            graph.ports().get(port_id).and_then(|port| port.connectable),
        ));
    }

    for (index, port_id) in output_ports.iter().enumerate() {
        canvas_node.handles.push(project_handle(
            *port_id,
            HandleRole::Source,
            node_size.width,
            port_y(index, output_ports.len(), node_size.height),
            graph.ports().get(port_id).and_then(|port| port.connectable),
        ));
    }

    canvas_node
}

fn jellyflow_kind_registry() -> CanvasKindRegistry {
    let mut registry = CanvasKindRegistry::open();
    for kind in [
        "data-card",
        "task-card",
        "decision-card",
        "output-card",
        "table-card",
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

fn demo_editor() -> (CanvasEditor, ProjectionSummary) {
    let store = make_demo_store();
    let (document, projection) = project_store(&store).expect("demo graph should project");
    let mut editor = CanvasEditor::try_new_with_kind_registry(document, jellyflow_kind_registry())
        .expect("canvas editor should accept projected Jellyflow graph");
    editor
        .apply_tool_intent(CanvasToolIntent::ReplaceSelection(HitTarget::Node(
            NodeId::from(canvas_node_id(&JellyNodeId::from_u128(INITIAL_SELECTION))),
        )))
        .expect("initial selection should exist");
    (editor, projection)
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
        let bounds = Bounds::centered(None, size(px(CANVAS_WIDTH), px(CANVAS_HEIGHT)), cx);
        let (editor, projection) = demo_editor();
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
                    focus_handle: cx.focus_handle(),
                    projection,
                    semantic_registry,
                    node_kit_registry,
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
    fn adapter_slot_limit_scales_with_available_height() {
        assert_eq!(adapter_slot_limit_for_height(px(148.0), usize::MAX), 2);
        assert_eq!(adapter_slot_limit_for_height(px(220.0), 3), 3);
        assert_eq!(adapter_slot_limit_for_height(px(88.0), 4), 0);
    }
}
