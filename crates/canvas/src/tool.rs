use crate::gesture::CanvasPreparedGestureCommit;
use crate::layer::{
    CanvasLayerRecord, CanvasZOrderCommand, reorder_layer_z_indices, sort_layer_records,
};
use crate::record_scope::{CanvasRecordScopeOptions, collect_selection_record_scope};
use crate::session::{CanvasToolSession, CanvasToolSessionEffect, CanvasToolSessionSnapshot};
use crate::{
    CanvasClipboardPayload, CanvasDefaultEdgeRouter, CanvasDocument, CanvasDocumentDiff,
    CanvasEdgeRouter, CanvasEndpoint, CanvasKindRegistry, CanvasPasteTransaction, CanvasRecordId,
    CanvasRuntime, CanvasStore, CanvasStoreChange, CanvasStoreListenerId, CanvasTransaction,
    CanvasViewport, DocumentCommand, DocumentError, EdgeId, HitTarget, NodeId, ShapeId,
};
use indexmap::IndexSet;
use open_gpui::{Bounds, Pixels, Point};
use serde::{Deserialize, Serialize};
use std::{fmt, sync::Arc};

mod action;
mod builtin;
mod context;
mod registry;
mod select;

use crate::session::ToolState;
pub use action::CanvasToolIntent;
pub(crate) use action::{CanvasEditorAction, CanvasToolEffect};
use builtin::BuiltInCanvasTool;
pub use context::CanvasToolContext;
pub(crate) use context::CanvasToolReducerContext;
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

    pub(crate) fn push_undo(&mut self, transaction: CanvasTransaction) {
        if !transaction.is_empty() {
            self.undo_stack.push(transaction);
            self.redo_stack.clear();
        }
    }

    pub(crate) fn pop_undo(&mut self) -> Option<CanvasTransaction> {
        self.undo_stack.pop()
    }

    pub(crate) fn push_redo(&mut self, transaction: CanvasTransaction) {
        if !transaction.is_empty() {
            self.redo_stack.push(transaction);
        }
    }

    pub(crate) fn pop_redo(&mut self) -> Option<CanvasTransaction> {
        self.redo_stack.pop()
    }
}

