use crate::{
    CanvasEdge, CanvasNode, CanvasRecordId, CanvasShape, CanvasTransaction, DocumentCommand,
};
use serde::{Deserialize, Serialize};

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
        }
    }

    pub fn record_change(&self) -> CanvasRecordChange {
        match self {
            Self::InsertNode(node) | Self::UpdateNode(node) => {
                CanvasRecordChange::Upsert(CanvasRecord::Node(node.clone()))
            }
            Self::RemoveNode(id) => CanvasRecordChange::Delete(CanvasRecordId::Node(id.clone())),
            Self::InsertEdge(edge) | Self::UpdateEdge(edge) => {
                CanvasRecordChange::Upsert(CanvasRecord::Edge(edge.clone()))
            }
            Self::RemoveEdge(id) => CanvasRecordChange::Delete(CanvasRecordId::Edge(id.clone())),
            Self::InsertShape(shape) | Self::UpdateShape(shape) => {
                CanvasRecordChange::Upsert(CanvasRecord::Shape(shape.clone()))
            }
            Self::RemoveShape(id) => CanvasRecordChange::Delete(CanvasRecordId::Shape(id.clone())),
        }
    }
}

impl CanvasTransaction {
    pub fn record_changes(&self) -> impl Iterator<Item = CanvasRecordChange> + '_ {
        self.commands.iter().map(DocumentCommand::record_change)
    }

    pub fn record_ids(&self) -> impl Iterator<Item = CanvasRecordId> + '_ {
        self.commands.iter().map(DocumentCommand::record_id)
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
            CanvasRecordChange::Upsert(CanvasRecord::Node(node))
        );
        assert_eq!(
            DocumentCommand::InsertEdge(edge.clone()).record_change(),
            CanvasRecordChange::Upsert(CanvasRecord::Edge(edge))
        );
        assert_eq!(
            DocumentCommand::UpdateShape(shape.clone()).record_change(),
            CanvasRecordChange::Upsert(CanvasRecord::Shape(shape))
        );
        assert_eq!(
            DocumentCommand::RemoveNode(NodeId::from("node")).record_change(),
            CanvasRecordChange::Delete(CanvasRecordId::Node(NodeId::from("node")))
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
}
