use crate::mutation::CanvasMutationJournal;
use crate::{
    CanvasCommittedMutation, CanvasDocument, CanvasKindRegistry, CanvasRecordId, CanvasTransaction,
    DocumentCommand, DocumentError,
};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CanvasGestureSession {
    baseline: CanvasDocument,
}

impl CanvasGestureSession {
    pub(crate) fn begin(document: &CanvasDocument) -> Self {
        Self {
            baseline: document.clone(),
        }
    }

    pub(crate) fn prepare_commit_with_kind_registry(
        &self,
        current: &CanvasDocument,
        kind_registry: &CanvasKindRegistry,
    ) -> Result<Option<CanvasPreparedGestureCommit>, DocumentError> {
        let transaction = transaction_between(&self.baseline, current);
        if transaction.is_empty() {
            return Ok(None);
        }

        let prepared = CanvasMutationJournal::prepare_with_kind_registry(
            &self.baseline,
            transaction,
            kind_registry,
        )?;
        Ok(Some(CanvasPreparedGestureCommit {
            committed: prepared.committed().clone(),
        }))
    }

    pub(crate) fn cancel_transaction(&self, current: &CanvasDocument) -> CanvasTransaction {
        transaction_between(current, &self.baseline)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CanvasPreparedGestureCommit {
    committed: CanvasCommittedMutation,
}

impl CanvasPreparedGestureCommit {
    pub(crate) fn committed(&self) -> &CanvasCommittedMutation {
        &self.committed
    }
}

pub(crate) fn transaction_between(
    previous: &CanvasDocument,
    target: &CanvasDocument,
) -> CanvasTransaction {
    let mut commands = Vec::new();

    for id in previous.edge_ids() {
        if !target.contains_edge(id) {
            commands.push(DocumentCommand::RemoveEdge(id.clone()));
        }
    }

    for id in previous.shape_ids() {
        if !target.contains_shape(id) {
            commands.push(DocumentCommand::RemoveShape(id.clone()));
        }
    }

    for id in previous.node_ids() {
        if !target.contains_node(id) {
            commands.push(DocumentCommand::RemoveNode(id.clone()));
        }
    }

    for (id, node) in target.node_entries() {
        match previous.node(id) {
            None => commands.push(DocumentCommand::InsertNode(node.clone())),
            Some(previous_node) if previous_node != node => {
                commands.push(DocumentCommand::UpdateNode(node.clone()));
            }
            Some(_) => {}
        }
    }

    for (id, shape) in target.shape_entries() {
        match previous.shape(id) {
            None => commands.push(DocumentCommand::InsertShape(shape.clone())),
            Some(previous_shape) if previous_shape != shape => {
                commands.push(DocumentCommand::UpdateShape(shape.clone()));
            }
            Some(_) => {}
        }
    }

    for (id, edge) in target.edge_entries() {
        match previous.edge(id) {
            None => commands.push(DocumentCommand::InsertEdge(edge.clone())),
            Some(previous_edge) if previous_edge != edge => {
                commands.push(DocumentCommand::UpdateEdge(edge.clone()));
            }
            Some(_) => {}
        }
    }

    append_relation_commands(previous, target, &mut commands);

    CanvasTransaction::new(commands)
}

fn append_relation_commands(
    previous: &CanvasDocument,
    target: &CanvasDocument,
    commands: &mut Vec<DocumentCommand>,
) {
    for relation in previous.relations().parents() {
        if target.relations().parent_of(&relation.child) != Some(&relation.parent)
            && target.relations().parent_of(&relation.child).is_none()
            && document_contains_record(target, &relation.child)
        {
            commands.push(DocumentCommand::ClearRecordParent {
                child: relation.child.clone(),
            });
        }
    }

    for relation in target.relations().parents() {
        if previous.relations().parent_of(&relation.child) != Some(&relation.parent) {
            commands.push(DocumentCommand::SetRecordParent {
                child: relation.child.clone(),
                parent: relation.parent.clone(),
            });
        }
    }

    for relation in previous.relations().groups() {
        if !target.relations().contains_group_relation(relation)
            && document_contains_record(target, &relation.group)
            && document_contains_record(target, &relation.member)
        {
            commands.push(DocumentCommand::RemoveRecordFromGroup {
                group: relation.group.clone(),
                member: relation.member.clone(),
            });
        }
    }

    for relation in target.relations().groups() {
        if !previous.relations().contains_group_relation(relation) {
            commands.push(DocumentCommand::AddRecordToGroup {
                group: relation.group.clone(),
                member: relation.member.clone(),
            });
        }
    }
}

fn document_contains_record(document: &CanvasDocument, id: &CanvasRecordId) -> bool {
    match id {
        CanvasRecordId::Node(id) => document.contains_node(id),
        CanvasRecordId::Edge(id) => document.contains_edge(id),
        CanvasRecordId::Shape(id) => document.contains_shape(id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CanvasEdge, CanvasEndpoint, CanvasNode, CanvasRecordId, CanvasShape, EdgeId, NodeId,
    };
    use open_gpui::{Bounds, point, px, size};

    #[test]
    fn gesture_commit_coalesces_transient_updates() {
        let mut baseline = CanvasDocument::default();
        let original = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        baseline.insert_node(original.clone()).unwrap();
        let session = CanvasGestureSession::begin(&baseline);

        let mut current = baseline.clone();
        let first = CanvasNode::new("a", point(px(12.0), px(0.0)), size(px(10.0), px(10.0)));
        let second = CanvasNode::new("a", point(px(40.0), px(0.0)), size(px(10.0), px(10.0)));
        current
            .apply_transaction(CanvasTransaction::single(DocumentCommand::UpdateNode(
                first,
            )))
            .unwrap();
        current
            .apply_transaction(CanvasTransaction::single(DocumentCommand::UpdateNode(
                second.clone(),
            )))
            .unwrap();

        let commit = session
            .prepare_commit_with_kind_registry(&current, &CanvasKindRegistry::open())
            .unwrap()
            .unwrap();

        assert_eq!(
            commit.committed().transaction().commands,
            vec![DocumentCommand::UpdateNode(second)]
        );
        assert_eq!(
            commit.committed().inverse().commands,
            vec![DocumentCommand::UpdateNode(original)]
        );
    }

    #[test]
    fn gesture_cancel_transaction_restores_baseline() {
        let mut baseline = connected_document();
        let session = CanvasGestureSession::begin(&baseline);
        let mut moved = baseline.node(&NodeId::from("a")).unwrap().clone();
        moved.position = point(px(40.0), px(0.0));
        baseline
            .apply_transaction(CanvasTransaction::single(DocumentCommand::UpdateNode(
                moved,
            )))
            .unwrap();

        let cancel = session.cancel_transaction(&baseline);
        baseline.apply_transaction(cancel).unwrap();

        assert_eq!(baseline, session.baseline);
    }

    #[test]
    fn transaction_between_updates_edge_after_node_changes() {
        let previous = connected_document();
        let mut target = previous.clone();
        let mut source = target.node(&NodeId::from("a")).unwrap().clone();
        source.position = point(px(100.0), px(0.0));
        target.update_node(source).unwrap();
        let changed_edge = CanvasEdge::new(
            "a-b",
            CanvasEndpoint::new("b", None::<&str>),
            CanvasEndpoint::new("a", None::<&str>),
        );
        target.update_edge(changed_edge.clone()).unwrap();

        let transaction = transaction_between(&previous, &target);
        let mut replayed = previous;
        replayed.apply_transaction(transaction).unwrap();

        assert_eq!(replayed, target);
        assert_eq!(replayed.edge(&EdgeId::from("a-b")).unwrap(), &changed_edge);
    }

    #[test]
    fn transaction_between_preserves_relation_updates() {
        let previous = related_document();
        let mut target = previous.clone();

        target
            .apply_transaction(CanvasTransaction::new([
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("child")),
                    parent: CanvasRecordId::Shape("frame-b".into()),
                },
                DocumentCommand::RemoveRecordFromGroup {
                    group: CanvasRecordId::Shape("group-a".into()),
                    member: CanvasRecordId::Node(NodeId::from("member")),
                },
                DocumentCommand::AddRecordToGroup {
                    group: CanvasRecordId::Shape("group-b".into()),
                    member: CanvasRecordId::Node(NodeId::from("member")),
                },
            ]))
            .unwrap();

        let transaction = transaction_between(&previous, &target);
        assert_eq!(
            transaction.commands,
            vec![
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("child")),
                    parent: CanvasRecordId::Shape("frame-b".into()),
                },
                DocumentCommand::RemoveRecordFromGroup {
                    group: CanvasRecordId::Shape("group-a".into()),
                    member: CanvasRecordId::Node(NodeId::from("member")),
                },
                DocumentCommand::AddRecordToGroup {
                    group: CanvasRecordId::Shape("group-b".into()),
                    member: CanvasRecordId::Node(NodeId::from("member")),
                },
            ]
        );

