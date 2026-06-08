use crate::{
    CanvasConnectionEndpointRole, CanvasDefaultEdgeRouter, CanvasDocument, CanvasEdge,
    CanvasEdgeRouter, CanvasEndpoint, CanvasHandle, CanvasKindRegistry, CanvasNode,
    CanvasRoutePath, CanvasRouteRequest, CanvasRouteSegment, CanvasShape, DocumentError,
    HitOptions, HitRecord, HitTarget,
};
use open_gpui::{Bounds, Pixels, Point, px};

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasResolvedEdgeGeometry {
    pub path: CanvasRoutePath,
    pub bounds: Bounds<Pixels>,
}

#[derive(Clone, Copy, Debug)]
pub struct CanvasGeometryResolver<'a, R = CanvasDefaultEdgeRouter> {
    document: &'a CanvasDocument,
    router: R,
    kind_registry: Option<&'a CanvasKindRegistry>,
}

impl<'a> CanvasGeometryResolver<'a> {
    pub fn new(document: &'a CanvasDocument) -> Self {
        Self::with_router(document, CanvasDefaultEdgeRouter)
    }

    pub fn with_kind_registry(
        document: &'a CanvasDocument,
        kind_registry: &'a CanvasKindRegistry,
    ) -> Self {
        Self::with_router_and_kind_registry(document, CanvasDefaultEdgeRouter, Some(kind_registry))
    }
}

