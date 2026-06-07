use crate::{
    CanvasDocument, CanvasEdge, CanvasEndpoint, CanvasViewport, DocumentCommand, DocumentError,
    EdgeId, HitOptions, HitTarget, NodeId, ShapeId, SpatialIndex,
};
use indexmap::IndexSet;
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

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CanvasSelection {
    pub nodes: IndexSet<NodeId>,
    pub edges: IndexSet<EdgeId>,
    pub shapes: IndexSet<ShapeId>,
    pub handles: IndexSet<CanvasEndpoint>,
}

impl CanvasSelection {
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.edges.clear();
        self.shapes.clear();
        self.handles.clear();
    }

    pub fn replace_with(&mut self, target: HitTarget) {
        self.clear();
        match target {
            HitTarget::Node(id) => {
                self.nodes.insert(id);
            }
            HitTarget::Handle { node_id, handle_id } => {
                self.handles.insert(CanvasEndpoint {
                    node_id,
                    handle_id: Some(handle_id),
                });
            }
            HitTarget::Edge(id) => {
                self.edges.insert(id);
            }
            HitTarget::Shape(id) => {
                self.shapes.insert(id);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
            && self.edges.is_empty()
            && self.shapes.is_empty()
            && self.handles.is_empty()
    }

    pub fn selected_nodes(&self) -> impl Iterator<Item = &NodeId> {
        self.nodes.iter()
    }
}

pub struct CanvasEditor {
    pub document: CanvasDocument,
    pub viewport: CanvasViewport,
    pub tool: CanvasTool,
    pub state: ToolState,
    pub index: SpatialIndex,
    pub selection: CanvasSelection,
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
            selection: CanvasSelection::default(),
        }
    }

    pub fn apply(&mut self, command: DocumentCommand) -> Result<(), DocumentError> {
        self.document.apply(command)?;
        self.rebuild_index();
        Ok(())
    }

    pub fn apply_all(
        &mut self,
        commands: impl IntoIterator<Item = DocumentCommand>,
    ) -> Result<(), DocumentError> {
        for command in commands {
            self.document.apply(command)?;
        }
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
                let hit = self
                    .index
                    .hit_test(document_position, HitOptions::default())
                    .map(|record| record.target.clone())
                    .next();

                match hit {
                    Some(HitTarget::Node(id)) => {
                        self.selection.replace_with(HitTarget::Node(id.clone()));
                        self.state = ToolState::Translating {
                            origin: document_position,
                            last: document_position,
                            node_ids: vec![id],
                        };
                    }
                    Some(target) => {
                        self.selection.replace_with(target);
                        self.state = ToolState::Pointing {
                            origin: document_position,
                        };
                    }
                    None => {
                        self.selection.clear();
                        self.state = ToolState::Pointing {
                            origin: document_position,
                        };
                    }
                }
            }
            (
                ToolState::Translating { last, node_ids, .. },
                CanvasEvent::PointerMove { position },
            ) => {
                let document_position = self.viewport.view_to_document(position);
                let delta = document_position - *last;
                let mut commands = Vec::new();
                for id in node_ids {
                    let mut node = self
                        .document
                        .nodes
                        .get(id)
                        .ok_or_else(|| DocumentError::MissingNode(id.clone()))?
                        .clone();
                    node.position += delta;
                    commands.push(DocumentCommand::UpdateNode(node));
                }
                self.apply_all(commands)?;

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
            .hit_test(
                point,
                HitOptions {
                    include_handles: true,
                    ..HitOptions::default()
                },
            )
            .find_map(|record| match &record.target {
                HitTarget::Handle { node_id, handle_id } => Some(CanvasEndpoint {
                    node_id: node_id.clone(),
                    handle_id: Some(handle_id.clone()),
                }),
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
        assert_eq!(
            editor.selection.nodes.iter().cloned().collect::<Vec<_>>(),
            vec![NodeId::from("n1")]
        );
        assert_eq!(editor.state, ToolState::Idle);
    }

    #[test]
    fn select_tool_clears_selection_when_canvas_is_pressed() {
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
        assert!(!editor.selection.is_empty());

        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(10.0), px(10.0)),
                button: PointerButton::Primary,
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(300.0), px(300.0)),
                button: PointerButton::Primary,
            })
            .unwrap();

        assert!(editor.selection.is_empty());
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

    #[test]
    fn connect_tool_uses_handles_when_available() {
        use crate::{CanvasHandle, HandleId};

        let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        source
            .handles
            .push(CanvasHandle::new("out", point(px(100.0), px(50.0))));

        let mut target =
            CanvasNode::new("b", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
        target
            .handles
            .push(CanvasHandle::new("in", point(px(0.0), px(50.0))));

        let mut document = CanvasDocument::default();
        document.insert_node(source).unwrap();
        document.insert_node(target).unwrap();
        let mut editor = CanvasEditor::new(document);
        editor.set_tool(CanvasTool::Connect);

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(100.0), px(50.0)),
                button: PointerButton::Primary,
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(200.0), px(50.0)),
                button: PointerButton::Primary,
            })
            .unwrap();

        let edge = editor.document.edges.values().next().unwrap();
        assert_eq!(edge.source.handle_id, Some(HandleId::from("out")));
        assert_eq!(edge.target.handle_id, Some(HandleId::from("in")));
    }
}
