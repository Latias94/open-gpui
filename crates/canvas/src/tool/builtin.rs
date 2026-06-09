use super::select::SelectToolStateMachine;
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
