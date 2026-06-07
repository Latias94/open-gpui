use crate::{
    CanvasDocument, CanvasEdge, CanvasEndpoint, CanvasViewport, DocumentCommand, DocumentError,
    EdgeId, HitOptions, HitTarget, NodeId, SpatialIndex,
};
use open_gpui::{Pixels, Point};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CanvasEvent {
    PointerDown {
        position: Point<Pixels>,
        button: PointerButton,
    },
    PointerMove {
        position: Point<Pixels>,
    },
    PointerUp {
        position: Point<Pixels>,
        button: PointerButton,
    },
    Wheel {
        delta: Point<Pixels>,
    },
    Cancel,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CanvasTool {
    Select,
    Pan,
    Connect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToolState {
    Idle,
    Pointing {
        origin: Point<Pixels>,
    },
    Translating {
        origin: Point<Pixels>,
        last: Point<Pixels>,
        node_ids: Vec<NodeId>,
    },
    Panning {
        origin: Point<Pixels>,
        last: Point<Pixels>,
    },
    Connecting {
        source: CanvasEndpoint,
        current: Point<Pixels>,
    },
}

pub struct CanvasEditor {
    pub document: CanvasDocument,
    pub viewport: CanvasViewport,
    pub tool: CanvasTool,
    pub state: ToolState,
    pub index: SpatialIndex,
}

impl Default for CanvasEditor {
    fn default() -> Self {
        Self::new(CanvasDocument::default())
    }
}

impl CanvasEditor {
    pub fn new(document: CanvasDocument) -> Self {
        let index = SpatialIndex::rebuild(&document);
        Self {
            document,
            viewport: CanvasViewport::default(),
            tool: CanvasTool::Select,
            state: ToolState::Idle,
            index,
        }
    }

    pub fn apply(&mut self, command: DocumentCommand) -> Result<(), DocumentError> {
        self.document.apply(command)?;
        self.rebuild_index();
        Ok(())
    }

    pub fn rebuild_index(&mut self) {
        self.index = SpatialIndex::rebuild(&self.document);
    }

    pub fn set_tool(&mut self, tool: CanvasTool) {
        self.tool = tool;
        self.state = ToolState::Idle;
    }

    pub fn handle_event(&mut self, event: CanvasEvent) -> Result<(), DocumentError> {
        match self.tool {
            CanvasTool::Select => self.handle_select_event(event),
            CanvasTool::Pan => self.handle_pan_event(event),
            CanvasTool::Connect => self.handle_connect_event(event),
        }
    }

    fn handle_select_event(&mut self, event: CanvasEvent) -> Result<(), DocumentError> {
        match (&self.state, event) {
            (
                ToolState::Idle,
                CanvasEvent::PointerDown {
                    position,
                    button: PointerButton::Primary,
                },
            ) => {
                let document_position = self.viewport.view_to_document(position);
                let node_ids = self
                    .index
                    .hit_test(document_position, HitOptions::default())
                    .find_map(|record| match &record.target {
                        HitTarget::Node(id) => Some(vec![id.clone()]),
                        _ => None,
                    });

                self.state = if let Some(node_ids) = node_ids {
                    ToolState::Translating {
                        origin: document_position,
                        last: document_position,
                        node_ids,
                    }
                } else {
                    ToolState::Pointing {
                        origin: document_position,
                    }
                };
            }
            (
                ToolState::Translating { last, node_ids, .. },
                CanvasEvent::PointerMove { position },
            ) => {
                let document_position = self.viewport.view_to_document(position);
                let delta = document_position - *last;
                for id in node_ids.clone() {
                    let mut node = self
                        .document
                        .nodes
                        .get(&id)
                        .ok_or_else(|| DocumentError::MissingNode(id.clone()))?
                        .clone();
                    node.position += delta;
                    self.document.update_node(node)?;
                }
                self.rebuild_index();

                if let ToolState::Translating { last, .. } = &mut self.state {
                    *last = document_position;
                }
            }
            (
                ToolState::Translating { .. } | ToolState::Pointing { .. },
                CanvasEvent::PointerUp { .. } | CanvasEvent::Cancel,
            ) => {
                self.state = ToolState::Idle;
            }
            (_, CanvasEvent::Wheel { delta }) => {
                self.viewport.pan_by(delta);
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_pan_event(&mut self, event: CanvasEvent) -> Result<(), DocumentError> {
        match (&self.state, event) {
            (
                ToolState::Idle,
                CanvasEvent::PointerDown {
                    position,
                    button: PointerButton::Primary,
                },
            ) => {
                self.state = ToolState::Panning {
                    origin: position,
                    last: position,
                };
            }
            (ToolState::Panning { last, .. }, CanvasEvent::PointerMove { position }) => {
                let delta = position - *last;
                self.viewport.pan_by(delta * -1.0);
                if let ToolState::Panning { last, .. } = &mut self.state {
                    *last = position;
                }
            }
            (ToolState::Panning { .. }, CanvasEvent::PointerUp { .. } | CanvasEvent::Cancel) => {
                self.state = ToolState::Idle;
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_connect_event(&mut self, event: CanvasEvent) -> Result<(), DocumentError> {
        match (&self.state, event) {
            (
                ToolState::Idle,
                CanvasEvent::PointerDown {
                    position,
                    button: PointerButton::Primary,
                },
            ) => {
                let document_position = self.viewport.view_to_document(position);
                if let Some(source) = self.node_endpoint_at(document_position) {
                    self.state = ToolState::Connecting {
                        source,
                        current: document_position,
                    };
                }
            }
            (ToolState::Connecting { .. }, CanvasEvent::PointerMove { position }) => {
                let document_position = self.viewport.view_to_document(position);
                if let ToolState::Connecting { current, .. } = &mut self.state {
                    *current = document_position;
                }
            }
            (
                ToolState::Connecting { source, .. },
                CanvasEvent::PointerUp {
                    position,
                    button: PointerButton::Primary,
                },
            ) => {
                let document_position = self.viewport.view_to_document(position);
                if let Some(target) = self.node_endpoint_at(document_position) {
                    if source.node_id != target.node_id || source.handle_id != target.handle_id {
                        let edge_id = EdgeId::new(format!(
                            "{}->{}:{}",
                            source.node_id,
                            target.node_id,
                            self.document.edges.len()
                        ));
                        self.apply(DocumentCommand::InsertEdge(CanvasEdge::new(
                            edge_id,
                            source.clone(),
                            target,
                        )))?;
                    }
                }
                self.state = ToolState::Idle;
            }
            (ToolState::Connecting { .. }, CanvasEvent::Cancel) => {
                self.state = ToolState::Idle;
            }
            _ => {}
        }

        Ok(())
    }

    fn node_endpoint_at(&self, point: Point<Pixels>) -> Option<CanvasEndpoint> {
        self.index
            .hit_test(point, HitOptions::default())
            .find_map(|record| match &record.target {
                HitTarget::Node(node_id) => Some(CanvasEndpoint {
                    node_id: node_id.clone(),
                    handle_id: None,
                }),
                _ => None,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CanvasNode;
    use open_gpui::{point, px, size};

    #[test]
    fn select_tool_translates_node() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "n1",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .unwrap();
        let mut editor = CanvasEditor::new(document);

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(10.0), px(10.0)),
                button: PointerButton::Primary,
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(20.0), px(25.0)),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(20.0), px(25.0)),
                button: PointerButton::Primary,
            })
            .unwrap();

        let node = editor.document.nodes.get(&NodeId::from("n1")).unwrap();
        assert_eq!(node.position, point(px(10.0), px(15.0)));
        assert_eq!(editor.state, ToolState::Idle);
    }

    #[test]
    fn pan_tool_moves_viewport() {
        let mut editor = CanvasEditor::default();
        editor.set_tool(CanvasTool::Pan);

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(10.0), px(10.0)),
                button: PointerButton::Primary,
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(20.0), px(25.0)),
            })
            .unwrap();

        assert_eq!(editor.viewport.origin, point(px(-10.0), px(-15.0)));
    }

    #[test]
    fn connect_tool_creates_edge_between_nodes() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .unwrap();
        document
            .insert_node(CanvasNode::new(
                "b",
                point(px(200.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor.set_tool(CanvasTool::Connect);

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(10.0), px(10.0)),
                button: PointerButton::Primary,
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(210.0), px(10.0)),
                button: PointerButton::Primary,
            })
            .unwrap();

        assert_eq!(editor.document.edges.len(), 1);
    }
}
