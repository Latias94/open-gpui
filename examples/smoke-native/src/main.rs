use open_gpui::{
    App, Bounds, Context, DispatchPhase, FocusHandle, Hsla, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollWheelEvent, Window, WindowBounds,
    WindowOptions, canvas, div, point, prelude::*, px, rgb, size,
};
use open_gpui_canvas::{
    CanvasClipboardPayload, CanvasDocument, CanvasEdge, CanvasEdgeRoute, CanvasEditor,
    CanvasEndpoint, CanvasEvent, CanvasHandle, CanvasInputMapper, CanvasKindRegistry, CanvasNode,
    CanvasNodeKind, CanvasPaintModel, CanvasPaintOptions, CanvasPaintTheme, CanvasSelection,
    CanvasShape, CanvasStyle, CanvasTool, CanvasToolContext, CanvasToolEffect, CanvasToolReducer,
    CanvasToolRegistry, CanvasTransaction, CanvasZOrderCommand, DocumentCommand, DocumentError,
    HandleRole, NodeId, PointerButton, collect_visible_records, paint_canvas_frame,
};
use open_gpui_platform::application;

const STAMP_TOOL_ID: &str = "stamp-node";

struct SmokeView {
    editor: CanvasEditor,
    tools: CanvasToolRegistry,
    clipboard: Option<CanvasClipboardPayload>,
    focus_handle: FocusHandle,
}

#[derive(Default)]
struct StampNodeTool {
    sequence: u64,
}

struct StampNodeKind;

impl CanvasNodeKind for StampNodeKind {
    fn node_bounds(&self, node: &CanvasNode) -> Option<Bounds<open_gpui::Pixels>> {
        Some(node.bounds().dilate(px(6.0)))
    }
}

impl CanvasToolReducer for StampNodeTool {
    fn handle_event(
        &mut self,
        context: CanvasToolContext<'_>,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        let CanvasEvent::PointerDown {
            position,
            button: PointerButton::Secondary,
            ..
        } = event
        else {
            return Ok(vec![CanvasToolEffect::SetTool(CanvasTool::Select)]);
        };

        self.sequence += 1;
        let id = NodeId::new(format!("stamp-{}", self.sequence));
        let mut node = CanvasNode::new(
            id.clone(),
            context.document_position(position),
            size(px(112.0), px(56.0)),
        );
        node.kind = "stamp".to_string();
        node.z_index = 4;
        node.style = style(Some("#ecfeff"), Some("#0891b2"), px(2.0));

        let mut selection = CanvasSelection::default();
        selection.nodes.insert(id);

        Ok(vec![
            CanvasToolEffect::ApplyTransaction(CanvasTransaction::single(
                DocumentCommand::InsertNode(node),
            )),
            CanvasToolEffect::SetSelection(selection),
            CanvasToolEffect::SetTool(CanvasTool::Select),
        ])
    }
}

