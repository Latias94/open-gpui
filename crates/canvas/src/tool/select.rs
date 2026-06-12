use super::*;
use crate::{CanvasResizeHandle, HitOptions};
use open_gpui::Axis;

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
                    shape_ids,
                    origin,
                    ..
                },
                CanvasEvent::PointerMove { position, .. },
            ) => self.handle_resizing_pointer_move(
                context, position, *origin, *last, *handle, node_ids, shape_ids,
            )?,
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

    fn handle_idle_pointer_down(
        &self,
        context: CanvasToolReducerContext<'_>,
        position: Point<Pixels>,
        modifiers: CanvasKeyModifiers,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        let document_position = context.viewport().view_to_document(position);
        if let Some(handle) = context.transform_handle_at(document_position) {
            let (node_ids, shape_ids) = context.resizable_selection_ids();
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
        shape_ids: &[ShapeId],
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        let document_position = context.viewport().view_to_document(position);
        let raw_delta = document_position - last;
        let snap = context.snap_delta_for_resize(handle, raw_delta, node_ids, shape_ids);
        let delta = snap.delta;
        let transaction =
            context.resize_selection_transaction(handle, delta, node_ids, shape_ids)?;

        Ok(vec![
            CanvasToolEffect::UpdateGesture(transaction),
            CanvasToolEffect::SetState(ToolState::Resizing {
                origin,
                last: last + delta,
                handle,
                node_ids: node_ids.to_vec(),
                shape_ids: shape_ids.to_vec(),
                snap_guides: snap.guides,
            }),
        ])
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
