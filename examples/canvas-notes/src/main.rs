use open_gpui::{
    AnyElement, App, Bounds, Context, DispatchPhase, FocusHandle, Hsla, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, ScrollWheelEvent, Window, WindowBounds,
    WindowOptions, canvas, div, point, prelude::*, px, rgb, size,
};
use open_gpui_canvas::{
    CanvasClipboardPayload, CanvasEditor, CanvasEvent, CanvasInputMapper, CanvasKindLabel,
    CanvasKindPaint, CanvasKindRegistry, CanvasNode, CanvasNodeKind, CanvasNodeResizeProposal,
    CanvasPaintModel, CanvasPaintOptions, CanvasPaintTheme, CanvasRecordKind, CanvasSchemaError,
    CanvasToolEffect, CanvasWidgetOverlayFrame, CanvasWidgetOverlayHitPriority,
    CanvasWidgetOverlayOptions, CanvasZOrderCommand, DocumentError, HitTarget, NodeId,
    PointerButton, document_from_json_canvas_str, paint_canvas_frame, prepaint_canvas_frame,
};
use open_gpui_platform::application;

const SAMPLE_CANVAS: &str = include_str!("../assets/sample.canvas");

struct NotesView {
    editor: CanvasEditor,
    clipboard: Option<CanvasClipboardPayload>,
    focus_handle: FocusHandle,
    overlay_frame: CanvasWidgetOverlayFrame,
}

#[derive(Clone, Copy)]
enum NoteKind {
    Text,
    File,
    Link,
    Group,
}

struct JsonCanvasNodeKind {
    kind: NoteKind,
}

struct NodeSummary {
    id: String,
    kind: String,
    title: String,
    detail: String,
}

impl CanvasNodeKind for JsonCanvasNodeKind {
    fn validate_node(&self, node: &CanvasNode) -> Result<(), CanvasSchemaError> {
        match self.kind {
            NoteKind::Text => require_string(node, "text"),
            NoteKind::File => require_string(node, "file"),
            NoteKind::Link => require_string(node, "url"),
            NoteKind::Group => Ok(()),
        }
    }

    fn node_paint(&self, _node: &CanvasNode) -> Option<CanvasKindPaint> {
        let (fill, stroke, stroke_width, corner_radius) = match self.kind {
            NoteKind::Text => ("#fff7ed", "#f59e0b", px(1.5), px(7.0)),
            NoteKind::File => ("#ecfeff", "#0891b2", px(1.5), px(7.0)),
            NoteKind::Link => ("#f0fdf4", "#16a34a", px(1.5), px(7.0)),
            NoteKind::Group => ("#eef2ff", "#6366f1", px(1.0), px(6.0)),
        };

        Some(CanvasKindPaint {
            fill: Some(fill.to_string()),
            stroke: Some(stroke.to_string()),
            stroke_width: Some(stroke_width),
            corner_radius: Some(corner_radius),
        })
    }

    fn node_label(&self, node: &CanvasNode) -> Option<CanvasKindLabel> {
        let text = match self.kind {
            NoteKind::Text => data_string(node, "text"),
            NoteKind::File => data_string(node, "label").or_else(|| data_string(node, "file")),
            NoteKind::Link => data_string(node, "label").or_else(|| data_string(node, "url")),
            NoteKind::Group => data_string(node, "label"),
        }?;

        let color = match self.kind {
            NoteKind::Group => "#3730a3",
            _ => "#1f2937",
        };

        Some(
            CanvasKindLabel::new(text)
                .with_inset(px(12.0))
                .with_color(color),
        )
    }

    fn resize_node_bounds(
        &self,
        proposal: CanvasNodeResizeProposal<'_>,
    ) -> Result<Bounds<Pixels>, CanvasSchemaError> {
        let min_size = match self.kind {
            NoteKind::Group => size(px(280.0), px(180.0)),
            _ => size(px(136.0), px(80.0)),
        };
        Ok(Bounds::new(
            proposal.bounds.origin,
            size(
                proposal.bounds.size.width.max(min_size.width),
                proposal.bounds.size.height.max(min_size.height),
            ),
        ))
    }
}

