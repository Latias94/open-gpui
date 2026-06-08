use crate::gesture::{CanvasGestureSession, CanvasPreparedGestureCommit};
use crate::{
    CanvasConnectionEndpointRole, CanvasDefaultEdgeRouter, CanvasDocument, CanvasDocumentDiff,
    CanvasEdge, CanvasEdgeRouter, CanvasEndpoint, CanvasGeometryResolver, CanvasKindRegistry,
    CanvasNode, CanvasRuntime, CanvasTransaction, CanvasValue, CanvasViewport, DocumentCommand,
    DocumentError, EdgeId, HitOptions, HitRecord, HitTarget, NodeId, ShapeId,
    connection_hit_options,
};
use indexmap::{IndexMap, IndexSet};
use open_gpui::{Axis, Bounds, Pixels, Point};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CanvasKey {
    Delete,
    Backspace,
    Escape,
    Enter,
    Character(String),
    Named(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CanvasKeyModifiers {
    pub shift: bool,
    pub alt: bool,
    pub control: bool,
    pub platform: bool,
    pub function: bool,
}

impl CanvasKeyModifiers {
    pub const NONE: Self = Self {
        shift: false,
        alt: false,
        control: false,
        platform: false,
        function: false,
    };

    pub fn modified(self) -> bool {
        self.shift || self.alt || self.control || self.platform || self.function
    }
}

impl Default for CanvasKeyModifiers {
    fn default() -> Self {
        Self::NONE
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CanvasEvent {
    PointerDown {
        position: Point<Pixels>,
        button: PointerButton,
        #[serde(default)]
        modifiers: CanvasKeyModifiers,
    },
    PointerMove {
        position: Point<Pixels>,
        #[serde(default)]
        modifiers: CanvasKeyModifiers,
    },
    PointerUp {
        position: Point<Pixels>,
        button: PointerButton,
        #[serde(default)]
        modifiers: CanvasKeyModifiers,
    },
    Wheel {
        delta: Point<Pixels>,
    },
    KeyDown {
        key: CanvasKey,
        modifiers: CanvasKeyModifiers,
        repeat: bool,
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
        selection_mode: CanvasSelectionMode,
        base_selection: CanvasSelection,
    },
    Selecting {
        origin: Point<Pixels>,
        current: Point<Pixels>,
        selection_mode: CanvasSelectionMode,
        base_selection: CanvasSelection,
    },
    Translating {
        origin: Point<Pixels>,
        last: Point<Pixels>,
        constraint_axis: Option<Axis>,
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
    Custom {
        tool_id: CanvasToolId,
        payload: CanvasValue,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum CanvasSelectionMode {
    #[default]
    Replace,
    Add,
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
        self.insert_target(target);
    }

    pub fn contains_target(&self, target: &HitTarget) -> bool {
        match target {
            HitTarget::Node(id) => self.nodes.contains(id),
            HitTarget::Handle { node_id, handle_id } => self.handles.contains(&CanvasEndpoint {
                node_id: node_id.clone(),
                handle_id: Some(handle_id.clone()),
            }),
            HitTarget::Edge(id) => self.edges.contains(id),
            HitTarget::Shape(id) => self.shapes.contains(id),
        }
    }

    pub fn insert_target(&mut self, target: HitTarget) -> bool {
        match target {
            HitTarget::Node(id) => self.nodes.insert(id),
            HitTarget::Handle { node_id, handle_id } => self.handles.insert(CanvasEndpoint {
                node_id,
                handle_id: Some(handle_id),
            }),
            HitTarget::Edge(id) => self.edges.insert(id),
            HitTarget::Shape(id) => self.shapes.insert(id),
        }
    }

    pub fn remove_target(&mut self, target: &HitTarget) -> bool {
        match target {
            HitTarget::Node(id) => self.nodes.shift_remove(id),
            HitTarget::Handle { node_id, handle_id } => {
                self.handles.shift_remove(&CanvasEndpoint {
                    node_id: node_id.clone(),
                    handle_id: Some(handle_id.clone()),
                })
            }
            HitTarget::Edge(id) => self.edges.shift_remove(id),
            HitTarget::Shape(id) => self.shapes.shift_remove(id),
        }
    }

    pub fn toggle_target(&mut self, target: HitTarget) -> bool {
        if self.contains_target(&target) {
            self.remove_target(&target);
            false
        } else {
            self.insert_target(target);
            true
        }
    }

    pub fn extend_selection(&mut self, selection: CanvasSelection) {
        self.nodes.extend(selection.nodes);
        self.edges.extend(selection.edges);
        self.shapes.extend(selection.shapes);
        self.handles.extend(selection.handles);
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

    pub fn selected_edges(&self) -> impl Iterator<Item = &EdgeId> {
        self.edges.iter()
    }

    pub fn selected_shapes(&self) -> impl Iterator<Item = &ShapeId> {
        self.shapes.iter()
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
    BeginGesture,
    UpdateGesture(CanvasTransaction),
    CommitGesture,
    CancelGesture,
    SetTool(CanvasTool),
    SetSelection(CanvasSelection),
    ReplaceSelection(HitTarget),
    AddSelection(HitTarget),
    RemoveSelection(HitTarget),
    ToggleSelection(HitTarget),
    ClearSelection,
    SetState(ToolState),
    PanViewport(Point<Pixels>),
    SetViewport(CanvasViewport),
}

#[derive(Clone, Copy)]
pub struct CanvasToolContext<'a> {
    pub document: &'a CanvasDocument,
    pub viewport: &'a CanvasViewport,
    pub tool: &'a CanvasTool,
    pub state: &'a ToolState,
    pub runtime: &'a CanvasRuntime,
    pub edge_router: &'a (dyn CanvasEdgeRouter + Send + Sync),
    pub kind_registry: &'a CanvasKindRegistry,
    pub selection: &'a CanvasSelection,
    pub history: &'a CanvasHistory,
}

impl fmt::Debug for CanvasToolContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CanvasToolContext")
            .field("document", self.document)
            .field("viewport", self.viewport)
            .field("tool", self.tool)
            .field("state", self.state)
            .field("runtime", self.runtime)
            .field("edge_router", &"<dyn CanvasEdgeRouter>")
            .field("kind_registry", self.kind_registry)
            .field("selection", self.selection)
            .field("history", self.history)
            .finish()
    }
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
        self.runtime
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
    document: CanvasDocument,
    viewport: CanvasViewport,
    tool: CanvasTool,
    state: ToolState,
    runtime: CanvasRuntime,
    edge_router: Arc<dyn CanvasEdgeRouter + Send + Sync>,
    kind_registry: Arc<CanvasKindRegistry>,
    selection: CanvasSelection,
    history: CanvasHistory,
    gesture: Option<CanvasGestureSession>,
}

impl Default for CanvasEditor {
    fn default() -> Self {
        Self::new(CanvasDocument::default())
    }
}

impl CanvasEditor {
    pub fn new(document: CanvasDocument) -> Self {
        Self::new_with_router(document, CanvasDefaultEdgeRouter)
    }

    pub fn new_with_router<R>(document: CanvasDocument, edge_router: R) -> Self
    where
        R: CanvasEdgeRouter + Send + Sync + 'static,
    {
        let edge_router = Arc::new(edge_router);
        let kind_registry = Arc::new(CanvasKindRegistry::open());
        let runtime = CanvasRuntime::rebuild_with_router_and_kind_registry(
            &document,
            edge_router.as_ref(),
            kind_registry.as_ref(),
        );
        Self {
            document,
            viewport: CanvasViewport::default(),
            tool: CanvasTool::Select,
            state: ToolState::Idle,
            runtime,
            edge_router,
            kind_registry,
            selection: CanvasSelection::default(),
            history: CanvasHistory::default(),
            gesture: None,
        }
    }

    pub fn try_new_with_kind_registry(
        document: CanvasDocument,
        kind_registry: CanvasKindRegistry,
    ) -> Result<Self, DocumentError> {
        Self::try_new_with_router_and_kind_registry(
            document,
            CanvasDefaultEdgeRouter,
            kind_registry,
        )
    }

    pub fn try_new_with_router_and_kind_registry<R>(
        document: CanvasDocument,
        edge_router: R,
        kind_registry: CanvasKindRegistry,
    ) -> Result<Self, DocumentError>
    where
        R: CanvasEdgeRouter + Send + Sync + 'static,
    {
        let document = CanvasDocument::from_snapshot_with_kind_registry(
            document.to_snapshot(),
            &kind_registry,
        )?;
        let edge_router = Arc::new(edge_router);
        let kind_registry = Arc::new(kind_registry);
        let runtime = CanvasRuntime::rebuild_with_router_and_kind_registry(
            &document,
            edge_router.as_ref(),
            kind_registry.as_ref(),
        );
        Ok(Self {
            document,
            viewport: CanvasViewport::default(),
            tool: CanvasTool::Select,
            state: ToolState::Idle,
            runtime,
            edge_router,
            kind_registry,
            selection: CanvasSelection::default(),
            history: CanvasHistory::default(),
            gesture: None,
        })
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

    pub fn document(&self) -> &CanvasDocument {
        &self.document
    }

    pub fn viewport(&self) -> CanvasViewport {
        self.viewport
    }

    pub fn tool(&self) -> &CanvasTool {
        &self.tool
    }

    pub fn state(&self) -> &ToolState {
        &self.state
    }

    pub fn runtime(&self) -> &CanvasRuntime {
        &self.runtime
    }

    pub fn edge_router(&self) -> &(dyn CanvasEdgeRouter + Send + Sync) {
        self.edge_router.as_ref()
    }

    pub fn kind_registry(&self) -> &CanvasKindRegistry {
        self.kind_registry.as_ref()
    }

    pub fn selection(&self) -> &CanvasSelection {
        &self.selection
    }

    pub fn history(&self) -> &CanvasHistory {
        &self.history
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

        let committed = self
            .document
            .commit_transaction_with_kind_registry(transaction, self.kind_registry.as_ref())?;
        let diff = committed.diff().clone();
        self.history.push_undo(committed.inverse().clone());
        self.selection.retain_document(&self.document);
        self.sync_runtime_diff(&diff);
        Ok(diff)
    }

    pub(crate) fn apply_prepared_document_mutation(
        &mut self,
        prepared: crate::journal::CanvasPreparedMutation,
    ) -> CanvasDocumentDiff {
        let committed = prepared.apply_to(&mut self.document);
        let diff = committed.diff().clone();
        self.history.push_undo(committed.inverse().clone());
        self.selection.retain_document(&self.document);
        self.sync_runtime_diff(&diff);
        diff
    }

    pub(crate) fn apply_prepared_undo_mutation(
        &mut self,
        prepared: crate::journal::CanvasPreparedMutation,
    ) -> CanvasDocumentDiff {
        debug_assert_eq!(
            self.history.next_undo_transaction(),
            Some(prepared.committed().transaction())
        );
        let committed = prepared.apply_to(&mut self.document);
        let diff = committed.diff().clone();
        let _ = self.history.pop_undo();
        self.history.push_redo(committed.inverse().clone());
        self.selection.retain_document(&self.document);
        self.sync_runtime_diff(&diff);
        diff
    }

    pub(crate) fn apply_prepared_redo_mutation(
        &mut self,
        prepared: crate::journal::CanvasPreparedMutation,
    ) -> CanvasDocumentDiff {
        debug_assert_eq!(
            self.history.next_redo_transaction(),
            Some(prepared.committed().transaction())
        );
        let committed = prepared.apply_to(&mut self.document);
        let diff = committed.diff().clone();
        let _ = self.history.pop_redo();
        self.history.push_undo(committed.inverse().clone());
        self.selection.retain_document(&self.document);
        self.sync_runtime_diff(&diff);
        diff
    }

    pub(crate) fn prepare_document_transaction(
        &self,
        transaction: CanvasTransaction,
    ) -> Result<crate::journal::CanvasPreparedMutation, DocumentError> {
        self.document
            .prepare_transaction_with_kind_registry(transaction, self.kind_registry.as_ref())
    }

    pub(crate) fn next_undo_transaction(&self) -> Option<&CanvasTransaction> {
        self.history.next_undo_transaction()
    }

    pub(crate) fn next_redo_transaction(&self) -> Option<&CanvasTransaction> {
        self.history.next_redo_transaction()
    }

    pub fn apply_tool_effect(&mut self, effect: CanvasToolEffect) -> Result<(), DocumentError> {
        match effect {
            CanvasToolEffect::ApplyTransaction(transaction) => {
                self.apply_transaction(transaction)?;
            }
            CanvasToolEffect::BeginGesture => {
                self.begin_gesture();
            }
            CanvasToolEffect::UpdateGesture(transaction) => {
                self.update_gesture(transaction)?;
            }
            CanvasToolEffect::CommitGesture => {
                self.commit_gesture()?;
            }
            CanvasToolEffect::CancelGesture => {
                self.cancel_gesture()?;
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
            CanvasToolEffect::AddSelection(target) => {
                self.selection.insert_target(target);
                self.selection.retain_document(&self.document);
            }
            CanvasToolEffect::RemoveSelection(target) => {
                self.selection.remove_target(&target);
                self.selection.retain_document(&self.document);
            }
            CanvasToolEffect::ToggleSelection(target) => {
                self.selection.toggle_target(target);
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

    pub(crate) fn prepare_gesture_commit(
        &self,
    ) -> Result<Option<CanvasPreparedGestureCommit>, DocumentError> {
        let Some(gesture) = &self.gesture else {
            return Ok(None);
        };
        gesture.prepare_commit_with_kind_registry(&self.document, self.kind_registry.as_ref())
    }

    pub(crate) fn apply_prepared_gesture_commit(
        &mut self,
        prepared: CanvasPreparedGestureCommit,
    ) -> CanvasDocumentDiff {
        let diff = prepared.committed().diff().clone();
        self.history
            .push_undo(prepared.committed().inverse().clone());
        self.gesture = None;
        self.selection.retain_document(&self.document);
        self.sync_runtime_diff(&diff);
        diff
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
        let Some(transaction) = self.history.next_undo_transaction().cloned() else {
            return Ok(false);
        };

        let prepared = self.prepare_document_transaction(transaction)?;
        self.apply_prepared_undo_mutation(prepared);
        Ok(true)
    }

    pub fn redo(&mut self) -> Result<bool, DocumentError> {
        let Some(transaction) = self.history.next_redo_transaction().cloned() else {
            return Ok(false);
        };

        let prepared = self.prepare_document_transaction(transaction)?;
        self.apply_prepared_redo_mutation(prepared);
        Ok(true)
    }

    pub fn rebuild_index(&mut self) {
        self.rebuild_runtime();
    }

    pub fn rebuild_runtime(&mut self) {
        self.runtime = CanvasRuntime::rebuild_with_router_and_kind_registry(
            &self.document,
            self.edge_router.as_ref(),
            self.kind_registry.as_ref(),
        );
    }

    pub fn set_edge_router<R>(&mut self, edge_router: R)
    where
        R: CanvasEdgeRouter + Send + Sync + 'static,
    {
        self.edge_router = Arc::new(edge_router);
        self.rebuild_runtime();
    }

    pub fn set_kind_registry(
        &mut self,
        kind_registry: CanvasKindRegistry,
    ) -> Result<(), DocumentError> {
        let document = CanvasDocument::from_snapshot_with_kind_registry(
            self.document.to_snapshot(),
            &kind_registry,
        )?;
        let document_changed = document != self.document;
        self.document = document;
        self.kind_registry = Arc::new(kind_registry);
        self.selection.retain_document(&self.document);
        self.gesture = None;
        if document_changed {
            self.history.clear();
        }
        self.rebuild_runtime();
        Ok(())
    }

    pub fn set_tool(&mut self, tool: CanvasTool) {
        self.tool = tool;
        self.state = ToolState::Idle;
    }

    pub fn set_viewport(&mut self, viewport: CanvasViewport) {
        self.viewport = viewport;
    }

    pub fn is_tool_state_idle(&self) -> bool {
        matches!(self.state, ToolState::Idle)
    }

    pub fn tool_context(&self) -> CanvasToolContext<'_> {
        CanvasToolContext {
            document: &self.document,
            viewport: &self.viewport,
            tool: &self.tool,
            state: &self.state,
            runtime: &self.runtime,
            edge_router: self.edge_router.as_ref(),
            kind_registry: self.kind_registry.as_ref(),
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
                CanvasEvent::KeyDown {
                    key: CanvasKey::Delete | CanvasKey::Backspace,
                    ..
                },
            ) => {
                let transaction = self.delete_selection_transaction();
                if transaction.is_empty() {
                    Vec::new()
                } else {
                    vec![CanvasToolEffect::ApplyTransaction(transaction)]
                }
            }
            (ToolState::Idle, CanvasEvent::Cancel) => {
                if self.selection.is_empty() {
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
                let document_position = self.viewport.view_to_document(position);
                let hit = self
                    .runtime
                    .hit_test(document_position, HitOptions::default())
                    .map(|record| record.target.clone())
                    .next();

                if modifiers.shift
                    && let Some(target) = hit
                {
                    return Ok(vec![CanvasToolEffect::ToggleSelection(target)]);
                }

                match hit {
                    Some(HitTarget::Node(id)) => {
                        let mut selection = self.selection.clone();
                        if !selection.nodes.contains(&id) {
                            selection.replace_with(HitTarget::Node(id.clone()));
                        }
                        let node_ids = self
                            .document_nodes_for_selection(&selection)
                            .map(|node| node.id)
                            .collect();
                        vec![
                            CanvasToolEffect::BeginGesture,
                            CanvasToolEffect::SetSelection(selection),
                            CanvasToolEffect::SetState(ToolState::Translating {
                                origin: document_position,
                                last: document_position,
                                constraint_axis: None,
                                node_ids,
                            }),
                        ]
                    }
                    Some(target) => {
                        vec![
                            CanvasToolEffect::ReplaceSelection(target),
                            CanvasToolEffect::SetState(ToolState::Pointing {
                                origin: document_position,
                                selection_mode: CanvasSelectionMode::Replace,
                                base_selection: self.selection.clone(),
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
                            base_selection: self.selection.clone(),
                        }));
                        effects
                    }
                }
            }
            (
                ToolState::Translating {
                    last,
                    node_ids,
                    origin,
                    constraint_axis,
                },
                CanvasEvent::PointerMove {
                    position,
                    modifiers,
                },
            ) => {
                let document_position = self.viewport.view_to_document(position);
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
                    CanvasToolEffect::UpdateGesture(CanvasTransaction::new(commands)),
                    CanvasToolEffect::SetState(ToolState::Translating {
                        origin,
                        last: document_position,
                        constraint_axis,
                        node_ids: node_ids.clone(),
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
                let document_position = self.viewport.view_to_document(position);
                let bounds = selection_bounds(origin, document_position);
                vec![
                    CanvasToolEffect::SetSelection(self.selection_for_intersections_with_mode(
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
                let document_position = self.viewport.view_to_document(position);
                let bounds = selection_bounds(origin, document_position);
                vec![
                    CanvasToolEffect::SetSelection(self.selection_for_intersections_with_mode(
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
            (ToolState::Translating { .. }, CanvasEvent::Cancel) => {
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

    fn pan_effects(&self, event: CanvasEvent) -> Vec<CanvasToolEffect> {
        match (&self.state, event) {
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
        }
    }

    fn connect_effects(&self, event: CanvasEvent) -> Vec<CanvasToolEffect> {
        match (&self.state, event) {
            (
                ToolState::Idle,
                CanvasEvent::PointerDown {
                    position,
                    button: PointerButton::Primary,
                    ..
                },
            ) => {
                let document_position = self.viewport.view_to_document(position);
                self.node_endpoint_at(document_position, CanvasConnectionEndpointRole::Source)
                    .map(|source| {
                        vec![CanvasToolEffect::SetState(ToolState::Connecting {
                            source,
                            current: document_position,
                        })]
                    })
                    .unwrap_or_default()
            }
            (ToolState::Connecting { source, .. }, CanvasEvent::PointerMove { position, .. }) => {
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
                    ..
                },
            ) => {
                let document_position = self.viewport.view_to_document(position);
                let mut effects = Vec::new();
                if let Some(target) =
                    self.node_endpoint_at(document_position, CanvasConnectionEndpointRole::Target)
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

    fn node_endpoint_at(
        &self,
        point: Point<Pixels>,
        role: CanvasConnectionEndpointRole,
    ) -> Option<CanvasEndpoint> {
        let resolver = CanvasGeometryResolver::with_router_and_kind_registry(
            &self.document,
            self.edge_router.as_ref(),
            Some(self.kind_registry.as_ref()),
        );
        resolver
            .connection_endpoint_at(self.runtime.hit_test(point, connection_hit_options()), role)
    }

    fn begin_gesture(&mut self) {
        self.gesture = Some(CanvasGestureSession::begin(&self.document));
    }

    fn update_gesture(
        &mut self,
        transaction: CanvasTransaction,
    ) -> Result<CanvasDocumentDiff, DocumentError> {
        if transaction.is_empty() {
            return Ok(CanvasDocumentDiff::default());
        }

        let implicit_gesture = self
            .gesture
            .is_none()
            .then(|| CanvasGestureSession::begin(&self.document));
        let diff = self.apply_transient_transaction(transaction)?;
        if let Some(gesture) = implicit_gesture {
            self.gesture = Some(gesture);
        }
        Ok(diff)
    }

    fn apply_transient_transaction(
        &mut self,
        transaction: CanvasTransaction,
    ) -> Result<CanvasDocumentDiff, DocumentError> {
        if transaction.is_empty() {
            return Ok(CanvasDocumentDiff::default());
        }

        let committed = self
            .document
            .commit_transaction_with_kind_registry(transaction, self.kind_registry.as_ref())?;
        let diff = committed.diff().clone();
        self.selection.retain_document(&self.document);
        self.sync_runtime_diff(&diff);
        Ok(diff)
    }

    fn sync_runtime_diff(&mut self, diff: &CanvasDocumentDiff) {
        self.runtime.apply_diff_with_router_and_kind_registry(
            &self.document,
            diff,
            self.edge_router.as_ref(),
            self.kind_registry.as_ref(),
        );
    }

    fn commit_gesture(&mut self) -> Result<CanvasDocumentDiff, DocumentError> {
        let Some(prepared) = self.prepare_gesture_commit()? else {
            self.gesture = None;
            return Ok(CanvasDocumentDiff::default());
        };
        Ok(self.apply_prepared_gesture_commit(prepared))
    }

    fn cancel_gesture(&mut self) -> Result<CanvasDocumentDiff, DocumentError> {
        let Some(gesture) = self.gesture.take() else {
            return Ok(CanvasDocumentDiff::default());
        };
        let transaction = gesture.cancel_transaction(&self.document);
        self.apply_transient_transaction(transaction)
    }

    fn delete_selection_transaction(&self) -> CanvasTransaction {
        let node_ids = self
            .selection
            .selected_nodes()
            .filter(|id| {
                self.document
                    .nodes
                    .get(*id)
                    .is_some_and(|node| !node.locked)
            })
            .cloned()
            .collect::<IndexSet<_>>();

        let mut commands = Vec::new();

        for id in self.selection.selected_edges() {
            let Some(edge) = self.document.edges.get(id) else {
                continue;
            };
            if edge.locked
                || node_ids.contains(&edge.source.node_id)
                || node_ids.contains(&edge.target.node_id)
            {
                continue;
            }

            commands.push(DocumentCommand::RemoveEdge(id.clone()));
        }

        commands.extend(node_ids.iter().cloned().map(DocumentCommand::RemoveNode));

        for id in self.selection.selected_shapes() {
            let Some(shape) = self.document.shapes.get(id) else {
                continue;
            };
            if shape.locked {
                continue;
            }

            commands.push(DocumentCommand::RemoveShape(id.clone()));
        }

        CanvasTransaction::new(commands)
    }

    fn selection_for_intersections(&self, bounds: Bounds<Pixels>) -> CanvasSelection {
        let mut selection = CanvasSelection::default();
        for record in self
            .runtime
            .query_with_options(bounds, HitOptions::default())
        {
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

    fn selection_for_intersections_with_mode(
        &self,
        bounds: Bounds<Pixels>,
        mode: CanvasSelectionMode,
        base_selection: &CanvasSelection,
    ) -> CanvasSelection {
        let selection = self.selection_for_intersections(bounds);
        match mode {
            CanvasSelectionMode::Replace => selection,
            CanvasSelectionMode::Add => {
                let mut combined = base_selection.clone();
                combined.extend_selection(selection);
                combined
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CanvasNode, CanvasNodeKind, CanvasRecordKind, CanvasRoutePath, CanvasRouteRequest,
        CanvasSchemaError, CanvasShape, HandleId,
    };
    use open_gpui::{point, px, size};
    use serde_json::{Value, json};

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
                ..
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

    struct RequiredTitleNodeKind;

    impl CanvasNodeKind for RequiredTitleNodeKind {
        fn default_data(&self) -> CanvasValue {
            CanvasValue::from_iter([("title".to_string(), json!("Untitled"))])
        }

        fn migrate_node(&self, node: &mut CanvasNode) -> Result<(), CanvasSchemaError> {
            if let Some(value) = node.data.remove("label") {
                node.data.insert("title".to_string(), value);
            }
            Ok(())
        }

        fn validate_node(&self, node: &CanvasNode) -> Result<(), CanvasSchemaError> {
            match node.data.get("title") {
                Some(Value::String(title)) if !title.trim().is_empty() => Ok(()),
                Some(_) => Err(CanvasSchemaError::invalid_data(
                    CanvasRecordKind::Node,
                    node.id.clone(),
                    &node.kind,
                    "title must be a non-empty string",
                )),
                None => Err(CanvasSchemaError::missing_required_data(
                    CanvasRecordKind::Node,
                    node.id.clone(),
                    &node.kind,
                    "title",
                )),
            }
        }
    }

    #[test]
    fn canvas_selection_adds_removes_and_toggles_targets() {
        let mut selection = CanvasSelection::default();
        let node = HitTarget::Node(NodeId::from("node"));
        let handle = HitTarget::Handle {
            node_id: NodeId::from("node"),
            handle_id: HandleId::from("handle"),
        };
        let edge = HitTarget::Edge(EdgeId::from("edge"));
        let shape = HitTarget::Shape(ShapeId::from("shape"));

        assert!(selection.insert_target(node.clone()));
        assert!(!selection.insert_target(node.clone()));
        assert!(selection.contains_target(&node));
        assert!(!selection.toggle_target(node.clone()));
        assert!(!selection.contains_target(&node));

        assert!(selection.toggle_target(handle.clone()));
        assert!(selection.insert_target(edge.clone()));
        assert!(selection.insert_target(shape.clone()));
        assert!(selection.contains_target(&handle));
        assert!(selection.remove_target(&edge));
        assert!(!selection.contains_target(&edge));
        assert!(selection.contains_target(&shape));
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
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(20.0), px(25.0)),
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(20.0), px(25.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
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
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(30.0), px(30.0)),
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(30.0), px(30.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
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
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        assert!(!editor.selection.is_empty());

        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(10.0), px(10.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(300.0), px(300.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        assert!(editor.selection.is_empty());
    }

    #[test]
    fn select_tool_cancel_restores_selection_after_canvas_press() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "base",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor.selection.nodes.insert(NodeId::from("base"));

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(300.0), px(300.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        assert!(editor.selection.is_empty());

        editor.handle_event(CanvasEvent::Cancel).unwrap();

        assert_eq!(
            editor.selection.nodes.iter().cloned().collect::<Vec<_>>(),
            vec![NodeId::from("base")]
        );
        assert_eq!(editor.state, ToolState::Idle);
    }

    #[test]
    fn select_tool_cancel_clears_selection_when_idle() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "base",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor.selection.nodes.insert(NodeId::from("base"));

        editor.handle_event(CanvasEvent::Cancel).unwrap();

        assert!(editor.selection.is_empty());
        assert_eq!(editor.state, ToolState::Idle);
        assert_eq!(editor.history.undo_depth(), 0);
    }

    #[test]
    fn select_tool_shift_click_toggles_selection() {
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

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(210.0), px(10.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers {
                    shift: true,
                    ..CanvasKeyModifiers::default()
                },
            })
            .unwrap();

        assert_eq!(
            editor.selection.nodes.iter().cloned().collect::<Vec<_>>(),
            vec![NodeId::from("a"), NodeId::from("b")]
        );
        assert_eq!(editor.state, ToolState::Idle);
        assert_eq!(editor.history.undo_depth(), 0);

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(210.0), px(10.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers {
                    shift: true,
                    ..CanvasKeyModifiers::default()
                },
            })
            .unwrap();

        assert_eq!(
            editor.selection.nodes.iter().cloned().collect::<Vec<_>>(),
            vec![NodeId::from("a")]
        );
        assert_eq!(editor.state, ToolState::Idle);
        assert_eq!(editor.history.undo_depth(), 0);
    }

    #[test]
    fn select_tool_delete_key_removes_selected_records() {
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
        document
            .insert_edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .unwrap();
        document
            .insert_shape(CanvasShape::new(
                "shape",
                Bounds::new(point(px(0.0), px(200.0)), size(px(40.0), px(40.0))),
            ))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor.selection.nodes.insert(NodeId::from("a"));
        editor.selection.edges.insert(EdgeId::from("a-b"));
        editor.selection.shapes.insert(ShapeId::from("shape"));

        editor
            .handle_event(CanvasEvent::KeyDown {
                key: CanvasKey::Delete,
                modifiers: CanvasKeyModifiers::default(),
                repeat: false,
            })
            .unwrap();

        assert!(!editor.document.nodes.contains_key(&NodeId::from("a")));
        assert!(editor.document.nodes.contains_key(&NodeId::from("b")));
        assert!(editor.document.edges.is_empty());
        assert!(editor.document.shapes.is_empty());
        assert!(editor.selection.is_empty());
        assert_eq!(editor.history.undo_depth(), 1);

        assert!(editor.undo().unwrap());
        assert!(editor.document.nodes.contains_key(&NodeId::from("a")));
        assert!(editor.document.edges.contains_key(&EdgeId::from("a-b")));
        assert!(editor.document.shapes.contains_key(&ShapeId::from("shape")));
    }

    #[test]
    fn select_tool_delete_key_skips_locked_selected_records() {
        let mut document = CanvasDocument::default();
        let mut locked_node = CanvasNode::new(
            "locked-node",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        );
        locked_node.locked = true;
        document.insert_node(locked_node).unwrap();
        document
            .insert_node(CanvasNode::new(
                "a",
                point(px(200.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .unwrap();
        document
            .insert_node(CanvasNode::new(
                "b",
                point(px(400.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .unwrap();
        let mut locked_edge = CanvasEdge::new(
            "locked-edge",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        );
        locked_edge.locked = true;
        document.insert_edge(locked_edge).unwrap();
        let mut locked_shape = CanvasShape::new(
            "locked-shape",
            Bounds::new(point(px(0.0), px(200.0)), size(px(40.0), px(40.0))),
        );
        locked_shape.locked = true;
        document.insert_shape(locked_shape).unwrap();
        let mut editor = CanvasEditor::new(document);
        editor.selection.nodes.insert(NodeId::from("locked-node"));
        editor.selection.edges.insert(EdgeId::from("locked-edge"));
        editor
            .selection
            .shapes
            .insert(ShapeId::from("locked-shape"));

        editor
            .handle_event(CanvasEvent::KeyDown {
                key: CanvasKey::Backspace,
                modifiers: CanvasKeyModifiers::default(),
                repeat: false,
            })
            .unwrap();

        assert!(
            editor
                .document
                .nodes
                .contains_key(&NodeId::from("locked-node"))
        );
        assert!(
            editor
                .document
                .edges
                .contains_key(&EdgeId::from("locked-edge"))
        );
        assert!(
            editor
                .document
                .shapes
                .contains_key(&ShapeId::from("locked-shape"))
        );
        assert_eq!(editor.history.undo_depth(), 0);
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
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(50.0), px(50.0)),
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(50.0), px(50.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        assert_eq!(
            editor.selection.nodes.iter().cloned().collect::<Vec<_>>(),
            vec![NodeId::from("inside")]
        );
        assert_eq!(editor.state, ToolState::Idle);
    }

    #[test]
    fn select_tool_cancel_restores_selection_after_box_select() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "base",
                point(px(200.0), px(200.0)),
                size(px(20.0), px(20.0)),
            ))
            .unwrap();
        document
            .insert_node(CanvasNode::new(
                "inside",
                point(px(10.0), px(10.0)),
                size(px(20.0), px(20.0)),
            ))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor.selection.nodes.insert(NodeId::from("base"));

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(0.0), px(0.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(40.0), px(40.0)),
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        assert_eq!(
            editor.selection.nodes.iter().cloned().collect::<Vec<_>>(),
            vec![NodeId::from("inside")]
        );

        editor.handle_event(CanvasEvent::Cancel).unwrap();

        assert_eq!(
            editor.selection.nodes.iter().cloned().collect::<Vec<_>>(),
            vec![NodeId::from("base")]
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
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(20.0), px(30.0)),
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(20.0), px(30.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
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
    fn translating_selected_node_with_shift_locks_to_dominant_axis() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor.selection.nodes.insert(NodeId::from("a"));

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(10.0), px(10.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(20.0), px(30.0)),
                modifiers: CanvasKeyModifiers {
                    shift: true,
                    ..CanvasKeyModifiers::default()
                },
            })
            .unwrap();

        assert_eq!(
            editor.document.nodes[&NodeId::from("a")].position,
            point(px(0.0), px(20.0))
        );

        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(80.0), px(35.0)),
                modifiers: CanvasKeyModifiers {
                    shift: true,
                    ..CanvasKeyModifiers::default()
                },
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(80.0), px(35.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers {
                    shift: true,
                    ..CanvasKeyModifiers::default()
                },
            })
            .unwrap();

        assert_eq!(
            editor.document.nodes[&NodeId::from("a")].position,
            point(px(0.0), px(25.0))
        );
        assert_eq!(editor.history.undo_depth(), 1);
    }

    #[test]
    fn select_tool_shift_box_adds_to_base_selection_without_accumulating() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "base",
                point(px(200.0), px(200.0)),
                size(px(20.0), px(20.0)),
            ))
            .unwrap();
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
                point(px(100.0), px(100.0)),
                size(px(20.0), px(20.0)),
            ))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor.selection.nodes.insert(NodeId::from("base"));

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(0.0), px(0.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers {
                    shift: true,
                    ..CanvasKeyModifiers::default()
                },
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(40.0), px(40.0)),
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        assert_eq!(
            editor.selection.nodes.iter().cloned().collect::<Vec<_>>(),
            vec![NodeId::from("base"), NodeId::from("inside")]
        );

        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(-40.0), px(-40.0)),
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        assert_eq!(
            editor.selection.nodes.iter().cloned().collect::<Vec<_>>(),
            vec![NodeId::from("base")]
        );
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
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(20.0), px(30.0)),
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(20.0), px(30.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
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
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(20.0), px(25.0)),
                modifiers: CanvasKeyModifiers::default(),
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
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(210.0), px(10.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
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
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(210.0), px(10.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        assert!(editor.document.edges.is_empty());
        assert_eq!(editor.history.undo_depth(), 0);
    }

    #[test]
    fn connect_tool_uses_handles_when_available() {
        use crate::{CanvasHandle, HandleId, HandleRole};

        let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        let mut source_handle = CanvasHandle::new("out", point(px(100.0), px(50.0)));
        source_handle.role = HandleRole::Source;
        source.handles.push(source_handle);

        let mut target =
            CanvasNode::new("b", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
        let mut target_handle = CanvasHandle::new("in", point(px(0.0), px(50.0)));
        target_handle.role = HandleRole::Target;
        target.handles.push(target_handle);

        let mut document = CanvasDocument::default();
        document.insert_node(source).unwrap();
        document.insert_node(target).unwrap();
        let mut editor = CanvasEditor::new(document);
        editor.set_tool(CanvasTool::Connect);

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(100.0), px(50.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(200.0), px(50.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        let edge = editor.document.edges.values().next().unwrap();
        assert_eq!(edge.source.handle_id, Some(HandleId::from("out")));
        assert_eq!(edge.target.handle_id, Some(HandleId::from("in")));
    }

    #[test]
    fn connect_tool_does_not_start_from_target_only_handle() {
        use crate::{CanvasHandle, HandleRole};

        let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        let mut target_only = CanvasHandle::new("in", point(px(100.0), px(50.0)));
        target_only.role = HandleRole::Target;
        source.handles.push(target_only);
        let target = CanvasNode::new("b", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));

        let mut document = CanvasDocument::default();
        document.insert_node(source).unwrap();
        document.insert_node(target).unwrap();
        let mut editor = CanvasEditor::new(document);
        editor.set_tool(CanvasTool::Connect);

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(100.0), px(50.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(210.0), px(10.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        assert!(matches!(editor.state, ToolState::Idle));
        assert!(editor.document.edges.is_empty());
        assert_eq!(editor.history.undo_depth(), 0);
    }

    #[test]
    fn connect_tool_does_not_end_on_source_only_handle() {
        use crate::{CanvasHandle, HandleRole};

        let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        let mut source_handle = CanvasHandle::new("out", point(px(100.0), px(50.0)));
        source_handle.role = HandleRole::Source;
        source.handles.push(source_handle);

        let mut target =
            CanvasNode::new("b", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
        let mut invalid_target_handle = CanvasHandle::new("out", point(px(0.0), px(50.0)));
        invalid_target_handle.role = HandleRole::Source;
        target.handles.push(invalid_target_handle);

        let mut document = CanvasDocument::default();
        document.insert_node(source).unwrap();
        document.insert_node(target).unwrap();
        let mut editor = CanvasEditor::new(document);
        editor.set_tool(CanvasTool::Connect);

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(100.0), px(50.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(200.0), px(50.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        assert!(matches!(editor.state, ToolState::Idle));
        assert!(editor.document.edges.is_empty());
        assert_eq!(editor.history.undo_depth(), 0);
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
                    modifiers: CanvasKeyModifiers::default(),
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
                    modifiers: CanvasKeyModifiers::default(),
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
                    modifiers: CanvasKeyModifiers::default(),
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
                    modifiers: CanvasKeyModifiers::default(),
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
            selection_mode: CanvasSelectionMode::Replace,
            base_selection: CanvasSelection::default(),
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
    fn editor_kind_registry_normalizes_and_validates_transactions() {
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("note", RequiredTitleNodeKind);
        let mut editor =
            CanvasEditor::try_new_with_kind_registry(CanvasDocument::default(), registry).unwrap();

        let mut note = CanvasNode::new("note", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        note.kind = "note".to_string();
        note.data.insert("label".to_string(), json!("Migrated"));

        editor.apply(DocumentCommand::InsertNode(note)).unwrap();

        assert_eq!(
            editor.document.nodes[&NodeId::from("note")]
                .data
                .get("title"),
            Some(&json!("Migrated"))
        );
        assert_eq!(editor.history.undo_depth(), 1);

        let mut invalid = CanvasNode::new(
            "invalid",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        );
        invalid.kind = "note".to_string();
        invalid.data.insert("title".to_string(), json!(false));
        let err = editor
            .apply(DocumentCommand::InsertNode(invalid))
            .unwrap_err();

        assert!(matches!(
            err,
            DocumentError::Schema(CanvasSchemaError::InvalidData {
                record_kind: CanvasRecordKind::Node,
                record_id: crate::CanvasRecordId::Node(id),
                kind,
                ..
            }) if id == NodeId::from("invalid") && kind == "note"
        ));
        assert!(!editor.document.nodes.contains_key(&NodeId::from("invalid")));
        assert_eq!(editor.history.undo_depth(), 1);
    }

    #[test]
    fn editor_set_kind_registry_normalizes_document_and_clears_stale_history() {
        let mut note = CanvasNode::new("note", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        note.kind = "note".to_string();
        note.data.insert("label".to_string(), json!("Migrated"));
        let mut editor = CanvasEditor::default();
        editor.apply(DocumentCommand::InsertNode(note)).unwrap();
        assert_eq!(editor.history.undo_depth(), 1);

        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("note", RequiredTitleNodeKind);
        editor.set_kind_registry(registry).unwrap();

        assert_eq!(
            editor.document.nodes[&NodeId::from("note")]
                .data
                .get("title"),
            Some(&json!("Migrated"))
        );
        assert_eq!(editor.history.undo_depth(), 0);
        assert!(
            editor
                .runtime()
                .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
                .next()
                .is_some()
        );
    }

    #[test]
    fn editor_set_kind_registry_rejects_invalid_existing_document_atomically() {
        let mut note = CanvasNode::new("note", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        note.kind = "note".to_string();
        note.data.insert("title".to_string(), json!(false));
        let mut document = CanvasDocument::default();
        document.insert_node(note).unwrap();
        let mut editor = CanvasEditor::new(document);

        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("note", RequiredTitleNodeKind);
        let err = editor.set_kind_registry(registry).unwrap_err();

        assert!(matches!(
            err,
            DocumentError::Schema(CanvasSchemaError::InvalidData {
                record_id: crate::CanvasRecordId::Node(id),
                ..
            }) if id == NodeId::from("note")
        ));
        assert_eq!(
            editor.document.nodes[&NodeId::from("note")]
                .data
                .get("title"),
            Some(&json!(false))
        );
        assert!(editor.kind_registry().node_kind("note").is_none());
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
                .runtime()
                .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
                .next()
                .is_some()
        );
    }

    #[test]
    fn tool_effect_updates_gesture_without_history() {
        let mut editor = CanvasEditor::default();

        editor
            .apply_tool_effect(CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                DocumentCommand::InsertNode(CanvasNode::new(
                    "a",
                    point(px(0.0), px(0.0)),
                    size(px(100.0), px(100.0)),
                )),
            )))
            .unwrap();

        assert!(editor.document.nodes.contains_key(&NodeId::from("a")));
        assert_eq!(editor.history.undo_depth(), 0);
        assert!(
            editor
                .runtime()
                .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
                .next()
                .is_some()
        );
    }

    #[test]
    fn gesture_update_uses_kind_registry_validation() {
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("note", RequiredTitleNodeKind);
        let mut editor =
            CanvasEditor::try_new_with_kind_registry(CanvasDocument::default(), registry).unwrap();
        let mut note = CanvasNode::new("note", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        note.kind = "note".to_string();
        note.data.insert("title".to_string(), json!("Valid"));
        editor
            .apply(DocumentCommand::InsertNode(note.clone()))
            .unwrap();

        let mut invalid = note.clone();
        invalid.data.insert("title".to_string(), json!(false));
        let err = editor
            .apply_tool_effects([
                CanvasToolEffect::BeginGesture,
                CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                    DocumentCommand::UpdateNode(invalid),
                )),
            ])
            .unwrap_err();

        assert!(matches!(
            err,
            DocumentError::Schema(CanvasSchemaError::InvalidData {
                record_id: crate::CanvasRecordId::Node(id),
                ..
            }) if id == NodeId::from("note")
        ));
        assert_eq!(editor.document.nodes[&NodeId::from("note")], note);
        assert_eq!(editor.history.undo_depth(), 1);
    }

    #[test]
    fn gesture_commit_pushes_one_undo_entry() {
        let mut editor = CanvasEditor::default();
        let original = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        let first = CanvasNode::new("a", point(px(12.0), px(0.0)), size(px(100.0), px(100.0)));
        let second = CanvasNode::new("a", point(px(40.0), px(0.0)), size(px(100.0), px(100.0)));
        editor
            .apply(DocumentCommand::InsertNode(original.clone()))
            .unwrap();

        editor
            .apply_tool_effects([
                CanvasToolEffect::BeginGesture,
                CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                    DocumentCommand::UpdateNode(first),
                )),
                CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                    DocumentCommand::UpdateNode(second.clone()),
                )),
                CanvasToolEffect::CommitGesture,
            ])
            .unwrap();

        assert_eq!(editor.document.nodes[&NodeId::from("a")], second);
        assert_eq!(editor.history.undo_depth(), 2);
        assert!(editor.undo().unwrap());
        assert_eq!(editor.document.nodes[&NodeId::from("a")], original);
    }

    #[test]
    fn gesture_cancel_restores_document_without_history() {
        let mut editor = CanvasEditor::default();
        let original = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        let moved = CanvasNode::new("a", point(px(40.0), px(0.0)), size(px(100.0), px(100.0)));
        editor
            .apply(DocumentCommand::InsertNode(original.clone()))
            .unwrap();
        let undo_depth = editor.history.undo_depth();

        editor
            .apply_tool_effects([
                CanvasToolEffect::BeginGesture,
                CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                    DocumentCommand::UpdateNode(moved),
                )),
                CanvasToolEffect::CancelGesture,
            ])
            .unwrap();

        assert_eq!(editor.document.nodes[&NodeId::from("a")], original);
        assert_eq!(editor.history.undo_depth(), undo_depth);
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
                    selection_mode: CanvasSelectionMode::Replace,
                    base_selection: CanvasSelection::default(),
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
                origin: point(px(10.0), px(20.0)),
                selection_mode: CanvasSelectionMode::Replace,
                base_selection: CanvasSelection::default(),
            }
        );
        assert_eq!(editor.viewport.origin, point(px(5.0), px(-3.0)));
    }

    #[test]
    fn tool_effects_update_selection_incrementally() {
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
        document
            .insert_edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .unwrap();
        document
            .insert_shape(CanvasShape::new(
                "shape",
                Bounds::new(point(px(0.0), px(200.0)), size(px(40.0), px(40.0))),
            ))
            .unwrap();
        let mut editor = CanvasEditor::new(document);

        editor
            .apply_tool_effects([
                CanvasToolEffect::AddSelection(HitTarget::Node(NodeId::from("a"))),
                CanvasToolEffect::ToggleSelection(HitTarget::Shape(ShapeId::from("shape"))),
                CanvasToolEffect::ToggleSelection(HitTarget::Edge(EdgeId::from("a-b"))),
                CanvasToolEffect::RemoveSelection(HitTarget::Node(NodeId::from("a"))),
                CanvasToolEffect::ToggleSelection(HitTarget::Shape(ShapeId::from("shape"))),
                CanvasToolEffect::AddSelection(HitTarget::Node(NodeId::from("missing"))),
            ])
            .unwrap();

        assert!(editor.selection.nodes.is_empty());
        assert!(editor.selection.shapes.is_empty());
        assert_eq!(
            editor.selection.edges.iter().cloned().collect::<Vec<_>>(),
            vec![EdgeId::from("a-b")]
        );
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
                .runtime()
                .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
                .next()
                .is_some()
        );

        assert!(editor.undo().unwrap());
        assert!(
            editor
                .runtime()
                .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
                .next()
                .is_none()
        );
    }

    #[test]
    fn editor_refreshes_runtime_geometry_with_installed_router() {
        let mut editor =
            CanvasEditor::new_with_router(connected_edge_document(), VerticalDetourRouter);

        assert_eq!(
            editor
                .runtime()
                .edge_geometry(&EdgeId::from("a-b"))
                .unwrap()
                .path
                .document_points(),
            vec![
                point(px(5.0), px(5.0)),
                point(px(5.0), px(80.0)),
                point(px(25.0), px(5.0)),
            ]
        );

        let mut target = editor.document().nodes[&NodeId::from("b")].clone();
        target.position = point(px(40.0), px(0.0));
        editor.apply(DocumentCommand::UpdateNode(target)).unwrap();

        assert_eq!(
            editor
                .runtime()
                .edge_geometry(&EdgeId::from("a-b"))
                .unwrap()
                .path
                .document_points(),
            vec![
                point(px(5.0), px(5.0)),
                point(px(5.0), px(80.0)),
                point(px(45.0), px(5.0)),
            ]
        );
    }

    fn connected_edge_document() -> CanvasDocument {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();
        document
            .insert_node(CanvasNode::new(
                "b",
                point(px(20.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();
        document
            .insert_edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .unwrap();
        document
    }

    struct VerticalDetourRouter;

    impl CanvasEdgeRouter for VerticalDetourRouter {
        fn route_edge(&self, request: CanvasRouteRequest<'_>) -> CanvasRoutePath {
            CanvasRoutePath::polyline([
                request.source,
                point(request.source.x, px(80.0)),
                request.target,
            ])
        }
    }
}
