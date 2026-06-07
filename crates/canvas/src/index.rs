use crate::{CanvasDocument, EdgeId, NodeId, ShapeId};
use open_gpui::{Bounds, Pixels, Point};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HitTarget {
    Node(NodeId),
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
    pub margin: Pixels,
}

impl Default for HitOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
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

    pub fn query(&self, viewport: Bounds<Pixels>) -> impl Iterator<Item = &HitRecord> {
        self.query_with_options(viewport, HitOptions::default())
    }

    pub fn query_with_options(
        &self,
        viewport: Bounds<Pixels>,
        options: HitOptions,
    ) -> impl Iterator<Item = &HitRecord> {
        self.records.iter().filter(move |record| {
            (options.include_hidden || !record.hidden) && record.bounds.intersects(&viewport)
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
    fn edge_bounds_are_indexed_from_endpoints() {
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
                && record.bounds.origin == point(px(10.0), px(10.0))
                && record.bounds.size.width == px(100.0)
                && record.bounds.size.height == px(0.0)
        }));
    }
}