impl Render for SmokeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let model = CanvasPaintModel::from(&self.editor);
        let prepaint_model = model.clone();
        let paint_model = model;
        let focus_handle = self.focus_handle.clone();
        let options = CanvasPaintOptions {
            include_handles: true,
            ..CanvasPaintOptions::default()
        };
        let theme = CanvasPaintTheme {
            background: Some(Hsla::from(rgb(0xf8fafc))),
            ..CanvasPaintTheme::default()
        };

        div()
            .size_full()
            .bg(rgb(0xf8fafc))
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                cx.stop_propagation();
                this.handle_key_down(event, cx);
            }))
            .child(
                canvas(
                    move |bounds, _, _| collect_visible_records(&prepaint_model, bounds, options),
                    move |bounds, frame, window, _cx| {
                        let mapper = CanvasInputMapper::new(bounds);

                        window.on_mouse_event({
                            let entity = entity.clone();
                            let focus_handle = focus_handle.clone();
                            move |event: &MouseDownEvent, phase, window, cx| {
                                if phase != DispatchPhase::Bubble {
                                    return;
                                }

                                window.focus(&focus_handle, cx);
                                entity.update(cx, |this, cx| {
                                    this.handle_canvas_event(mapper.mouse_down(event), cx);
                                });
                            }
                        });

                        window.on_mouse_event({
                            let entity = entity.clone();
                            move |event: &MouseMoveEvent, phase, _, cx| {
                                if phase != DispatchPhase::Bubble {
                                    return;
                                }

                                entity.update(cx, |this, cx| {
                                    let event = if this.is_pointer_interacting() {
                                        Some(CanvasEvent::PointerMove {
                                            position: event.position - mapper.bounds.origin,
                                            modifiers: CanvasInputMapper::modifiers(
                                                event.modifiers,
                                            ),
                                        })
                                    } else {
                                        mapper.mouse_move(event)
                                    };
                                    this.handle_canvas_event(event, cx);
                                });
                            }
                        });

                        window.on_mouse_event({
                            let entity = entity.clone();
                            move |event: &MouseUpEvent, phase, _, cx| {
                                if phase != DispatchPhase::Bubble {
                                    return;
                                }

                                entity.update(cx, |this, cx| {
                                    let event = if this.is_pointer_interacting() {
                                        pointer_button(event.button).map(|button| {
                                            CanvasEvent::PointerUp {
                                                position: event.position - mapper.bounds.origin,
                                                button,
                                                modifiers: CanvasInputMapper::modifiers(
                                                    event.modifiers,
                                                ),
                                            }
                                        })
                                    } else {
                                        mapper.mouse_up(event)
                                    };
                                    this.handle_canvas_event(event, cx);
                                });
                            }
                        });

                        window.on_mouse_event({
                            let entity = entity.clone();
                            move |event: &ScrollWheelEvent, phase, _, cx| {
                                if phase != DispatchPhase::Bubble {
                                    return;
                                }

                                entity.update(cx, |this, cx| {
                                    this.handle_canvas_event(mapper.scroll_wheel(event), cx);
                                });
                            }
                        });

                        paint_canvas_frame(bounds, &paint_model, &frame, theme, window);
                    },
                )
                .size_full(),
            )
    }
}

