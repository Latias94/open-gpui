use crate::{
    CanvasDocument, CanvasDocumentDiff, CanvasKindRegistry, CanvasRecord, CanvasRecordChange,
    CanvasRecordId, CanvasRecordOperationBatch, CanvasRecordRelation, CanvasRelationChange,
    CanvasRelationOperationBatch, CanvasTransaction, DocumentCommand, DocumentError,
};

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasCommittedMutation {
    transaction: CanvasTransaction,
    inverse: CanvasTransaction,
    diff: CanvasDocumentDiff,
    record_changes: Vec<CanvasRecordChange>,
    relation_changes: Vec<CanvasRelationChange>,
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

    pub fn relation_changes(&self) -> &[CanvasRelationChange] {
        &self.relation_changes
    }

    pub fn record_operation_batch(&self, transaction_sequence: u64) -> CanvasRecordOperationBatch {
        CanvasRecordOperationBatch::from_record_changes(
            transaction_sequence,
            self.transaction.metadata.clone(),
            self.record_changes.clone(),
        )
    }

    pub fn relation_operation_batch(
        &self,
        transaction_sequence: u64,
    ) -> CanvasRelationOperationBatch {
        CanvasRelationOperationBatch::from_relation_changes(
            transaction_sequence,
            self.transaction.metadata.clone(),
            self.relation_changes.clone(),
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
pub struct CanvasPreparedMutation {
    document: CanvasDocument,
    committed: CanvasCommittedMutation,
}

impl CanvasPreparedMutation {
    pub fn committed(&self) -> &CanvasCommittedMutation {
        &self.committed
    }

    pub(crate) fn apply_to(self, document: &mut CanvasDocument) -> CanvasCommittedMutation {
        *document = self.document;
        self.committed
    }
}

pub(crate) struct CanvasMutationJournal;

impl CanvasMutationJournal {
    pub fn prepare_with_kind_registry(
        document: &CanvasDocument,
        transaction: CanvasTransaction,
        kind_registry: &CanvasKindRegistry,
    ) -> Result<CanvasPreparedMutation, DocumentError> {
        let transaction = kind_registry.normalize_transaction(transaction)?;
        let mut inverse = document.invert_transaction(&transaction)?;
        let mut draft = document.clone();

        for command in transaction.commands.iter().cloned() {
            draft.apply(command)?;
        }
        draft.prune_missing_relations();
        draft.validate_relations()?;
        kind_registry.validate_document(&draft)?;
        complete_inverse_relation_commands(document, &draft, &mut inverse)?;

        let diff = draft.diff_against(document);
        let record_changes = record_changes_from_diff(&draft, &diff);
        let relation_changes = relation_changes_from_diff(&draft, document);
        let committed = CanvasCommittedMutation {
            transaction,
            inverse,
            diff,
            record_changes,
            relation_changes,
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
        Self::commit_with_kind_registry(document, transaction, &CanvasKindRegistry::open())
    }

    pub fn commit_with_kind_registry(
        document: &mut CanvasDocument,
        transaction: CanvasTransaction,
        kind_registry: &CanvasKindRegistry,
    ) -> Result<CanvasCommittedMutation, DocumentError> {
        let prepared = Self::prepare_with_kind_registry(document, transaction, kind_registry)?;
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

fn relation_changes_from_diff(
    document: &CanvasDocument,
    previous: &CanvasDocument,
) -> Vec<CanvasRelationChange> {
    let current = document.relations();
    let previous = previous.relations();
    let mut changes = Vec::new();

    for relation in previous.parents() {
        if current.parent_of(&relation.child) != Some(&relation.parent) {
            changes.push(CanvasRelationChange::Delete(CanvasRecordRelation::Parent(
                relation.clone(),
            )));
        }
    }

    for relation in current.parents() {
        if previous.parent_of(&relation.child) != Some(&relation.parent) {
            changes.push(CanvasRelationChange::Upsert(CanvasRecordRelation::Parent(
                relation.clone(),
            )));
        }
    }

    for relation in previous.groups() {
        if !current.contains_group_relation(relation) {
            changes.push(CanvasRelationChange::Delete(CanvasRecordRelation::Group(
                relation.clone(),
            )));
        }
    }

    for relation in current.groups() {
        if !previous.contains_group_relation(relation) {
            changes.push(CanvasRelationChange::Upsert(CanvasRecordRelation::Group(
                relation.clone(),
            )));
        }
    }

    changes
}

fn complete_inverse_relation_commands(
    previous: &CanvasDocument,
    current: &CanvasDocument,
    inverse: &mut CanvasTransaction,
) -> Result<(), DocumentError> {
    let mut restored = current.clone();
    for command in inverse.commands.iter().cloned() {
        restored.apply(command)?;
    }
    restored.prune_missing_relations();
    restored.validate_relations()?;

    let commands = relation_changes_from_diff(previous, &restored)
        .into_iter()
        .map(relation_change_to_command)
        .collect::<Vec<_>>();
    if commands.is_empty() {
        return Ok(());
    }

    for command in commands.iter().cloned() {
        restored.apply(command)?;
    }
    restored.prune_missing_relations();
    restored.validate_relations()?;
    inverse.commands.extend(commands);
    Ok(())
}

fn relation_change_to_command(change: CanvasRelationChange) -> DocumentCommand {
    match change {
        CanvasRelationChange::Upsert(CanvasRecordRelation::Parent(relation)) => {
            DocumentCommand::SetRecordParent {
                child: relation.child,
                parent: relation.parent,
            }
        }
        CanvasRelationChange::Delete(CanvasRecordRelation::Parent(relation)) => {
            DocumentCommand::ClearRecordParent {
                child: relation.child,
            }
        }
        CanvasRelationChange::Upsert(CanvasRecordRelation::Group(relation)) => {
            DocumentCommand::AddRecordToGroup {
                group: relation.group,
                member: relation.member,
            }
        }
        CanvasRelationChange::Delete(CanvasRecordRelation::Group(relation)) => {
            DocumentCommand::RemoveRecordFromGroup {
                group: relation.group,
                member: relation.member,
            }
        }
    }
}

fn record_from_document(document: &CanvasDocument, id: &CanvasRecordId) -> Option<CanvasRecord> {
    match id {
        CanvasRecordId::Node(id) => document.node(id).cloned().map(CanvasRecord::Node),
        CanvasRecordId::Edge(id) => document.edge(id).cloned().map(CanvasRecord::Edge),
        CanvasRecordId::Shape(id) => document.shape(id).cloned().map(CanvasRecord::Shape),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CanvasEdge, CanvasEndpoint, CanvasNode, CanvasRecordGroupRelation, CanvasRecordId,
        CanvasRecordParentRelation, DocumentCommand, EdgeId, NodeId, ShapeId,
    };
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
    fn committed_mutation_prunes_relations_for_actual_deleted_records() {
        let mut document = connected_document();
        document
            .apply_transaction(CanvasTransaction::single(
                DocumentCommand::AddRecordToGroup {
                    group: CanvasRecordId::Node(NodeId::from("b")),
                    member: CanvasRecordId::Edge(EdgeId::from("a-b")),
                },
            ))
            .unwrap();

        let committed = document
            .commit_transaction(CanvasTransaction::single(DocumentCommand::RemoveNode(
                NodeId::from("a"),
            )))
            .unwrap();

        assert!(committed.diff().relations_changed);
        assert!(document.relations().is_empty());
        assert_eq!(
            committed.record_changes(),
            &[
                CanvasRecordChange::Delete(CanvasRecordId::Node(NodeId::from("a"))),
                CanvasRecordChange::Delete(CanvasRecordId::Edge(EdgeId::from("a-b"))),
            ]
        );
        assert_eq!(
            committed.relation_changes(),
            &[CanvasRelationChange::Delete(CanvasRecordRelation::Group(
                CanvasRecordGroupRelation::new(NodeId::from("b"), EdgeId::from("a-b"))
            ))]
        );
    }

    #[test]
    fn committed_mutation_reports_relation_only_changes() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "child",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();
        document
            .insert_shape(crate::CanvasShape::new(
                "frame",
                open_gpui::Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            ))
            .unwrap();
        let mut transaction = CanvasTransaction::new([
            DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Node(NodeId::from("child")),
                parent: CanvasRecordId::Shape(ShapeId::from("frame")),
            },
            DocumentCommand::AddRecordToGroup {
                group: CanvasRecordId::Shape(ShapeId::from("frame")),
                member: CanvasRecordId::Node(NodeId::from("child")),
            },
        ]);
        transaction
            .metadata
            .insert("origin".into(), serde_json::json!("test"));

        let committed = document.commit_transaction(transaction).unwrap();

        assert!(committed.diff().relations_changed);
        assert!(committed.record_changes().is_empty());
        assert_eq!(
            committed.relation_changes(),
            &[
                CanvasRelationChange::Upsert(CanvasRecordRelation::Parent(
                    CanvasRecordParentRelation::new(NodeId::from("child"), ShapeId::from("frame"))
                )),
                CanvasRelationChange::Upsert(CanvasRecordRelation::Group(
                    CanvasRecordGroupRelation::new(ShapeId::from("frame"), NodeId::from("child"))
                )),
            ]
        );

        let batch = committed.relation_operation_batch(7);
        assert_eq!(batch.transaction_sequence, 7);
        assert_eq!(
            batch.transaction_metadata.get("origin"),
            Some(&serde_json::json!("test"))
        );
        assert_eq!(
            batch
                .operations
                .iter()
                .map(|operation| operation.operation_index)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn committed_mutation_reports_parent_replacement_as_delete_and_upsert() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "child",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();
        for id in ["old-frame", "new-frame"] {
            document
                .insert_shape(crate::CanvasShape::new(
                    id,
                    open_gpui::Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
                ))
                .unwrap();
        }
        document
            .apply_transaction(CanvasTransaction::single(
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("child")),
                    parent: CanvasRecordId::Shape(ShapeId::from("old-frame")),
                },
            ))
            .unwrap();

        let committed = document
            .commit_transaction(CanvasTransaction::single(
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("child")),
                    parent: CanvasRecordId::Shape(ShapeId::from("new-frame")),
                },
            ))
            .unwrap();

        assert_eq!(
            committed.relation_changes(),
            &[
                CanvasRelationChange::Delete(CanvasRecordRelation::Parent(
                    CanvasRecordParentRelation::new(
                        NodeId::from("child"),
                        ShapeId::from("old-frame")
                    )
                )),
                CanvasRelationChange::Upsert(CanvasRecordRelation::Parent(
                    CanvasRecordParentRelation::new(
                        NodeId::from("child"),
                        ShapeId::from("new-frame")
                    )
                )),
            ]
        );
    }

    #[test]
    fn committed_mutation_omits_noop_relation_changes() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "child",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();
        document
            .insert_shape(crate::CanvasShape::new(
                "frame",
                open_gpui::Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
            ))
            .unwrap();
        let relation_transaction = CanvasTransaction::new([
            DocumentCommand::SetRecordParent {
                child: CanvasRecordId::Node(NodeId::from("child")),
                parent: CanvasRecordId::Shape(ShapeId::from("frame")),
            },
            DocumentCommand::AddRecordToGroup {
                group: CanvasRecordId::Shape(ShapeId::from("frame")),
                member: CanvasRecordId::Node(NodeId::from("child")),
            },
        ]);
        document
            .commit_transaction(relation_transaction.clone())
            .unwrap();

        let committed = document.commit_transaction(relation_transaction).unwrap();

        assert!(committed.diff().is_empty());
        assert!(committed.record_changes().is_empty());
        assert!(committed.relation_changes().is_empty());
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

    #[test]
    fn committed_inverse_restores_relations_pruned_by_deleted_records() {
        let mut document = connected_document();
        let edge = CanvasRecordId::Edge(EdgeId::from("a-b"));
        let group = CanvasRecordId::Node(NodeId::from("b"));
        document
            .commit_transaction(CanvasTransaction::new([
                DocumentCommand::SetRecordParent {
                    child: edge.clone(),
                    parent: group.clone(),
                },
                DocumentCommand::AddRecordToGroup {
                    group: group.clone(),
                    member: edge.clone(),
                },
            ]))
            .unwrap();
        let before = document.clone();

        let committed = document
            .commit_transaction(CanvasTransaction::single(DocumentCommand::RemoveNode(
                NodeId::from("a"),
            )))
            .unwrap();

        assert!(document.relations().is_empty());

        document
            .commit_transaction(committed.inverse().clone())
            .unwrap();
        assert_eq!(document, before);
        assert_eq!(document.relations().parent_of(&edge), Some(&group));
        assert_eq!(
            document
                .relations()
                .members_of(&group)
                .cloned()
                .collect::<Vec<_>>(),
            vec![edge]
        );
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