impl Render for NotesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let model = CanvasPaintModel::from(&self.editor);
        let prepaint_model = model.clone();
        let paint_model = model;
        let focus_handle = self.focus_handle.clone();
        let overlay_surfaces = self.render_overlay_surfaces();
        let selected = self.selected_node_summary();
        let overlay_count = self.overlay_frame.len();
        let options = CanvasPaintOptions {
            include_handles: true,
            ..CanvasPaintOptions::default()
        };
        let theme = CanvasPaintTheme {
            background: Some(Hsla::from(rgb(0xf8fafc))),
            label_line_clamp: Some(4),
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
                    .child(self.render_toolbar(overlay_count))
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .overflow_hidden()
                            .child(
                                canvas(
                                    move |bounds, window, _| {
                                        prepaint_canvas_frame(
                                            &prepaint_model,
                                            bounds,
                                            options,
                                            theme,
                                            window,
                                        )
                                    },
                                    move |bounds, frame, window, cx| {
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
                                                    this.handle_canvas_event(
                                                        mapper.mouse_down(event),
                                                        cx,
                                                    );
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
                                                            position: event.position
                                                                - mapper.bounds.origin,
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
                                                                position: event.position
                                                                    - mapper.bounds.origin,
                                                                button,
                                                                modifiers:
                                                                    CanvasInputMapper::modifiers(
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
                                                    this.handle_canvas_event(
                                                        mapper.scroll_wheel(event),
                                                        cx,
                                                    );
                                                });
                                            }
                                        });

                                        let overlay_frame = frame.widget_overlay_frame(
                                            CanvasWidgetOverlayOptions::selected_nodes()
                                                .with_hit_priority(
                                                    CanvasWidgetOverlayHitPriority::WidgetFirst,
                                                ),
                                        );
                                        entity.update(cx, |this, cx| {
                                            if this.overlay_frame != overlay_frame {
                                                this.overlay_frame = overlay_frame;
                                                cx.notify();
                                            }
                                        });

                                        paint_canvas_frame(
                                            bounds,
                                            &paint_model,
                                            &frame,
                                            theme,
                                            window,
                                            cx,
                                        );
                                    },
                                )
                                .size_full(),
                            )
                            .children(overlay_surfaces),
                    ),
            )
            .child(self.render_sidebar(selected))
    }
}

impl NotesView {
    fn render_toolbar(&self, overlay_count: usize) -> impl IntoElement {
        let document = self.editor.document();
        div()
            .h(px(44.0))
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
                            .child("Canvas Notes"),
                    )
                    .child(div().text_xs().text_color(rgb(0x64748b)).child(format!(
                        "{} nodes / {} edges",
                        document.nodes.len(),
                        document.edges.len()
                    ))),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x475569))
                    .child(format!("{} active overlay", overlay_count)),
            )
    }

    fn render_sidebar(&self, selected: Option<NodeSummary>) -> impl IntoElement {
        let content = match selected {
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
                .child(div().text_xs().text_color(rgb(0x64748b)).child(summary.id)),
            None => div().flex().flex_col().gap_3().child(
                div()
                    .text_sm()
                    .text_color(rgb(0x64748b))
                    .child("No record selected"),
            ),
        };

        div()
            .w(px(300.0))
            .h_full()
            .flex_none()
            .border_l_1()
            .border_color(rgb(0xdbe3ea))
            .bg(rgb(0xffffff))
            .p_4()
            .child(content)
    }

    fn render_overlay_surfaces(&self) -> Vec<AnyElement> {
        self.overlay_frame
            .placements
            .iter()
            .filter_map(|placement| {
                let HitTarget::Node(id) = &placement.target else {
                    return None;
                };
                let node = self.editor.document().nodes.get(id)?;
                let title = compact_title(&node_title(node), 48);
                let left = placement.view_bounds.left() + px(10.0);
                let top = placement.view_bounds.bottom() - px(34.0);
                let width = overlay_width(placement.view_bounds.size.width);

                Some(
                    div()
                        .absolute()
                        .left(left)
                        .top(top)
                        .w(width)
                        .h(px(26.0))
                        .flex()
                        .items_center()
                        .px_2()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0xcbd5e1))
                        .bg(rgb(0xffffff))
                        .shadow_sm()
                        .text_xs()
                        .text_color(rgb(0x334155))
                        .truncate()
                        .child(title)
                        .into_any_element(),
                )
            })
            .collect()
    }

    fn selected_node_summary(&self) -> Option<NodeSummary> {
        let id = self.editor.selection().nodes.iter().next()?;
        let node = self.editor.document().nodes.get(id)?;
        let title = compact_title(&node_title(node), 80);
        Some(NodeSummary {
            id: node.id.as_str().to_string(),
            kind: node.kind.clone(),
            title,
            detail: node_detail(node),
        })
    }

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

        if let Err(error) = self.editor.handle_event(event) {
            eprintln!("canvas event failed: {error}");
        }
        cx.notify();
    }

    fn is_pointer_interacting(&self) -> bool {
        !self.editor.is_tool_state_idle()
    }
}