impl SmokeView {
    fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        match self.handle_canvas_shortcut(event) {
            Ok(true) => {
                cx.notify();
            }
            Ok(false) => {
                self.handle_canvas_event(Some(CanvasInputMapper::key_down_event(event)), cx);
            }
            Err(error) => {
                eprintln!("canvas shortcut failed: {error}");
            }
        }
    }

    fn handle_canvas_shortcut(&mut self, event: &KeyDownEvent) -> Result<bool, DocumentError> {
        let modifiers = event.keystroke.modifiers;
        if !(modifiers.platform || modifiers.control) {
            return Ok(false);
        }

        match event.keystroke.key.as_str() {
            "c" => {
                self.clipboard = self.editor.copy_selection();
                Ok(true)
            }
            "x" => {
                self.clipboard = self.editor.cut_selection()?;
                Ok(true)
            }
            "v" => {
                if let Some(payload) = self.clipboard.clone() {
                    self.editor
                        .paste_clipboard(&payload, point(px(24.0), px(24.0)))?;
                }
                Ok(true)
            }
            "d" => {
                self.editor.duplicate_selection(point(px(24.0), px(24.0)))?;
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
            "]" if modifiers.shift => {
                self.editor
                    .reorder_selection(CanvasZOrderCommand::BringToFront)?;
                Ok(true)
            }
            "]" => {
                self.editor
                    .reorder_selection(CanvasZOrderCommand::BringForward)?;
                Ok(true)
            }
            "[" if modifiers.shift => {
                self.editor
                    .reorder_selection(CanvasZOrderCommand::SendToBack)?;
                Ok(true)
            }
            "[" => {
                self.editor
                    .reorder_selection(CanvasZOrderCommand::SendBackward)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn handle_canvas_event(&mut self, event: Option<CanvasEvent>, cx: &mut Context<Self>) {
        let Some(event) = event else {
            return;
        };

        if matches!(
            event,
            CanvasEvent::PointerDown {
                button: PointerButton::Secondary,
                ..
            }
        ) {
            self.editor.set_tool(CanvasTool::custom(STAMP_TOOL_ID));
        }

        if let Err(error) = self
            .editor
            .handle_event_with_tool_registry(event, &mut self.tools)
        {
            eprintln!("canvas event failed: {error}");
        }
        cx.notify();
    }

    fn is_pointer_interacting(&self) -> bool {
        !self.editor.is_tool_state_idle()
    }
}

fn demo_tools() -> CanvasToolRegistry {
    let mut registry = CanvasToolRegistry::new();
    registry.insert(STAMP_TOOL_ID, StampNodeTool::default());
    registry
}

fn demo_kind_registry() -> CanvasKindRegistry {
    let mut registry = CanvasKindRegistry::open();
    registry.register_node_kind("stamp", StampNodeKind);
    registry
}

fn demo_document() -> CanvasDocument {
    let mut document = CanvasDocument::default();

    let mut frame = CanvasShape::new(
        "frame",
        Bounds::new(point(px(36.0), px(36.0)), size(px(660.0), px(400.0))),
    );
    frame.z_index = -10;
    frame.style = style(Some("#eef6ff"), Some("#d0d7de"), px(1.0));
    document.insert_shape(frame).unwrap();

    let mut source = node("source", px(72.0), px(116.0), px(180.0), px(104.0));
    source.style = style(Some("#ffffff"), Some("#0969da"), px(2.0));
    source
        .handles
        .push(source_handle("out", px(180.0), px(52.0)));
    document.insert_node(source).unwrap();

    let mut branch = node("branch", px(376.0), px(84.0), px(184.0), px(96.0));
    branch.style = style(Some("#fff7ed"), Some("#ea580c"), px(2.0));
    branch.handles.push(target_handle("in", px(0.0), px(48.0)));
    branch
        .handles
        .push(source_handle("out", px(184.0), px(48.0)));
    document.insert_node(branch).unwrap();

    let mut note = node("note", px(376.0), px(264.0), px(184.0), px(96.0));
    note.style = style(Some("#f0fdf4"), Some("#16a34a"), px(2.0));
    note.handles.push(target_handle("in", px(0.0), px(48.0)));
    document.insert_node(note).unwrap();

    let mut first_edge = CanvasEdge::new(
        "source-branch",
        CanvasEndpoint::new("source", Some("out")),
        CanvasEndpoint::new("branch", Some("in")),
    );
    first_edge.z_index = 2;
    first_edge.style = style(None, Some("#0969da"), px(2.0));
    first_edge.route = CanvasEdgeRoute::polyline([point(px(316.0), px(132.0))]);
    document.insert_edge(first_edge).unwrap();

    let mut second_edge = CanvasEdge::new(
        "branch-note",
        CanvasEndpoint::new("branch", Some("out")),
        CanvasEndpoint::new("note", Some("in")),
    );
    second_edge.z_index = 2;
    second_edge.style = style(None, Some("#16a34a"), px(2.0));
    second_edge.route =
        CanvasEdgeRoute::polyline([point(px(624.0), px(132.0)), point(px(624.0), px(312.0))]);
    document.insert_edge(second_edge).unwrap();

    document
}

fn node(
    id: &'static str,
    x: open_gpui::Pixels,
    y: open_gpui::Pixels,
    width: open_gpui::Pixels,
    height: open_gpui::Pixels,
) -> CanvasNode {
    let mut node = CanvasNode::new(id, point(x, y), size(width, height));
    node.z_index = 1;
    node
}

fn source_handle(id: &'static str, x: open_gpui::Pixels, y: open_gpui::Pixels) -> CanvasHandle {
    let mut handle = CanvasHandle::new(id, point(x, y));
    handle.role = HandleRole::Source;
    handle
}

fn target_handle(id: &'static str, x: open_gpui::Pixels, y: open_gpui::Pixels) -> CanvasHandle {
    let mut handle = CanvasHandle::new(id, point(x, y));
    handle.role = HandleRole::Target;
    handle
}

fn style(fill: Option<&str>, stroke: Option<&str>, stroke_width: open_gpui::Pixels) -> CanvasStyle {
    CanvasStyle {
        fill: fill.map(str::to_string),
        stroke: stroke.map(str::to_string),
        stroke_width,
    }
}

fn pointer_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        MouseButton::Navigate(_) => None,
    }
}

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(760.0), px(520.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|cx| SmokeView {
                    editor: CanvasEditor::try_new_with_kind_registry(
                        demo_document(),
                        demo_kind_registry(),
                    )
                    .expect("failed to create canvas editor"),
                    tools: demo_tools(),
                    clipboard: None,
                    focus_handle: cx.focus_handle(),
                })
            },
        )
        .expect("failed to open native smoke window");

        cx.activate(true);
    });
}
