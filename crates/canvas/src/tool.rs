use crate::{
    CanvasDocument, CanvasDocumentDiff, CanvasEdge, CanvasEndpoint, CanvasNode, CanvasTransaction,
    CanvasValue, CanvasViewport, DocumentCommand, DocumentError, EdgeId, HitOptions, HitRecord,
    HitTarget, NodeId, ShapeId, SpatialIndex,
};
use indexmap::{IndexMap, IndexSet};
use open_gpui::{Bounds, Pixels, Point};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

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

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CanvasToolId(String);

impl CanvasToolId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for CanvasToolId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for CanvasToolId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for CanvasToolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CanvasTool {
    Select,
    Pan,
    Connect,
    Custom(CanvasToolId),
}

impl CanvasTool {
    pub fn custom(id: impl Into<CanvasToolId>) -> Self {
        Self::Custom(id.into())
    }

    pub fn custom_id(&self) -> Option<&CanvasToolId> {
        match self {
            Self::Custom(id) => Some(id),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToolState {
    Idle,
    Pointing {
        origin: Point<Pixels>,
    },
    Selecting {
        origin: Point<Pixels>,
        current: Point<Pixels>,
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
    Custom {
        tool_id: CanvasToolId,
        payload: CanvasValue,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CanvasToolEffect {
    ApplyTransaction(CanvasTransaction),
    ApplyUnrecorded(CanvasTransaction),
    PushUndo(CanvasTransaction),
    SetTool(CanvasTool),
    SetSelection(CanvasSelection),
    ReplaceSelection(HitTarget),
    ClearSelection,
    SetState(ToolState),
    PanViewport(Point<Pixels>),
    SetViewport(CanvasViewport),
}

#[derive(Clone, Copy, Debug)]
pub struct CanvasToolContext<'a> {
    pub document: &'a CanvasDocument,
    pub viewport: &'a CanvasViewport,
    pub tool: &'a CanvasTool,
    pub state: &'a ToolState,
    pub index: &'a SpatialIndex,
    pub selection: &'a CanvasSelection,
    pub history: &'a CanvasHistory,
}

impl CanvasToolContext<'_> {
    pub fn active_custom_tool_id(&self) -> Option<&CanvasToolId> {
        self.tool.custom_id()
    }

    pub fn document_position(&self, view_position: Point<Pixels>) -> Point<Pixels> {
        self.viewport.view_to_document(view_position)
    }

    pub fn hit_test_view(
        &self,
        view_position: Point<Pixels>,
        options: HitOptions,
    ) -> impl Iterator<Item = &HitRecord> {
        self.index
            .hit_test(self.document_position(view_position), options)
    }
}

pub trait CanvasToolReducer {
    fn handle_event(
        &mut self,
        context: CanvasToolContext<'_>,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError>;
}

impl<F> CanvasToolReducer for F
where
    F: for<'a> FnMut(
        CanvasToolContext<'a>,
        CanvasEvent,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError>,
{
    fn handle_event(
        &mut self,
        context: CanvasToolContext<'_>,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        self(context, event)
    }
}

#[derive(Default)]
pub struct CanvasToolRegistry {
    reducers: IndexMap<CanvasToolId, Box<dyn CanvasToolReducer>>,
}

impl CanvasToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<T>(
        &mut self,
        id: impl Into<CanvasToolId>,
        reducer: T,
    ) -> Option<Box<dyn CanvasToolReducer>>
    where
        T: CanvasToolReducer + 'static,
    {
        self.reducers.insert(id.into(), Box::new(reducer))
    }

    pub fn insert_boxed(
        &mut self,
        id: impl Into<CanvasToolId>,
        reducer: Box<dyn CanvasToolReducer>,
    ) -> Option<Box<dyn CanvasToolReducer>> {
        self.reducers.insert(id.into(), reducer)
    }

    pub fn remove(&mut self, id: &CanvasToolId) -> Option<Box<dyn CanvasToolReducer>> {
        self.reducers.shift_remove(id)
    }

    pub fn contains(&self, id: &CanvasToolId) -> bool {
        self.reducers.contains_key(id)
    }

    pub fn reducer_mut(&mut self, id: &CanvasToolId) -> Option<&mut (dyn CanvasToolReducer + '_)> {
        let reducer = self.reducers.get_mut(id)?;
        Some(reducer.as_mut())
    }

    pub fn ids(&self) -> impl Iterator<Item = &CanvasToolId> {
        self.reducers.keys()
    }

    pub fn len(&self) -> usize {
        self.reducers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.reducers.is_empty()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CanvasToolRegistryError {
    MissingTool(CanvasToolId),
    Document(DocumentError),
}

impl From<DocumentError> for CanvasToolRegistryError {
    fn from(value: DocumentError) -> Self {
        Self::Document(value)
    }
}

impl fmt::Display for CanvasToolRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTool(id) => write!(f, "canvas custom tool `{id}` is not registered"),
            Self::Document(error) => fmt::Display::fmt(error, f),
        }
    }
}

impl Error for CanvasToolRegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MissingTool(_) => None,
            Self::Document(error) => Some(error),
        }
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

    pub fn next_undo_transaction(&self) -> Option<&CanvasTransaction> {
        self.undo_stack.last()
    }

    pub fn next_redo_transaction(&self) -> Option<&CanvasTransaction> {
        self.redo_stack.last()
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
        self.index.apply_diff(&self.document, &diff);
        Ok(diff)
    }

    pub fn apply_tool_effect(&mut self, effect: CanvasToolEffect) -> Result<(), DocumentError> {
        match effect {
            CanvasToolEffect::ApplyTransaction(transaction) => {
                self.apply_transaction(transaction)?;
            }
            CanvasToolEffect::ApplyUnrecorded(transaction) => {
                self.apply_unrecorded(transaction)?;
            }
            CanvasToolEffect::PushUndo(transaction) => {
                self.history.push_undo(transaction);
            }
            CanvasToolEffect::SetTool(tool) => {
                self.set_tool(tool);
            }
            CanvasToolEffect::SetSelection(mut selection) => {
                selection.retain_document(&self.document);
                self.selection = selection;
            }
            CanvasToolEffect::ReplaceSelection(target) => {
                self.selection.replace_with(target);
                self.selection.retain_document(&self.document);
            }
            CanvasToolEffect::ClearSelection => {
                self.selection.clear();
            }
            CanvasToolEffect::SetState(state) => {
                self.state = state;
            }
            CanvasToolEffect::PanViewport(delta) => {
                self.viewport.pan_by(delta);
            }
            CanvasToolEffect::SetViewport(viewport) => {
                self.viewport = viewport;
            }
        }

        Ok(())
    }

    pub fn apply_tool_effects(
        &mut self,
        effects: impl IntoIterator<Item = CanvasToolEffect>,
    ) -> Result<(), DocumentError> {
        for effect in effects {
            self.apply_tool_effect(effect)?;
        }

        Ok(())
    }

    pub fn undo(&mut self) -> Result<bool, DocumentError> {
        let Some(transaction) = self.history.pop_undo() else {
            return Ok(false);
        };

        let redo = self.document.invert_transaction(&transaction)?;
        let diff = self.document.apply_transaction_with_diff(transaction)?;
        self.history.push_redo(redo);
        self.selection.retain_document(&self.document);
        self.index.apply_diff(&self.document, &diff);
        Ok(true)
    }

    pub fn redo(&mut self) -> Result<bool, DocumentError> {
        let Some(transaction) = self.history.pop_redo() else {
            return Ok(false);
        };

        let undo = self.document.invert_transaction(&transaction)?;
        let diff = self.document.apply_transaction_with_diff(transaction)?;
        self.history.push_undo(undo);
        self.selection.retain_document(&self.document);
        self.index.apply_diff(&self.document, &diff);
        Ok(true)
    }

    pub fn rebuild_index(&mut self) {
        self.index = SpatialIndex::rebuild(&self.document);
    }

    pub fn set_tool(&mut self, tool: CanvasTool) {
        self.tool = tool;
        self.state = ToolState::Idle;
    }

    pub fn tool_context(&self) -> CanvasToolContext<'_> {
        CanvasToolContext {
            document: &self.document,
            viewport: &self.viewport,
            tool: &self.tool,
            state: &self.state,
            index: &self.index,
            selection: &self.selection,
            history: &self.history,
        }
    }