impl<'a, R> CanvasGeometryResolver<'a, R>
where
    R: CanvasEdgeRouter,
{
    pub fn with_router(document: &'a CanvasDocument, router: R) -> Self {
        Self::with_router_and_kind_registry(document, router, None)
    }

    pub fn with_router_and_kind_registry(
        document: &'a CanvasDocument,
        router: R,
        kind_registry: Option<&'a CanvasKindRegistry>,
    ) -> Self {
        Self {
            document,
            router,
            kind_registry,
        }
    }

    pub fn document(&self) -> &'a CanvasDocument {
        self.document
    }

    pub fn kind_registry(&self) -> Option<&'a CanvasKindRegistry> {
        self.kind_registry
    }

    pub fn node_bounds(&self, node: &CanvasNode) -> Bounds<Pixels> {
        self.kind_registry
            .and_then(|registry| registry.node_bounds(node))
            .unwrap_or_else(|| node.bounds())
    }

    pub fn shape_bounds(&self, shape: &CanvasShape) -> Bounds<Pixels> {
        self.kind_registry
            .and_then(|registry| registry.shape_bounds(shape))
            .unwrap_or(shape.bounds)
    }

    pub fn handle_bounds(&self, node: &CanvasNode, handle: &CanvasHandle) -> Bounds<Pixels> {
        Bounds::centered_at(self.resolved_handle_position(node, handle), handle.size)
    }

    pub fn endpoint_position(
        &self,
        endpoint: &CanvasEndpoint,
    ) -> Result<Point<Pixels>, DocumentError> {
        let node = self
            .document
            .nodes
            .get(&endpoint.node_id)
            .ok_or_else(|| DocumentError::MissingNode(endpoint.node_id.clone()))?;

        if let Some(handle_id) = &endpoint.handle_id {
            let handle =
                node.handle(Some(handle_id))
                    .ok_or_else(|| DocumentError::MissingHandle {
                        node_id: endpoint.node_id.clone(),
                        handle_id: handle_id.clone(),
                    })?;
            return Ok(self.resolved_handle_position(node, handle));
        }

        Ok(self.node_bounds(node).center())
    }

    pub fn edge_route_path(&self, edge: &CanvasEdge) -> Result<CanvasRoutePath, DocumentError> {
        let source = self.endpoint_position(&edge.source)?;
        let target = self.endpoint_position(&edge.target)?;
        Ok(self.router.route_edge(CanvasRouteRequest {
            edge,
            source,
            target,
        }))
    }

    pub fn edge_geometry(
        &self,
        edge: &CanvasEdge,
    ) -> Result<CanvasResolvedEdgeGeometry, DocumentError> {
        let path = self.edge_route_path(edge)?;
        let bounds = match path.bounds() {
            Some(bounds) => bounds,
            None => {
                let source = self.endpoint_position(&edge.source)?;
                let target = self.endpoint_position(&edge.target)?;
                Bounds::from_corners(
                    Point::new(source.x.min(target.x), source.y.min(target.y)),
                    Point::new(source.x.max(target.x), source.y.max(target.y)),
                )
            }
        };

        Ok(CanvasResolvedEdgeGeometry {
            path,
            bounds: bounds.dilate(edge_interaction_radius(edge)),
        })
    }

    pub fn edge_bounds(&self, edge: &CanvasEdge) -> Result<Bounds<Pixels>, DocumentError> {
        Ok(self.edge_geometry(edge)?.bounds)
    }

    pub fn record_contains_point(
        &self,
        record: &HitRecord,
        point: Point<Pixels>,
        options: HitOptions,
    ) -> bool {
        self.record_contains_point_with_edge_geometry(record, point, options, None)
    }

    pub(crate) fn record_contains_point_with_edge_geometry(
        &self,
        record: &HitRecord,
        point: Point<Pixels>,
        options: HitOptions,
        edge_geometry: Option<&CanvasResolvedEdgeGeometry>,
    ) -> bool {
        if !record_options_match(record, options) {
            return false;
        }

        let bounds = if options.margin == Pixels::ZERO {
            record.bounds
        } else {
            record.bounds.dilate(options.margin)
        };
        if !bounds.contains(&point) {
            return false;
        }

        match &record.target {
            HitTarget::Node(id) => {
                let Some(node) = self.document.nodes.get(id) else {
                    return false;
                };
                self.kind_registry
                    .and_then(|registry| {
                        registry.node_contains_point(node, point, record.bounds, options.margin)
                    })
                    .unwrap_or(true)
            }
            HitTarget::Handle { .. } => true,
            HitTarget::Shape(id) => {
                let Some(shape) = self.document.shapes.get(id) else {
                    return false;
                };
                self.kind_registry
                    .and_then(|registry| {
                        registry.shape_contains_point(shape, point, record.bounds, options.margin)
                    })
                    .unwrap_or(true)
            }
            HitTarget::Edge(id) => {
                let Some(edge) = self.document.edges.get(id) else {
                    return false;
                };
                if let Some(edge_geometry) = edge_geometry {
                    return edge_geometry_contains_point(
                        edge,
                        edge_geometry,
                        point,
                        options.margin,
                    );
                }

                self.edge_contains_point(edge, point, options.margin)
                    .unwrap_or(false)
            }
        }
    }

    pub fn connection_endpoint_at<'h>(
        &self,
        records: impl IntoIterator<Item = &'h HitRecord>,
        role: CanvasConnectionEndpointRole,
    ) -> Option<CanvasEndpoint> {
        for record in records {
            match &record.target {
                HitTarget::Handle { node_id, handle_id } => {
                    let node = self.document.nodes.get(node_id)?;
                    let handle = node.handle(Some(handle_id))?;
                    return handle
                        .is_pickable_connection_endpoint(role)
                        .then(|| CanvasEndpoint {
                            node_id: node_id.clone(),
                            handle_id: Some(handle_id.clone()),
                        });
                }
                HitTarget::Node(node_id) => {
                    return Some(CanvasEndpoint {
                        node_id: node_id.clone(),
                        handle_id: None,
                    });
                }
                HitTarget::Edge(_) | HitTarget::Shape(_) => {}
            }
        }

        None
    }

    pub fn connection_preview_target<'h>(
        &self,
        records: impl IntoIterator<Item = &'h HitRecord>,
        source: Point<Pixels>,
        _current: Point<Pixels>,
    ) -> Option<Point<Pixels>> {
        let target = self.connection_endpoint_at(records, CanvasConnectionEndpointRole::Target)?;
        let target_position = self.endpoint_position(&target).ok()?;
        (target_position != source).then_some(target_position)
    }

    fn resolved_handle_position(&self, node: &CanvasNode, handle: &CanvasHandle) -> Point<Pixels> {
        self.kind_registry
            .and_then(|registry| registry.handle_position(node, &handle.id))
            .unwrap_or(node.position + handle.position)
    }

    fn edge_contains_point(
        &self,
        edge: &CanvasEdge,
        point: Point<Pixels>,
        margin: Pixels,
    ) -> Result<bool, DocumentError> {
        let geometry = self.edge_geometry(edge)?;
        Ok(route_contains_point(
            &geometry.path,
            point,
            edge_interaction_radius(edge) + margin,
        ))
    }
}

fn edge_geometry_contains_point(
    edge: &CanvasEdge,
    geometry: &CanvasResolvedEdgeGeometry,
    point: Point<Pixels>,
    margin: Pixels,
) -> bool {
    route_contains_point(
        &geometry.path,
        point,
        edge_interaction_radius(edge) + margin,
    )
}

fn edge_interaction_radius(edge: &CanvasEdge) -> Pixels {
    let stroke_width =
        if edge.style.stroke_width.as_f32().is_finite() && edge.style.stroke_width > Pixels::ZERO {
            edge.style.stroke_width
        } else {
            Pixels::ZERO
        };
    let interaction_width = if edge.route.interaction_width > stroke_width {
        edge.route.interaction_width
    } else {
        stroke_width
    };

    interaction_width * 0.5
}

fn route_contains_point(path: &CanvasRoutePath, point: Point<Pixels>, radius: Pixels) -> bool {
    let radius = radius.as_f32().max(0.0);
    let radius_squared = radius * radius;
    path.segments
        .iter()
        .any(|segment| segment_distance_squared(segment, point) <= radius_squared)
}

