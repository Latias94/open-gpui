use super::*;

trait BuiltInCanvasToolReducer {
    fn handle_event(
        &self,
        editor: &CanvasEditor,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BuiltInCanvasTool {
    Select,
    Pan,
    Connect,
}

impl BuiltInCanvasTool {
    pub(super) fn from_canvas_tool(tool: &CanvasTool) -> Option<Self> {
        match tool {
            CanvasTool::Select => Some(Self::Select),
            CanvasTool::Pan => Some(Self::Pan),
            CanvasTool::Connect => Some(Self::Connect),
            CanvasTool::Custom(_) => None,
        }
    }

    pub(super) fn handle_event(
        self,
        editor: &CanvasEditor,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        match self {
            Self::Select => SelectToolStateMachine.handle_event(editor, event),
            Self::Pan => PanToolStateMachine.handle_event(editor, event),
            Self::Connect => ConnectToolStateMachine.handle_event(editor, event),
        }
    }
}

struct SelectToolStateMachine;

impl BuiltInCanvasToolReducer for SelectToolStateMachine {
    fn handle_event(
        &self,
        editor: &CanvasEditor,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        let effects = match (&editor.state, event) {
            (
                ToolState::Idle,
                CanvasEvent::KeyDown {
                    key: CanvasKey::Delete | CanvasKey::Backspace,
                    ..
                },
            ) => {
                let transaction = editor.delete_selection_transaction();
                if transaction.is_empty() {
                    Vec::new()
                } else {
                    vec![CanvasToolEffect::ApplyTransaction(transaction)]
                }
            }
            (ToolState::Idle, CanvasEvent::Cancel) => {
                if editor.selection.is_empty() {
                    Vec::new()
                } else {
                    vec![CanvasToolEffect::ClearSelection]
                }
            }
            (
                ToolState::Idle,
                CanvasEvent::PointerDown {
                    position,
                    button: PointerButton::Primary,
                    modifiers,
                    ..
                },
            ) => {
                let document_position = editor.viewport.view_to_document(position);
                if let Some(handle) = editor.transform_handle_at(document_position) {
                    let (node_ids, shape_ids) = editor.resizable_selection_ids();
                    return Ok(vec![
                        CanvasToolEffect::BeginGesture,
                        CanvasToolEffect::SetState(ToolState::Resizing {
                            origin: document_position,
                            last: document_position,
                            handle: handle.handle,
                            node_ids,
                            shape_ids,
                            snap_guides: Vec::new(),
                        }),
                    ]);
                }

                let hit = editor
                    .runtime
                    .precise_hit_test_with_kind_registry(
                        editor.document.as_ref(),
                        editor.kind_registry.as_ref(),
                        document_position,
                        HitOptions::default(),
                    )
                    .map(|record| record.target.clone())
                    .next();

                if modifiers.shift
                    && let Some(target) = hit
                {
                    return Ok(vec![CanvasToolEffect::ToggleSelection(target)]);
                }

                match hit {
                    Some(target @ (HitTarget::Node(_) | HitTarget::Shape(_))) => {
                        let mut selection = editor.selection.clone();
                        if !selection.contains_target(&target) {
                            selection.replace_with(target);
                        }
                        let (node_ids, shape_ids) = editor.translatable_selection_ids(&selection);
                        vec![
                            CanvasToolEffect::BeginGesture,
                            CanvasToolEffect::SetSelection(selection),
                            CanvasToolEffect::SetState(ToolState::Translating {
                                origin: document_position,
                                last: document_position,
                                constraint_axis: None,
                                node_ids,
                                shape_ids,
                                snap_guides: Vec::new(),
                            }),
                        ]
                    }
                    Some(target) => {
                        vec![
                            CanvasToolEffect::ReplaceSelection(target),
                            CanvasToolEffect::SetState(ToolState::Pointing {
                                origin: document_position,
                                selection_mode: CanvasSelectionMode::Replace,
                                base_selection: editor.selection.clone(),
                            }),
                        ]
                    }
                    None => {
                        let selection_mode = if modifiers.shift {
                            CanvasSelectionMode::Add
                        } else {
                            CanvasSelectionMode::Replace
                        };
                        let mut effects = Vec::new();
                        if !modifiers.shift {
                            effects.push(CanvasToolEffect::ClearSelection);
                        }
                        effects.push(CanvasToolEffect::SetState(ToolState::Pointing {
                            origin: document_position,
                            selection_mode,
                            base_selection: editor.selection.clone(),
                        }));
                        effects
                    }
                }
            }
            (
                ToolState::Translating {
                    last,
                    node_ids,
                    shape_ids,
                    origin,
                    constraint_axis,
                    ..
                },
                CanvasEvent::PointerMove {
                    position,
                    modifiers,
                },
            ) => {
                let document_position = editor.viewport.view_to_document(position);
                let origin = *origin;
                let constraint_axis = if modifiers.shift {
                    Some(
                        constraint_axis
                            .unwrap_or_else(|| drag_constraint_axis(document_position - origin)),
                    )
                } else {
                    None
                };
                let document_position = constraint_axis
                    .map(|axis| constrained_drag_position(origin, document_position, axis))
                    .unwrap_or(document_position);
                let raw_delta = document_position - *last;
                let snap = editor.snap_delta_for_translation(raw_delta, node_ids, shape_ids);
                let delta = snap.delta;
                let mut commands = Vec::new();
                for id in node_ids {
                    let mut node = editor
                        .document
                        .node(id)
                        .ok_or_else(|| DocumentError::MissingNode(id.clone()))?
                        .clone();
                    node.position += delta;
                    commands.push(DocumentCommand::UpdateNode(node));
                }
                for id in shape_ids {
                    let mut shape = editor
                        .document
                        .shape(id)
                        .ok_or_else(|| DocumentError::MissingShape(id.clone()))?
                        .clone();
                    shape.bounds.origin += delta;
                    commands.push(DocumentCommand::UpdateShape(shape));
                }

                vec![
                    CanvasToolEffect::UpdateGesture(CanvasTransaction::new(commands)),
                    CanvasToolEffect::SetState(ToolState::Translating {
                        origin,
                        last: *last + delta,
                        constraint_axis,
                        node_ids: node_ids.clone(),
                        shape_ids: shape_ids.clone(),
                        snap_guides: snap.guides,
                    }),
                ]
            }
            (
                ToolState::Resizing {
                    last,
                    handle,
                    node_ids,
                    shape_ids,
                    origin,
                    ..
                },
                CanvasEvent::PointerMove { position, .. },
            ) => {
                let document_position = editor.viewport.view_to_document(position);
                let raw_delta = document_position - *last;
                let snap = editor.snap_delta_for_resize(*handle, raw_delta, node_ids, shape_ids);
                let delta = snap.delta;
                let transaction =
                    editor.resize_selection_transaction(*handle, delta, node_ids, shape_ids)?;

                vec![
                    CanvasToolEffect::UpdateGesture(transaction),
                    CanvasToolEffect::SetState(ToolState::Resizing {
                        origin: *origin,
                        last: *last + delta,
                        handle: *handle,
                        node_ids: node_ids.clone(),
                        shape_ids: shape_ids.clone(),
                        snap_guides: snap.guides,
                    }),
                ]
            }
            (
                ToolState::Pointing {
                    origin,
                    selection_mode,
                    base_selection,
                },
                CanvasEvent::PointerMove { position, .. },
            ) => {
                let origin = *origin;
                let document_position = editor.viewport.view_to_document(position);
                let bounds = selection_bounds(origin, document_position);
                vec![
                    CanvasToolEffect::SetSelection(editor.selection_for_intersections_with_mode(
                        bounds,
                        *selection_mode,
                        base_selection,
                    )),
                    CanvasToolEffect::SetState(ToolState::Selecting {
                        origin,
                        current: document_position,
                        selection_mode: *selection_mode,
                        base_selection: base_selection.clone(),
                    }),
                ]
            }
            (
                ToolState::Selecting {
                    origin,
                    selection_mode,
                    base_selection,
                    ..
                },
                CanvasEvent::PointerMove { position, .. },
            ) => {
                let origin = *origin;
                let document_position = editor.viewport.view_to_document(position);
                let bounds = selection_bounds(origin, document_position);
                vec![
                    CanvasToolEffect::SetSelection(editor.selection_for_intersections_with_mode(
                        bounds,
                        *selection_mode,
                        base_selection,
                    )),
                    CanvasToolEffect::SetState(ToolState::Selecting {
                        origin,
                        current: document_position,
                        selection_mode: *selection_mode,
                        base_selection: base_selection.clone(),
                    }),
                ]
            }
            (ToolState::Translating { .. }, CanvasEvent::PointerUp { .. }) => {
                vec![
                    CanvasToolEffect::CommitGesture,
                    CanvasToolEffect::SetState(ToolState::Idle),
                ]
            }
            (ToolState::Resizing { .. }, CanvasEvent::PointerUp { .. }) => {
                vec![
                    CanvasToolEffect::CommitGesture,
                    CanvasToolEffect::SetState(ToolState::Idle),
                ]
            }
            (ToolState::Translating { .. } | ToolState::Resizing { .. }, CanvasEvent::Cancel) => {
                vec![
                    CanvasToolEffect::CancelGesture,
                    CanvasToolEffect::SetState(ToolState::Idle),
                ]
            }
            (ToolState::Pointing { base_selection, .. }, CanvasEvent::Cancel)
            | (ToolState::Selecting { base_selection, .. }, CanvasEvent::Cancel) => {
                vec![
                    CanvasToolEffect::SetSelection(base_selection.clone()),
                    CanvasToolEffect::SetState(ToolState::Idle),
                ]
            }
            (ToolState::Pointing { .. }, CanvasEvent::PointerUp { .. }) => {
                vec![CanvasToolEffect::SetState(ToolState::Idle)]
            }
            (ToolState::Selecting { .. }, CanvasEvent::PointerUp { .. }) => {
                vec![CanvasToolEffect::SetState(ToolState::Idle)]
            }
            (_, CanvasEvent::Wheel { delta }) => {
                vec![CanvasToolEffect::PanViewport(delta)]
            }
            _ => Vec::new(),
        };

        Ok(effects)
    }
}

struct PanToolStateMachine;

impl BuiltInCanvasToolReducer for PanToolStateMachine {
    fn handle_event(
        &self,
        editor: &CanvasEditor,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        Ok(match (&editor.state, event) {
            (
                ToolState::Idle,
                CanvasEvent::PointerDown {
                    position,
                    button: PointerButton::Primary,
                    ..
                },
            ) => {
                vec![CanvasToolEffect::SetState(ToolState::Panning {
                    origin: position,
                    last: position,
                })]
            }
            (ToolState::Panning { last, origin }, CanvasEvent::PointerMove { position, .. }) => {
                let delta = position - *last;
                vec![
                    CanvasToolEffect::PanViewport(delta * -1.0),
                    CanvasToolEffect::SetState(ToolState::Panning {
                        origin: *origin,
                        last: position,
                    }),
                ]
            }
            (ToolState::Panning { .. }, CanvasEvent::PointerUp { .. } | CanvasEvent::Cancel) => {
                vec![CanvasToolEffect::SetState(ToolState::Idle)]
            }
            _ => Vec::new(),
        })
    }
}

struct ConnectToolStateMachine;

impl BuiltInCanvasToolReducer for ConnectToolStateMachine {
    fn handle_event(
        &self,
        editor: &CanvasEditor,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        Ok(match (&editor.state, event) {
            (
                ToolState::Idle,
                CanvasEvent::PointerDown {
                    position,
                    button: PointerButton::Primary,
                    ..
                },
            ) => {
                let document_position = editor.viewport.view_to_document(position);
                editor
                    .node_endpoint_at(document_position, CanvasConnectionEndpointRole::Source)
                    .map(|source| {
                        vec![CanvasToolEffect::SetState(ToolState::Connecting {
                            source,
                            current: document_position,
                        })]
                    })
                    .unwrap_or_default()
            }
            (ToolState::Connecting { source, .. }, CanvasEvent::PointerMove { position, .. }) => {
                let document_position = editor.viewport.view_to_document(position);
                vec![CanvasToolEffect::SetState(ToolState::Connecting {
                    source: source.clone(),
                    current: document_position,
                })]
            }
            (
                ToolState::Connecting { source, .. },
                CanvasEvent::PointerUp {
                    position,
                    button: PointerButton::Primary,
                    ..
                },
            ) => {
                let document_position = editor.viewport.view_to_document(position);
                let mut effects = Vec::new();
                if let Some(target) =
                    editor.node_endpoint_at(document_position, CanvasConnectionEndpointRole::Target)
                    && (source.node_id != target.node_id || source.handle_id != target.handle_id)
                {
                    let edge_id = EdgeId::new(format!(
                        "{}->{}:{}",
                        source.node_id,
                        target.node_id,
                        editor.document.edge_count()
                    ));
                    effects.push(CanvasToolEffect::ApplyTransaction(
                        CanvasTransaction::single(DocumentCommand::InsertEdge(CanvasEdge::new(
                            edge_id,
                            source.clone(),
                            target,
                        ))),
                    ));
                }
                effects.push(CanvasToolEffect::SetState(ToolState::Idle));
                effects
            }
            (ToolState::Connecting { .. }, CanvasEvent::Cancel) => {
                vec![CanvasToolEffect::SetState(ToolState::Idle)]
            }
            _ => Vec::new(),
        })
    }
}