    pub fn handle_event(&mut self, event: CanvasEvent) -> Result<(), DocumentError> {
        let effects = self.event_effects(event)?;
        self.apply_tool_effects(effects)
    }

    pub fn event_effects(
        &self,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        Ok(match &self.tool {
            CanvasTool::Select => self.select_effects(event)?,
            CanvasTool::Pan => self.pan_effects(event),
            CanvasTool::Connect => self.connect_effects(event),
            CanvasTool::Custom(_) => Vec::new(),
        })
    }

    pub fn handle_event_with_custom_tool<T>(
        &mut self,
        event: CanvasEvent,
        custom_tool: &mut T,
    ) -> Result<(), DocumentError>
    where
        T: CanvasToolReducer + ?Sized,
    {
        let effects = self.event_effects_with_custom_tool(event, custom_tool)?;
        self.apply_tool_effects(effects)
    }

    pub fn event_effects_with_custom_tool<T>(
        &self,
        event: CanvasEvent,
        custom_tool: &mut T,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError>
    where
        T: CanvasToolReducer + ?Sized,
    {
        Ok(match &self.tool {
            CanvasTool::Select => self.select_effects(event)?,
            CanvasTool::Pan => self.pan_effects(event),
            CanvasTool::Connect => self.connect_effects(event),
            CanvasTool::Custom(_) => custom_tool.handle_event(self.tool_context(), event)?,
        })
    }

    pub fn handle_event_with_tool_registry(
        &mut self,
        event: CanvasEvent,
        registry: &mut CanvasToolRegistry,
    ) -> Result<(), CanvasToolRegistryError> {
        let effects = self.event_effects_with_tool_registry(event, registry)?;
        self.apply_tool_effects(effects)?;
        Ok(())
    }

    pub fn event_effects_with_tool_registry(
        &self,
        event: CanvasEvent,
        registry: &mut CanvasToolRegistry,
    ) -> Result<Vec<CanvasToolEffect>, CanvasToolRegistryError> {
        let Some(tool_id) = self.tool.custom_id().cloned() else {
            return Ok(self.event_effects(event)?);
        };

        let reducer = registry
            .reducer_mut(&tool_id)
            .ok_or_else(|| CanvasToolRegistryError::MissingTool(tool_id.clone()))?;
        Ok(self.event_effects_with_custom_tool(event, reducer)?)
    }

    fn select_effects(&self, event: CanvasEvent) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        let effects = match (&self.state, event) {
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
                        let mut selection = self.selection.clone();
                        if !selection.nodes.contains(&id) {
                            selection.replace_with(HitTarget::Node(id.clone()));
                        }
                        let original_nodes = self
                            .document_nodes_for_selection(&selection)
                            .collect::<Vec<_>>();
                        let node_ids = original_nodes.iter().map(|node| node.id.clone()).collect();
                        vec![
                            CanvasToolEffect::SetSelection(selection),
                            CanvasToolEffect::SetState(ToolState::Translating {
                                origin: document_position,
                                last: document_position,
                                node_ids,
                                original_nodes,
                            }),
                        ]
                    }
                    Some(target) => {
                        vec![
                            CanvasToolEffect::ReplaceSelection(target),
                            CanvasToolEffect::SetState(ToolState::Pointing {
                                origin: document_position,
                            }),
                        ]
                    }
                    None => {
                        vec![
                            CanvasToolEffect::ClearSelection,
                            CanvasToolEffect::SetState(ToolState::Pointing {
                                origin: document_position,
                            }),
                        ]
                    }
                }
            }
            (
                ToolState::Translating {
                    last,
                    node_ids,
                    origin,
                    original_nodes,
                },
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

                vec![
                    CanvasToolEffect::ApplyUnrecorded(CanvasTransaction::new(commands)),
                    CanvasToolEffect::SetState(ToolState::Translating {
                        origin: *origin,
                        last: document_position,
                        node_ids: node_ids.clone(),
                        original_nodes: original_nodes.clone(),
                    }),
                ]
            }
            (ToolState::Pointing { origin }, CanvasEvent::PointerMove { position }) => {
                let origin = *origin;
                let document_position = self.viewport.view_to_document(position);
                vec![
                    CanvasToolEffect::SetSelection(
                        self.selection_for_intersections(selection_bounds(
                            origin,
                            document_position,
                        )),
                    ),
                    CanvasToolEffect::SetState(ToolState::Selecting {
                        origin,
                        current: document_position,
                    }),
                ]
            }
            (ToolState::Selecting { origin, .. }, CanvasEvent::PointerMove { position }) => {
                let origin = *origin;
                let document_position = self.viewport.view_to_document(position);
                vec![
                    CanvasToolEffect::SetSelection(
                        self.selection_for_intersections(selection_bounds(
                            origin,
                            document_position,
                        )),
                    ),
                    CanvasToolEffect::SetState(ToolState::Selecting {
                        origin,
                        current: document_position,
                    }),
                ]
            }
            (ToolState::Translating { original_nodes, .. }, CanvasEvent::PointerUp { .. }) => {
                vec![
                    CanvasToolEffect::PushUndo(self.inverse_for_changed_nodes(original_nodes)),
                    CanvasToolEffect::SetState(ToolState::Idle),
                ]
            }
            (ToolState::Translating { original_nodes, .. }, CanvasEvent::Cancel) => {
                let inverse = CanvasTransaction::new(
                    original_nodes
                        .iter()
                        .cloned()
                        .map(DocumentCommand::UpdateNode),
                );
                vec![
                    CanvasToolEffect::ApplyUnrecorded(inverse),
                    CanvasToolEffect::SetState(ToolState::Idle),
                ]
            }
            (ToolState::Pointing { .. }, CanvasEvent::PointerUp { .. } | CanvasEvent::Cancel) => {
                vec![CanvasToolEffect::SetState(ToolState::Idle)]
            }
            (ToolState::Selecting { .. }, CanvasEvent::PointerUp { .. } | CanvasEvent::Cancel) => {
                vec![CanvasToolEffect::SetState(ToolState::Idle)]
            }
            (_, CanvasEvent::Wheel { delta }) => {
                vec![CanvasToolEffect::PanViewport(delta)]
            }
            _ => Vec::new(),
        };

        Ok(effects)
    }

    fn pan_effects(&self, event: CanvasEvent) -> Vec<CanvasToolEffect> {
        match (&self.state, event) {
            (
                ToolState::Idle,
                CanvasEvent::PointerDown {
                    position,
                    button: PointerButton::Primary,
                },
            ) => {
                vec![CanvasToolEffect::SetState(ToolState::Panning {
                    origin: position,
                    last: position,
                })]
            }
            (ToolState::Panning { last, origin }, CanvasEvent::PointerMove { position }) => {
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
        }
    }

    fn connect_effects(&self, event: CanvasEvent) -> Vec<CanvasToolEffect> {
        match (&self.state, event) {
            (
                ToolState::Idle,
                CanvasEvent::PointerDown {
                    position,
                    button: PointerButton::Primary,
                },
            ) => {
                let document_position = self.viewport.view_to_document(position);
                self.node_endpoint_at(document_position)
                    .map(|source| {
                        vec![CanvasToolEffect::SetState(ToolState::Connecting {
                            source,
                            current: document_position,
                        })]
                    })
                    .unwrap_or_default()
            }
            (ToolState::Connecting { source, .. }, CanvasEvent::PointerMove { position }) => {
                let document_position = self.viewport.view_to_document(position);
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
                },
            ) => {
                let document_position = self.viewport.view_to_document(position);
                let mut effects = Vec::new();
                if let Some(target) = self.node_endpoint_at(document_position)
                    && (source.node_id != target.node_id || source.handle_id != target.handle_id)
                {
                    let edge_id = EdgeId::new(format!(
                        "{}->{}:{}",
                        source.node_id,
                        target.node_id,
                        self.document.edges.len()
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
        }
    }

    fn document_nodes_for_selection<'a>(
        &'a self,
        selection: &'a CanvasSelection,
    ) -> impl Iterator<Item = CanvasNode> + 'a {
        selection
            .selected_nodes()
            .filter_map(|id| self.document.nodes.get(id))
            .filter(|node| !node.locked)
            .cloned()
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
        let diff = self.document.apply_transaction_with_diff(transaction)?;
        self.selection.retain_document(&self.document);
        self.index.apply_diff(&self.document, &diff);
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

    fn selection_for_intersections(&self, bounds: Bounds<Pixels>) -> CanvasSelection {
        let mut selection = CanvasSelection::default();
        for record in self.index.query_with_options(bounds, HitOptions::default()) {
            match &record.target {
                HitTarget::Node(id) => {
                    selection.nodes.insert(id.clone());
                }
                HitTarget::Edge(id) => {
                    selection.edges.insert(id.clone());
                }
                HitTarget::Shape(id) => {
                    selection.shapes.insert(id.clone());
                }
                HitTarget::Handle { .. } => {}
            }
        }
        selection
    }
}

fn selection_bounds(origin: Point<Pixels>, current: Point<Pixels>) -> Bounds<Pixels> {
    Bounds::from_corners(
        Point::new(origin.x.min(current.x), origin.y.min(current.y)),
        Point::new(origin.x.max(current.x), origin.y.max(current.y)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CanvasNode;
    use open_gpui::{point, px, size};

    #[derive(Default)]
    struct StampTool {
        calls: usize,
        last_tool_id: Option<CanvasToolId>,
        last_hit: Option<HitTarget>,
    }

    impl CanvasToolReducer for StampTool {
        fn handle_event(
            &mut self,
            context: CanvasToolContext<'_>,
            event: CanvasEvent,
        ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
            self.calls += 1;
            self.last_tool_id = context.active_custom_tool_id().cloned();

            let CanvasEvent::PointerDown {
                position,
                button: PointerButton::Primary,
            } = event
            else {
                return Ok(Vec::new());
            };

            self.last_hit = context
                .hit_test_view(position, HitOptions::default())
                .next()
                .map(|record| record.target.clone());

            let node_id = NodeId::new(format!("stamp-{}", context.document.nodes.len()));
            let mut selection = CanvasSelection::default();
            selection.nodes.insert(node_id.clone());
            let mut payload = CanvasValue::new();
            payload.insert("phase".into(), serde_json::Value::String("pressed".into()));

            Ok(vec![
                CanvasToolEffect::ApplyTransaction(CanvasTransaction::single(
                    DocumentCommand::InsertNode(CanvasNode::new(
                        node_id.clone(),
                        context.document_position(position),
                        size(px(20.0), px(20.0)),
                    )),
                )),
                CanvasToolEffect::SetSelection(selection),
                CanvasToolEffect::SetState(ToolState::Custom {
                    tool_id: self
                        .last_tool_id
                        .clone()
                        .unwrap_or_else(|| CanvasToolId::from("stamp")),
                    payload,
                }),
            ])
        }
    }

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
    fn select_tool_ignores_locked_node_hits() {
        let mut node = CanvasNode::new(
            "locked",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        );
        node.locked = true;
        let mut document = CanvasDocument::default();
        document.insert_node(node).unwrap();
        let mut editor = CanvasEditor::new(document);

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(10.0), px(10.0)),
                button: PointerButton::Primary,
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(30.0), px(30.0)),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(30.0), px(30.0)),
                button: PointerButton::Primary,
            })
            .unwrap();

        assert!(editor.selection.is_empty());
        assert_eq!(
            editor.document.nodes[&NodeId::from("locked")].position,
            point(px(0.0), px(0.0))
        );
        assert_eq!(editor.history.undo_depth(), 0);
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
    fn select_tool_box_selects_intersecting_records() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "inside",
                point(px(10.0), px(10.0)),
                size(px(20.0), px(20.0)),
            ))
            .unwrap();
        document
            .insert_node(CanvasNode::new(
                "outside",
                point(px(200.0), px(200.0)),
                size(px(20.0), px(20.0)),
            ))
            .unwrap();
        let mut locked = CanvasNode::new(
            "locked",
            point(px(15.0), px(15.0)),
            size(px(20.0), px(20.0)),
        );
        locked.locked = true;
        document.insert_node(locked).unwrap();
        let mut editor = CanvasEditor::new(document);

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(0.0), px(0.0)),
                button: PointerButton::Primary,
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(50.0), px(50.0)),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(50.0), px(50.0)),
                button: PointerButton::Primary,
            })
            .unwrap();

        assert_eq!(
            editor.selection.nodes.iter().cloned().collect::<Vec<_>>(),
            vec![NodeId::from("inside")]
        );
        assert_eq!(editor.state, ToolState::Idle);
    }

    #[test]
    fn translating_selected_node_moves_all_selected_nodes() {
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
        editor.selection.nodes.insert(NodeId::from("a"));
        editor.selection.nodes.insert(NodeId::from("b"));

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(10.0), px(10.0)),
                button: PointerButton::Primary,
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(20.0), px(30.0)),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(20.0), px(30.0)),
                button: PointerButton::Primary,
            })
            .unwrap();

        assert_eq!(
            editor.document.nodes[&NodeId::from("a")].position,
            point(px(10.0), px(20.0))
        );
        assert_eq!(
            editor.document.nodes[&NodeId::from("b")].position,
            point(px(210.0), px(20.0))
        );
        assert_eq!(editor.history.undo_depth(), 1);
    }

    #[test]
    fn translating_selected_nodes_skips_locked_nodes() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "free",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .unwrap();
        let mut locked = CanvasNode::new(
            "locked",
            point(px(200.0), px(0.0)),
            size(px(100.0), px(100.0)),
        );
        locked.locked = true;
        document.insert_node(locked).unwrap();
        let mut editor = CanvasEditor::new(document);
        editor.selection.nodes.insert(NodeId::from("free"));
        editor.selection.nodes.insert(NodeId::from("locked"));

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(10.0), px(10.0)),
                button: PointerButton::Primary,
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(20.0), px(30.0)),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(20.0), px(30.0)),
                button: PointerButton::Primary,
            })
            .unwrap();

        assert_eq!(
            editor.document.nodes[&NodeId::from("free")].position,
            point(px(10.0), px(20.0))
        );
        assert_eq!(
            editor.document.nodes[&NodeId::from("locked")].position,
            point(px(200.0), px(0.0))
        );
        assert_eq!(editor.history.undo_depth(), 1);
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
    fn connect_tool_ignores_locked_endpoints() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .unwrap();
        let mut locked =
            CanvasNode::new("b", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
        locked.locked = true;
        document.insert_node(locked).unwrap();
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

        assert!(editor.document.edges.is_empty());
        assert_eq!(editor.history.undo_depth(), 0);
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
    fn custom_tool_reducer_applies_effects_through_editor() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "anchor",
                point(px(100.0), px(50.0)),
                size(px(80.0), px(80.0)),
            ))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor.viewport = CanvasViewport::new(point(px(100.0), px(50.0)), 2.0).unwrap();
        editor.set_tool(CanvasTool::custom("stamp"));
        let mut tool = StampTool::default();

        editor
            .handle_event_with_custom_tool(
                CanvasEvent::PointerDown {
                    position: point(px(20.0), px(10.0)),
                    button: PointerButton::Primary,
                },
                &mut tool,
            )
            .unwrap();

        assert_eq!(tool.calls, 1);
        assert_eq!(tool.last_tool_id, Some(CanvasToolId::from("stamp")));
        assert_eq!(tool.last_hit, Some(HitTarget::Node(NodeId::from("anchor"))));

        let stamped = editor.document.nodes.get(&NodeId::from("stamp-1")).unwrap();
        assert_eq!(stamped.position, point(px(110.0), px(55.0)));
        assert_eq!(editor.history.undo_depth(), 1);
        assert_eq!(
            editor.selection.nodes.iter().cloned().collect::<Vec<_>>(),
            vec![NodeId::from("stamp-1")]
        );
        assert!(matches!(
            editor.state,
            ToolState::Custom {
                ref tool_id,
                ..
            } if tool_id == &CanvasToolId::from("stamp")
        ));

        assert!(editor.undo().unwrap());
        assert!(!editor.document.nodes.contains_key(&NodeId::from("stamp-1")));
    }

    #[test]
    fn custom_tool_entry_uses_builtin_tools_without_calling_custom_reducer() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "n1",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        let mut tool = StampTool::default();

        editor
            .handle_event_with_custom_tool(
                CanvasEvent::PointerDown {
                    position: point(px(10.0), px(10.0)),
                    button: PointerButton::Primary,
                },
                &mut tool,
            )
            .unwrap();

        assert_eq!(tool.calls, 0);
        assert_eq!(
            editor.selection.nodes.iter().cloned().collect::<Vec<_>>(),
            vec![NodeId::from("n1")]
        );
    }

    #[test]
    fn tool_registry_dispatches_registered_custom_tool() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "anchor",
                point(px(100.0), px(50.0)),
                size(px(80.0), px(80.0)),
            ))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor.viewport = CanvasViewport::new(point(px(100.0), px(50.0)), 2.0).unwrap();
        editor.set_tool(CanvasTool::custom("stamp"));
        let mut registry = CanvasToolRegistry::new();

        assert!(registry.is_empty());
        assert!(registry.insert("stamp", StampTool::default()).is_none());
        assert!(registry.contains(&CanvasToolId::from("stamp")));
        assert_eq!(
            registry.ids().cloned().collect::<Vec<_>>(),
            vec![CanvasToolId::from("stamp")]
        );

        editor
            .handle_event_with_tool_registry(
                CanvasEvent::PointerDown {
                    position: point(px(20.0), px(10.0)),
                    button: PointerButton::Primary,
                },
                &mut registry,
            )
            .unwrap();

        let stamped = editor.document.nodes.get(&NodeId::from("stamp-1")).unwrap();
        assert_eq!(stamped.position, point(px(110.0), px(55.0)));
        assert_eq!(editor.history.undo_depth(), 1);
        assert!(registry.remove(&CanvasToolId::from("stamp")).is_some());
        assert!(!registry.contains(&CanvasToolId::from("stamp")));
    }

    #[test]
    fn tool_registry_accepts_boxed_reducers() {
        let mut registry = CanvasToolRegistry::new();

        assert!(
            registry
                .insert_boxed("stamp", Box::new(StampTool::default()))
                .is_none()
        );

        assert_eq!(registry.len(), 1);
        assert!(registry.reducer_mut(&CanvasToolId::from("stamp")).is_some());
    }

    #[test]
    fn tool_registry_reports_missing_custom_tool() {
        let mut editor = CanvasEditor::default();
        editor.set_tool(CanvasTool::custom("missing"));
        let mut registry = CanvasToolRegistry::new();

        let err = editor
            .handle_event_with_tool_registry(CanvasEvent::Cancel, &mut registry)
            .unwrap_err();

        assert_eq!(
            err,
            CanvasToolRegistryError::MissingTool(CanvasToolId::from("missing"))
        );
    }

    #[test]
    fn tool_registry_entry_uses_builtin_tools_without_registered_reducer() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "n1",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        let mut registry = CanvasToolRegistry::new();

        editor
            .handle_event_with_tool_registry(
                CanvasEvent::PointerDown {
                    position: point(px(10.0), px(10.0)),
                    button: PointerButton::Primary,
                },
                &mut registry,
            )
            .unwrap();

        assert_eq!(
            editor.selection.nodes.iter().cloned().collect::<Vec<_>>(),
            vec![NodeId::from("n1")]
        );
    }

    #[test]
    fn set_tool_effect_switches_tool_and_resets_state() {
        let mut editor = CanvasEditor::default();
        editor.state = ToolState::Pointing {
            origin: point(px(10.0), px(20.0)),
        };

        editor
            .apply_tool_effect(CanvasToolEffect::SetTool(CanvasTool::custom("stamp")))
            .unwrap();

        assert_eq!(editor.tool, CanvasTool::custom("stamp"));
        assert_eq!(editor.state, ToolState::Idle);
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
    fn tool_effect_applies_recorded_transaction() {
        let mut editor = CanvasEditor::default();

        editor
            .apply_tool_effect(CanvasToolEffect::ApplyTransaction(
                CanvasTransaction::single(DocumentCommand::InsertNode(CanvasNode::new(
                    "a",
                    point(px(0.0), px(0.0)),
                    size(px(100.0), px(100.0)),
                ))),
            ))
            .unwrap();

        assert!(editor.document.nodes.contains_key(&NodeId::from("a")));
        assert_eq!(editor.history.undo_depth(), 1);
        assert!(
            editor
                .index
                .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
                .next()
                .is_some()
        );
    }

    #[test]
    fn tool_effect_applies_unrecorded_transaction_without_history() {
        let mut editor = CanvasEditor::default();

        editor
            .apply_tool_effect(CanvasToolEffect::ApplyUnrecorded(
                CanvasTransaction::single(DocumentCommand::InsertNode(CanvasNode::new(
                    "a",
                    point(px(0.0), px(0.0)),
                    size(px(100.0), px(100.0)),
                ))),
            ))
            .unwrap();

        assert!(editor.document.nodes.contains_key(&NodeId::from("a")));
        assert_eq!(editor.history.undo_depth(), 0);
        assert!(
            editor
                .index
                .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
                .next()
                .is_some()
        );
    }

    #[test]
    fn tool_effects_update_transient_editor_state() {
        let mut editor = CanvasEditor::default();
        editor
            .apply(DocumentCommand::InsertNode(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            )))
            .unwrap();

        let mut selection = CanvasSelection::default();
        selection.nodes.insert(NodeId::from("a"));
        selection.nodes.insert(NodeId::from("missing"));

        editor
            .apply_tool_effects([
                CanvasToolEffect::SetSelection(selection),
                CanvasToolEffect::SetState(ToolState::Pointing {
                    origin: point(px(10.0), px(20.0)),
                }),
                CanvasToolEffect::PanViewport(point(px(5.0), px(-3.0))),
            ])
            .unwrap();

        assert_eq!(
            editor.selection.nodes.iter().cloned().collect::<Vec<_>>(),
            vec![NodeId::from("a")]
        );
        assert_eq!(
            editor.state,
            ToolState::Pointing {
                origin: point(px(10.0), px(20.0))
            }
        );
        assert_eq!(editor.viewport.origin, point(px(5.0), px(-3.0)));
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

    #[test]
    fn editor_keeps_spatial_index_in_sync_with_transactions() {
        let mut editor = CanvasEditor::default();
        editor
            .apply(DocumentCommand::InsertNode(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            )))
            .unwrap();

        assert!(
            editor
                .index
                .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
                .next()
                .is_some()
        );

        assert!(editor.undo().unwrap());
        assert!(
            editor
                .index
                .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
                .next()
                .is_none()
        );
    }
}
