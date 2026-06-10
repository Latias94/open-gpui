use crate::{
    CanvasEdge, CanvasNode, CanvasRecordGroupRelation, CanvasRecordId, CanvasRecordParentRelation,
    CanvasShape, CanvasTransaction, CanvasValue, DocumentCommand,
};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CanvasRecord {
    Node(CanvasNode),
    Edge(CanvasEdge),
    Shape(CanvasShape),
}

impl CanvasRecord {
    pub fn id(&self) -> CanvasRecordId {
        match self {
            Self::Node(node) => CanvasRecordId::Node(node.id.clone()),
            Self::Edge(edge) => CanvasRecordId::Edge(edge.id.clone()),
            Self::Shape(shape) => CanvasRecordId::Shape(shape.id.clone()),
        }
    }
}

impl From<CanvasNode> for CanvasRecord {
    fn from(value: CanvasNode) -> Self {
        Self::Node(value)
    }
}

impl From<CanvasEdge> for CanvasRecord {
    fn from(value: CanvasEdge) -> Self {
        Self::Edge(value)
    }
}

impl From<CanvasShape> for CanvasRecord {
    fn from(value: CanvasShape) -> Self {
        Self::Shape(value)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CanvasRecordChange {
    Upsert(CanvasRecord),
    Delete(CanvasRecordId),
}

impl CanvasRecordChange {
    pub fn id(&self) -> CanvasRecordId {
        match self {
            Self::Upsert(record) => record.id(),
            Self::Delete(id) => id.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CanvasRecordRelation {
    Parent(CanvasRecordParentRelation),
    Group(CanvasRecordGroupRelation),
}

impl CanvasRecordRelation {
    pub fn relation_id(&self) -> (&CanvasRecordId, &CanvasRecordId) {
        match self {
            Self::Parent(relation) => (&relation.child, &relation.parent),
            Self::Group(relation) => (&relation.group, &relation.member),
        }
    }
}

impl From<CanvasRecordParentRelation> for CanvasRecordRelation {
    fn from(value: CanvasRecordParentRelation) -> Self {
        Self::Parent(value)
    }
}

impl From<CanvasRecordGroupRelation> for CanvasRecordRelation {
    fn from(value: CanvasRecordGroupRelation) -> Self {
        Self::Group(value)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CanvasRelationChange {
    Upsert(CanvasRecordRelation),
    Delete(CanvasRecordRelation),
}

impl CanvasRelationChange {
    pub fn relation(&self) -> &CanvasRecordRelation {
        match self {
            Self::Upsert(relation) | Self::Delete(relation) => relation,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CanvasChangeOrigin(String);

impl CanvasChangeOrigin {
    pub fn new(origin: impl Into<String>) -> Self {
        Self(origin.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for CanvasChangeOrigin {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for CanvasChangeOrigin {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for CanvasChangeOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasRecordOperation {
    pub transaction_sequence: u64,
    pub operation_index: u64,
    pub change: CanvasRecordChange,
}

impl CanvasRecordOperation {
    pub fn new(
        transaction_sequence: u64,
        operation_index: u64,
        change: CanvasRecordChange,
    ) -> Self {
        Self {
            transaction_sequence,
            operation_index,
            change,
        }
    }

    pub fn id(&self) -> CanvasRecordId {
        self.change.id()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CanvasRecordOperationBatch {
    pub transaction_sequence: u64,
    #[serde(default)]
    pub origin: Option<CanvasChangeOrigin>,
    #[serde(default)]
    pub transaction_metadata: CanvasValue,
    #[serde(default)]
    pub operations: Vec<CanvasRecordOperation>,
}

impl CanvasRecordOperationBatch {
    pub fn new(transaction_sequence: u64, transaction: &CanvasTransaction) -> Self {
        Self::from_record_changes(
            transaction_sequence,
            transaction.metadata.clone(),
            transaction.record_changes(),
        )
    }

    pub fn from_record_changes(
        transaction_sequence: u64,
        transaction_metadata: CanvasValue,
        changes: impl IntoIterator<Item = CanvasRecordChange>,
    ) -> Self {
        Self {
            transaction_sequence,
            origin: None,
            transaction_metadata,
            operations: changes
                .into_iter()
                .enumerate()
                .map(|(index, change)| {
                    CanvasRecordOperation::new(transaction_sequence, index as u64, change)
                })
                .collect(),
        }
    }

    pub fn with_origin(mut self, origin: impl Into<CanvasChangeOrigin>) -> Self {
        self.origin = Some(origin.into());
        self
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    pub fn changes(&self) -> impl Iterator<Item = &CanvasRecordChange> {
        self.operations.iter().map(|operation| &operation.change)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasRelationOperation {
    pub transaction_sequence: u64,
    pub operation_index: u64,
    pub change: CanvasRelationChange,
}

impl CanvasRelationOperation {
    pub fn new(
        transaction_sequence: u64,
        operation_index: u64,
        change: CanvasRelationChange,
    ) -> Self {
        Self {
            transaction_sequence,
            operation_index,
            change,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CanvasRelationOperationBatch {
    pub transaction_sequence: u64,
    #[serde(default)]
    pub origin: Option<CanvasChangeOrigin>,
    #[serde(default)]
    pub transaction_metadata: CanvasValue,
    #[serde(default)]
    pub operations: Vec<CanvasRelationOperation>,
}

impl CanvasRelationOperationBatch {
    pub fn from_relation_changes(
        transaction_sequence: u64,
        transaction_metadata: CanvasValue,
        changes: impl IntoIterator<Item = CanvasRelationChange>,
    ) -> Self {
        Self {
            transaction_sequence,
            origin: None,
            transaction_metadata,
            operations: changes
                .into_iter()
                .enumerate()
                .map(|(index, change)| {
                    CanvasRelationOperation::new(transaction_sequence, index as u64, change)
                })
                .collect(),
        }
    }

    pub fn with_origin(mut self, origin: impl Into<CanvasChangeOrigin>) -> Self {
        self.origin = Some(origin.into());
        self
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    pub fn changes(&self) -> impl Iterator<Item = &CanvasRelationChange> {
        self.operations.iter().map(|operation| &operation.change)
    }
}

impl DocumentCommand {
    pub fn record_id(&self) -> CanvasRecordId {
        match self {
            Self::InsertNode(node) | Self::UpdateNode(node) => {
                CanvasRecordId::Node(node.id.clone())
            }
            Self::RemoveNode(id) => CanvasRecordId::Node(id.clone()),
            Self::InsertEdge(edge) | Self::UpdateEdge(edge) => {
                CanvasRecordId::Edge(edge.id.clone())
            }
            Self::RemoveEdge(id) => CanvasRecordId::Edge(id.clone()),
            Self::InsertShape(shape) | Self::UpdateShape(shape) => {
                CanvasRecordId::Shape(shape.id.clone())
            }
            Self::RemoveShape(id) => CanvasRecordId::Shape(id.clone()),
            Self::SetRecordParent { child, .. } | Self::ClearRecordParent { child } => {
                child.clone()
            }
            Self::AddRecordToGroup { group, .. } | Self::RemoveRecordFromGroup { group, .. } => {
                group.clone()
            }
        }
    }

    pub fn record_change(&self) -> Option<CanvasRecordChange> {
        match self {
            Self::InsertNode(node) | Self::UpdateNode(node) => {
                Some(CanvasRecordChange::Upsert(CanvasRecord::Node(node.clone())))
            }
            Self::RemoveNode(id) => {
                Some(CanvasRecordChange::Delete(CanvasRecordId::Node(id.clone())))
            }
            Self::InsertEdge(edge) | Self::UpdateEdge(edge) => {
                Some(CanvasRecordChange::Upsert(CanvasRecord::Edge(edge.clone())))
            }
            Self::RemoveEdge(id) => {
                Some(CanvasRecordChange::Delete(CanvasRecordId::Edge(id.clone())))
            }
            Self::InsertShape(shape) | Self::UpdateShape(shape) => Some(
                CanvasRecordChange::Upsert(CanvasRecord::Shape(shape.clone())),
            ),
            Self::RemoveShape(id) => Some(CanvasRecordChange::Delete(CanvasRecordId::Shape(
                id.clone(),
            ))),
            Self::SetRecordParent { .. }
            | Self::ClearRecordParent { .. }
            | Self::AddRecordToGroup { .. }
            | Self::RemoveRecordFromGroup { .. } => None,
        }
    }
}

impl CanvasTransaction {
    pub fn record_changes(&self) -> impl Iterator<Item = CanvasRecordChange> + '_ {
        self.commands
            .iter()
            .filter_map(DocumentCommand::record_change)
    }

    pub fn record_ids(&self) -> impl Iterator<Item = CanvasRecordId> + '_ {
        self.commands.iter().map(DocumentCommand::record_id)
    }

    pub fn record_operations(
        &self,
        transaction_sequence: u64,
    ) -> impl Iterator<Item = CanvasRecordOperation> + '_ {
        self.record_changes()
            .enumerate()
            .map(move |(index, change)| {
                CanvasRecordOperation::new(transaction_sequence, index as u64, change)
            })
    }

    pub fn record_operation_batch(&self, transaction_sequence: u64) -> CanvasRecordOperationBatch {
        CanvasRecordOperationBatch::new(transaction_sequence, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanvasEdge, CanvasEndpoint, EdgeId, NodeId, ShapeId};
    use open_gpui::{Bounds, point, px, size};

    #[test]
    fn document_commands_expose_record_ids() {
        assert_eq!(
            DocumentCommand::InsertNode(CanvasNode::new(
                "node",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .record_id(),
            CanvasRecordId::Node(NodeId::from("node"))
        );
        assert_eq!(
            DocumentCommand::RemoveEdge(EdgeId::from("edge")).record_id(),
            CanvasRecordId::Edge(EdgeId::from("edge"))
        );
        assert_eq!(
            DocumentCommand::RemoveShape(ShapeId::from("shape")).record_id(),
            CanvasRecordId::Shape(ShapeId::from("shape"))
        );
    }

    #[test]
    fn document_commands_expose_record_changes() {
        let node = CanvasNode::new("node", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        let edge = CanvasEdge::new(
            "edge",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        );
        let shape = CanvasShape::new(
            "shape",
            Bounds::new(point(px(0.0), px(0.0)), size(px(10.0), px(10.0))),
        );

        assert_eq!(
            DocumentCommand::UpdateNode(node.clone()).record_change(),
            Some(CanvasRecordChange::Upsert(CanvasRecord::Node(node)))
        );
        assert_eq!(
            DocumentCommand::InsertEdge(edge.clone()).record_change(),
            Some(CanvasRecordChange::Upsert(CanvasRecord::Edge(edge)))
        );
        assert_eq!(
            DocumentCommand::UpdateShape(shape.clone()).record_change(),
            Some(CanvasRecordChange::Upsert(CanvasRecord::Shape(shape)))
        );
        assert_eq!(
            DocumentCommand::RemoveNode(NodeId::from("node")).record_change(),
            Some(CanvasRecordChange::Delete(CanvasRecordId::Node(
                NodeId::from("node")
            )))
        );
    }

    #[test]
    fn transactions_preserve_record_change_order() {
        let transaction = CanvasTransaction::new([
            DocumentCommand::InsertNode(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            )),
            DocumentCommand::RemoveShape(ShapeId::from("old-shape")),
        ]);

        assert_eq!(
            transaction.record_ids().collect::<Vec<_>>(),
            vec![
                CanvasRecordId::Node(NodeId::from("a")),
                CanvasRecordId::Shape(ShapeId::from("old-shape")),
            ]
        );
        assert_eq!(
            transaction.record_changes().collect::<Vec<_>>(),
            vec![
                CanvasRecordChange::Upsert(CanvasRecord::Node(CanvasNode::new(
                    "a",
                    point(px(0.0), px(0.0)),
                    size(px(10.0), px(10.0)),
                ))),
                CanvasRecordChange::Delete(CanvasRecordId::Shape(ShapeId::from("old-shape"))),
            ]
        );
    }

    #[test]
    fn transactions_expose_record_operation_batches() {
        let mut transaction = CanvasTransaction::new([
            DocumentCommand::InsertNode(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            )),
            DocumentCommand::RemoveShape(ShapeId::from("old-shape")),
        ]);
        transaction
            .metadata
            .insert("reason".into(), serde_json::json!("sync"));

        let batch = transaction
            .record_operation_batch(42)
            .with_origin("client-a");

        assert_eq!(batch.transaction_sequence, 42);
        assert_eq!(
            batch.origin.as_ref().map(CanvasChangeOrigin::as_str),
            Some("client-a")
        );
        assert_eq!(
            batch.transaction_metadata.get("reason"),
            Some(&serde_json::json!("sync"))
        );
        assert_eq!(
            batch
                .operations
                .iter()
                .map(|operation| (operation.transaction_sequence, operation.operation_index))
                .collect::<Vec<_>>(),
            vec![(42, 0), (42, 1)]
        );
        assert_eq!(
            batch.changes().cloned().collect::<Vec<_>>(),
            transaction.record_changes().collect::<Vec<_>>()
        );
        assert!(!batch.is_empty());
    }

    #[test]
    fn empty_transaction_operation_batch_is_empty() {
        let batch = CanvasTransaction::default().record_operation_batch(7);

        assert_eq!(batch.transaction_sequence, 7);
        assert!(batch.is_empty());
        assert!(batch.transaction_metadata.is_empty());
    }

    #[test]
    fn relation_operation_batches_preserve_order_and_metadata() {
        let mut metadata = CanvasValue::new();
        metadata.insert("reason".into(), serde_json::json!("sync"));
        let changes = vec![
            CanvasRelationChange::Upsert(CanvasRecordRelation::Parent(
                CanvasRecordParentRelation::new(NodeId::from("child"), ShapeId::from("frame")),
            )),
            CanvasRelationChange::Upsert(CanvasRecordRelation::Group(
                CanvasRecordGroupRelation::new(ShapeId::from("frame"), NodeId::from("child")),
            )),
        ];

        let batch = CanvasRelationOperationBatch::from_relation_changes(
            42,
            metadata.clone(),
            changes.clone(),
        )
        .with_origin("client-a");

        assert_eq!(batch.transaction_sequence, 42);
        assert_eq!(
            batch.origin.as_ref().map(CanvasChangeOrigin::as_str),
            Some("client-a")
        );
        assert_eq!(batch.transaction_metadata, metadata);
        assert_eq!(
            batch
                .operations
                .iter()
                .map(|operation| (operation.transaction_sequence, operation.operation_index))
                .collect::<Vec<_>>(),
            vec![(42, 0), (42, 1)]
        );
        assert_eq!(batch.changes().cloned().collect::<Vec<_>>(), changes);
    }
}
