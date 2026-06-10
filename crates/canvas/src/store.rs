use std::sync::Arc;

use crate::gesture::CanvasPreparedGestureCommit;
use crate::mutation::CanvasPreparedMutation;
use crate::{
    CanvasCommittedMutation, CanvasDefaultEdgeRouter, CanvasDocument, CanvasDocumentDiff,
    CanvasEdgeRouter, CanvasHistory, CanvasKindRegistry, CanvasRuntime, CanvasTransaction,
    DocumentError,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CanvasStoreMutationSource {
    #[default]
    Local,
    Gesture,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CanvasStoreHistoryEffect {
    #[default]
    None,
    PushUndo,
    Undo,
    Redo,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CanvasStoreListenerId(u64);

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasStoreChange {
    source: CanvasStoreMutationSource,
    history_effect: CanvasStoreHistoryEffect,
    committed: CanvasCommittedMutation,
    document: Arc<CanvasDocument>,
    runtime: Arc<CanvasRuntime>,
}

impl CanvasStoreChange {
    pub fn source(&self) -> CanvasStoreMutationSource {
        self.source
    }

    pub fn history_effect(&self) -> CanvasStoreHistoryEffect {
        self.history_effect
    }

    pub fn committed(&self) -> &CanvasCommittedMutation {
        &self.committed
    }

    pub fn diff(&self) -> &CanvasDocumentDiff {
        self.committed.diff()
    }

    pub fn document(&self) -> &CanvasDocument {
        self.document.as_ref()
    }

    pub fn document_snapshot(&self) -> Arc<CanvasDocument> {
        Arc::clone(&self.document)
    }

    pub fn runtime(&self) -> &CanvasRuntime {
        self.runtime.as_ref()
    }

    pub fn runtime_snapshot(&self) -> Arc<CanvasRuntime> {
        Arc::clone(&self.runtime)
    }
}

struct CanvasStoreListener {
    id: CanvasStoreListenerId,
    callback: Box<dyn Fn(&CanvasStoreChange) + Send + Sync + 'static>,
}

pub struct CanvasStore {
    document: Arc<CanvasDocument>,
    runtime: Arc<CanvasRuntime>,
    edge_router: Arc<dyn CanvasEdgeRouter + Send + Sync>,
    kind_registry: Arc<CanvasKindRegistry>,
    history: CanvasHistory,
    listeners: Vec<CanvasStoreListener>,
    next_listener_id: u64,
}

impl Default for CanvasStore {
    fn default() -> Self {
        Self::new(CanvasDocument::default())
    }
}

impl CanvasStore {
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
            document: Arc::new(document),
            runtime: Arc::new(runtime),
            edge_router,
            kind_registry,
            history: CanvasHistory::default(),
            listeners: Vec::new(),
            next_listener_id: 0,
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
            document: Arc::new(document),
            runtime: Arc::new(runtime),
            edge_router,
            kind_registry,
            history: CanvasHistory::default(),
            listeners: Vec::new(),
            next_listener_id: 0,
        })
    }

    pub fn document(&self) -> &CanvasDocument {
        self.document.as_ref()
    }

    pub(crate) fn document_snapshot(&self) -> Arc<CanvasDocument> {
        Arc::clone(&self.document)
    }

    pub fn runtime(&self) -> &CanvasRuntime {
        self.runtime.as_ref()
    }

    pub(crate) fn runtime_snapshot(&self) -> Arc<CanvasRuntime> {
        Arc::clone(&self.runtime)
    }

    pub fn edge_router(&self) -> &(dyn CanvasEdgeRouter + Send + Sync) {
        self.edge_router.as_ref()
    }

    pub fn kind_registry(&self) -> &CanvasKindRegistry {
        self.kind_registry.as_ref()
    }

    pub(crate) fn kind_registry_snapshot(&self) -> Arc<CanvasKindRegistry> {
        Arc::clone(&self.kind_registry)
    }

    pub fn history(&self) -> &CanvasHistory {
        &self.history
    }

    #[cfg(test)]
    pub(crate) fn history_mut_for_test(&mut self) -> &mut CanvasHistory {
        &mut self.history
    }

    pub fn listen(
        &mut self,
        listener: impl Fn(&CanvasStoreChange) + Send + Sync + 'static,
    ) -> CanvasStoreListenerId {
        let id = CanvasStoreListenerId(self.next_listener_id);
        self.next_listener_id += 1;
        self.listeners.push(CanvasStoreListener {
            id,
            callback: Box::new(listener),
        });
        id
    }

    pub fn remove_listener(&mut self, id: CanvasStoreListenerId) -> bool {
        let Some(index) = self.listeners.iter().position(|listener| listener.id == id) else {
            return false;
        };
        self.listeners.remove(index);
        true
    }

    pub fn apply_transaction(
        &mut self,
        transaction: CanvasTransaction,
    ) -> Result<CanvasDocumentDiff, DocumentError> {
        self.commit_transaction(transaction).map(|change| {
            change.map_or_else(CanvasDocumentDiff::default, |change| change.diff().clone())
        })
    }

    pub fn commit_transaction(
        &mut self,
        transaction: CanvasTransaction,
    ) -> Result<Option<CanvasStoreChange>, DocumentError> {
        if transaction.is_empty() {
            return Ok(None);
        }

        let prepared = self.prepare_transaction(transaction)?;
        Ok(self.apply_prepared_mutation(
            prepared,
            CanvasStoreMutationSource::Local,
            CanvasStoreHistoryEffect::PushUndo,
        ))
    }

    pub fn prepare_transaction(
        &self,
        transaction: CanvasTransaction,
    ) -> Result<CanvasPreparedMutation, DocumentError> {
        self.document
            .prepare_transaction_with_kind_registry(transaction, self.kind_registry.as_ref())
    }

    pub fn apply_prepared_transaction(
        &mut self,
        prepared: CanvasPreparedMutation,
    ) -> Option<CanvasStoreChange> {
        self.apply_prepared_mutation(
            prepared,
            CanvasStoreMutationSource::Local,
            CanvasStoreHistoryEffect::PushUndo,
        )
    }

    pub fn prepare_undo(&self) -> Result<Option<CanvasPreparedMutation>, DocumentError> {
        let Some(transaction) = self.history.next_undo_transaction().cloned() else {
            return Ok(None);
        };
        self.prepare_transaction(transaction).map(Some)
    }

    pub fn apply_prepared_undo(
        &mut self,
        prepared: CanvasPreparedMutation,
    ) -> Option<CanvasStoreChange> {
        debug_assert_eq!(
            self.history.next_undo_transaction(),
            Some(prepared.committed().transaction())
        );
        let committed = prepared.apply_to(self.document_mut());
        if committed.diff().is_empty() {
            let _ = self.history.pop_undo();
            return None;
        }

        let _ = self.history.pop_undo();
        self.history.push_redo(committed.inverse().clone());
        Some(self.finish_committed_mutation(
            committed,
            CanvasStoreMutationSource::Local,
            CanvasStoreHistoryEffect::Undo,
        ))
    }

    pub fn prepare_redo(&self) -> Result<Option<CanvasPreparedMutation>, DocumentError> {
        let Some(transaction) = self.history.next_redo_transaction().cloned() else {
            return Ok(None);
        };
        self.prepare_transaction(transaction).map(Some)
    }

    pub fn apply_prepared_redo(
        &mut self,
        prepared: CanvasPreparedMutation,
    ) -> Option<CanvasStoreChange> {
        debug_assert_eq!(
            self.history.next_redo_transaction(),
            Some(prepared.committed().transaction())
        );
        let committed = prepared.apply_to(self.document_mut());
        if committed.diff().is_empty() {
            let _ = self.history.pop_redo();
            return None;
        }

        let _ = self.history.pop_redo();
        self.history.push_undo(committed.inverse().clone());
        Some(self.finish_committed_mutation(
            committed,
            CanvasStoreMutationSource::Local,
            CanvasStoreHistoryEffect::Redo,
        ))
    }

    pub fn undo(&mut self) -> Result<bool, DocumentError> {
        let Some(prepared) = self.prepare_undo()? else {
            return Ok(false);
        };
        Ok(self.apply_prepared_undo(prepared).is_some())
    }

    pub fn redo(&mut self) -> Result<bool, DocumentError> {
        let Some(prepared) = self.prepare_redo()? else {
            return Ok(false);
        };
        Ok(self.apply_prepared_redo(prepared).is_some())
    }

    pub(crate) fn apply_transient_transaction(
        &mut self,
        transaction: CanvasTransaction,
    ) -> Result<CanvasDocumentDiff, DocumentError> {
        if transaction.is_empty() {
            return Ok(CanvasDocumentDiff::default());
        }

        let prepared = self.prepare_transaction(transaction)?;
        let committed = prepared.apply_to(self.document_mut());
        let diff = committed.diff().clone();
        if diff.is_empty() {
            return Ok(diff);
        }

        self.sync_runtime_committed(&committed);
        Ok(diff)
    }

    pub(crate) fn apply_prepared_gesture_commit(
        &mut self,
        prepared: CanvasPreparedGestureCommit,
    ) -> Option<CanvasStoreChange> {
        let committed = prepared.committed().clone();
        if committed.diff().is_empty() {
            return None;
        }

        self.history.push_undo(committed.inverse().clone());
        Some(self.finish_committed_mutation(
            committed,
            CanvasStoreMutationSource::Gesture,
            CanvasStoreHistoryEffect::PushUndo,
        ))
    }

    pub fn rebuild_runtime(&mut self) {
        self.runtime = Arc::new(CanvasRuntime::rebuild_with_router_and_kind_registry(
            self.document.as_ref(),
            self.edge_router.as_ref(),
            self.kind_registry.as_ref(),
        ));
    }

    pub fn set_edge_router<R>(&mut self, edge_router: R)
    where
        R: CanvasEdgeRouter + Send + Sync + 'static,
    {
        self.edge_router = Arc::new(edge_router);
        self.rebuild_runtime();
    }

    pub(crate) fn set_kind_registry(
        &mut self,
        kind_registry: CanvasKindRegistry,
    ) -> Result<bool, DocumentError> {
        let document = CanvasDocument::from_snapshot_with_kind_registry(
            self.document.to_snapshot(),
            &kind_registry,
        )?;
        let document_changed = document != *self.document;
        self.document = Arc::new(document);
        self.kind_registry = Arc::new(kind_registry);
        if document_changed {
            self.history.clear();
        }
        self.rebuild_runtime();
        Ok(document_changed)
    }

    fn apply_prepared_mutation(
        &mut self,
        prepared: CanvasPreparedMutation,
        source: CanvasStoreMutationSource,
        history_effect: CanvasStoreHistoryEffect,
    ) -> Option<CanvasStoreChange> {
        let committed = prepared.apply_to(self.document_mut());
        if committed.diff().is_empty() {
            return None;
        }

        if history_effect == CanvasStoreHistoryEffect::PushUndo {
            self.history.push_undo(committed.inverse().clone());
        }

        Some(self.finish_committed_mutation(committed, source, history_effect))
    }

    pub(crate) fn finish_committed_mutation(
        &mut self,
        committed: CanvasCommittedMutation,
        source: CanvasStoreMutationSource,
        history_effect: CanvasStoreHistoryEffect,
    ) -> CanvasStoreChange {
        self.sync_runtime_committed(&committed);
        let change = CanvasStoreChange {
            source,
            history_effect,
            committed,
            document: Arc::clone(&self.document),
            runtime: Arc::clone(&self.runtime),
        };
        self.emit_change(&change);
        change
    }

    fn sync_runtime_committed(&mut self, committed: &CanvasCommittedMutation) {
        let document = Arc::clone(&self.document);
        let edge_router = Arc::clone(&self.edge_router);
        let kind_registry = Arc::clone(&self.kind_registry);
        self.runtime_mut()
            .apply_committed_mutation_with_router_and_kind_registry(
                document.as_ref(),
                committed,
                edge_router.as_ref(),
                kind_registry.as_ref(),
            );
    }

    fn emit_change(&self, change: &CanvasStoreChange) {
        for listener in &self.listeners {
            (listener.callback)(change);
        }
    }

    fn document_mut(&mut self) -> &mut CanvasDocument {
        Arc::make_mut(&mut self.document)
    }

    fn runtime_mut(&mut self) -> &mut CanvasRuntime {
        Arc::make_mut(&mut self.runtime)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use open_gpui::{Bounds, point, px, size};

    use super::*;
    use crate::{
        CanvasNode, CanvasRecordId, CanvasRecordRelation, CanvasRelationChange, CanvasShape,
        DocumentCommand, NodeId, ShapeId, test_support::document_fixture,
    };

    #[test]
    fn store_rebuilds_runtime_from_initial_document() {
        let document = document_fixture()
            .node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .build();

        let store = CanvasStore::new(document);

        assert!(store.document().contains_node(&NodeId::from("a")));
        assert!(
            store
                .runtime()
                .hit_test(point(px(5.0), px(5.0)), crate::HitOptions::default())
                .any(|record| record.target == crate::HitTarget::Node(NodeId::from("a")))
        );
    }

    #[test]
    fn direct_transaction_updates_document_runtime_and_history() {
        let mut store = CanvasStore::default();

        let diff = store
            .apply_transaction(CanvasTransaction::single(DocumentCommand::InsertNode(
                CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0))),
            )))
            .unwrap();

        assert!(!diff.is_empty());
        assert!(store.document().contains_node(&NodeId::from("a")));
        assert_eq!(store.history().undo_depth(), 1);
        assert!(
            store
                .runtime()
                .hit_test(point(px(5.0), px(5.0)), crate::HitOptions::default())
                .any(|record| record.target == crate::HitTarget::Node(NodeId::from("a")))
        );
    }

    #[test]
    fn no_op_transaction_does_not_push_history_or_notify_listeners() {
        let mut store = CanvasStore::default();
        let changes = Arc::new(Mutex::new(Vec::new()));
        let observed = Arc::clone(&changes);
        store.listen(move |change| observed.lock().unwrap().push(change.clone()));

        let diff = store
            .apply_transaction(CanvasTransaction::single(DocumentCommand::RemoveNode(
                NodeId::from("missing"),
            )))
            .unwrap_err();
        assert_eq!(diff, DocumentError::MissingNode(NodeId::from("missing")));
        assert_eq!(store.history().undo_depth(), 0);
        assert!(changes.lock().unwrap().is_empty());

        let diff = store
            .apply_transaction(CanvasTransaction::default())
            .unwrap();
        assert!(diff.is_empty());
        assert_eq!(store.history().undo_depth(), 0);
        assert!(changes.lock().unwrap().is_empty());
    }

    #[test]
    fn listeners_receive_post_commit_change_facts() {
        let mut store = CanvasStore::default();
        let changes = Arc::new(Mutex::new(Vec::new()));
        let observed = Arc::clone(&changes);
        store.listen(move |change| observed.lock().unwrap().push(change.clone()));

        store
            .apply_transaction(CanvasTransaction::single(DocumentCommand::InsertNode(
                CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0))),
            )))
            .unwrap();

        let changes = changes.lock().unwrap();
        assert_eq!(changes.len(), 1);
        let change = &changes[0];
        assert_eq!(change.source(), CanvasStoreMutationSource::Local);
        assert_eq!(change.history_effect(), CanvasStoreHistoryEffect::PushUndo);
        assert!(change.document().contains_node(&NodeId::from("a")));
        assert_eq!(change.committed().record_changes().len(), 1);
    }

    #[test]
    fn listeners_receive_relation_only_change_facts() {
        let child = CanvasRecordId::Node(NodeId::from("child"));
        let frame = CanvasRecordId::Shape(ShapeId::from("frame"));
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
        let mut store = CanvasStore::new(document);
        let changes = Arc::new(Mutex::new(Vec::new()));
        let observed = Arc::clone(&changes);
        store.listen(move |change| observed.lock().unwrap().push(change.clone()));

        let diff = store
            .apply_transaction(CanvasTransaction::single(
                DocumentCommand::SetRecordParent {
                    child: child.clone(),
                    parent: frame.clone(),
                },
            ))
            .unwrap();

        assert!(diff.relations_changed);
        let changes = changes.lock().unwrap();
        assert_eq!(changes.len(), 1);
        let change = &changes[0];
        assert!(change.committed().record_changes().is_empty());
        assert_eq!(
            change.committed().relation_changes(),
            &[CanvasRelationChange::Upsert(CanvasRecordRelation::Parent(
                crate::relations::CanvasRecordParentRelation {
                    child: child.clone(),
                    parent: frame.clone(),
                },
            ))],
        );
        assert_eq!(
            change.document().relations().parent_of(&child),
            Some(&frame)
        );
    }

    #[test]
    fn store_changes_expose_relation_cleanup_for_deleted_records() {
        let child = CanvasRecordId::Node(NodeId::from("child"));
        let frame = CanvasRecordId::Shape(ShapeId::from("frame"));
        let mut document = document_fixture()
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
        document
            .apply_transaction(CanvasTransaction::single(
                DocumentCommand::SetRecordParent {
                    child: child.clone(),
                    parent: frame.clone(),
                },
            ))
            .unwrap();
        let mut store = CanvasStore::new(document);
        let changes = Arc::new(Mutex::new(Vec::new()));
        let observed = Arc::clone(&changes);
        store.listen(move |change| observed.lock().unwrap().push(change.clone()));

        store
            .apply_transaction(CanvasTransaction::single(DocumentCommand::RemoveNode(
                NodeId::from("child"),
            )))
            .unwrap();

        assert!(store.document().relations().is_empty());
        let changes = changes.lock().unwrap();
        assert_eq!(changes.len(), 1);
        assert!(
            changes[0]
                .committed()
                .relation_changes()
                .iter()
                .any(|change| matches!(
                    change,
                    CanvasRelationChange::Delete(CanvasRecordRelation::Parent(relation))
                        if relation.child == child && relation.parent == frame
                ))
        );
        drop(changes);

        assert!(store.undo().unwrap());
        assert_eq!(store.document().relations().parent_of(&child), Some(&frame));
    }

    #[test]
    fn failed_transactions_do_not_notify_listeners() {
        let mut store = CanvasStore::default();
        let changes = Arc::new(Mutex::new(Vec::new()));
        let observed = Arc::clone(&changes);
        store.listen(move |change| observed.lock().unwrap().push(change.clone()));

        let error = store
            .apply_transaction(CanvasTransaction::single(
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("missing")),
                    parent: CanvasRecordId::Shape(ShapeId::from("frame")),
                },
            ))
            .unwrap_err();

        assert_eq!(
            error,
            DocumentError::MissingRelationRecord(CanvasRecordId::Node(NodeId::from("missing")))
        );
        assert!(changes.lock().unwrap().is_empty());
        assert_eq!(store.history().undo_depth(), 0);
    }

    #[test]
    fn listeners_fire_in_registration_order() {
        let mut store = CanvasStore::default();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let first = Arc::clone(&calls);
        store.listen(move |_| first.lock().unwrap().push(1));
        let second = Arc::clone(&calls);
        store.listen(move |_| second.lock().unwrap().push(2));

        store
            .apply_transaction(CanvasTransaction::single(DocumentCommand::InsertNode(
                CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0))),
            )))
            .unwrap();

        assert_eq!(*calls.lock().unwrap(), vec![1, 2]);
    }
}
