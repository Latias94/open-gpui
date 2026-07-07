use crate::gesture::CanvasPreparedGestureCommit;
use crate::layer::CanvasZOrderCommand;
use crate::session::{CanvasToolSession, CanvasToolSessionEffect, CanvasToolSessionSnapshot};
use crate::{
    CanvasClipboardPayload, CanvasConnectionEndpointRole, CanvasDefaultEdgeRouter, CanvasDocument,
    CanvasDocumentDiff, CanvasEdgeRouter, CanvasEndpoint, CanvasKindRegistry,
    CanvasPasteTransaction, CanvasRecordId, CanvasRuntime, CanvasStore, CanvasStoreChange,
    CanvasStoreListenerId, CanvasTransaction, CanvasViewport, DocumentCommand, DocumentError,
    EdgeId, HitTarget, NodeId, ShapeId,
};
use indexmap::IndexSet;
use open_gpui::{Bounds, Pixels, Point};
use serde::{Deserialize, Serialize};
use std::{fmt, sync::Arc};

mod action;
mod builtin;
mod clipboard;
mod context;
mod group;
mod history;
mod registry;
mod select;
mod z_order;

use crate::session::ToolState;
pub use action::CanvasToolIntent;
pub(crate) use action::{CanvasEditorAction, CanvasToolEffect};
use builtin::BuiltInCanvasTool;
pub use context::CanvasToolContext;
pub(crate) use context::CanvasToolReducerContext;
pub(crate) use context::RECONNECT_HANDLE_VIEW_SIZE;
pub use history::CanvasHistory;
pub use registry::{CanvasToolReducer, CanvasToolRegistry, CanvasToolRegistryError};

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

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasConnectionDragState {
    pub source: CanvasEndpoint,
    pub current: Point<Pixels>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CanvasConnectionRelease {
    Connected(CanvasConnectedRelease),
    Dropped(CanvasDroppedConnectionRelease),
    Reconnected(CanvasReconnectedRelease),
    ReconnectDropped(CanvasDroppedReconnectRelease),
    Rejected(CanvasRejectedConnectionRelease),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasConnectedRelease {
    pub source: CanvasEndpoint,
    pub target: CanvasEndpoint,
    pub edge_id: EdgeId,
    pub position: Point<Pixels>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasDroppedConnectionRelease {
    pub source: CanvasEndpoint,
    pub position: Point<Pixels>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasReconnectedRelease {
    pub edge_id: EdgeId,
    pub endpoint: CanvasConnectionEndpointRole,
    pub fixed: CanvasEndpoint,
    pub replacement: CanvasEndpoint,
    pub position: Point<Pixels>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasDroppedReconnectRelease {
    pub edge_id: EdgeId,
    pub endpoint: CanvasConnectionEndpointRole,
    pub fixed: CanvasEndpoint,
    pub position: Point<Pixels>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CanvasConnectionRejectReason {
    InvalidSource,
    InvalidTarget,
    NoTarget,
    SameEndpoint,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasRejectedConnectionRelease {
    pub reason: CanvasConnectionRejectReason,
    pub source: Option<CanvasEndpoint>,
    pub edge_id: Option<EdgeId>,
    pub endpoint: Option<CanvasConnectionEndpointRole>,
    pub position: Point<Pixels>,
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum CanvasSelectionMode {
    #[default]
    Replace,
    Add,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CanvasSelection {
    nodes: IndexSet<NodeId>,
    edges: IndexSet<EdgeId>,
    shapes: IndexSet<ShapeId>,
    handles: IndexSet<CanvasEndpoint>,
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

    pub fn contains_node(&self, id: &NodeId) -> bool {
        self.nodes.contains(id)
    }

    pub fn contains_edge(&self, id: &EdgeId) -> bool {
        self.edges.contains(id)
    }

    pub fn contains_shape(&self, id: &ShapeId) -> bool {
        self.shapes.contains(id)
    }

    pub fn contains_handle(&self, endpoint: &CanvasEndpoint) -> bool {
        self.handles.contains(endpoint)
    }

    pub fn contains_target(&self, target: &HitTarget) -> bool {
        match target {
            HitTarget::Node(id) => self.contains_node(id),
            HitTarget::Handle { node_id, handle_id } => self.contains_handle(&CanvasEndpoint {
                node_id: node_id.clone(),
                handle_id: Some(handle_id.clone()),
            }),
            HitTarget::Edge(id) => self.contains_edge(id),
            HitTarget::Shape(id) => self.contains_shape(id),
        }
    }

    pub fn insert_node(&mut self, id: NodeId) -> bool {
        self.nodes.insert(id)
    }

    pub fn insert_edge(&mut self, id: EdgeId) -> bool {
        self.edges.insert(id)
    }

    pub fn insert_shape(&mut self, id: ShapeId) -> bool {
        self.shapes.insert(id)
    }

    pub fn insert_handle(&mut self, endpoint: CanvasEndpoint) -> bool {
        self.handles.insert(endpoint)
    }

    pub fn insert_target(&mut self, target: HitTarget) -> bool {
        match target {
            HitTarget::Node(id) => self.insert_node(id),
            HitTarget::Handle { node_id, handle_id } => self.insert_handle(CanvasEndpoint {
                node_id,
                handle_id: Some(handle_id),
            }),
            HitTarget::Edge(id) => self.insert_edge(id),
            HitTarget::Shape(id) => self.insert_shape(id),
        }
    }

    pub fn remove_node(&mut self, id: &NodeId) -> bool {
        self.nodes.shift_remove(id)
    }

    pub fn remove_edge(&mut self, id: &EdgeId) -> bool {
        self.edges.shift_remove(id)
    }

    pub fn remove_shape(&mut self, id: &ShapeId) -> bool {
        self.shapes.shift_remove(id)
    }

    pub fn remove_handle(&mut self, endpoint: &CanvasEndpoint) -> bool {
        self.handles.shift_remove(endpoint)
    }

    pub fn remove_target(&mut self, target: &HitTarget) -> bool {
        match target {
            HitTarget::Node(id) => self.remove_node(id),
            HitTarget::Handle { node_id, handle_id } => self.remove_handle(&CanvasEndpoint {
                node_id: node_id.clone(),
                handle_id: Some(handle_id.clone()),
            }),
            HitTarget::Edge(id) => self.remove_edge(id),
            HitTarget::Shape(id) => self.remove_shape(id),
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

    pub fn clear_shapes(&mut self) {
        self.shapes.clear();
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

    pub fn selected_handles(&self) -> impl Iterator<Item = &CanvasEndpoint> {
        self.handles.iter()
    }

    pub fn retain_document(&mut self, document: &CanvasDocument) {
        self.nodes.retain(|id| document.contains_node(id));
        self.edges.retain(|id| document.contains_edge(id));
        self.shapes.retain(|id| document.contains_shape(id));
        self.handles
            .retain(|endpoint| document.validate_endpoint(endpoint).is_ok());
    }

    pub(crate) fn selected_records(&self) -> impl Iterator<Item = CanvasRecordId> + '_ {
        self.selected_nodes()
            .cloned()
            .map(CanvasRecordId::Node)
            .chain(self.selected_edges().cloned().map(CanvasRecordId::Edge))
            .chain(self.selected_shapes().cloned().map(CanvasRecordId::Shape))
    }

    pub(crate) fn insert_record(&mut self, record_id: CanvasRecordId) -> bool {
        match record_id {
            CanvasRecordId::Node(id) => self.insert_node(id),
            CanvasRecordId::Edge(id) => self.insert_edge(id),
            CanvasRecordId::Shape(id) => self.insert_shape(id),
        }
    }
}

pub struct CanvasEditor {
    store: CanvasStore,
    session: CanvasToolSession,
    connection_release: Option<CanvasConnectionRelease>,
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
        Self {
            store: CanvasStore::new_with_router(document, edge_router),
            session: CanvasToolSession::default(),
            connection_release: None,
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
        Ok(Self {
            store: CanvasStore::try_new_with_router_and_kind_registry(
                document,
                edge_router,
                kind_registry,
            )?,
            session: CanvasToolSession::default(),
            connection_release: None,
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
        self.store.document()
    }

    pub fn viewport(&self) -> CanvasViewport {
        self.session.viewport()
    }

    pub fn tool(&self) -> &CanvasTool {
        self.session.tool()
    }

    pub(crate) fn state(&self) -> &ToolState {
        self.session.state()
    }

    pub fn runtime(&self) -> &CanvasRuntime {
        self.store.runtime()
    }

    pub(crate) fn document_snapshot(&self) -> Arc<CanvasDocument> {
        self.store.document_snapshot()
    }

    pub(crate) fn runtime_snapshot(&self) -> Arc<CanvasRuntime> {
        self.store.runtime_snapshot()
    }

    pub(crate) fn kind_registry_snapshot(&self) -> Arc<CanvasKindRegistry> {
        self.store.kind_registry_snapshot()
    }

    pub(crate) fn session_snapshot(&self) -> CanvasToolSessionSnapshot {
        self.session.snapshot()
    }

    pub fn edge_router(&self) -> &(dyn CanvasEdgeRouter + Send + Sync) {
        self.store.edge_router()
    }

    pub fn kind_registry(&self) -> &CanvasKindRegistry {
        self.store.kind_registry()
    }

    pub fn selection(&self) -> &CanvasSelection {
        self.session.selection()
    }

    pub fn history(&self) -> &CanvasHistory {
        self.store.history()
    }

    pub fn store(&self) -> &CanvasStore {
        &self.store
    }

    pub(crate) fn store_mut(&mut self) -> &mut CanvasStore {
        &mut self.store
    }

    pub fn listen(
        &mut self,
        listener: impl Fn(&CanvasStoreChange) + Send + Sync + 'static,
    ) -> CanvasStoreListenerId {
        self.store.listen(listener)
    }

    pub fn remove_listener(&mut self, id: CanvasStoreListenerId) -> bool {
        self.store.remove_listener(id)
    }

    #[cfg(test)]
    fn history_mut_for_test(&mut self) -> &mut CanvasHistory {
        self.store.history_mut_for_test()
    }

    pub(crate) fn retain_selection_for_current_document(&mut self) {
        let document = self.store.document_snapshot();
        self.session
            .retain_selection_for_document(document.as_ref());
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

        let diff = self.store.apply_transaction(transaction)?;
        if !diff.is_empty() {
            self.retain_selection_for_current_document();
        }
        Ok(diff)
    }

    pub(crate) fn apply_tool_effect(
        &mut self,
        effect: CanvasToolEffect,
    ) -> Result<(), DocumentError> {
        self.apply_editor_action(effect.into())
    }

    fn apply_editor_action(&mut self, action: CanvasEditorAction) -> Result<(), DocumentError> {
        match action {
            CanvasEditorAction::ApplyTransaction(transaction) => {
                self.apply_transaction(transaction)?;
            }
            CanvasEditorAction::BeginGesture => {
                self.begin_gesture();
            }
            CanvasEditorAction::UpdateGesture(transaction) => {
                self.update_gesture(transaction)?;
            }
            CanvasEditorAction::CommitGesture => {
                self.commit_gesture()?;
            }
            CanvasEditorAction::CancelGesture => {
                self.cancel_gesture()?;
            }
            CanvasEditorAction::SetTool(tool) => {
                self.set_tool(tool)?;
            }
            CanvasEditorAction::SetConnectionRelease(release) => {
                self.connection_release = release;
            }
            CanvasEditorAction::Session(effect) => {
                self.apply_session_effect(effect);
            }
        }

        Ok(())
    }

    pub(crate) fn prepare_gesture_commit(
        &self,
    ) -> Result<Option<CanvasPreparedGestureCommit>, DocumentError> {
        self.session
            .prepare_gesture_commit(self.document(), self.kind_registry())
    }

    pub(crate) fn apply_prepared_gesture_store_change(
        &mut self,
        prepared: CanvasPreparedGestureCommit,
    ) -> Option<CanvasStoreChange> {
        self.session.clear_gesture();
        let change = self.store.apply_prepared_gesture_commit(prepared)?;
        self.retain_selection_for_current_document();
        Some(change)
    }

    pub(crate) fn apply_tool_effects(
        &mut self,
        effects: impl IntoIterator<Item = CanvasToolEffect>,
    ) -> Result<(), DocumentError> {
        for effect in effects {
            self.apply_tool_effect(effect)?;
        }

        Ok(())
    }

    fn apply_session_effect(&mut self, effect: CanvasToolSessionEffect) {
        let document = self.store.document_snapshot();
        self.session.apply_effect(effect, document.as_ref());
    }

    pub fn apply_tool_intent(&mut self, intent: CanvasToolIntent) -> Result<(), DocumentError> {
        self.apply_editor_action(intent.into())
    }

    pub(crate) fn apply_custom_tool_intent(
        &mut self,
        intent: CanvasToolIntent,
    ) -> Result<(), DocumentError> {
        match intent {
            CanvasToolIntent::ApplyTransaction(transaction) => {
                if transaction.is_empty() {
                    return Ok(());
                }

                self.apply_tool_effects([
                    CanvasToolEffect::BeginGesture,
                    CanvasToolEffect::UpdateGesture(transaction),
                ])?;
            }
            CanvasToolIntent::CommitTransaction => {
                self.apply_tool_effect(CanvasToolEffect::CommitGesture)?;
            }
            CanvasToolIntent::CancelTransaction => {
                self.apply_tool_effect(CanvasToolEffect::CancelGesture)?;
            }
            intent => {
                self.apply_tool_intent(intent)?;
            }
        }

        Ok(())
    }

    pub fn undo(&mut self) -> Result<bool, DocumentError> {
        let changed = self.store.undo()?;
        if changed {
            self.retain_selection_for_current_document();
        }
        Ok(changed)
    }

    pub fn redo(&mut self) -> Result<bool, DocumentError> {
        let changed = self.store.redo()?;
        if changed {
            self.retain_selection_for_current_document();
        }
        Ok(changed)
    }

    pub fn rebuild_index(&mut self) {
        self.rebuild_runtime();
    }

    pub fn rebuild_runtime(&mut self) {
        self.store.rebuild_runtime();
    }

    pub fn set_edge_router<R>(&mut self, edge_router: R)
    where
        R: CanvasEdgeRouter + Send + Sync + 'static,
    {
        self.store.set_edge_router(edge_router);
    }

    pub fn set_kind_registry(
        &mut self,
        kind_registry: CanvasKindRegistry,
    ) -> Result<(), DocumentError> {
        self.store.set_kind_registry(kind_registry)?;
        let document = self.store.document_snapshot();
        self.session
            .reset_for_kind_registry_change(document.as_ref());
        Ok(())
    }

    pub fn set_tool(&mut self, tool: CanvasTool) -> Result<(), DocumentError> {
        self.cancel_gesture()?;
        self.session.set_tool(tool);
        Ok(())
    }

    pub fn set_viewport(&mut self, viewport: CanvasViewport) {
        self.session.set_viewport(viewport);
    }

    pub fn is_tool_state_idle(&self) -> bool {
        matches!(self.state(), ToolState::Idle)
    }

    pub fn connection_drag_state(&self) -> Option<CanvasConnectionDragState> {
        match self.state() {
            ToolState::Connecting { source, current } => Some(CanvasConnectionDragState {
                source: source.clone(),
                current: *current,
            }),
            _ => None,
        }
    }

    pub fn take_connection_release(&mut self) -> Option<CanvasConnectionRelease> {
        self.connection_release.take()
    }

    pub fn tool_context(&self) -> CanvasToolContext<'_> {
        CanvasToolContext {
            document: self.document(),
            viewport: self.session.viewport(),
            tool: self.tool(),
            runtime: self.runtime(),
            edge_router: self.edge_router(),
            kind_registry: self.kind_registry(),
            selection: self.selection(),
            history: self.history(),
        }
    }

    pub(crate) fn reducer_context(&self) -> CanvasToolReducerContext<'_> {
        CanvasToolReducerContext {
            document: self.document(),
            viewport: self.viewport(),
            state: self.state(),
            runtime: self.runtime(),
            edge_router: self.edge_router(),
            kind_registry: self.kind_registry(),
            selection: self.selection(),
        }
    }

    pub fn handle_event(&mut self, event: CanvasEvent) -> Result<(), DocumentError> {
        let effects = self.event_effects(event)?;
        self.apply_tool_effects(effects)
    }

    pub fn delete_selection(&mut self) -> Result<bool, DocumentError> {
        let transaction = self.delete_selection_transaction();
        if transaction.is_empty() {
            return Ok(false);
        }

        self.apply_transaction(transaction)?;
        Ok(true)
    }

    pub fn copy_selection(&self) -> Option<CanvasClipboardPayload> {
        clipboard::copy_selection(self.document(), self.selection())
    }

    pub fn cut_selection(&mut self) -> Result<Option<CanvasClipboardPayload>, DocumentError> {
        let Some(payload) = self.copy_selection() else {
            return Ok(None);
        };
        self.delete_selection()?;
        Ok(Some(payload))
    }

    pub fn paste_clipboard(
        &mut self,
        payload: &CanvasClipboardPayload,
        offset: Point<Pixels>,
    ) -> Result<bool, DocumentError> {
        let pasted = clipboard::paste_clipboard(self.document(), payload, offset);
        self.apply_paste_transaction(pasted)
    }

    pub fn duplicate_selection(&mut self, offset: Point<Pixels>) -> Result<bool, DocumentError> {
        let Some(pasted) =
            clipboard::duplicate_selection(self.document(), self.selection(), offset)
        else {
            return Ok(false);
        };
        self.apply_paste_transaction(pasted)
    }

    pub fn group_selection(&mut self, group_id: impl Into<ShapeId>) -> Result<bool, DocumentError> {
        let Some(edit) =
            group::group_selection_edit(self.document(), self.selection(), group_id.into())
        else {
            return Ok(false);
        };
        self.apply_group_edit(edit)
    }

    pub fn ungroup_selection(&mut self) -> Result<bool, DocumentError> {
        let Some(edit) = group::ungroup_selection_edit(self.document(), self.selection()) else {
            return Ok(false);
        };
        self.apply_group_edit(edit)
    }

    pub fn reorder_selection(
        &mut self,
        command: CanvasZOrderCommand,
    ) -> Result<bool, DocumentError> {
        let transaction = self.reorder_selection_transaction(command);
        if transaction.is_empty() {
            return Ok(false);
        }

        self.apply_transaction(transaction)?;
        Ok(true)
    }

    pub(crate) fn event_effects(
        &self,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        let Some(tool) = BuiltInCanvasTool::from_canvas_tool(self.tool()) else {
            return Ok(Vec::new());
        };
        tool.handle_event(self.reducer_context(), event)
    }

    pub fn handle_event_with_custom_tool<T>(
        &mut self,
        event: CanvasEvent,
        custom_tool: &mut T,
    ) -> Result<(), DocumentError>
    where
        T: CanvasToolReducer + ?Sized,
    {
        if BuiltInCanvasTool::from_canvas_tool(self.tool()).is_some() {
            let effects = self.event_effects(event)?;
            self.apply_tool_effects(effects)
        } else {
            let intents = custom_tool.handle_event(self.tool_context(), event)?;
            for intent in intents {
                self.apply_custom_tool_intent(intent)?;
            }

            Ok(())
        }
    }

    pub fn handle_event_with_tool_registry(
        &mut self,
        event: CanvasEvent,
        registry: &mut CanvasToolRegistry,
    ) -> Result<(), CanvasToolRegistryError> {
        if let Some(tool_id) = self.tool().custom_id().cloned() {
            let reducer = registry
                .reducer_mut(&tool_id)
                .ok_or_else(|| CanvasToolRegistryError::MissingTool(tool_id.clone()))?;
            let intents = reducer.handle_event(self.tool_context(), event)?;
            for intent in intents {
                self.apply_custom_tool_intent(intent)?;
            }
        } else {
            let effects = self.event_effects(event)?;
            self.apply_tool_effects(effects)?;
        }

        Ok(())
    }

    fn begin_gesture(&mut self) {
        let document = self.store.document_snapshot();
        self.session.begin_gesture(document.as_ref());
    }

    fn update_gesture(
        &mut self,
        transaction: CanvasTransaction,
    ) -> Result<CanvasDocumentDiff, DocumentError> {
        if transaction.is_empty() {
            return Ok(CanvasDocumentDiff::default());
        }

        let document = self.store.document_snapshot();
        let implicit_gesture = self.session.begin_implicit_gesture(document.as_ref());
        let diff = self.apply_transient_transaction(transaction)?;
        if let Some(gesture) = implicit_gesture {
            self.session.install_implicit_gesture(gesture);
        }
        Ok(diff)
    }

    fn apply_transient_transaction(
        &mut self,
        transaction: CanvasTransaction,
    ) -> Result<CanvasDocumentDiff, DocumentError> {
        let diff = self.store.apply_transient_transaction(transaction)?;
        if !diff.is_empty() {
            self.retain_selection_for_current_document();
        }
        Ok(diff)
    }

    fn commit_gesture(&mut self) -> Result<CanvasDocumentDiff, DocumentError> {
        let Some(prepared) = self.prepare_gesture_commit()? else {
            self.session.clear_gesture();
            return Ok(CanvasDocumentDiff::default());
        };
        let Some(change) = self.apply_prepared_gesture_store_change(prepared) else {
            return Ok(CanvasDocumentDiff::default());
        };
        Ok(change.diff().clone())
    }

    fn cancel_gesture(&mut self) -> Result<CanvasDocumentDiff, DocumentError> {
        let Some(transaction) = self.session.cancel_gesture_transaction(self.document()) else {
            return Ok(CanvasDocumentDiff::default());
        };
        let diff = self.apply_transient_transaction(transaction)?;
        self.session.clear_gesture();
        Ok(diff)
    }

    fn delete_selection_transaction(&self) -> CanvasTransaction {
        self.reducer_context().delete_selection_transaction()
    }

    fn apply_paste_transaction(
        &mut self,
        pasted: CanvasPasteTransaction,
    ) -> Result<bool, DocumentError> {
        let Some((transaction, selection)) = clipboard::paste_transaction_parts(pasted) else {
            return Ok(false);
        };

        self.apply_transaction(transaction)?;
        let document = self.store.document_snapshot();
        self.session.set_selection(selection, document.as_ref());
        Ok(true)
    }

    fn apply_group_edit(&mut self, edit: group::CanvasGroupEdit) -> Result<bool, DocumentError> {
        self.apply_transaction(edit.transaction)?;
        let document = self.store.document_snapshot();
        self.session
            .set_selection(edit.selection, document.as_ref());
        Ok(true)
    }

    fn reorder_selection_transaction(&self, command: CanvasZOrderCommand) -> CanvasTransaction {
        z_order::reorder_selection_transaction(self.document(), self.selection(), command)
    }
}

#[cfg(test)]
mod tests;