fn note_kind_registry() -> CanvasKindRegistry {
    let mut registry = CanvasKindRegistry::open();
    registry.register_node_kind(
        "text",
        JsonCanvasNodeKind {
            kind: NoteKind::Text,
        },
    );
    registry.register_node_kind(
        "file",
        JsonCanvasNodeKind {
            kind: NoteKind::File,
        },
    );
    registry.register_node_kind(
        "link",
        JsonCanvasNodeKind {
            kind: NoteKind::Link,
        },
    );
    registry.register_node_kind(
        "group",
        JsonCanvasNodeKind {
            kind: NoteKind::Group,
        },
    );
    registry
}

fn demo_editor() -> CanvasEditor {
    let document =
        document_from_json_canvas_str(SAMPLE_CANVAS).expect("failed to parse sample canvas");
    let mut editor = CanvasEditor::try_new_with_kind_registry(document, note_kind_registry())
        .expect("failed to create notes canvas editor");
    editor
        .apply_tool_effect(CanvasToolEffect::ReplaceSelection(HitTarget::Node(
            NodeId::from("research-question"),
        )))
        .expect("failed to select initial note");
    editor
}

fn require_string(node: &CanvasNode, field: &'static str) -> Result<(), CanvasSchemaError> {
    data_string(node, field).map(drop).ok_or_else(|| {
        CanvasSchemaError::missing_required_data(
            CanvasRecordKind::Node,
            node.id.clone(),
            &node.kind,
            field,
        )
    })
}

fn data_string<'a>(node: &'a CanvasNode, field: &str) -> Option<&'a str> {
    node.data.get(field).and_then(|value| value.as_str())
}

fn node_title(node: &CanvasNode) -> String {
    data_string(node, "label")
        .or_else(|| data_string(node, "text"))
        .or_else(|| data_string(node, "file"))
        .or_else(|| data_string(node, "url"))
        .unwrap_or_else(|| node.id.as_str())
        .to_string()
}

fn node_detail(node: &CanvasNode) -> String {
    match node.kind.as_str() {
        "file" => {
            let file = data_string(node, "file").unwrap_or(node.id.as_str());
            let subpath = data_string(node, "subpath").unwrap_or("");
            format!("{file}{subpath}")
        }
        "link" => data_string(node, "url")
            .unwrap_or(node.id.as_str())
            .to_string(),
        "group" => data_string(node, "purpose").unwrap_or("group").to_string(),
        _ => data_string(node, "text")
            .unwrap_or(node.id.as_str())
            .to_string(),
    }
}

fn compact_title(input: &str, max_chars: usize) -> String {
    let normalized = input
        .lines()
        .find_map(|line| {
            let line = line.trim();
            (!line.is_empty()).then_some(line)
        })
        .unwrap_or(input.trim());

    if normalized.chars().count() <= max_chars {
        return normalized.to_string();
    }

    let mut output = normalized
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    output.push_str("...");
    output
}

fn overlay_width(width: Pixels) -> Pixels {
    let width = width - px(20.0);
    if width > px(80.0) { width } else { px(80.0) }
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
        let bounds = Bounds::centered(None, size(px(1120.0), px(640.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|cx| NotesView {
                    editor: demo_editor(),
                    clipboard: None,
                    focus_handle: cx.focus_handle(),
                    overlay_frame: CanvasWidgetOverlayFrame::default(),
                })
            },
        )
        .expect("failed to open canvas notes window");

        cx.activate(true);
    });
}
