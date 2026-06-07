use crate::{
    CanvasDocument, CanvasDocumentDiff, CanvasEdge, CanvasEndpoint, CanvasNode, CanvasTransaction,
    CanvasViewport, DocumentCommand, DocumentError, EdgeId, HitOptions, HitTarget, NodeId, ShapeId,
    SpatialIndex,
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
        original_nodes: Vec<CanvasNode>,
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

    pub fn retain_document(&mut self, document: &CanvasDocument) {
        self.nodes.retain(|id| document.nodes.contains_key(id));
        self.edges.retain(|id| document.edges.contains_key(id));
        self.shapes.retain(|id| document.shapes.contains_key(id));
        self.handles
            .retain(|endpoint| document.validate_endpoint(endpoint).is_ok());
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CanvasHistory {
    undo_stack: Vec<CanvasTransaction>,
    redo_stack: Vec<CanvasTransaction>,
}

impl CanvasHistory {
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_depth(&self) -> usize {
        self.redo_stack.len()
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    fn push_undo(&mut self, transaction: CanvasTransaction) {
        if !transaction.is_empty() {
            self.undo_stack.push(transaction);
            self.redo_stack.clear();
        }
    }

    fn pop_undo(&mut self) -> Option<CanvasTransaction> {
        self.undo_stack.pop()
    }

    fn push_redo(&mut self, transaction: CanvasTransaction) {
        if !transaction.is_empty() {
            self.redo_stack.push(transaction);
        }
    }

    fn pop_redo(&mut self) -> Option<CanvasTransaction> {
        self.redo_stack.pop()
    }
}

pub struct CanvasEditor {
    pub document: CanvasDocument,
    pub viewport: CanvasViewport,
    pub tool: CanvasTool,
    pub state: ToolState,
    pub index: SpatialIndex,
    pub selection: CanvasSelection,
    pub history: CanvasHistory,
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
            history: CanvasHistory::default(),
        }
    }

    pub fn apply(&mut self, command: DocumentCommand) -> Result<(), DocumentError> {
        self.apply_transaction(CanvasTransaction::single(command))
    }

    pub fn apply_all(
        &mut self,
        commands: impl IntoIterator<Item = DocumentCommand>,
    ) -> Result<(), DocumentError> {
        self.apply_transaction(CanvasTransaction::new(commands))
    }

    pub fn apply_transaction(
        &mut self,
        transaction: CanvasTransaction,
    ) -> Result<(), DocumentError> {
        self.apply_transaction_with_diff(transaction).map(drop)
    }

    pub fn apply_transaction_with_diff(
        &mut self,
        transaction: CanvasTransaction,
    ) -> Result<CanvasDocumentDiff, DocumentError> {
        if transaction.is_empty() {
            return Ok(CanvasDocumentDiff::default());
        }

        let inverse = self.document.invert_transaction(&transaction)?;
        let diff = self.document.apply_transaction_with_diff(transaction)?;
        self.history.push_undo(inverse);
        self.selection.retain_document(&self.document);
        self.rebuild_index();
        Ok(diff)
    }

    pub fn undo(&mut self) -> Result<bool, DocumentError> {
        let Some(transaction) = self.history.pop_undo() else {
            return Ok(false);
        };

        let redo = self.document.invert_transaction(&transaction)?;
        self.document.apply_transaction(transaction)?;
        self.history.push_redo(redo);
        self.selection.retain_document(&self.document);
        self.rebuild_index();
        Ok(true)
    }

    pub fn redo(&mut self) -> Result<bool, DocumentError> {
        let Some(transaction) = self.history.pop_redo() else {
            return Ok(false);
        };

        let undo = self.document.invert_transaction(&transaction)?;
        self.document.apply_transaction(transaction)?;
        self.history.push_undo(undo);
        self.selection.retain_document(&self.document);
        self.rebuild_index();
        Ok(true)
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
                        let original_nodes = self
                            .selection
                            .selected_nodes()
                            .filter_map(|id| self.document.nodes.get(id).cloned())
                            .collect();
                        self.state = ToolState::Translating {
                            origin: document_position,
                            last: document_position,
                            node_ids: vec![id],
                            original_nodes,
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
                self.apply_unrecorded(CanvasTransaction::new(commands))?;

                if let ToolState::Translating { last, .. } = &mut self.state {
                    *last = document_position;
                }
            }
            (ToolState::Translating { original_nodes, .. }, CanvasEvent::PointerUp { .. }) => {
                let inverse = self.inverse_for_changed_nodes(original_nodes);
                self.history.push_undo(inverse);
                self.state = ToolState::Idle;
            }
            (ToolState::Translating { original_nodes, .. }, CanvasEvent::Cancel) => {
                let inverse = CanvasTransaction::new(
                    original_nodes
                        .iter()
                        .cloned()
                        .map(DocumentCommand::UpdateNode),
                );
                self.apply_unrecorded(inverse)?;
                self.state = ToolState::Idle;
            }
            (ToolState::Pointing { .. }, CanvasEvent::PointerUp { .. } | CanvasEvent::Cancel) => {
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

    fn apply_unrecorded(&mut self, transaction: CanvasTransaction) -> Result<(), DocumentError> {
        self.document.apply_transaction(transaction)?;
        self.selection.retain_document(&self.document);
        self.rebuild_index();
        Ok(())
    }

    fn inverse_for_changed_nodes(&self, original_nodes: &[CanvasNode]) -> CanvasTransaction {
        CanvasTransaction::new(
            original_nodes
                .iter()
                .filter(|node| self.document.nodes.get(&node.id) != Some(*node))
                .cloned()
                .map(DocumentCommand::UpdateNode),
        )
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
        assert_eq!(editor.history.undo_depth(), 1);

        assert!(editor.undo().unwrap());
        let node = editor.document.nodes.get(&NodeId::from("n1")).unwrap();
        assert_eq!(node.position, point(px(0.0), px(0.0)));
        assert_eq!(editor.history.redo_depth(), 1);

        assert!(editor.redo().unwrap());
        let node = editor.document.nodes.get(&NodeId::from("n1")).unwrap();
        assert_eq!(node.position, point(px(10.0), px(15.0)));
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
        assert_eq!(editor.history.undo_depth(), 1);

        assert!(editor.undo().unwrap());
        assert!(editor.document.edges.is_empty());

        assert!(editor.redo().unwrap());
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

    #[test]
    fn direct_transactions_clear_redo_history() {
        let mut editor = CanvasEditor::default();
        editor
            .apply(DocumentCommand::InsertNode(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            )))
            .unwrap();

        assert!(editor.undo().unwrap());
        assert_eq!(editor.history.redo_depth(), 1);

        editor
            .apply(DocumentCommand::InsertNode(CanvasNode::new(
                "b",
                point(px(100.0), px(0.0)),
                size(px(100.0), px(100.0)),
            )))
            .unwrap();

        assert_eq!(editor.history.undo_depth(), 1);
        assert_eq!(editor.history.redo_depth(), 0);
        assert!(editor.document.nodes.contains_key(&NodeId::from("b")));
        assert!(!editor.document.nodes.contains_key(&NodeId::from("a")));
    }

    #[test]
    fn editor_transactions_return_document_diff() {
        let mut editor = CanvasEditor::default();

        let diff = editor
            .apply_transaction_with_diff(CanvasTransaction::single(DocumentCommand::InsertNode(
                CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            )))
            .unwrap();

        assert_eq!(
            diff.inserted.iter().cloned().collect::<Vec<_>>(),
            vec![crate::CanvasRecordId::Node(NodeId::from("a"))]
        );
        assert!(editor.history.can_undo());
    }

    #[test]
    fn selection_discards_removed_records_after_transaction() {
        let mut editor = CanvasEditor::default();
        editor
            .apply(DocumentCommand::InsertNode(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            )))
            .unwrap();
        editor.selection.nodes.insert(NodeId::from("a"));

        editor
            .apply(DocumentCommand::RemoveNode(NodeId::from("a")))
            .unwrap();

        assert!(editor.selection.is_empty());
    }
}