pub struct CanvasEditor {
    store: CanvasStore,
    session: CanvasToolSession,
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
        let payload =
            CanvasClipboardPayload::from_document_selection(self.document(), self.selection());
        (!payload.is_empty()).then_some(payload)
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
        let pasted = payload.paste_transaction(self.document(), offset);
        self.apply_paste_transaction(pasted)
    }

    pub fn duplicate_selection(&mut self, offset: Point<Pixels>) -> Result<bool, DocumentError> {
        let Some(payload) = self.copy_selection() else {
            return Ok(false);
        };
        self.paste_clipboard(&payload, offset)
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
        if pasted.transaction.is_empty() {
            return Ok(false);
        }

        self.apply_transaction(pasted.transaction)?;
        let document = self.store.document_snapshot();
        self.session
            .set_selection(pasted.selection, document.as_ref());
        Ok(true)
    }

    fn reorder_selection_transaction(&self, command: CanvasZOrderCommand) -> CanvasTransaction {
        let selection_records = self.z_order_selection_record_ids();
        let mut records = self.z_order_records(&selection_records);
        let commands = reorder_layer_z_indices(&mut records, command)
            .into_iter()
            .filter_map(|(record_id, z_index)| self.z_order_update_command(&record_id, z_index))
            .collect::<Vec<_>>();

        CanvasTransaction::new(commands)
    }

    fn z_order_selection_record_ids(&self) -> IndexSet<CanvasRecordId> {
        collect_selection_record_scope(
            self.document(),
            self.selection(),
            CanvasRecordScopeOptions::structural_with_internal_edges(),
            |record_id| self.is_reorderable_record(record_id),
        )
    }

    fn is_reorderable_record(&self, record_id: &CanvasRecordId) -> bool {
        match record_id {
            CanvasRecordId::Node(id) => self.document().node(id).is_some_and(|node| !node.locked),
            CanvasRecordId::Edge(id) => self.document().edge(id).is_some_and(|edge| !edge.locked),
            CanvasRecordId::Shape(id) => {
                self.document().shape(id).is_some_and(|shape| !shape.locked)
            }
        }
    }

    fn z_order_records(
        &self,
        selection_records: &IndexSet<CanvasRecordId>,
    ) -> Vec<CanvasLayerRecord> {
        let mut ordinal = 0;
        let mut records = Vec::new();

        records.extend(self.document().nodes().map(|node| {
            let record = CanvasLayerRecord {
                id: CanvasRecordId::Node(node.id.clone()),
                z_index: node.z_index,
                ordinal,
                selected: selection_records.contains(&CanvasRecordId::Node(node.id.clone())),
            };
            ordinal += 1;
            record
        }));
        records.extend(self.document().shapes().map(|shape| {
            let record = CanvasLayerRecord {
                id: CanvasRecordId::Shape(shape.id.clone()),
                z_index: shape.z_index,
                ordinal,
                selected: selection_records.contains(&CanvasRecordId::Shape(shape.id.clone())),
            };
            ordinal += 1;
            record
        }));
        records.extend(self.document().edges().map(|edge| {
            let record = CanvasLayerRecord {
                id: CanvasRecordId::Edge(edge.id.clone()),
                z_index: edge.z_index,
                ordinal,
                selected: selection_records.contains(&CanvasRecordId::Edge(edge.id.clone())),
            };
            ordinal += 1;
            record
        }));

        sort_layer_records(&mut records);
        records
    }

    fn z_order_update_command(
        &self,
        record_id: &CanvasRecordId,
        z_index: i32,
    ) -> Option<DocumentCommand> {
        match record_id {
            CanvasRecordId::Node(id) => {
                let mut node = self.document().node(id)?.clone();
                node.z_index = z_index;
                Some(DocumentCommand::UpdateNode(node))
            }
            CanvasRecordId::Edge(id) => {
                let mut edge = self.document().edge(id)?.clone();
                edge.z_index = z_index;
                Some(DocumentCommand::UpdateEdge(edge))
            }
            CanvasRecordId::Shape(id) => {
                let mut shape = self.document().shape(id)?.clone();
                shape.z_index = z_index;
                Some(DocumentCommand::UpdateShape(shape))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::{
        CanvasEdge, CanvasNode, CanvasNodeGeometryPolicy, CanvasNodeHitTest,
        CanvasNodeInteractionPolicy, CanvasNodeKind, CanvasNodeResizeProposal,
        CanvasNodeSchemaPolicy, CanvasNodeTransformPolicy, CanvasRecordKind, CanvasResizeHandle,
        CanvasRoutePath, CanvasRouteRequest, CanvasSchemaError, CanvasShape, CanvasTransformTarget,
        CanvasValue, HandleId, HitOptions, canvas_transform_handles,
        test_support::{connected_pair_fixture, document_fixture},
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
        ) -> Result<Vec<CanvasToolIntent>, DocumentError> {
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

            let node_id = NodeId::new(format!("stamp-{}", context.document().node_count()));
            let mut selection = CanvasSelection::default();
            selection.nodes.insert(node_id.clone());

            Ok(vec![
                CanvasToolIntent::ApplyTransaction(CanvasTransaction::single(
                    DocumentCommand::InsertNode(CanvasNode::new(
                        node_id.clone(),
                        context.document_position(position),
                        size(px(20.0), px(20.0)),
                    )),
                )),
                CanvasToolIntent::SetSelection(selection),
                CanvasToolIntent::CommitTransaction,
            ])
        }
    }

    struct RequiredTitleNodeKind;

    impl CanvasNodeSchemaPolicy for RequiredTitleNodeKind {
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

    struct WideBoundsNodeKind;

    impl CanvasNodeGeometryPolicy for WideBoundsNodeKind {
        fn node_bounds(&self, node: &CanvasNode) -> Option<Bounds<Pixels>> {
            Some(Bounds::new(
                node.position,
                size(node.size.width + px(30.0), node.size.height),
            ))
        }
    }

    struct MinimumResizeNodeKind;

    impl CanvasNodeTransformPolicy for MinimumResizeNodeKind {
        fn resize_node_bounds(
            &self,
            proposal: CanvasNodeResizeProposal<'_>,
        ) -> Result<Bounds<Pixels>, CanvasSchemaError> {
            Ok(Bounds::new(
                proposal.bounds.origin,
                size(
                    proposal.bounds.size.width.max(px(64.0)),
                    proposal.bounds.size.height.max(px(48.0)),
                ),
            ))
        }
    }

    struct RejectResizeNodeKind;

    impl CanvasNodeTransformPolicy for RejectResizeNodeKind {
        fn resize_node_bounds(
            &self,
            proposal: CanvasNodeResizeProposal<'_>,
        ) -> Result<Bounds<Pixels>, CanvasSchemaError> {
            Err(CanvasSchemaError::invalid_data(
                CanvasRecordKind::Node,
                proposal.node.id.clone(),
                &proposal.node.kind,
                "resize is disabled",
            ))
        }
    }

    struct RightHalfNodeKind;

    impl CanvasNodeInteractionPolicy for RightHalfNodeKind {
        fn node_contains_point(&self, hit: CanvasNodeHitTest<'_>) -> Option<bool> {
            Some(hit.point.x >= hit.bounds.center().x)
        }
    }

    fn required_title_node_kind() -> CanvasNodeKind {
        CanvasNodeKind::new().with_schema_policy(RequiredTitleNodeKind)
    }

    fn wide_bounds_node_kind() -> CanvasNodeKind {
        CanvasNodeKind::new().with_geometry_policy(WideBoundsNodeKind)
    }

    fn minimum_resize_node_kind() -> CanvasNodeKind {
        CanvasNodeKind::new().with_transform_policy(MinimumResizeNodeKind)
    }

    fn reject_resize_node_kind() -> CanvasNodeKind {
        CanvasNodeKind::new().with_transform_policy(RejectResizeNodeKind)
    }

    fn right_half_node_kind() -> CanvasNodeKind {
        CanvasNodeKind::new().with_interaction_policy(RightHalfNodeKind)
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
        let document = document_fixture()
            .node(CanvasNode::new(
                "n1",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .build();
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

        let node = editor.document().node(&NodeId::from("n1")).unwrap();
        assert_eq!(node.position, point(px(10.0), px(15.0)));
        assert_eq!(
            editor
                .session
                .selection
                .nodes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("n1")]
        );
        assert_eq!(editor.session.state, ToolState::Idle);
        assert_eq!(editor.history().undo_depth(), 1);

        assert!(editor.undo().unwrap());
        let node = editor.document().node(&NodeId::from("n1")).unwrap();
        assert_eq!(node.position, point(px(0.0), px(0.0)));
        assert_eq!(editor.history().redo_depth(), 1);

        assert!(editor.redo().unwrap());
        let node = editor.document().node(&NodeId::from("n1")).unwrap();
        assert_eq!(node.position, point(px(10.0), px(15.0)));
    }

    #[test]
    fn select_tool_translates_shape() {
        let document = document_fixture()
            .shape(CanvasShape::new(
                "shape",
                Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(80.0))),
            ))
            .build();
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
                position: point(px(30.0), px(25.0)),
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(30.0), px(25.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        let shape = editor.document().shape(&ShapeId::from("shape")).unwrap();
        assert_eq!(shape.bounds.origin, point(px(20.0), px(15.0)));
        assert_eq!(
            editor
                .session
                .selection
                .shapes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![ShapeId::from("shape")]
        );
        assert_eq!(editor.history().undo_depth(), 1);

        assert!(editor.undo().unwrap());
        let shape = editor.document().shape(&ShapeId::from("shape")).unwrap();
        assert_eq!(shape.bounds.origin, point(px(0.0), px(0.0)));
    }

    #[test]
    fn select_tool_ignores_locked_node_hits() {
        let mut node = CanvasNode::new(
            "locked",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        );
        node.locked = true;
        let document = document_fixture().node(node).build();
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

        assert!(editor.session.selection.is_empty());
        assert_eq!(
            editor
                .document()
                .node(&NodeId::from("locked"))
                .unwrap()
                .position,
            point(px(0.0), px(0.0))
        );
        assert_eq!(editor.history().undo_depth(), 0);
    }

    #[test]
    fn select_tool_clears_selection_when_canvas_is_pressed() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "n1",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(10.0), px(10.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        assert!(!editor.session.selection.is_empty());

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

        assert!(editor.session.selection.is_empty());
    }

    #[test]
    fn select_tool_cancel_restores_selection_after_canvas_press() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "base",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        editor.session.selection.nodes.insert(NodeId::from("base"));

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(300.0), px(300.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        assert!(editor.session.selection.is_empty());

        editor.handle_event(CanvasEvent::Cancel).unwrap();

        assert_eq!(
            editor
                .session
                .selection
                .nodes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("base")]
        );
        assert_eq!(editor.session.state, ToolState::Idle);
    }

    #[test]
    fn select_tool_cancel_clears_selection_when_idle() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "base",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        editor.session.selection.nodes.insert(NodeId::from("base"));

        editor.handle_event(CanvasEvent::Cancel).unwrap();

        assert!(editor.session.selection.is_empty());
        assert_eq!(editor.session.state, ToolState::Idle);
        assert_eq!(editor.history().undo_depth(), 0);
    }

    #[test]
    fn select_tool_shift_click_toggles_selection() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .node(CanvasNode::new(
                "b",
                point(px(200.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        editor.session.selection.nodes.insert(NodeId::from("a"));

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
            editor
                .session
                .selection
                .nodes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("a"), NodeId::from("b")]
        );
        assert_eq!(editor.session.state, ToolState::Idle);
        assert_eq!(editor.history().undo_depth(), 0);

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
            editor
                .session
                .selection
                .nodes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("a")]
        );
        assert_eq!(editor.session.state, ToolState::Idle);
        assert_eq!(editor.history().undo_depth(), 0);
    }

    #[test]
    fn select_tool_uses_registered_precise_hit_policy() {
        let mut node = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        node.kind = "right-half".to_string();
        let document = document_fixture().node(node).build();
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("right-half", right_half_node_kind());
        let mut editor =
            CanvasEditor::try_new_with_kind_registry(document.clone(), registry.clone()).unwrap();

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(25.0), px(25.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        assert!(editor.session.selection.is_empty());
        assert!(
            editor
                .runtime()
                .hit_test(point(px(25.0), px(25.0)), HitOptions::default())
                .next()
                .is_some()
        );
        assert!(
            editor
                .runtime()
                .precise_hit_test_with_kind_registry(
                    editor.document(),
                    editor.kind_registry(),
                    point(px(25.0), px(25.0)),
                    HitOptions::default(),
                )
                .next()
                .is_none()
        );

        let mut editor = CanvasEditor::try_new_with_kind_registry(document, registry).unwrap();
        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(75.0), px(25.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        assert_eq!(
            editor
                .session
                .selection
                .nodes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("a")]
        );
    }

    #[test]
    fn select_tool_delete_key_removes_selected_records() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .node(CanvasNode::new(
                "b",
                point(px(200.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .shape(CanvasShape::new(
                "shape",
                Bounds::new(point(px(0.0), px(200.0)), size(px(40.0), px(40.0))),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        editor.session.selection.nodes.insert(NodeId::from("a"));
        editor.session.selection.edges.insert(EdgeId::from("a-b"));
        editor
            .session
            .selection
            .shapes
            .insert(ShapeId::from("shape"));

        editor
            .handle_event(CanvasEvent::KeyDown {
                key: CanvasKey::Delete,
                modifiers: CanvasKeyModifiers::default(),
                repeat: false,
            })
            .unwrap();

        assert!(!editor.document().contains_node(&NodeId::from("a")));
        assert!(editor.document().contains_node(&NodeId::from("b")));
        assert!(editor.document().edge_count() == 0);
        assert!(editor.document().shape_count() == 0);
        assert!(editor.session.selection.is_empty());
        assert_eq!(editor.history().undo_depth(), 1);

        assert!(editor.undo().unwrap());
        assert!(editor.document().contains_node(&NodeId::from("a")));
        assert!(editor.document().contains_edge(&EdgeId::from("a-b")));
        assert!(editor.document().contains_shape(&ShapeId::from("shape")));
    }

    #[test]
    fn select_tool_delete_key_removes_related_descendants() {
        let mut document = document_fixture()
            .shape(CanvasShape::new(
                "frame",
                Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(200.0))),
            ))
            .shape(CanvasShape::new(
                "group",
                Bounds::new(point(px(40.0), px(0.0)), size(px(50.0), px(50.0))),
            ))
            .node(CanvasNode::new(
                "leaf",
                point(px(60.0), px(0.0)),
                size(px(20.0), px(20.0)),
            ))
            .node(CanvasNode::new(
                "outside",
                point(px(260.0), px(0.0)),
                size(px(20.0), px(20.0)),
            ))
            .edge(CanvasEdge::new(
                "leaf-outside",
                CanvasEndpoint::new("leaf", None::<&str>),
                CanvasEndpoint::new("outside", None::<&str>),
            ))
            .build();
        document
            .apply_transaction(CanvasTransaction::new([
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Shape(ShapeId::from("group")),
                    parent: CanvasRecordId::Shape(ShapeId::from("frame")),
                },
                DocumentCommand::AddRecordToGroup {
                    group: CanvasRecordId::Shape(ShapeId::from("group")),
                    member: CanvasRecordId::Node(NodeId::from("leaf")),
                },
            ]))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor
            .session
            .selection
            .shapes
            .insert(ShapeId::from("frame"));

        editor
            .handle_event(CanvasEvent::KeyDown {
                key: CanvasKey::Delete,
                modifiers: CanvasKeyModifiers::default(),
                repeat: false,
            })
            .unwrap();

        assert!(!editor.document().contains_shape(&ShapeId::from("frame")));
        assert!(!editor.document().contains_shape(&ShapeId::from("group")));
        assert!(!editor.document().contains_node(&NodeId::from("leaf")));
        assert!(editor.document().contains_node(&NodeId::from("outside")));
        assert!(
            !editor
                .document()
                .contains_edge(&EdgeId::from("leaf-outside"))
        );
        assert!(editor.document().relations().is_empty());
        assert!(editor.session.selection.is_empty());

        assert!(editor.undo().unwrap());
        let group = CanvasRecordId::Shape(ShapeId::from("group"));
        let frame = CanvasRecordId::Shape(ShapeId::from("frame"));
        let leaf = CanvasRecordId::Node(NodeId::from("leaf"));
        assert!(editor.document().contains_shape(&ShapeId::from("frame")));
        assert!(editor.document().contains_shape(&ShapeId::from("group")));
        assert!(editor.document().contains_node(&NodeId::from("leaf")));
        assert!(
            editor
                .document()
                .contains_edge(&EdgeId::from("leaf-outside"))
        );
        assert_eq!(
            editor.document().relations().parent_of(&group),
            Some(&frame)
        );
        assert_eq!(
            editor
                .document()
                .relations()
                .members_of(&group)
                .cloned()
                .collect::<Vec<_>>(),
            vec![leaf]
        );
    }

    #[test]
    fn select_tool_delete_key_skips_locked_selected_records() {
        let mut locked_node = CanvasNode::new(
            "locked-node",
            point(px(0.0), px(0.0)),
            size(px(100.0), px(100.0)),
        );
        locked_node.locked = true;
        let mut locked_edge = CanvasEdge::new(
            "locked-edge",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        );
        locked_edge.locked = true;
        let mut locked_shape = CanvasShape::new(
            "locked-shape",
            Bounds::new(point(px(0.0), px(200.0)), size(px(40.0), px(40.0))),
        );
        locked_shape.locked = true;
        let document = document_fixture()
            .node(locked_node)
            .node(CanvasNode::new(
                "a",
                point(px(200.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .node(CanvasNode::new(
                "b",
                point(px(400.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .edge(locked_edge)
            .shape(locked_shape)
            .build();
        let mut editor = CanvasEditor::new(document);
        editor
            .session
            .selection
            .nodes
            .insert(NodeId::from("locked-node"));
        editor
            .session
            .selection
            .edges
            .insert(EdgeId::from("locked-edge"));
        editor
            .session
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
                .document()
                .contains_node(&NodeId::from("locked-node"))
        );
        assert!(
            editor
                .document()
                .contains_edge(&EdgeId::from("locked-edge"))
        );
        assert!(
            editor
                .document()
                .contains_shape(&ShapeId::from("locked-shape"))
        );
        assert_eq!(editor.history().undo_depth(), 0);
    }

    #[test]
    fn editor_duplicate_selection_remaps_internal_edges_and_selects_paste() {
        let mut document = document_fixture()
            .node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .node(CanvasNode::new(
                "b",
                point(px(200.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .node(CanvasNode::new(
                "outside",
                point(px(400.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .edge(CanvasEdge::new(
                "a-outside",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("outside", None::<&str>),
            ))
            .shape(CanvasShape::new(
                "frame",
                Bounds::new(point(px(-20.0), px(-20.0)), size(px(360.0), px(160.0))),
            ))
            .build();
        document
            .apply_transaction(CanvasTransaction::new([
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("a")),
                    parent: CanvasRecordId::Shape(ShapeId::from("frame")),
                },
                DocumentCommand::AddRecordToGroup {
                    group: CanvasRecordId::Shape(ShapeId::from("frame")),
                    member: CanvasRecordId::Node(NodeId::from("a")),
                },
            ]))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor.session.selection.nodes.insert(NodeId::from("a"));
        editor.session.selection.nodes.insert(NodeId::from("b"));
        editor
            .session
            .selection
            .shapes
            .insert(ShapeId::from("frame"));

        assert!(
            editor
                .duplicate_selection(point(px(20.0), px(30.0)))
                .unwrap()
        );

        assert!(editor.document().contains_node(&NodeId::from("a-copy")));
        assert!(editor.document().contains_node(&NodeId::from("b-copy")));
        assert!(editor.document().contains_edge(&EdgeId::from("a-b-copy")));
        assert!(
            editor
                .document()
                .contains_shape(&ShapeId::from("frame-copy"))
        );
        assert!(
            !editor
                .document()
                .contains_edge(&EdgeId::from("a-outside-copy"))
        );
        assert_eq!(
            editor
                .document()
                .node(&NodeId::from("a-copy"))
                .unwrap()
                .position,
            point(px(20.0), px(30.0))
        );
        assert_eq!(
            editor
                .session
                .selection
                .nodes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("a-copy"), NodeId::from("b-copy")]
        );
        assert_eq!(
            editor
                .session
                .selection
                .edges
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![EdgeId::from("a-b-copy")]
        );
        assert_eq!(
            editor
                .session
                .selection
                .shapes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![ShapeId::from("frame-copy")]
        );
        let copied_node = CanvasRecordId::Node(NodeId::from("a-copy"));
        let copied_frame = CanvasRecordId::Shape(ShapeId::from("frame-copy"));
        assert_eq!(
            editor.document().relations().parent_of(&copied_node),
            Some(&copied_frame)
        );
        assert_eq!(
            editor
                .document()
                .relations()
                .members_of(&copied_frame)
                .cloned()
                .collect::<Vec<_>>(),
            vec![copied_node]
        );
        assert_eq!(editor.history().undo_depth(), 1);
        assert!(
            editor
                .runtime()
                .hit_test(point(px(25.0), px(35.0)), HitOptions::default())
                .any(|record| record.target == HitTarget::Node(NodeId::from("a-copy")))
        );

        assert!(editor.undo().unwrap());
        assert!(!editor.document().contains_node(&NodeId::from("a-copy")));
        assert!(!editor.document().contains_edge(&EdgeId::from("a-b-copy")));
        assert!(
            !editor
                .document()
                .contains_shape(&ShapeId::from("frame-copy"))
        );
    }

    #[test]
    fn editor_cut_and_paste_selection_use_command_path() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .shape(CanvasShape::new(
                "shape",
                Bounds::new(point(px(0.0), px(200.0)), size(px(40.0), px(40.0))),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        editor.session.selection.nodes.insert(NodeId::from("a"));
        editor
            .session
            .selection
            .shapes
            .insert(ShapeId::from("shape"));

        let payload = editor.cut_selection().unwrap().unwrap();
        assert!(!editor.document().contains_node(&NodeId::from("a")));
        assert!(!editor.document().contains_shape(&ShapeId::from("shape")));
        assert!(editor.session.selection.is_empty());
        assert_eq!(editor.history().undo_depth(), 1);

        assert!(
            editor
                .paste_clipboard(&payload, point(px(10.0), px(20.0)))
                .unwrap()
        );
        assert!(editor.document().contains_node(&NodeId::from("a-copy")));
        assert!(
            editor
                .document()
                .contains_shape(&ShapeId::from("shape-copy"))
        );
        assert_eq!(
            editor
                .session
                .selection
                .nodes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("a-copy")]
        );
        assert_eq!(
            editor
                .session
                .selection
                .shapes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![ShapeId::from("shape-copy")]
        );
        assert_eq!(editor.history().undo_depth(), 2);
    }

    #[test]
    fn editor_reorders_selected_records_and_supports_undo() {
        let mut back = CanvasNode::new("back", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        back.z_index = 1;
        let mut front = CanvasNode::new(
            "front",
            point(px(10.0), px(10.0)),
            size(px(100.0), px(100.0)),
        );
        front.z_index = 2;
        let document = document_fixture().node(back).node(front).build();
        let mut editor = CanvasEditor::new(document);
        editor.session.selection.nodes.insert(NodeId::from("back"));

        assert!(
            editor
                .reorder_selection(CanvasZOrderCommand::BringToFront)
                .unwrap()
        );
        assert_eq!(
            editor
                .document()
                .node(&NodeId::from("back"))
                .unwrap()
                .z_index,
            2
        );
        assert_eq!(editor.history().undo_depth(), 1);
        assert_eq!(
            editor
                .runtime()
                .hit_test(point(px(20.0), px(20.0)), HitOptions::default())
                .next()
                .map(|record| record.target.clone()),
            Some(HitTarget::Node(NodeId::from("back")))
        );

        assert!(editor.undo().unwrap());
        assert_eq!(
            editor
                .document()
                .node(&NodeId::from("back"))
                .unwrap()
                .z_index,
            1
        );
    }

    #[test]
    fn bring_forward_crosses_sparse_adjacent_layer() {
        let mut back = CanvasNode::new("back", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        back.z_index = 1;
        let mut front = CanvasShape::new(
            "front",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        );
        front.z_index = 10;
        let document = document_fixture().node(back).shape(front).build();
        let mut editor = CanvasEditor::new(document);
        editor.session.selection.nodes.insert(NodeId::from("back"));

        assert!(
            editor
                .reorder_selection(CanvasZOrderCommand::BringForward)
                .unwrap()
        );

        assert_eq!(
            editor
                .runtime()
                .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
                .next()
                .map(|record| record.target.clone()),
            Some(HitTarget::Node(NodeId::from("back")))
        );
        assert!(
            editor
                .document()
                .node(&NodeId::from("back"))
                .unwrap()
                .z_index
                > editor
                    .document()
                    .shape(&ShapeId::from("front"))
                    .unwrap()
                    .z_index
        );
    }

    #[test]
    fn send_backward_crosses_duplicate_adjacent_layer() {
        let mut back = CanvasNode::new("back", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        back.z_index = 0;
        let mut front = CanvasShape::new(
            "front",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        );
        front.z_index = 0;
        let document = document_fixture().node(back).shape(front).build();
        let mut editor = CanvasEditor::new(document);
        editor
            .session
            .selection
            .shapes
            .insert(ShapeId::from("front"));

        assert!(
            editor
                .reorder_selection(CanvasZOrderCommand::SendBackward)
                .unwrap()
        );

        assert_eq!(
            editor
                .runtime()
                .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
                .next()
                .map(|record| record.target.clone()),
            Some(HitTarget::Node(NodeId::from("back")))
        );
        assert!(
            editor
                .document()
                .shape(&ShapeId::from("front"))
                .unwrap()
                .z_index
                < editor
                    .document()
                    .node(&NodeId::from("back"))
                    .unwrap()
                    .z_index
        );
    }

    #[test]
    fn z_order_multi_select_preserves_relative_order_across_record_kinds() {
        let mut node = CanvasNode::new("node", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        node.z_index = 0;
        let mut shape = CanvasShape::new(
            "shape",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        );
        shape.z_index = 1;
        let mut edge = CanvasEdge::new(
            "edge",
            CanvasEndpoint::new("node", None::<&str>),
            CanvasEndpoint::new("node", None::<&str>),
        );
        edge.z_index = 2;
        let mut top = CanvasShape::new(
            "top",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        );
        top.z_index = 3;
        let document = document_fixture()
            .node(node)
            .shape(shape)
            .shape(top)
            .edge(edge)
            .build();
        let mut editor = CanvasEditor::new(document);
        editor.session.selection.nodes.insert(NodeId::from("node"));
        editor.session.selection.edges.insert(EdgeId::from("edge"));

        assert!(
            editor
                .reorder_selection(CanvasZOrderCommand::BringToFront)
                .unwrap()
        );

        let hits = editor
            .runtime()
            .hit_test(
                point(px(50.0), px(50.0)),
                HitOptions {
                    include_handles: false,
                    margin: px(24.0),
                    ..HitOptions::default()
                },
            )
            .map(|record| record.target.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            hits,
            vec![
                HitTarget::Edge(EdgeId::from("edge")),
                HitTarget::Node(NodeId::from("node")),
                HitTarget::Shape(ShapeId::from("top")),
                HitTarget::Shape(ShapeId::from("shape")),
            ]
        );
    }

    #[test]
    fn z_order_selected_parent_reorders_related_descendants() {
        let mut child =
            CanvasNode::new("child", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        child.z_index = 0;
        let mut peer = CanvasNode::new("peer", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        peer.z_index = 1;
        let mut edge = CanvasEdge::new(
            "child-peer",
            CanvasEndpoint::new("child", None::<&str>),
            CanvasEndpoint::new("peer", None::<&str>),
        );
        edge.z_index = 2;
        let mut frame = CanvasShape::new(
            "frame",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        );
        frame.z_index = 3;
        let mut top = CanvasShape::new(
            "top",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        );
        top.z_index = 4;
        let mut document = document_fixture()
            .node(child)
            .node(peer)
            .edge(edge)
            .shape(frame)
            .shape(top)
            .build();
        document
            .apply_transaction(CanvasTransaction::new([
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("child")),
                    parent: CanvasRecordId::Shape(ShapeId::from("frame")),
                },
                DocumentCommand::AddRecordToGroup {
                    group: CanvasRecordId::Shape(ShapeId::from("frame")),
                    member: CanvasRecordId::Node(NodeId::from("peer")),
                },
            ]))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor
            .session
            .selection
            .shapes
            .insert(ShapeId::from("frame"));

        assert!(
            editor
                .reorder_selection(CanvasZOrderCommand::BringToFront)
                .unwrap()
        );

        let hits = editor
            .runtime()
            .hit_test(
                point(px(50.0), px(50.0)),
                HitOptions {
                    include_handles: false,
                    margin: px(24.0),
                    ..HitOptions::default()
                },
            )
            .map(|record| record.target.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            hits,
            vec![
                HitTarget::Shape(ShapeId::from("frame")),
                HitTarget::Edge(EdgeId::from("child-peer")),
                HitTarget::Node(NodeId::from("peer")),
                HitTarget::Node(NodeId::from("child")),
                HitTarget::Shape(ShapeId::from("top")),
            ]
        );

        assert_eq!(editor.history().undo_depth(), 1);
        assert!(editor.undo().unwrap());
        assert_eq!(
            editor
                .runtime()
                .hit_test(
                    point(px(50.0), px(50.0)),
                    HitOptions {
                        include_handles: false,
                        margin: px(24.0),
                        ..HitOptions::default()
                    },
                )
                .next()
                .map(|record| record.target.clone()),
            Some(HitTarget::Shape(ShapeId::from("top")))
        );
    }

    #[test]
    fn canvas_transform_handles_follow_selected_record_bounds() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "node",
                point(px(10.0), px(20.0)),
                size(px(100.0), px(80.0)),
            ))
            .shape(CanvasShape::new(
                "shape",
                Bounds::new(point(px(200.0), px(40.0)), size(px(50.0), px(30.0))),
            ))
            .build();
        let mut selection = CanvasSelection::default();
        selection.nodes.insert(NodeId::from("node"));
        selection.shapes.insert(ShapeId::from("shape"));

        let handles =
            canvas_transform_handles(&document, &selection, CanvasViewport::default(), None);

        assert_eq!(handles.len(), 8);
        assert!(handles.iter().any(|handle| {
            handle.target == CanvasTransformTarget::Node(NodeId::from("node"))
                && handle.handle == CanvasResizeHandle::BottomRight
                && handle
                    .document_bounds
                    .contains(&point(px(110.0), px(100.0)))
        }));
        assert!(handles.iter().any(|handle| {
            handle.target == CanvasTransformTarget::Shape(ShapeId::from("shape"))
                && handle.handle == CanvasResizeHandle::TopLeft
                && handle.document_bounds.contains(&point(px(200.0), px(40.0)))
        }));
    }

    #[test]
    fn canvas_transform_handles_use_registered_geometry_bounds() {
        let mut node =
            CanvasNode::new("node", point(px(10.0), px(20.0)), size(px(100.0), px(80.0)));
        node.kind = "wide".to_string();
        let document = document_fixture().node(node).build();
        let mut selection = CanvasSelection::default();
        selection.nodes.insert(NodeId::from("node"));
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("wide", wide_bounds_node_kind());

        let handles = canvas_transform_handles(
            &document,
            &selection,
            CanvasViewport::default(),
            Some(&registry),
        );

        assert_eq!(handles.len(), 4);
        assert!(handles.iter().any(|handle| {
            handle.target == CanvasTransformTarget::Node(NodeId::from("node"))
                && handle.handle == CanvasResizeHandle::BottomRight
                && handle
                    .document_bounds
                    .contains(&point(px(140.0), px(100.0)))
        }));
    }

    #[test]
    fn select_tool_resizes_selected_node_with_one_undo_entry() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "node",
                point(px(10.0), px(20.0)),
                size(px(100.0), px(80.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        editor.session.selection.nodes.insert(NodeId::from("node"));

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(110.0), px(100.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(130.0), px(125.0)),
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(130.0), px(125.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        let node = &editor.document().node(&NodeId::from("node")).unwrap();
        assert_eq!(node.position, point(px(10.0), px(20.0)));
        assert_eq!(node.size, size(px(120.0), px(105.0)));
        assert_eq!(editor.history().undo_depth(), 1);
        assert!(matches!(editor.session.state, ToolState::Idle));
        assert!(
            editor
                .runtime()
                .hit_test(point(px(128.0), px(123.0)), HitOptions::default())
                .any(|record| record.target == HitTarget::Node(NodeId::from("node")))
        );

        assert!(editor.undo().unwrap());
        let node = &editor.document().node(&NodeId::from("node")).unwrap();
        assert_eq!(node.size, size(px(100.0), px(80.0)));
    }

    #[test]
    fn select_tool_resize_uses_registered_kind_policy() {
        let mut node =
            CanvasNode::new("node", point(px(10.0), px(20.0)), size(px(100.0), px(80.0)));
        node.kind = "min-resize".to_string();
        let document = document_fixture().node(node).build();
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("min-resize", minimum_resize_node_kind());
        let mut editor = CanvasEditor::try_new_with_kind_registry(document, registry).unwrap();
        editor.session.selection.nodes.insert(NodeId::from("node"));

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(110.0), px(100.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(-100.0), px(-100.0)),
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(-100.0), px(-100.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        let node = &editor.document().node(&NodeId::from("node")).unwrap();
        assert_eq!(node.position, point(px(10.0), px(20.0)));
        assert_eq!(node.size, size(px(64.0), px(48.0)));
        assert_eq!(editor.history().undo_depth(), 1);
    }

    #[test]
    fn select_tool_resize_policy_rejection_is_atomic() {
        let mut node =
            CanvasNode::new("node", point(px(10.0), px(20.0)), size(px(100.0), px(80.0)));
        node.kind = "reject-resize".to_string();
        let original = node.clone();
        let document = document_fixture().node(node).build();
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("reject-resize", reject_resize_node_kind());
        let mut editor = CanvasEditor::try_new_with_kind_registry(document, registry).unwrap();
        editor.session.selection.nodes.insert(NodeId::from("node"));

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(110.0), px(100.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        let err = editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(130.0), px(125.0)),
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap_err();

        assert!(matches!(
            err,
            DocumentError::Schema(CanvasSchemaError::InvalidData {
                record_kind: CanvasRecordKind::Node,
                record_id: crate::CanvasRecordId::Node(id),
                kind,
                message,
            }) if id == NodeId::from("node")
                && kind == "reject-resize"
                && message == "resize is disabled"
        ));
        assert_eq!(
            editor.document().node(&NodeId::from("node")).unwrap(),
            &original
        );
        assert_eq!(editor.history().undo_depth(), 0);
        assert!(
            editor
                .runtime()
                .hit_test(point(px(108.0), px(98.0)), HitOptions::default())
                .any(|record| record.target == HitTarget::Node(NodeId::from("node")))
        );
    }

    #[test]
    fn select_tool_cancel_restores_resize_baseline() {
        let document = document_fixture()
            .shape(CanvasShape::new(
                "shape",
                Bounds::new(point(px(10.0), px(20.0)), size(px(100.0), px(80.0))),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        editor
            .session
            .selection
            .shapes
            .insert(ShapeId::from("shape"));

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(10.0), px(20.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(30.0), px(45.0)),
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        assert_eq!(
            editor
                .document()
                .shape(&ShapeId::from("shape"))
                .unwrap()
                .bounds,
            Bounds::new(point(px(30.0), px(45.0)), size(px(80.0), px(55.0)))
        );

        editor.handle_event(CanvasEvent::Cancel).unwrap();

        assert_eq!(
            editor
                .document()
                .shape(&ShapeId::from("shape"))
                .unwrap()
                .bounds,
            Bounds::new(point(px(10.0), px(20.0)), size(px(100.0), px(80.0)))
        );
        assert_eq!(editor.history().undo_depth(), 0);
        assert!(matches!(editor.session.state, ToolState::Idle));
    }

    #[test]
    fn select_tool_box_selects_intersecting_records() {
        let mut locked = CanvasNode::new(
            "locked",
            point(px(15.0), px(15.0)),
            size(px(20.0), px(20.0)),
        );
        locked.locked = true;
        let document = document_fixture()
            .node(CanvasNode::new(
                "inside",
                point(px(10.0), px(10.0)),
                size(px(20.0), px(20.0)),
            ))
            .node(CanvasNode::new(
                "outside",
                point(px(200.0), px(200.0)),
                size(px(20.0), px(20.0)),
            ))
            .node(locked)
            .build();
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
            editor
                .session
                .selection
                .nodes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("inside")]
        );
        assert_eq!(editor.session.state, ToolState::Idle);
    }

    #[test]
    fn select_tool_cancel_restores_selection_after_box_select() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "base",
                point(px(200.0), px(200.0)),
                size(px(20.0), px(20.0)),
            ))
            .node(CanvasNode::new(
                "inside",
                point(px(10.0), px(10.0)),
                size(px(20.0), px(20.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        editor.session.selection.nodes.insert(NodeId::from("base"));

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
            editor
                .session
                .selection
                .nodes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("inside")]
        );

        editor.handle_event(CanvasEvent::Cancel).unwrap();

        assert_eq!(
            editor
                .session
                .selection
                .nodes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("base")]
        );
        assert_eq!(editor.session.state, ToolState::Idle);
    }

    #[test]
    fn translating_selected_record_moves_node_and_shape_selection() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .node(CanvasNode::new(
                "b",
                point(px(200.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .shape(CanvasShape::new(
                "shape",
                Bounds::new(point(px(400.0), px(0.0)), size(px(100.0), px(100.0))),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        editor.session.selection.nodes.insert(NodeId::from("a"));
        editor.session.selection.nodes.insert(NodeId::from("b"));
        editor
            .session
            .selection
            .shapes
            .insert(ShapeId::from("shape"));

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
            editor.document().node(&NodeId::from("a")).unwrap().position,
            point(px(10.0), px(20.0))
        );
        assert_eq!(
            editor.document().node(&NodeId::from("b")).unwrap().position,
            point(px(210.0), px(20.0))
        );
        assert_eq!(
            editor
                .document()
                .shape(&ShapeId::from("shape"))
                .unwrap()
                .bounds
                .origin,
            point(px(410.0), px(20.0))
        );
        assert_eq!(editor.history().undo_depth(), 1);
    }

    #[test]
    fn translating_selected_parent_moves_related_descendants() {
        let mut document = document_fixture()
            .shape(CanvasShape::new(
                "frame",
                Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(200.0))),
            ))
            .shape(CanvasShape::new(
                "group",
                Bounds::new(point(px(40.0), px(0.0)), size(px(50.0), px(50.0))),
            ))
            .node(CanvasNode::new(
                "leaf",
                point(px(60.0), px(0.0)),
                size(px(20.0), px(20.0)),
            ))
            .build();
        document
            .apply_transaction(CanvasTransaction::new([
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Shape(ShapeId::from("group")),
                    parent: CanvasRecordId::Shape(ShapeId::from("frame")),
                },
                DocumentCommand::AddRecordToGroup {
                    group: CanvasRecordId::Shape(ShapeId::from("group")),
                    member: CanvasRecordId::Node(NodeId::from("leaf")),
                },
            ]))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor
            .session
            .selection
            .shapes
            .insert(ShapeId::from("frame"));

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(150.0), px(150.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(160.0), px(170.0)),
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(160.0), px(170.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        assert_eq!(
            editor
                .document()
                .shape(&ShapeId::from("frame"))
                .unwrap()
                .bounds
                .origin,
            point(px(10.0), px(20.0))
        );
        assert_eq!(
            editor
                .document()
                .shape(&ShapeId::from("group"))
                .unwrap()
                .bounds
                .origin,
            point(px(50.0), px(20.0))
        );
        assert_eq!(
            editor
                .document()
                .node(&NodeId::from("leaf"))
                .unwrap()
                .position,
            point(px(70.0), px(20.0))
        );
        assert_eq!(editor.history().undo_depth(), 1);
    }

    #[test]
    fn translating_from_related_descendant_keeps_parent_selection() {
        let mut document = document_fixture()
            .shape(CanvasShape::new(
                "frame",
                Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(200.0))),
            ))
            .node(CanvasNode::new(
                "leaf",
                point(px(60.0), px(40.0)),
                size(px(20.0), px(20.0)),
            ))
            .build();
        document
            .apply_transaction(CanvasTransaction::single(
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("leaf")),
                    parent: CanvasRecordId::Shape(ShapeId::from("frame")),
                },
            ))
            .unwrap();
        let mut editor = CanvasEditor::new(document);
        editor
            .session
            .selection
            .shapes
            .insert(ShapeId::from("frame"));

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(65.0), px(45.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(75.0), px(65.0)),
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(75.0), px(65.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        assert_eq!(
            editor
                .session
                .selection
                .shapes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![ShapeId::from("frame")]
        );
        assert!(editor.session.selection.nodes.is_empty());
        assert_eq!(
            editor
                .document()
                .shape(&ShapeId::from("frame"))
                .unwrap()
                .bounds
                .origin,
            point(px(10.0), px(20.0))
        );
        assert_eq!(
            editor
                .document()
                .node(&NodeId::from("leaf"))
                .unwrap()
                .position,
            point(px(70.0), px(60.0))
        );
        assert_eq!(editor.history().undo_depth(), 1);
    }

    #[test]
    fn translating_selected_node_with_shift_locks_to_dominant_axis() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        editor.session.selection.nodes.insert(NodeId::from("a"));

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
            editor.document().node(&NodeId::from("a")).unwrap().position,
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
            editor.document().node(&NodeId::from("a")).unwrap().position,
            point(px(0.0), px(25.0))
        );
        assert_eq!(editor.history().undo_depth(), 1);
    }

    #[test]
    fn translating_selected_node_snaps_to_nearby_alignment() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "active",
                point(px(0.0), px(0.0)),
                size(px(40.0), px(40.0)),
            ))
            .node(CanvasNode::new(
                "target",
                point(px(100.0), px(0.0)),
                size(px(40.0), px(40.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        editor
            .session
            .selection
            .nodes
            .insert(NodeId::from("active"));

        editor
            .handle_event(CanvasEvent::PointerDown {
                position: point(px(10.0), px(10.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(106.0), px(10.0)),
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        assert_eq!(
            editor
                .document()
                .node(&NodeId::from("active"))
                .unwrap()
                .position,
            point(px(100.0), px(0.0))
        );
        assert!(matches!(
            &editor.session.state,
            ToolState::Translating { snap_guides, .. } if !snap_guides.is_empty()
        ));

        editor
            .handle_event(CanvasEvent::PointerUp {
                position: point(px(106.0), px(10.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();
        assert_eq!(editor.history().undo_depth(), 1);
    }

    #[test]
    fn select_tool_shift_box_adds_to_base_selection_without_accumulating() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "base",
                point(px(200.0), px(200.0)),
                size(px(20.0), px(20.0)),
            ))
            .node(CanvasNode::new(
                "inside",
                point(px(10.0), px(10.0)),
                size(px(20.0), px(20.0)),
            ))
            .node(CanvasNode::new(
                "outside",
                point(px(100.0), px(100.0)),
                size(px(20.0), px(20.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        editor.session.selection.nodes.insert(NodeId::from("base"));

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
            editor
                .session
                .selection
                .nodes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("base"), NodeId::from("inside")]
        );

        editor
            .handle_event(CanvasEvent::PointerMove {
                position: point(px(-40.0), px(-40.0)),
                modifiers: CanvasKeyModifiers::default(),
            })
            .unwrap();

        assert_eq!(
            editor
                .session
                .selection
                .nodes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("base")]
        );
    }

    #[test]
    fn translating_selected_nodes_skips_locked_nodes() {
        let mut locked = CanvasNode::new(
            "locked",
            point(px(200.0), px(0.0)),
            size(px(100.0), px(100.0)),
        );
        locked.locked = true;
        let document = document_fixture()
            .node(CanvasNode::new(
                "free",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .node(locked)
            .build();
        let mut editor = CanvasEditor::new(document);
        editor.session.selection.nodes.insert(NodeId::from("free"));
        editor
            .session
            .selection
            .nodes
            .insert(NodeId::from("locked"));

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
            editor
                .document()
                .node(&NodeId::from("free"))
                .unwrap()
                .position,
            point(px(10.0), px(20.0))
        );
        assert_eq!(
            editor
                .document()
                .node(&NodeId::from("locked"))
                .unwrap()
                .position,
            point(px(200.0), px(0.0))
        );
        assert_eq!(editor.history().undo_depth(), 1);
    }

    #[test]
    fn pan_tool_moves_viewport() {
        let mut editor = CanvasEditor::default();
        editor.set_tool(CanvasTool::Pan).unwrap();

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

        assert_eq!(editor.session.viewport.origin, point(px(-10.0), px(-15.0)));
    }

    #[test]
    fn connect_tool_creates_edge_between_nodes() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .node(CanvasNode::new(
                "b",
                point(px(200.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        editor.set_tool(CanvasTool::Connect).unwrap();

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

        assert_eq!(editor.document().edge_count(), 1);
        assert_eq!(editor.history().undo_depth(), 1);

        assert!(editor.undo().unwrap());
        assert!(editor.document().edge_count() == 0);

        assert!(editor.redo().unwrap());
        assert_eq!(editor.document().edge_count(), 1);
    }

    #[test]
    fn connect_tool_ignores_locked_endpoints() {
        let mut locked =
            CanvasNode::new("b", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
        locked.locked = true;
        let document = document_fixture()
            .node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .node(locked)
            .build();
        let mut editor = CanvasEditor::new(document);
        editor.set_tool(CanvasTool::Connect).unwrap();

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

        assert!(editor.document().edge_count() == 0);
        assert_eq!(editor.history().undo_depth(), 0);
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

        let document = document_fixture().node(source).node(target).build();
        let mut editor = CanvasEditor::new(document);
        editor.set_tool(CanvasTool::Connect).unwrap();

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

        let edge = editor.document().edges().next().unwrap();
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

        let document = document_fixture().node(source).node(target).build();
        let mut editor = CanvasEditor::new(document);
        editor.set_tool(CanvasTool::Connect).unwrap();

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

        assert!(matches!(editor.session.state, ToolState::Idle));
        assert!(editor.document().edge_count() == 0);
        assert_eq!(editor.history().undo_depth(), 0);
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

        let document = document_fixture().node(source).node(target).build();
        let mut editor = CanvasEditor::new(document);
        editor.set_tool(CanvasTool::Connect).unwrap();

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

        assert!(matches!(editor.session.state, ToolState::Idle));
        assert!(editor.document().edge_count() == 0);
        assert_eq!(editor.history().undo_depth(), 0);
    }

    #[test]
    fn custom_tool_reducer_applies_effects_through_editor() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "anchor",
                point(px(100.0), px(50.0)),
                size(px(80.0), px(80.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        editor.session.viewport = CanvasViewport::new(point(px(100.0), px(50.0)), 2.0).unwrap();
        editor.set_tool(CanvasTool::custom("stamp")).unwrap();
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

        let stamped = editor.document().node(&NodeId::from("stamp-1")).unwrap();
        assert_eq!(stamped.position, point(px(110.0), px(55.0)));
        assert_eq!(editor.history().undo_depth(), 1);
        assert_eq!(
            editor
                .session
                .selection
                .nodes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("stamp-1")]
        );
        assert_eq!(editor.session.state, ToolState::Idle);

        assert!(editor.undo().unwrap());
        assert!(!editor.document().contains_node(&NodeId::from("stamp-1")));
    }

    #[test]
    fn custom_tool_context_exposes_selection_record_scope() {
        #[derive(Clone)]
        struct ScopeProbeTool {
            observed: Arc<Mutex<Vec<CanvasRecordId>>>,
        }

        impl CanvasToolReducer for ScopeProbeTool {
            fn handle_event(
                &mut self,
                context: CanvasToolContext<'_>,
                _event: CanvasEvent,
            ) -> Result<Vec<CanvasToolIntent>, DocumentError> {
                let scope = context
                    .selection_record_scope(
                        CanvasRecordScopeOptions::structural_with_internal_edges(),
                    )
                    .records()
                    .cloned()
                    .collect::<Vec<_>>();
                *self.observed.lock().unwrap() = scope;
                Ok(Vec::new())
            }
        }

        let mut document = document_fixture()
            .shape(CanvasShape::new(
                "frame",
                Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(200.0))),
            ))
            .node(CanvasNode::new(
                "child",
                point(px(20.0), px(20.0)),
                size(px(40.0), px(40.0)),
            ))
            .node(CanvasNode::new(
                "peer",
                point(px(80.0), px(20.0)),
                size(px(40.0), px(40.0)),
            ))
            .edge(CanvasEdge::new(
                "child-peer",
                CanvasEndpoint::new("child", None::<&str>),
                CanvasEndpoint::new("peer", None::<&str>),
            ))
            .build();
        document
            .apply_transaction(CanvasTransaction::new([
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("child")),
                    parent: CanvasRecordId::Shape(ShapeId::from("frame")),
                },
                DocumentCommand::AddRecordToGroup {
                    group: CanvasRecordId::Shape(ShapeId::from("frame")),
                    member: CanvasRecordId::Node(NodeId::from("peer")),
                },
            ]))
            .unwrap();

        let observed = Arc::new(Mutex::new(Vec::new()));
        let mut editor = CanvasEditor::new(document);
        editor.set_tool(CanvasTool::custom("probe")).unwrap();
        editor
            .session
            .selection
            .shapes
            .insert(ShapeId::from("frame"));
        let mut tool = ScopeProbeTool {
            observed: Arc::clone(&observed),
        };

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

        assert_eq!(
            *observed.lock().unwrap(),
            vec![
                CanvasRecordId::Shape(ShapeId::from("frame")),
                CanvasRecordId::Node(NodeId::from("child")),
                CanvasRecordId::Node(NodeId::from("peer")),
                CanvasRecordId::Edge(EdgeId::from("child-peer")),
            ]
        );
    }

    #[test]
    fn custom_tool_entry_uses_builtin_tools_without_calling_custom_reducer() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "n1",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .build();
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
            editor
                .session
                .selection
                .nodes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("n1")]
        );
    }

    #[test]
    fn tool_registry_dispatches_registered_custom_tool() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "anchor",
                point(px(100.0), px(50.0)),
                size(px(80.0), px(80.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        editor.session.viewport = CanvasViewport::new(point(px(100.0), px(50.0)), 2.0).unwrap();
        editor.set_tool(CanvasTool::custom("stamp")).unwrap();
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

        let stamped = editor.document().node(&NodeId::from("stamp-1")).unwrap();
        assert_eq!(stamped.position, point(px(110.0), px(55.0)));
        assert_eq!(editor.history().undo_depth(), 1);
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
        editor.set_tool(CanvasTool::custom("missing")).unwrap();
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
        let document = document_fixture()
            .node(CanvasNode::new(
                "n1",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .build();
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
            editor
                .session
                .selection
                .nodes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("n1")]
        );
    }

    #[test]
    fn set_tool_effect_switches_tool_and_resets_state() {
        let mut editor = CanvasEditor::default();
        editor.session.state = ToolState::Pointing {
            origin: point(px(10.0), px(20.0)),
            selection_mode: CanvasSelectionMode::Replace,
            base_selection: CanvasSelection::default(),
        };

        editor
            .apply_tool_effect(CanvasToolEffect::SetTool(CanvasTool::custom("stamp")))
            .unwrap();

        assert_eq!(editor.session.tool, CanvasTool::custom("stamp"));
        assert_eq!(editor.session.state, ToolState::Idle);
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
        assert_eq!(editor.history().redo_depth(), 1);

        editor
            .apply(DocumentCommand::InsertNode(CanvasNode::new(
                "b",
                point(px(100.0), px(0.0)),
                size(px(100.0), px(100.0)),
            )))
            .unwrap();

        assert_eq!(editor.history().undo_depth(), 1);
        assert_eq!(editor.history().redo_depth(), 0);
        assert!(editor.document().contains_node(&NodeId::from("b")));
        assert!(!editor.document().contains_node(&NodeId::from("a")));
    }

    #[test]
    fn no_op_committed_transactions_do_not_push_history_or_clear_redo() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "child",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .shape(CanvasShape::new(
                "frame",
                Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        let relation_transaction = CanvasTransaction::single(DocumentCommand::SetRecordParent {
            child: CanvasRecordId::Node(NodeId::from("child")),
            parent: CanvasRecordId::Shape(ShapeId::from("frame")),
        });

        let first_diff = editor
            .apply_transaction_with_diff(relation_transaction.clone())
            .unwrap();
        assert!(!first_diff.is_empty());
        assert_eq!(editor.history().undo_depth(), 1);

        assert!(editor.undo().unwrap());
        assert_eq!(editor.history().undo_depth(), 0);
        assert_eq!(editor.history().redo_depth(), 1);

        let second_diff = editor
            .apply_transaction_with_diff(CanvasTransaction::single(
                DocumentCommand::ClearRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("child")),
                },
            ))
            .unwrap();

        assert!(second_diff.is_empty());
        assert_eq!(editor.history().undo_depth(), 0);
        assert_eq!(editor.history().redo_depth(), 1);
    }

    #[test]
    fn relation_order_only_transactions_do_not_push_history() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "member",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .build();
        let mut document = document;
        for id in ["group-a", "group-b"] {
            document
                .insert_shape(CanvasShape::new(
                    id,
                    Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
                ))
                .unwrap();
        }
        let member = CanvasRecordId::Node(NodeId::from("member"));
        let group_a = CanvasRecordId::Shape(ShapeId::from("group-a"));
        let group_b = CanvasRecordId::Shape(ShapeId::from("group-b"));
        document
            .apply_transaction(CanvasTransaction::new([
                DocumentCommand::AddRecordToGroup {
                    group: group_a.clone(),
                    member: member.clone(),
                },
                DocumentCommand::AddRecordToGroup {
                    group: group_b,
                    member: member.clone(),
                },
            ]))
            .unwrap();
        let mut editor = CanvasEditor::new(document);

        let diff = editor
            .apply_transaction_with_diff(CanvasTransaction::new([
                DocumentCommand::RemoveRecordFromGroup {
                    group: group_a.clone(),
                    member: member.clone(),
                },
                DocumentCommand::AddRecordToGroup {
                    group: group_a,
                    member,
                },
            ]))
            .unwrap();

        assert!(diff.is_empty());
        assert_eq!(editor.history().undo_depth(), 0);
    }

    #[test]
    fn no_op_undo_and_redo_discard_stale_history_entries() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "child",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);
        let noop = CanvasTransaction::single(DocumentCommand::ClearRecordParent {
            child: CanvasRecordId::Node(NodeId::from("child")),
        });
        editor.history_mut_for_test().push_undo(noop.clone());
        editor.history_mut_for_test().push_redo(noop);

        assert!(!editor.undo().unwrap());
        assert_eq!(editor.history().undo_depth(), 0);
        assert_eq!(editor.history().redo_depth(), 1);

        assert!(!editor.redo().unwrap());
        assert_eq!(editor.history().undo_depth(), 0);
        assert_eq!(editor.history().redo_depth(), 0);
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
        assert!(editor.history().can_undo());
    }

    #[test]
    fn editor_kind_registry_normalizes_and_validates_transactions() {
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("note", required_title_node_kind());
        let mut editor =
            CanvasEditor::try_new_with_kind_registry(document_fixture().build(), registry).unwrap();

        let mut note = CanvasNode::new("note", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        note.kind = "note".to_string();
        note.data.insert("label".to_string(), json!("Migrated"));

        editor.apply(DocumentCommand::InsertNode(note)).unwrap();

        assert_eq!(
            editor
                .document()
                .node(&NodeId::from("note"))
                .unwrap()
                .data
                .get("title"),
            Some(&json!("Migrated"))
        );
        assert_eq!(editor.history().undo_depth(), 1);

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
        assert!(!editor.document().contains_node(&NodeId::from("invalid")));
        assert_eq!(editor.history().undo_depth(), 1);
    }

    #[test]
    fn editor_set_kind_registry_normalizes_document_and_clears_stale_history() {
        let mut note = CanvasNode::new("note", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        note.kind = "note".to_string();
        note.data.insert("label".to_string(), json!("Migrated"));
        let mut editor = CanvasEditor::default();
        editor.apply(DocumentCommand::InsertNode(note)).unwrap();
        assert_eq!(editor.history().undo_depth(), 1);

        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("note", required_title_node_kind());
        editor.set_kind_registry(registry).unwrap();

        assert_eq!(
            editor
                .document()
                .node(&NodeId::from("note"))
                .unwrap()
                .data
                .get("title"),
            Some(&json!("Migrated"))
        );
        assert_eq!(editor.history().undo_depth(), 0);
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
        let document = document_fixture().node(note).build();
        let mut editor = CanvasEditor::new(document);

        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind("note", required_title_node_kind());
        let err = editor.set_kind_registry(registry).unwrap_err();

        assert!(matches!(
            err,
            DocumentError::Schema(CanvasSchemaError::InvalidData {
                record_id: crate::CanvasRecordId::Node(id),
                ..
            }) if id == NodeId::from("note")
        ));
        assert_eq!(
            editor
                .document()
                .node(&NodeId::from("note"))
                .unwrap()
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

        assert!(editor.document().contains_node(&NodeId::from("a")));
        assert_eq!(editor.history().undo_depth(), 1);
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

        assert!(editor.document().contains_node(&NodeId::from("a")));
        assert_eq!(editor.history().undo_depth(), 0);
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
        registry.register_node_kind("note", required_title_node_kind());
        let mut editor =
            CanvasEditor::try_new_with_kind_registry(document_fixture().build(), registry).unwrap();
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
        assert_eq!(
            editor.document().node(&NodeId::from("note")).unwrap(),
            &note
        );
        assert_eq!(editor.history().undo_depth(), 1);
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

        assert_eq!(editor.document().node(&NodeId::from("a")).unwrap(), &second);
        assert_eq!(editor.history().undo_depth(), 2);
        assert!(editor.undo().unwrap());
        assert_eq!(
            editor.document().node(&NodeId::from("a")).unwrap(),
            &original
        );
    }

    #[test]
    fn gesture_updates_notify_listeners_only_on_commit() {
        let mut editor = CanvasEditor::default();
        let original = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        let moved = CanvasNode::new("a", point(px(40.0), px(0.0)), size(px(100.0), px(100.0)));
        editor
            .apply(DocumentCommand::InsertNode(original.clone()))
            .unwrap();
        let baseline_depth = editor.history().undo_depth();
        let changes = Arc::new(Mutex::new(Vec::new()));
        let observed = Arc::clone(&changes);
        editor.listen(move |change| observed.lock().unwrap().push(change.clone()));

        editor
            .apply_tool_effects([
                CanvasToolEffect::BeginGesture,
                CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                    DocumentCommand::UpdateNode(moved.clone()),
                )),
            ])
            .unwrap();

        assert!(changes.lock().unwrap().is_empty());
        assert_eq!(editor.history().undo_depth(), baseline_depth);

        editor
            .apply_tool_effect(CanvasToolEffect::CommitGesture)
            .unwrap();

        let changes = changes.lock().unwrap();
        assert_eq!(changes.len(), 1);
        let change = &changes[0];
        assert_eq!(change.source(), crate::CanvasStoreMutationSource::Gesture);
        assert_eq!(
            change.history_effect(),
            crate::CanvasStoreHistoryEffect::PushUndo
        );
        assert_eq!(change.document().node(&NodeId::from("a")).unwrap(), &moved);
        assert_eq!(editor.history().undo_depth(), baseline_depth + 1);
    }

    #[test]
    fn gesture_commit_records_relation_updates() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "child",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .shape(CanvasShape::new(
                "frame",
                Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            ))
            .build();
        let child = CanvasRecordId::Node(NodeId::from("child"));
        let frame = CanvasRecordId::Shape(ShapeId::from("frame"));
        let mut editor = CanvasEditor::new(document);

        editor
            .apply_tool_effects([
                CanvasToolEffect::BeginGesture,
                CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                    DocumentCommand::SetRecordParent {
                        child: child.clone(),
                        parent: frame.clone(),
                    },
                )),
                CanvasToolEffect::CommitGesture,
            ])
            .unwrap();

        assert_eq!(
            editor.document().relations().parent_of(&child),
            Some(&frame)
        );
        assert_eq!(editor.history().undo_depth(), 1);

        assert!(editor.undo().unwrap());
        assert_eq!(editor.document().relations().parent_of(&child), None);
    }

    #[test]
    fn empty_gesture_commit_does_not_push_history() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "child",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .build();
        let mut editor = CanvasEditor::new(document);

        editor
            .apply_tool_effects([
                CanvasToolEffect::BeginGesture,
                CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                    DocumentCommand::ClearRecordParent {
                        child: CanvasRecordId::Node(NodeId::from("child")),
                    },
                )),
            ])
            .unwrap();

        editor
            .apply_tool_effect(CanvasToolEffect::CommitGesture)
            .unwrap();

        assert_eq!(editor.history().undo_depth(), 0);
        assert!(editor.is_tool_state_idle());
    }

    #[test]
    fn set_tool_cancels_active_gesture_before_switching() {
        let mut editor = CanvasEditor::default();
        let original = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        let moved = CanvasNode::new("a", point(px(400.0), px(0.0)), size(px(100.0), px(100.0)));
        editor
            .apply(DocumentCommand::InsertNode(original.clone()))
            .unwrap();

        editor
            .apply_tool_effects([
                CanvasToolEffect::BeginGesture,
                CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                    DocumentCommand::UpdateNode(moved),
                )),
            ])
            .unwrap();
        assert_eq!(
            editor.document().node(&NodeId::from("a")).unwrap().position,
            point(px(400.0), px(0.0))
        );

        editor.set_tool(CanvasTool::Pan).unwrap();

        assert_eq!(editor.tool(), &CanvasTool::Pan);
        assert_eq!(
            editor.document().node(&NodeId::from("a")).unwrap(),
            &original
        );
        assert_eq!(editor.history().undo_depth(), 1);
        assert!(
            editor
                .runtime()
                .hit_test(point(px(10.0), px(10.0)), HitOptions::default())
                .any(|record| record.target == HitTarget::Node(NodeId::from("a")))
        );
        assert!(
            editor
                .runtime()
                .hit_test(point(px(410.0), px(10.0)), HitOptions::default())
                .next()
                .is_none()
        );
    }

    #[test]
    fn begin_gesture_preserves_existing_baseline() {
        let mut editor = CanvasEditor::default();
        let original = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        let first = CanvasNode::new("a", point(px(100.0), px(0.0)), size(px(100.0), px(100.0)));
        let second = CanvasNode::new("a", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
        editor
            .apply(DocumentCommand::InsertNode(original.clone()))
            .unwrap();

        editor
            .apply_tool_effects([
                CanvasToolEffect::BeginGesture,
                CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                    DocumentCommand::UpdateNode(first),
                )),
                CanvasToolEffect::BeginGesture,
                CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                    DocumentCommand::UpdateNode(second),
                )),
                CanvasToolEffect::CancelGesture,
            ])
            .unwrap();

        assert_eq!(
            editor.document().node(&NodeId::from("a")).unwrap(),
            &original
        );
        assert_eq!(editor.history().undo_depth(), 1);
    }

    #[test]
    fn public_tool_intents_commit_transaction_as_one_undo_entry() {
        let mut editor = CanvasEditor::default();
        let original = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        let first = CanvasNode::new("a", point(px(100.0), px(0.0)), size(px(100.0), px(100.0)));
        let second = CanvasNode::new("a", point(px(200.0), px(0.0)), size(px(100.0), px(100.0)));
        editor
            .apply(DocumentCommand::InsertNode(original.clone()))
            .unwrap();
        let baseline_depth = editor.history().undo_depth();

        for intent in [
            CanvasToolIntent::ApplyTransaction(CanvasTransaction::single(
                DocumentCommand::UpdateNode(first),
            )),
            CanvasToolIntent::ApplyTransaction(CanvasTransaction::single(
                DocumentCommand::UpdateNode(second.clone()),
            )),
            CanvasToolIntent::CommitTransaction,
        ] {
            editor.apply_custom_tool_intent(intent).unwrap();
        }

        assert_eq!(editor.document().node(&NodeId::from("a")).unwrap(), &second);
        assert_eq!(editor.history().undo_depth(), baseline_depth + 1);
        assert!(editor.undo().unwrap());
        assert_eq!(
            editor.document().node(&NodeId::from("a")).unwrap(),
            &original
        );
    }

    #[test]
    fn public_tool_intents_cancel_transaction_without_history() {
        let mut editor = CanvasEditor::default();
        let original = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        let moved = CanvasNode::new("a", point(px(100.0), px(0.0)), size(px(100.0), px(100.0)));
        editor
            .apply(DocumentCommand::InsertNode(original.clone()))
            .unwrap();
        let baseline_depth = editor.history().undo_depth();

        for intent in [
            CanvasToolIntent::ApplyTransaction(CanvasTransaction::single(
                DocumentCommand::UpdateNode(moved),
            )),
            CanvasToolIntent::CancelTransaction,
        ] {
            editor.apply_custom_tool_intent(intent).unwrap();
        }

        assert_eq!(
            editor.document().node(&NodeId::from("a")).unwrap(),
            &original
        );
        assert_eq!(editor.history().undo_depth(), baseline_depth);
    }

    #[test]
    fn gesture_cancel_restores_document_without_history() {
        let mut editor = CanvasEditor::default();
        let original = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        let moved = CanvasNode::new("a", point(px(40.0), px(0.0)), size(px(100.0), px(100.0)));
        editor
            .apply(DocumentCommand::InsertNode(original.clone()))
            .unwrap();
        let undo_depth = editor.history().undo_depth();

        editor
            .apply_tool_effects([
                CanvasToolEffect::BeginGesture,
                CanvasToolEffect::UpdateGesture(CanvasTransaction::single(
                    DocumentCommand::UpdateNode(moved),
                )),
                CanvasToolEffect::CancelGesture,
            ])
            .unwrap();

        assert_eq!(
            editor.document().node(&NodeId::from("a")).unwrap(),
            &original
        );
        assert_eq!(editor.history().undo_depth(), undo_depth);
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
            editor
                .session
                .selection
                .nodes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![NodeId::from("a")]
        );
        assert_eq!(
            editor.session.state,
            ToolState::Pointing {
                origin: point(px(10.0), px(20.0)),
                selection_mode: CanvasSelectionMode::Replace,
                base_selection: CanvasSelection::default(),
            }
        );
        assert_eq!(editor.session.viewport.origin, point(px(5.0), px(-3.0)));
    }

    #[test]
    fn tool_effects_update_selection_incrementally() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .node(CanvasNode::new(
                "b",
                point(px(200.0), px(0.0)),
                size(px(100.0), px(100.0)),
            ))
            .edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .shape(CanvasShape::new(
                "shape",
                Bounds::new(point(px(0.0), px(200.0)), size(px(40.0), px(40.0))),
            ))
            .build();
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

        assert!(editor.session.selection.nodes.is_empty());
        assert!(editor.session.selection.shapes.is_empty());
        assert_eq!(
            editor
                .session
                .selection
                .edges
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
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
        editor.session.selection.nodes.insert(NodeId::from("a"));

        editor
            .apply(DocumentCommand::RemoveNode(NodeId::from("a")))
            .unwrap();

        assert!(editor.session.selection.is_empty());
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

        let mut target = editor.document().node(&NodeId::from("b")).unwrap().clone();
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
        connected_pair_fixture().build()
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