fn segment_distance_squared(segment: &CanvasRouteSegment, point: Point<Pixels>) -> f32 {
    match segment {
        CanvasRouteSegment::Line { from, to } => {
            point_to_line_segment_distance_squared(point, *from, *to)
        }
        CanvasRouteSegment::CubicBezier {
            from,
            control_1,
            control_2,
            to,
        } => cubic_bezier_distance_squared(point, *from, *control_1, *control_2, *to),
    }
}

fn cubic_bezier_distance_squared(
    point: Point<Pixels>,
    from: Point<Pixels>,
    control_1: Point<Pixels>,
    control_2: Point<Pixels>,
    to: Point<Pixels>,
) -> f32 {
    const STEPS: usize = 24;

    let mut closest = f32::INFINITY;
    let mut previous = from;
    for step in 1..=STEPS {
        let t = step as f32 / STEPS as f32;
        let current = cubic_bezier_point(from, control_1, control_2, to, t);
        closest = closest.min(point_to_line_segment_distance_squared(
            point, previous, current,
        ));
        previous = current;
    }
    closest
}

fn cubic_bezier_point(
    from: Point<Pixels>,
    control_1: Point<Pixels>,
    control_2: Point<Pixels>,
    to: Point<Pixels>,
    t: f32,
) -> Point<Pixels> {
    let mt = 1.0 - t;
    Point::new(
        px(mt * mt * mt * from.x.as_f32()
            + 3.0 * mt * mt * t * control_1.x.as_f32()
            + 3.0 * mt * t * t * control_2.x.as_f32()
            + t * t * t * to.x.as_f32()),
        px(mt * mt * mt * from.y.as_f32()
            + 3.0 * mt * mt * t * control_1.y.as_f32()
            + 3.0 * mt * t * t * control_2.y.as_f32()
            + t * t * t * to.y.as_f32()),
    )
}

fn point_to_line_segment_distance_squared(
    point: Point<Pixels>,
    from: Point<Pixels>,
    to: Point<Pixels>,
) -> f32 {
    let px = point.x.as_f32();
    let py = point.y.as_f32();
    let ax = from.x.as_f32();
    let ay = from.y.as_f32();
    let bx = to.x.as_f32();
    let by = to.y.as_f32();
    let dx = bx - ax;
    let dy = by - ay;
    let length_squared = dx * dx + dy * dy;

    if length_squared <= f32::EPSILON {
        let dx = px - ax;
        let dy = py - ay;
        return dx * dx + dy * dy;
    }

    let t = (((px - ax) * dx + (py - ay) * dy) / length_squared).clamp(0.0, 1.0);
    let nearest_x = ax + t * dx;
    let nearest_y = ay + t * dy;
    let dx = px - nearest_x;
    let dy = py - nearest_y;
    dx * dx + dy * dy
}

fn record_options_match(record: &HitRecord, options: HitOptions) -> bool {
    (options.include_hidden || !record.hidden)
        && (options.include_locked || !record.locked)
        && (options.include_handles || !matches!(record.target, HitTarget::Handle { .. }))
}

pub fn connection_hit_options() -> HitOptions {
    HitOptions {
        include_handles: true,
        ..HitOptions::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanvasHandle, CanvasNode, HandleRole};
    use open_gpui::{point, px, size};

    #[test]
    fn resolver_uses_same_endpoint_position_for_handles_and_node_centers() {
        let mut document = CanvasDocument::default();
        let mut node = CanvasNode::new("a", point(px(10.0), px(20.0)), size(px(40.0), px(60.0)));
        node.handles
            .push(CanvasHandle::new("out", point(px(40.0), px(30.0))));
        document.insert_node(node).unwrap();
        let resolver = CanvasGeometryResolver::new(&document);

        assert_eq!(
            resolver
                .endpoint_position(&CanvasEndpoint::new("a", None::<&str>))
                .unwrap(),
            point(px(30.0), px(50.0))
        );
        assert_eq!(
            resolver
                .endpoint_position(&CanvasEndpoint::new("a", Some("out")))
                .unwrap(),
            point(px(50.0), px(50.0))
        );
    }

    #[test]
    fn resolver_picks_connection_handles_by_role() {
        let mut document = CanvasDocument::default();
        let mut node = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        let mut target_only = CanvasHandle::new("in", point(px(100.0), px(50.0)));
        target_only.role = HandleRole::Target;
        node.handles.push(target_only);
        document.insert_node(node).unwrap();
        let resolver = CanvasGeometryResolver::new(&document);
        let records = [HitRecord {
            target: HitTarget::Handle {
                node_id: "a".into(),
                handle_id: "in".into(),
            },
            bounds: Bounds::centered_at(point(px(100.0), px(50.0)), size(px(12.0), px(12.0))),
            z_index: 0,
            hidden: false,
            locked: false,
        }];

        assert_eq!(
            resolver.connection_endpoint_at(&records, CanvasConnectionEndpointRole::Target),
            Some(CanvasEndpoint::new("a", Some("in")))
        );
        assert_eq!(
            resolver.connection_endpoint_at(&records, CanvasConnectionEndpointRole::Source),
            None
        );
    }
}
