use crate::{
    CanvasDocument, CanvasDocumentDiff, CanvasRecordId, EdgeId, HandleId, NodeId, ShapeId,
};
use open_gpui::{Bounds, Pixels, Point};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HitTarget {
    Node(NodeId),
    Handle {
        node_id: NodeId,
        handle_id: HandleId,
    },
    Shape(ShapeId),
    Edge(EdgeId),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HitRecord {
    pub target: HitTarget,
    pub bounds: Bounds<Pixels>,
    pub z_index: i32,
    pub hidden: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HitOptions {
    pub include_hidden: bool,
    pub include_handles: bool,
    pub margin: Pixels,
}

impl Default for HitOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
            include_handles: false,
            margin: Pixels::ZERO,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpatialIndex {
    records: Vec<HitRecord>,
}

impl SpatialIndex {
    pub fn rebuild(document: &CanvasDocument) -> Self {
        let mut records = Vec::new();

        for node in document.nodes.values() {
            records.push(HitRecord {
                target: HitTarget::Node(node.id.clone()),
                bounds: node.bounds(),
                z_index: node.z_index,
                hidden: node.hidden,
            });

            for handle in &node.handles {
                records.push(HitRecord {
                    target: HitTarget::Handle {
                        node_id: node.id.clone(),
                        handle_id: handle.id.clone(),
                    },
                    bounds: handle.bounds_in_document(node),
                    z_index: node.z_index,
                    hidden: node.hidden || !handle.connectable,
                });
            }
        }

        for shape in document.shapes.values() {
            records.push(HitRecord {
                target: HitTarget::Shape(shape.id.clone()),
                bounds: shape.bounds,
                z_index: shape.z_index,
                hidden: shape.hidden,
            });
        }

        for edge in document.edges.values() {
            if let Ok(bounds) = document.edge_bounds(edge) {
                records.push(HitRecord {
                    target: HitTarget::Edge(edge.id.clone()),
                    bounds,
                    z_index: edge.z_index,
                    hidden: edge.hidden,
                });
            }
        }

        records.sort_by(|a, b| a.z_index.cmp(&b.z_index));
        Self { records }
    }

    pub fn apply_diff(&mut self, document: &CanvasDocument, diff: &CanvasDocumentDiff) {
        if diff.is_empty() {
            return;
        }

        for record_id in &diff.removed {
            self.remove_record(record_id);
        }

        for record_id in diff.updated.iter().chain(&diff.inserted) {
            self.refresh_record(document, record_id);
        }

        self.records.sort_by(|a, b| a.z_index.cmp(&b.z_index));
    }

    pub fn query(&self, viewport: Bounds<Pixels>) -> impl Iterator<Item = &HitRecord> {
        self.query_with_options(viewport, HitOptions::default())
    }

    pub fn query_with_options(
        &self,
        viewport: Bounds<Pixels>,
        options: HitOptions,
    ) -> impl Iterator<Item = &HitRecord> {
        self.records.iter().filter(move |record| {
            (options.include_hidden || !record.hidden)
                && (options.include_handles || !matches!(record.target, HitTarget::Handle { .. }))
                && record.bounds.intersects(&viewport)
        })
    }

    pub fn hit_test(
        &self,
        point: Point<Pixels>,
        options: HitOptions,
    ) -> impl Iterator<Item = &HitRecord> {
        self.records
            .iter()
            .rev()
            .filter(move |record| options.include_hidden || !record.hidden)
            .filter(move |record| {
                options.include_handles || !matches!(record.target, HitTarget::Handle { .. })
            })
            .filter(move |record| {
                let bounds = if options.margin == Pixels::ZERO {
                    record.bounds
                } else {
                    record.bounds.dilate(options.margin)
                };
                bounds.contains(&point)
            })
    }

    pub fn records(&self) -> &[HitRecord] {
        &self.records
    }

    fn refresh_record(&mut self, document: &CanvasDocument, record_id: &CanvasRecordId) {
        self.remove_record(record_id);

        match record_id {
            CanvasRecordId::Node(id) => {
                let Some(node) = document.nodes.get(id) else {
                    return;
                };

                self.records.push(HitRecord {
                    target: HitTarget::Node(node.id.clone()),
                    bounds: node.bounds(),
                    z_index: node.z_index,
                    hidden: node.hidden,
                });

                for handle in &node.handles {
                    self.records.push(HitRecord {
                        target: HitTarget::Handle {
                            node_id: node.id.clone(),
                            handle_id: handle.id.clone(),
                        },
                        bounds: handle.bounds_in_document(node),
                        z_index: node.z_index,
                        hidden: node.hidden || !handle.connectable,
                    });
                }

                for edge in document
                    .edges
                    .values()
                    .filter(|edge| edge.source.node_id == *id || edge.target.node_id == *id)
                {
                    self.refresh_record(document, &CanvasRecordId::Edge(edge.id.clone()));
                }
            }
            CanvasRecordId::Edge(id) => {
                let Some(edge) = document.edges.get(id) else {
                    return;
                };

                if let Ok(bounds) = document.edge_bounds(edge) {
                    self.records.push(HitRecord {
                        target: HitTarget::Edge(edge.id.clone()),
                        bounds,
                        z_index: edge.z_index,
                        hidden: edge.hidden,
                    });
                }
            }
            CanvasRecordId::Shape(id) => {
                let Some(shape) = document.shapes.get(id) else {
                    return;
                };

                self.records.push(HitRecord {
                    target: HitTarget::Shape(shape.id.clone()),
                    bounds: shape.bounds,
                    z_index: shape.z_index,
                    hidden: shape.hidden,
                });
            }
        }
    }

    fn remove_record(&mut self, record_id: &CanvasRecordId) {
        self.records
            .retain(|record| match (record_id, &record.target) {
                (CanvasRecordId::Node(id), HitTarget::Node(target_id)) => target_id != id,
                (CanvasRecordId::Node(id), HitTarget::Handle { node_id, .. }) => node_id != id,
                (CanvasRecordId::Node(_), _) => true,
                (CanvasRecordId::Edge(id), HitTarget::Edge(target_id)) => target_id != id,
                (CanvasRecordId::Edge(_), _) => true,
                (CanvasRecordId::Shape(id), HitTarget::Shape(target_id)) => target_id != id,
                (CanvasRecordId::Shape(_), _) => true,
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanvasDocument, CanvasNode, CanvasShape};
    use open_gpui::{Bounds, point, px, size};

    #[test]
    fn hit_test_returns_topmost_first() {
        let mut document = CanvasDocument::default();
        let mut back = CanvasNode::new("back", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        back.z_index = 1;
        let mut front = CanvasShape::new(
            "front",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        );
        front.z_index = 2;
        document.insert_node(back).unwrap();
        document.insert_shape(front).unwrap();

        let index = SpatialIndex::rebuild(&document);
        let hits = index
            .hit_test(point(px(50.0), px(50.0)), HitOptions::default())
            .map(|record| record.target.clone())
            .collect::<Vec<_>>();

        assert_eq!(
            hits,
            vec![
                HitTarget::Shape(ShapeId::from("front")),
                HitTarget::Node(NodeId::from("back"))
            ]
        );
    }

    #[test]
    fn query_culls_outside_records() {
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "inside",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();
        document
            .insert_node(CanvasNode::new(
                "outside",
                point(px(100.0), px(100.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();

        let index = SpatialIndex::rebuild(&document);
        let visible = index
            .query(Bounds::new(
                point(px(0.0), px(0.0)),
                size(px(50.0), px(50.0)),
            ))
            .map(|record| record.target.clone())
            .collect::<Vec<_>>();

        assert_eq!(visible, vec![HitTarget::Node(NodeId::from("inside"))]);
    }

    #[test]
    fn hidden_records_are_only_returned_when_requested() {
        let mut document = CanvasDocument::default();
        let mut node = CanvasNode::new("hidden", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        node.hidden = true;
        document.insert_node(node).unwrap();

        let index = SpatialIndex::rebuild(&document);
        assert!(
            index
                .hit_test(point(px(5.0), px(5.0)), HitOptions::default())
                .next()
                .is_none()
        );

        let options = HitOptions {
            include_hidden: true,
            ..HitOptions::default()
        };
        assert_eq!(
            index
                .hit_test(point(px(5.0), px(5.0)), options)
                .map(|record| record.target.clone())
                .collect::<Vec<_>>(),
            vec![HitTarget::Node(NodeId::from("hidden"))]
        );
    }

    #[test]
    fn edge_bounds_are_indexed_from_route_hit_area() {
        use crate::{CanvasEdge, CanvasEndpoint};

        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(20.0), px(20.0)),
            ))
            .unwrap();
        document
            .insert_node(CanvasNode::new(
                "b",
                point(px(100.0), px(0.0)),
                size(px(20.0), px(20.0)),
            ))
            .unwrap();
        document
            .insert_edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .unwrap();

        let index = SpatialIndex::rebuild(&document);
        assert!(index.records().iter().any(|record| {
            record.target == HitTarget::Edge(EdgeId::from("a-b"))
                && record.bounds.origin == point(px(4.0), px(4.0))
                && record.bounds.size.width == px(112.0)
                && record.bounds.size.height == px(12.0)
        }));
    }

    #[test]
    fn handles_are_hit_only_when_requested() {
        use crate::CanvasHandle;

        let mut document = CanvasDocument::default();
        let mut node = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        node.handles
            .push(CanvasHandle::new("out", point(px(95.0), px(50.0))));
        document.insert_node(node).unwrap();

        let index = SpatialIndex::rebuild(&document);
        let point = point(px(95.0), px(50.0));
        assert_eq!(
            index
                .hit_test(point, HitOptions::default())
                .map(|record| record.target.clone())
                .collect::<Vec<_>>(),
            vec![HitTarget::Node(NodeId::from("a"))]
        );

        let options = HitOptions {
            include_handles: true,
            ..HitOptions::default()
        };
        assert_eq!(
            index
                .hit_test(point, options)
                .map(|record| record.target.clone())
                .collect::<Vec<_>>(),
            vec![
                HitTarget::Handle {
                    node_id: NodeId::from("a"),
                    handle_id: HandleId::from("out"),
                },
                HitTarget::Node(NodeId::from("a")),
            ]
        );
    }

    #[test]
    fn applies_diff_for_inserted_records() {
        let previous = CanvasDocument::default();
        let mut document = CanvasDocument::default();
        document
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .unwrap();

        let mut index = SpatialIndex::rebuild(&previous);
        let diff = document.diff_against(&previous);
        index.apply_diff(&document, &diff);

        assert_eq!(
            index
                .hit_test(point(px(5.0), px(5.0)), HitOptions::default())
                .map(|record| record.target.clone())
                .collect::<Vec<_>>(),
            vec![HitTarget::Node(NodeId::from("a"))]
        );
    }

    #[test]
    fn applies_diff_for_moved_node_and_incident_edge() {
        use crate::{CanvasEdge, CanvasEndpoint};

        let mut previous = CanvasDocument::default();
        previous
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(20.0), px(20.0)),
            ))
            .unwrap();
        previous
            .insert_node(CanvasNode::new(
                "b",
                point(px(100.0), px(0.0)),
                size(px(20.0), px(20.0)),
            ))
            .unwrap();
        previous
            .insert_edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .unwrap();

        let mut document = previous.clone();
        let mut node = document.nodes.get(&NodeId::from("a")).unwrap().clone();
        node.position = point(px(40.0), px(0.0));
        document.update_node(node).unwrap();

        let mut index = SpatialIndex::rebuild(&previous);
        let diff = document.diff_against(&previous);
        index.apply_diff(&document, &diff);

        assert!(index.records().iter().any(|record| {
            record.target == HitTarget::Edge(EdgeId::from("a-b"))
                && record.bounds.origin == point(px(44.0), px(4.0))
                && record.bounds.size.width == px(72.0)
                && record.bounds.size.height == px(12.0)
        }));
    }

    #[test]
    fn applies_diff_for_updated_edge_route() {
        use crate::{CanvasEdge, CanvasEdgeRoute, CanvasEndpoint};

        let mut previous = CanvasDocument::default();
        previous
            .insert_node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(20.0), px(20.0)),
            ))
            .unwrap();
        previous
            .insert_node(CanvasNode::new(
                "b",
                point(px(100.0), px(0.0)),
                size(px(20.0), px(20.0)),
            ))
            .unwrap();
        previous
            .insert_edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", None::<&str>),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .unwrap();

        let mut document = previous.clone();
        let mut edge = document.edges.get(&EdgeId::from("a-b")).unwrap().clone();
        edge.route = CanvasEdgeRoute::polyline([point(px(60.0), px(80.0))]);
        edge.route.interaction_width = px(20.0);
        document.update_edge(edge).unwrap();

        let mut index = SpatialIndex::rebuild(&previous);
        let diff = document.diff_against(&previous);
        index.apply_diff(&document, &diff);

        assert!(index.records().iter().any(|record| {
            record.target == HitTarget::Edge(EdgeId::from("a-b"))
                && record.bounds.origin == point(px(0.0), px(0.0))
                && record.bounds.size == size(px(120.0), px(90.0))
        }));
    }
}