        let mut replayed = previous;
        replayed.apply_transaction(transaction).unwrap();

        assert_eq!(replayed, target);
    }

    #[test]
    fn transaction_between_preserves_parent_removal() {
        let previous = related_document();
        let mut target = previous.clone();
        target
            .apply_transaction(CanvasTransaction::single(
                DocumentCommand::ClearRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("child")),
                },
            ))
            .unwrap();

        let transaction = transaction_between(&previous, &target);
        assert_eq!(
            transaction.commands,
            vec![DocumentCommand::ClearRecordParent {
                child: CanvasRecordId::Node(NodeId::from("child")),
            }]
        );

        let mut replayed = previous;
        replayed.apply_transaction(transaction).unwrap();

        assert_eq!(replayed, target);
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

    fn related_document() -> CanvasDocument {
        let mut document = CanvasDocument::default();
        for id in ["child", "member"] {
            document
                .insert_node(CanvasNode::new(
                    id,
                    point(px(0.0), px(0.0)),
                    size(px(10.0), px(10.0)),
                ))
                .unwrap();
        }
        for id in ["frame-a", "frame-b", "group-a", "group-b"] {
            document
                .insert_shape(CanvasShape::new(
                    id,
                    Bounds::new(point(px(0.0), px(0.0)), size(px(10.0), px(10.0))),
                ))
                .unwrap();
        }
        document
            .apply_transaction(CanvasTransaction::new([
                DocumentCommand::SetRecordParent {
                    child: CanvasRecordId::Node(NodeId::from("child")),
                    parent: CanvasRecordId::Shape("frame-a".into()),
                },
                DocumentCommand::AddRecordToGroup {
                    group: CanvasRecordId::Shape("group-a".into()),
                    member: CanvasRecordId::Node(NodeId::from("member")),
                },
            ]))
            .unwrap();
        document
    }
}
