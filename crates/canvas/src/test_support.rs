use crate::{
    CanvasDocument, CanvasDocumentBuilder, CanvasEdge, CanvasEdgeRoute, CanvasEndpoint,
    CanvasHandle, CanvasNode, CanvasShape, DocumentCommand, EdgeId, NodeId, ShapeId,
};
use open_gpui::{Bounds, Pixels, Point, point, px, size};

#[derive(Clone, Debug, Default)]
pub(crate) struct CanvasDocumentFixtureBuilder {
    builder: CanvasDocumentBuilder,
}

pub(crate) fn document_fixture() -> CanvasDocumentFixtureBuilder {
    CanvasDocumentFixtureBuilder::new()
}

impl CanvasDocumentFixtureBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn node(mut self, node: CanvasNode) -> Self {
        self.add_node(node);
        self
    }

    pub(crate) fn edge(mut self, edge: CanvasEdge) -> Self {
        self.add_edge(edge);
        self
    }

    pub(crate) fn add_node(&mut self, node: CanvasNode) -> &mut Self {
        self.builder.add_node(node).unwrap();
        self
    }

    pub(crate) fn add_edge(&mut self, edge: CanvasEdge) -> &mut Self {
        self.builder.add_edge(edge).unwrap();
        self
    }

    pub(crate) fn add_shape(&mut self, shape: CanvasShape) -> &mut Self {
        self.builder.add_shape(shape).unwrap();
        self
    }

    pub(crate) fn build(self) -> CanvasDocument {
        self.builder.build().unwrap()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TestRng {
    state: u64,
}

impl TestRng {
    pub(crate) fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub(crate) fn usize(&mut self, upper: usize) -> usize {
        assert!(upper > 0);
        (self.next_u32() as usize) % upper
    }

    pub(crate) fn bool(&mut self, one_in: usize) -> bool {
        self.usize(one_in) == 0
    }

    pub(crate) fn point(&mut self) -> Point<Pixels> {
        point(
            self.pixels_between(-500.0, 500.0),
            self.pixels_between(-500.0, 500.0),
        )
    }

    pub(crate) fn positive_pixels(&mut self, min: f32, max: f32) -> Pixels {
        self.pixels_between(min, max)
    }

    pub(crate) fn z_index(&mut self) -> i32 {
        self.usize(41) as i32 - 20
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    fn pixels_between(&mut self, min: f32, max: f32) -> Pixels {
        let unit = self.next_u32() as f32 / u32::MAX as f32;
        px(min + (max - min) * unit)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CanvasCommandGenerator {
    next_node: u64,
    next_edge: u64,
    next_shape: u64,
}

impl CanvasCommandGenerator {
    pub(crate) fn next_command(
        &mut self,
        document: &CanvasDocument,
        rng: &mut TestRng,
    ) -> DocumentCommand {
        for _ in 0..12 {
            if let Some(command) = self.try_command(document, rng.usize(9), rng) {
                return command;
            }
        }

        if document.node_count() < 2 || rng.bool(2) {
            self.insert_node(rng)
        } else {
            self.insert_shape(rng)
        }
    }

    fn try_command(
        &mut self,
        document: &CanvasDocument,
        choice: usize,
        rng: &mut TestRng,
    ) -> Option<DocumentCommand> {
        match choice {
            0 if document.node_count() < 32 => Some(self.insert_node(rng)),
            1 if document.node_count() != 0 => Some(self.update_node(document, rng)),
            2 if document.node_count() > 2 => Some(self.remove_node(document, rng)),
            3 if document.node_count() != 0 && document.edge_count() < 48 => {
                Some(self.insert_edge(document, rng))
            }
            4 if document.edge_count() != 0 => Some(self.update_edge(document, rng)),
            5 if document.edge_count() != 0 => Some(self.remove_edge(document, rng)),
            6 if document.shape_count() < 32 => Some(self.insert_shape(rng)),
            7 if document.shape_count() != 0 => Some(self.update_shape(document, rng)),
            8 if document.shape_count() != 0 => Some(self.remove_shape(document, rng)),
            _ => None,
        }
    }

    fn insert_node(&mut self, rng: &mut TestRng) -> DocumentCommand {
        let id = NodeId::new(format!("node-{}", self.next_node));
        self.next_node += 1;
        DocumentCommand::InsertNode(self.node_with_id(id, rng))
    }

    fn update_node(&mut self, document: &CanvasDocument, rng: &mut TestRng) -> DocumentCommand {
        let id = node_id_at(document, rng.usize(document.node_count()));
        DocumentCommand::UpdateNode(self.node_with_id(id, rng))
    }

    fn remove_node(&mut self, document: &CanvasDocument, rng: &mut TestRng) -> DocumentCommand {
        DocumentCommand::RemoveNode(node_id_at(document, rng.usize(document.node_count())))
    }

    fn insert_edge(&mut self, document: &CanvasDocument, rng: &mut TestRng) -> DocumentCommand {
        let id = EdgeId::new(format!("edge-{}", self.next_edge));
        self.next_edge += 1;
        DocumentCommand::InsertEdge(self.edge_with_id(id, document, rng))
    }

    fn update_edge(&mut self, document: &CanvasDocument, rng: &mut TestRng) -> DocumentCommand {
        let id = edge_id_at(document, rng.usize(document.edge_count()));
        DocumentCommand::UpdateEdge(self.edge_with_id(id, document, rng))
    }

    fn remove_edge(&mut self, document: &CanvasDocument, rng: &mut TestRng) -> DocumentCommand {
        DocumentCommand::RemoveEdge(edge_id_at(document, rng.usize(document.edge_count())))
    }

    fn insert_shape(&mut self, rng: &mut TestRng) -> DocumentCommand {
        let id = ShapeId::new(format!("shape-{}", self.next_shape));
        self.next_shape += 1;
        DocumentCommand::InsertShape(self.shape_with_id(id, rng))
    }

    fn update_shape(&mut self, document: &CanvasDocument, rng: &mut TestRng) -> DocumentCommand {
        let id = shape_id_at(document, rng.usize(document.shape_count()));
        DocumentCommand::UpdateShape(self.shape_with_id(id, rng))
    }

    fn remove_shape(&mut self, document: &CanvasDocument, rng: &mut TestRng) -> DocumentCommand {
        DocumentCommand::RemoveShape(shape_id_at(document, rng.usize(document.shape_count())))
    }

    fn node_with_id(&mut self, id: NodeId, rng: &mut TestRng) -> CanvasNode {
        let width = rng.positive_pixels(24.0, 160.0);
        let height = rng.positive_pixels(24.0, 120.0);
        let mut node = CanvasNode::new(id, rng.point(), size(width, height));
        node.z_index = rng.z_index();
        node.hidden = rng.bool(7);
        node.locked = rng.bool(11);

        if rng.bool(2) {
            let mut source = CanvasHandle::new("source", point(width, height * 0.5));
            source.role = crate::HandleRole::Source;
            source.connectable = !rng.bool(7);
            node.handles.push(source);
        }

        if rng.bool(2) {
            let mut target = CanvasHandle::new("target", point(Pixels::ZERO, height * 0.5));
            target.role = crate::HandleRole::Target;
            target.hidden = rng.bool(5);
            node.handles.push(target);
        }

        node
    }

    fn edge_with_id(
        &mut self,
        id: EdgeId,
        document: &CanvasDocument,
        rng: &mut TestRng,
    ) -> CanvasEdge {
        let source = node_id_at(document, rng.usize(document.node_count()));
        let target = node_id_at(document, rng.usize(document.node_count()));
        let mut edge = CanvasEdge::new(
            id,
            CanvasEndpoint::new(source, None::<crate::HandleId>),
            CanvasEndpoint::new(target, None::<crate::HandleId>),
        );
        edge.z_index = rng.z_index();
        edge.hidden = rng.bool(9);
        edge.locked = rng.bool(13);
        edge.route = match rng.usize(4) {
            0 => CanvasEdgeRoute::straight(),
            1 => CanvasEdgeRoute::polyline([rng.point()]),
            2 => {
                let mut route = CanvasEdgeRoute::new(crate::CanvasEdgeRouteKind::ORTHOGONAL);
                route.waypoints.push(rng.point());
                route.waypoints.push(rng.point());
                route
            }
            _ => {
                let mut route = CanvasEdgeRoute::new(crate::CanvasEdgeRouteKind::CUBIC_BEZIER);
                route.control_points.push(rng.point());
                route.control_points.push(rng.point());
                route
            }
        };
        edge.route.interaction_width = rng.positive_pixels(1.0, 32.0);
        edge.style.stroke_width = rng.positive_pixels(0.0, 8.0);
        edge
    }

    fn shape_with_id(&mut self, id: ShapeId, rng: &mut TestRng) -> CanvasShape {
        let mut shape = CanvasShape::new(
            id,
            Bounds::new(
                rng.point(),
                size(
                    rng.positive_pixels(8.0, 180.0),
                    rng.positive_pixels(8.0, 180.0),
                ),
            ),
        );
        shape.z_index = rng.z_index();
        shape.hidden = rng.bool(8);
        shape.locked = rng.bool(13);
        shape
    }
}

fn node_id_at(document: &CanvasDocument, index: usize) -> NodeId {
    document.node_ids().nth(index).unwrap().clone()
}

fn edge_id_at(document: &CanvasDocument, index: usize) -> EdgeId {
    document.edge_ids().nth(index).unwrap().clone()
}

fn shape_id_at(document: &CanvasDocument, index: usize) -> ShapeId {
    document.shape_ids().nth(index).unwrap().clone()
}
