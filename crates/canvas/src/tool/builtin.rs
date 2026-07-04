use super::context::CanvasConnectionHit;
use super::select::SelectToolStateMachine;
use super::*;
use crate::{
    CanvasConnectedRelease, CanvasConnectionEndpointRole, CanvasConnectionRejectReason,
    CanvasConnectionRelease, CanvasDroppedConnectionRelease, CanvasEdge,
    CanvasRejectedConnectionRelease,
};

trait BuiltInCanvasToolReducer {
    fn handle_event(
        &self,
        context: CanvasToolReducerContext<'_>,
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
        context: CanvasToolReducerContext<'_>,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        match self {
            Self::Select => SelectToolStateMachine.handle_event(context, event),
            Self::Pan => PanToolStateMachine.handle_event(context, event),
            Self::Connect => ConnectToolStateMachine.handle_event(context, event),
        }
    }
}

struct PanToolStateMachine;

impl BuiltInCanvasToolReducer for PanToolStateMachine {
    fn handle_event(
        &self,
        context: CanvasToolReducerContext<'_>,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        Ok(match (context.state(), event) {
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
        context: CanvasToolReducerContext<'_>,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        Ok(match (context.state(), event) {
            (
                ToolState::Idle,
                CanvasEvent::PointerDown {
                    position,
                    button: PointerButton::Primary,
                    ..
                },
            ) => {
                let document_position = context.viewport().view_to_document(position);
                match context
                    .connection_hit_at(document_position, CanvasConnectionEndpointRole::Source)
                {
                    CanvasConnectionHit::Valid(source) => {
                        begin_connection_from_source(source, document_position)
                    }
                    CanvasConnectionHit::Invalid => {
                        vec![CanvasToolEffect::SetConnectionRelease(Some(
                            CanvasConnectionRelease::Rejected(CanvasRejectedConnectionRelease {
                                reason: CanvasConnectionRejectReason::InvalidSource,
                                source: None,
                                edge_id: None,
                                endpoint: Some(CanvasConnectionEndpointRole::Source),
                                position: document_position,
                            }),
                        ))]
                    }
                    CanvasConnectionHit::Empty => {
                        vec![CanvasToolEffect::SetConnectionRelease(None)]
                    }
                }
            }
            (ToolState::Connecting { source, .. }, CanvasEvent::PointerMove { position, .. }) => {
                let document_position = context.viewport().view_to_document(position);
                update_connection_from_source(source, document_position)
            }
            (
                ToolState::Connecting { source, .. },
                CanvasEvent::PointerUp {
                    position,
                    button: PointerButton::Primary,
                    ..
                },
            ) => {
                let document_position = context.viewport().view_to_document(position);
                finish_connection_from_source(context, source, document_position)?
            }
            (ToolState::Connecting { .. }, CanvasEvent::Cancel) => cancel_connection(),
            _ => Vec::new(),
        })
    }
}

pub(super) fn begin_connection_from_source(
    source: CanvasEndpoint,
    current: Point<Pixels>,
) -> Vec<CanvasToolEffect> {
    vec![
        CanvasToolEffect::SetConnectionRelease(None),
        CanvasToolEffect::SetState(ToolState::Connecting { source, current }),
    ]
}

pub(super) fn update_connection_from_source(
    source: &CanvasEndpoint,
    current: Point<Pixels>,
) -> Vec<CanvasToolEffect> {
    vec![CanvasToolEffect::SetState(ToolState::Connecting {
        source: source.clone(),
        current,
    })]
}

pub(super) fn finish_connection_from_source(
    context: CanvasToolReducerContext<'_>,
    source: &CanvasEndpoint,
    position: Point<Pixels>,
) -> Result<Vec<CanvasToolEffect>, DocumentError> {
    let mut effects = Vec::new();
    match context.connection_hit_at(position, CanvasConnectionEndpointRole::Target) {
        CanvasConnectionHit::Valid(target)
            if source.node_id != target.node_id || source.handle_id != target.handle_id =>
        {
            let edge_id = EdgeId::new(format!(
                "{}->{}:{}",
                source.node_id,
                target.node_id,
                context.document().edge_count()
            ));
            effects.push(CanvasToolEffect::ApplyTransaction(
                CanvasTransaction::single(DocumentCommand::InsertEdge(CanvasEdge::new(
                    edge_id.clone(),
                    source.clone(),
                    target.clone(),
                ))),
            ));
            effects.push(CanvasToolEffect::SetConnectionRelease(Some(
                CanvasConnectionRelease::Connected(CanvasConnectedRelease {
                    source: source.clone(),
                    target,
                    edge_id,
                    position,
                }),
            )));
        }
        CanvasConnectionHit::Valid(_) => {
            effects.push(CanvasToolEffect::SetConnectionRelease(Some(
                CanvasConnectionRelease::Rejected(CanvasRejectedConnectionRelease {
                    reason: CanvasConnectionRejectReason::SameEndpoint,
                    source: Some(source.clone()),
                    edge_id: None,
                    endpoint: Some(CanvasConnectionEndpointRole::Target),
                    position,
                }),
            )));
        }
        CanvasConnectionHit::Invalid => {
            effects.push(CanvasToolEffect::SetConnectionRelease(Some(
                CanvasConnectionRelease::Rejected(CanvasRejectedConnectionRelease {
                    reason: CanvasConnectionRejectReason::InvalidTarget,
                    source: Some(source.clone()),
                    edge_id: None,
                    endpoint: Some(CanvasConnectionEndpointRole::Target),
                    position,
                }),
            )));
        }
        CanvasConnectionHit::Empty => {
            effects.push(CanvasToolEffect::SetConnectionRelease(Some(
                CanvasConnectionRelease::Dropped(CanvasDroppedConnectionRelease {
                    source: source.clone(),
                    position,
                }),
            )));
        }
    }
    effects.push(CanvasToolEffect::SetState(ToolState::Idle));
    Ok(effects)
}

pub(super) fn cancel_connection() -> Vec<CanvasToolEffect> {
    vec![CanvasToolEffect::SetState(ToolState::Idle)]
}
