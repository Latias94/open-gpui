use super::context::CanvasConnectionHit;
use super::*;
use crate::{
    CanvasConnectionEndpointRole, CanvasConnectionRejectReason, CanvasConnectionRelease,
    CanvasReconnectedRelease, CanvasRejectedConnectionRelease, CanvasResizeHandle, CanvasViewport,
    HitOptions,
};
use open_gpui::Axis;

const TRANSLATION_DRAG_THRESHOLD: f32 = 3.0;

pub(super) struct SelectToolStateMachine;

impl SelectToolStateMachine {
    pub(super) fn handle_event(
        &self,
        context: CanvasToolReducerContext<'_>,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        let effects = match (context.state(), event) {
            (
                ToolState::Idle,
                CanvasEvent::KeyDown {
                    key: CanvasKey::Delete | CanvasKey::Backspace,
                    ..
                },
            ) => {
                let transaction = context.delete_selection_transaction();
                if transaction.is_empty() {
                    Vec::new()
                } else {
                    vec![CanvasToolEffect::ApplyTransaction(transaction)]
                }
            }
            (ToolState::Idle, CanvasEvent::Cancel) => {
                if context.selection().is_empty() {
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
            ) => self.handle_idle_pointer_down(context, position, modifiers)?,
            (
                ToolState::PendingTranslation {
                    origin,
                    node_ids,
                    shape_ids,
                    ..
                },
                CanvasEvent::PointerMove {
                    position,
                    modifiers,
                },
            ) => self.handle_pending_translation_pointer_move(
                context, position, modifiers, *origin, node_ids, shape_ids,
            )?,
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
            ) => self.handle_translating_pointer_move(
                context,
                position,
                modifiers,
                *origin,
                *last,
                *constraint_axis,
                node_ids,
                shape_ids,
            )?,
            (
                ToolState::Resizing {
                    last,
                    handle,
                    node_ids,
                    edge_ids,
                    shape_ids,
                    origin,
                    structural,
                    ..
                },
                CanvasEvent::PointerMove { position, .. },
            ) => self.handle_resizing_pointer_move(
                context,
                position,
                *origin,
                *last,
                *handle,
                node_ids,
                edge_ids,
                shape_ids,
                *structural,
            )?,
            (
                ToolState::Reconnecting {
                    edge_id,
                    endpoint,
                    fixed,
                    ..
                },
                CanvasEvent::PointerMove { position, .. },
            ) => {
                self.handle_reconnecting_pointer_move(context, position, edge_id, *endpoint, fixed)
            }
            (
                ToolState::Pointing {
                    origin,
                    selection_mode,
                    base_selection,
                },
                CanvasEvent::PointerMove { position, .. },
            ) => self.handle_pointing_pointer_move(
                context,
                position,
                *origin,
                *selection_mode,
                base_selection,
            ),
            (
                ToolState::Selecting {
                    origin,
                    selection_mode,
                    base_selection,
                    ..
                },
                CanvasEvent::PointerMove { position, .. },
            ) => self.handle_pointing_pointer_move(
                context,
                position,
                *origin,
                *selection_mode,
                base_selection,
            ),
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
            (
                ToolState::Reconnecting {
                    edge_id,
                    endpoint,
                    fixed,
                    ..
                },
                CanvasEvent::PointerUp {
                    position,
                    button: PointerButton::Primary,
                    ..
                },
            ) => {
                self.handle_reconnecting_pointer_up(context, position, edge_id, *endpoint, fixed)?
            }
            (ToolState::Translating { .. } | ToolState::Resizing { .. }, CanvasEvent::Cancel) => {
                vec![
                    CanvasToolEffect::CancelGesture,
                    CanvasToolEffect::SetState(ToolState::Idle),
                ]
            }
            (ToolState::Reconnecting { .. }, CanvasEvent::Cancel) => {
                vec![CanvasToolEffect::SetState(ToolState::Idle)]
            }
            (ToolState::PendingTranslation { base_selection, .. }, CanvasEvent::Cancel) => {
                vec![
                    CanvasToolEffect::SetSelection(base_selection.clone()),
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
            (ToolState::PendingTranslation { .. }, CanvasEvent::PointerUp { .. }) => {
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

    fn handle_idle_pointer_down(
        &self,
        context: CanvasToolReducerContext<'_>,
        position: Point<Pixels>,
        modifiers: CanvasKeyModifiers,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        let document_position = context.viewport().view_to_document(position);
        if let Some(target) = context.selected_reconnect_target_at(document_position) {
            return Ok(vec![CanvasToolEffect::SetState(ToolState::Reconnecting {
                edge_id: target.edge_id,
                endpoint: target.endpoint,
                fixed: target.fixed,
                current: document_position,
            })]);
        }

        if let Some(handle) = context.transform_handle_at(document_position) {
            let resize_scope = context.resize_selection_scope();
            return Ok(vec![
                CanvasToolEffect::BeginGesture,
                CanvasToolEffect::SetState(ToolState::Resizing {
                    origin: document_position,
                    last: document_position,
                    handle: handle.handle,
                    node_ids: resize_scope.node_ids,
                    edge_ids: resize_scope.edge_ids,
                    shape_ids: resize_scope.shape_ids,
                    structural: resize_scope.structural,
                    snap_guides: Vec::new(),
                }),
            ]);
        }

        let hit = context
            .runtime()
            .precise_hit_test_with_kind_registry(
                context.document(),
                context.kind_registry(),
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

        Ok(match hit {
            Some(target @ (HitTarget::Node(_) | HitTarget::Shape(_))) => {
                self.begin_translation_from_hit(context, document_position, target)
            }
            Some(target) => {
                vec![
                    CanvasToolEffect::ReplaceSelection(target),
                    CanvasToolEffect::SetState(ToolState::Pointing {
                        origin: document_position,
                        selection_mode: CanvasSelectionMode::Replace,
                        base_selection: context.selection().clone(),
                    }),
                ]
            }
            None => self.begin_blank_pointing(context, document_position, modifiers),
        })
    }

    fn begin_translation_from_hit(
        &self,
        context: CanvasToolReducerContext<'_>,
        document_position: Point<Pixels>,
        target: HitTarget,
    ) -> Vec<CanvasToolEffect> {
        let mut selection = context.selection().clone();
        if !selection.contains_target(&target)
            && !context.selection_structurally_contains_target(&target)
        {
            selection.replace_with(target);
        }
        let (node_ids, shape_ids) = context.translatable_selection_ids(&selection);
        vec![
            CanvasToolEffect::SetSelection(selection),
            CanvasToolEffect::SetState(ToolState::PendingTranslation {
                origin: document_position,
                node_ids,
                shape_ids,
                base_selection: context.selection().clone(),
            }),
        ]
    }

    fn begin_blank_pointing(
        &self,
        context: CanvasToolReducerContext<'_>,
        document_position: Point<Pixels>,
        modifiers: CanvasKeyModifiers,
    ) -> Vec<CanvasToolEffect> {
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
            base_selection: context.selection().clone(),
        }));
        effects
    }

    fn handle_pending_translation_pointer_move(
        &self,
        context: CanvasToolReducerContext<'_>,
        position: Point<Pixels>,
        modifiers: CanvasKeyModifiers,
        origin: Point<Pixels>,
        node_ids: &[NodeId],
        shape_ids: &[ShapeId],
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        if !translation_drag_threshold_exceeded(context.viewport(), origin, position) {
            return Ok(Vec::new());
        }

        let mut effects = vec![CanvasToolEffect::BeginGesture];
        effects.extend(self.handle_translating_pointer_move(
            context, position, modifiers, origin, origin, None, node_ids, shape_ids,
        )?);
        Ok(effects)
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_translating_pointer_move(
        &self,
        context: CanvasToolReducerContext<'_>,
        position: Point<Pixels>,
        modifiers: CanvasKeyModifiers,
        origin: Point<Pixels>,
        last: Point<Pixels>,
        constraint_axis: Option<Axis>,
        node_ids: &[NodeId],
        shape_ids: &[ShapeId],
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        let document_position = context.viewport().view_to_document(position);
        let constraint_axis = if modifiers.shift {
            Some(
                constraint_axis.unwrap_or_else(|| drag_constraint_axis(document_position - origin)),
            )
        } else {
            None
        };
        let document_position = constraint_axis
            .map(|axis| constrained_drag_position(origin, document_position, axis))
            .unwrap_or(document_position);
        let raw_delta = document_position - last;
        let snap = context.snap_delta_for_translation(raw_delta, node_ids, shape_ids);
        let delta = snap.delta;
        let mut commands = Vec::new();
        for id in node_ids {
            let mut node = context
                .document()
                .node(id)
                .ok_or_else(|| DocumentError::MissingNode(id.clone()))?
                .clone();
            node.position += delta;
            commands.push(DocumentCommand::UpdateNode(node));
        }
        for id in shape_ids {
            let mut shape = context
                .document()
                .shape(id)
                .ok_or_else(|| DocumentError::MissingShape(id.clone()))?
                .clone();
            shape.bounds.origin += delta;
            commands.push(DocumentCommand::UpdateShape(shape));
        }

        Ok(vec![
            CanvasToolEffect::UpdateGesture(CanvasTransaction::new(commands)),
            CanvasToolEffect::SetState(ToolState::Translating {
                origin,
                last: last + delta,
                constraint_axis,
                node_ids: node_ids.to_vec(),
                shape_ids: shape_ids.to_vec(),
                snap_guides: snap.guides,
            }),
        ])
    }

    fn handle_resizing_pointer_move(
        &self,
        context: CanvasToolReducerContext<'_>,
        position: Point<Pixels>,
        origin: Point<Pixels>,
        last: Point<Pixels>,
        handle: CanvasResizeHandle,
        node_ids: &[NodeId],
        edge_ids: &[EdgeId],
        shape_ids: &[ShapeId],
        structural: bool,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        let document_position = context.viewport().view_to_document(position);
        let raw_delta = document_position - last;
        let snap = context.snap_delta_for_resize(handle, raw_delta, node_ids, shape_ids);
        let delta = snap.delta;
        let transaction = context.resize_selection_transaction(
            handle, delta, node_ids, edge_ids, shape_ids, structural,
        )?;

        Ok(vec![
            CanvasToolEffect::UpdateGesture(transaction),
            CanvasToolEffect::SetState(ToolState::Resizing {
                origin,
                last: last + delta,
                handle,
                node_ids: node_ids.to_vec(),
                edge_ids: edge_ids.to_vec(),
                shape_ids: shape_ids.to_vec(),
                structural,
                snap_guides: snap.guides,
            }),
        ])
    }

    fn handle_reconnecting_pointer_move(
        &self,
        context: CanvasToolReducerContext<'_>,
        position: Point<Pixels>,
        edge_id: &EdgeId,
        endpoint: CanvasConnectionEndpointRole,
        fixed: &CanvasEndpoint,
    ) -> Vec<CanvasToolEffect> {
        let document_position = context.viewport().view_to_document(position);
        vec![CanvasToolEffect::SetState(ToolState::Reconnecting {
            edge_id: edge_id.clone(),
            endpoint,
            fixed: fixed.clone(),
            current: document_position,
        })]
    }

    fn handle_reconnecting_pointer_up(
        &self,
        context: CanvasToolReducerContext<'_>,
        position: Point<Pixels>,
        edge_id: &EdgeId,
        endpoint: CanvasConnectionEndpointRole,
        fixed: &CanvasEndpoint,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        let document_position = context.viewport().view_to_document(position);
        let mut effects = Vec::new();
        match context.connection_hit_at(document_position, endpoint) {
            CanvasConnectionHit::Valid(candidate) => {
                let transaction =
                    context.reconnect_edge_transaction(edge_id, endpoint, candidate.clone())?;
                if !transaction.is_empty() {
                    effects.push(CanvasToolEffect::ApplyTransaction(transaction));
                    effects.push(CanvasToolEffect::SetConnectionRelease(Some(
                        CanvasConnectionRelease::Reconnected(CanvasReconnectedRelease {
                            edge_id: edge_id.clone(),
                            endpoint,
                            fixed: fixed.clone(),
                            replacement: candidate,
                            position: document_position,
                        }),
                    )));
                } else {
                    effects.push(CanvasToolEffect::SetConnectionRelease(Some(
                        CanvasConnectionRelease::Rejected(CanvasRejectedConnectionRelease {
                            reason: CanvasConnectionRejectReason::SameEndpoint,
                            source: None,
                            edge_id: Some(edge_id.clone()),
                            endpoint: Some(endpoint),
                            position: document_position,
                        }),
                    )));
                }
            }
            CanvasConnectionHit::Invalid => {
                effects.push(CanvasToolEffect::SetConnectionRelease(Some(
                    CanvasConnectionRelease::Rejected(CanvasRejectedConnectionRelease {
                        reason: CanvasConnectionRejectReason::InvalidTarget,
                        source: None,
                        edge_id: Some(edge_id.clone()),
                        endpoint: Some(endpoint),
                        position: document_position,
                    }),
                )));
            }
            CanvasConnectionHit::Empty => {
                effects.push(CanvasToolEffect::SetConnectionRelease(Some(
                    CanvasConnectionRelease::Rejected(CanvasRejectedConnectionRelease {
                        reason: CanvasConnectionRejectReason::InvalidTarget,
                        source: None,
                        edge_id: Some(edge_id.clone()),
                        endpoint: Some(endpoint),
                        position: document_position,
                    }),
                )));
            }
        }
        effects.push(CanvasToolEffect::SetState(ToolState::Idle));
        Ok(effects)
    }

    fn handle_pointing_pointer_move(
        &self,
        context: CanvasToolReducerContext<'_>,
        position: Point<Pixels>,
        origin: Point<Pixels>,
        selection_mode: CanvasSelectionMode,
        base_selection: &CanvasSelection,
    ) -> Vec<CanvasToolEffect> {
        let document_position = context.viewport().view_to_document(position);
        let bounds = selection_bounds(origin, document_position);
        vec![
            CanvasToolEffect::SetSelection(context.selection_for_intersections_with_mode(
                bounds,
                selection_mode,
                base_selection,
            )),
            CanvasToolEffect::SetState(ToolState::Selecting {
                origin,
                current: document_position,
                selection_mode,
                base_selection: base_selection.clone(),
            }),
        ]
    }
}

fn selection_bounds(origin: Point<Pixels>, current: Point<Pixels>) -> Bounds<Pixels> {
    Bounds::from_corners(
        Point::new(origin.x.min(current.x), origin.y.min(current.y)),
        Point::new(origin.x.max(current.x), origin.y.max(current.y)),
    )
}

fn translation_drag_threshold_exceeded(
    viewport: CanvasViewport,
    origin: Point<Pixels>,
    current_view_position: Point<Pixels>,
) -> bool {
    let origin_view_position = viewport.document_to_view(origin);
    let delta = current_view_position - origin_view_position;
    let dx = delta.x.as_f32();
    let dy = delta.y.as_f32();
    dx.mul_add(dx, dy * dy) >= TRANSLATION_DRAG_THRESHOLD * TRANSLATION_DRAG_THRESHOLD
}

fn constrained_drag_position(
    origin: Point<Pixels>,
    current: Point<Pixels>,
    axis: Axis,
) -> Point<Pixels> {
    match axis {
        Axis::Horizontal => Point::new(current.x, origin.y),
        Axis::Vertical => Point::new(origin.x, current.y),
    }
}

fn drag_constraint_axis(delta: Point<Pixels>) -> Axis {
    if delta.x.abs() >= delta.y.abs() {
        Axis::Horizontal
    } else {
        Axis::Vertical
    }
}
