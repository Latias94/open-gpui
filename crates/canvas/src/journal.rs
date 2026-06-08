use crate::{
    CanvasDocument, CanvasDocumentDiff, CanvasRecord, CanvasRecordChange, CanvasRecordId,
    CanvasRecordOperationBatch, CanvasTransaction, DocumentError,
};

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasCommittedMutation {
    transaction: CanvasTransaction,
    inverse: CanvasTransaction,
    diff: CanvasDocumentDiff,
    record_changes: Vec<CanvasRecordChange>,
}

impl CanvasCommittedMutation {
    pub fn transaction(&self) -> &CanvasTransaction {
        &self.transaction
    }

    pub fn inverse(&self) -> &CanvasTransaction {
        &self.inverse
    }

    pub fn diff(&self) -> &CanvasDocumentDiff {
        &self.diff
    }

    pub fn record_changes(&self) -> &[CanvasRecordChange] {
        &self.record_changes
    }

    pub fn record_operation_batch(&self, transaction_sequence: u64) -> CanvasRecordOperationBatch {
        CanvasRecordOperationBatch::from_record_changes(
            transaction_sequence,
            self.transaction.metadata.clone(),
            self.record_changes.clone(),
        )
    }

    pub fn into_diff(self) -> CanvasDocumentDiff {
        self.diff
    }

    pub fn into_inverse(self) -> CanvasTransaction {
        self.inverse
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CanvasPreparedMutation {
    document: CanvasDocument,
    committed: CanvasCommittedMutation,
}

impl CanvasPreparedMutation {
    pub fn committed(&self) -> &CanvasCommittedMutation {
        &self.committed
    }

    pub fn apply_to(self, document: &mut CanvasDocument) -> CanvasCommittedMutation {
        *document = self.document;
        self.committed
    }
}

pub(crate) struct CanvasMutationJournal;

impl CanvasMutationJournal {
    pub fn prepare(
        document: &CanvasDocument,
        transaction: CanvasTransaction,
    ) -> Result<CanvasPreparedMutation, DocumentError> {
        let inverse = document.invert_transaction(&transaction)?;
        let mut draft = document.clone();

        for command in transaction.commands.iter().cloned() {
            draft.apply(command)?;
        }

        let diff = draft.diff_against(document);
        let record_changes = record_changes_from_diff(&draft, &diff);
        let committed = CanvasCommittedMutation {
            transaction,
            inverse,
            diff,
            record_changes,
        };

        Ok(CanvasPreparedMutation {
            document: draft,
            committed,
        })
    }

    pub fn commit(
        document: &mut CanvasDocument,
        transaction: CanvasTransaction,
    ) -> Result<CanvasCommittedMutation, DocumentError> {
        let prepared = Self::prepare(document, transaction)?;
        Ok(prepared.apply_to(document))
    }
}

fn record_changes_from_diff(
    document: &CanvasDocument,
    diff: &CanvasDocumentDiff,
) -> Vec<CanvasRecordChange> {
    let mut changes =
        Vec::with_capacity(diff.inserted.len() + diff.updated.len() + diff.removed.len());

    changes.extend(
        diff.inserted
            .iter()
            .filter_map(|id| record_from_document(document, id))
            .map(CanvasRecordChange::Upsert),
    );
    changes.extend(
        diff.updated
            .iter()
            .filter_map(|id| record_from_document(document, id))
            .map(CanvasRecordChange::Upsert),
    );
    changes.extend(diff.removed.iter().cloned().map(CanvasRecordChange::Delete));

    changes
}

fn record_from_document(document: &CanvasDocument, id: &CanvasRecordId) -> Option<CanvasRecord> {
    match id {
        CanvasRecordId::Node(id) => document.nodes.get(id).cloned().map(CanvasRecord::Node),
        CanvasRecordId::Edge(id) => document.edges.get(id).cloned().map(CanvasRecord::Edge),
        CanvasRecordId::Shape(id) => document.shapes.get(id).cloned().map(CanvasRecord::Shape),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanvasEdge, CanvasEndpoint, CanvasNode, DocumentCommand, EdgeId, NodeId};
    use open_gpui::{point, px, size};

    #[test]
    fn committed_mutation_reports_actual_incident_edge_deletes() {
        let mut document = connected_document();
        let committed = document
            .commit_transaction(CanvasTransaction::single(DocumentCommand::RemoveNode(
                NodeId::from("a"),
            )))
            .unwrap();

        assert_eq!(
            committed.record_changes(),
            &[
                CanvasRecordChange::Delete(CanvasRecordId::Node(NodeId::from("a"))),
                CanvasRecordChange::Delete(CanvasRecordId::Edge(EdgeId::from("a-b"))),
            ]
        );
        assert_eq!(
            committed
                .record_operation_batch(9)
                .operations
                .iter()
                .map(|operation| (
                    operation.transaction_sequence,
                    operation.operation_index,
                    operation.id()
                ))
                .collect::<Vec<_>>(),
            vec![
                (9, 0, CanvasRecordId::Node(NodeId::from("a"))),
                (9, 1, CanvasRecordId::Edge(EdgeId::from("a-b"))),
            ]
        );
    }

    #[test]
    fn failed_transaction_leaves_document_unchanged() {
        let mut document = CanvasDocument::default();
        let before = document.clone();
        let transaction = CanvasTransaction::new([
            DocumentCommand::InsertNode(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            )),
            DocumentCommand::InsertEdge(CanvasEdge::new(
                "bad",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("missing", None::<&str>),
            )),
        ]);

        let err = document.commit_transaction(transaction).unwrap_err();

        assert_eq!(err, DocumentError::MissingNode(NodeId::from("missing")));
        assert_eq!(document, before);
    }

    #[test]
    fn committed_mutation_preserves_transaction_metadata() {
        let mut document = CanvasDocument::default();
        let mut transaction = CanvasTransaction::single(DocumentCommand::InsertNode(
            CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0))),
        ));
        transaction
            .metadata
            .insert("origin".into(), serde_json::json!("test"));

        let committed = document.commit_transaction(transaction).unwrap();
        let batch = committed.record_operation_batch(3);

        assert_eq!(
            batch.transaction_metadata.get("origin"),
            Some(&serde_json::json!("test"))
        );
    }

    #[test]
    fn committed_inverse_restores_previous_document() {
        let mut document = connected_document();
        let before = document.clone();
        let committed = document
            .commit_transaction(CanvasTransaction::single(DocumentCommand::RemoveNode(
                NodeId::from("a"),
            )))
            .unwrap();

        assert_ne!(document, before);

        document
            .commit_transaction(committed.inverse().clone())
            .unwrap();
        assert_eq!(document, before);
    }

    fn connected_document() -> CanvasDocument {
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
}
