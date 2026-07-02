use crate::{
    CanvasConnectionEndpointRole, CanvasDefaultEdgeRouter, CanvasDocument, CanvasEdge,
    CanvasEdgeRouter, CanvasEndpoint, CanvasHandle, CanvasKindRegistry, CanvasNode, CanvasRecordId,
    CanvasRoutePath, CanvasRouteRequest, CanvasRouteSegment, CanvasSelection, CanvasShape,
    DocumentError, HitOptions, HitRecord, HitTarget,
};
use open_gpui::{Bounds, Pixels, Point, px};

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasResolvedEdgeGeometry {
    pub path: CanvasRoutePath,
    pub bounds: Bounds<Pixels>,
    pub hit_radius: Pixels,
}

impl CanvasResolvedEdgeGeometry {
    pub fn contains_point(&self, point: Point<Pixels>, margin: Pixels) -> bool {
        let radius = (self.hit_radius + margin).as_f32().max(0.0);
        let Some(distance_squared) = self.distance_to_point_squared(point) else {
            return false;
        };
        distance_squared <= radius * radius
    }

    pub fn distance_to_point_squared(&self, point: Point<Pixels>) -> Option<f32> {
        self.nearest_point_to_route(point)
            .map(|nearest| nearest.distance_squared)
    }

    pub fn nearest_point(&self, point: Point<Pixels>) -> Option<Point<Pixels>> {
        self.nearest_point_to_route(point)
            .map(|nearest| nearest.point)
    }

    fn nearest_point_to_route(&self, point: Point<Pixels>) -> Option<NearestPoint> {
        self.path
            .segments
            .iter()
            .filter_map(|segment| segment_nearest_point(segment, point))
            .min_by(|left, right| left.distance_squared.total_cmp(&right.distance_squared))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CanvasGeometryFacts<'a, R = CanvasDefaultEdgeRouter> {
    document: &'a CanvasDocument,
    router: R,
    kind_registry: Option<&'a CanvasKindRegistry>,
}

impl<'a> CanvasGeometryFacts<'a> {
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

impl<'a, R> CanvasGeometryFacts<'a, R>
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
            .node(&endpoint.node_id)
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

        let hit_radius = edge_interaction_radius(edge);
        Ok(CanvasResolvedEdgeGeometry {
            path,
            bounds: bounds.dilate(hit_radius),
            hit_radius,
        })
    }

    pub fn edge_bounds(&self, edge: &CanvasEdge) -> Result<Bounds<Pixels>, DocumentError> {
        Ok(self.edge_geometry(edge)?.bounds)
    }

    pub fn record_geometry(&self, record_id: &CanvasRecordId) -> Option<CanvasRecordGeometry> {
        match record_id {
            CanvasRecordId::Node(id) => {
                let node = self.document.node(id)?;
                Some(CanvasRecordGeometry {
                    id: CanvasRecordId::Node(node.id.clone()),
                    bounds: self.node_bounds(node),
                    z_index: node.z_index,
                    hidden: node.hidden,
                    locked: node.locked,
                })
            }
            CanvasRecordId::Edge(id) => {
                let edge = self.document.edge(id)?;
                let bounds = self.edge_bounds(edge).ok()?;
                Some(CanvasRecordGeometry {
                    id: CanvasRecordId::Edge(edge.id.clone()),
                    bounds,
                    z_index: edge.z_index,
                    hidden: edge.hidden,
                    locked: edge.locked,
                })
            }
            CanvasRecordId::Shape(id) => {
                let shape = self.document.shape(id)?;
                Some(CanvasRecordGeometry {
                    id: CanvasRecordId::Shape(shape.id.clone()),
                    bounds: self.shape_bounds(shape),
                    z_index: shape.z_index,
                    hidden: shape.hidden,
                    locked: shape.locked,
                })
            }
        }
    }

    pub fn record_geometries(&self) -> Vec<CanvasRecordGeometry> {
        self.document
            .nodes()
            .filter_map(|node| self.record_geometry(&CanvasRecordId::Node(node.id.clone())))
            .chain(
                self.document.shapes().filter_map(|shape| {
                    self.record_geometry(&CanvasRecordId::Shape(shape.id.clone()))
                }),
            )
            .chain(
                self.document.edges().filter_map(|edge| {
                    self.record_geometry(&CanvasRecordId::Edge(edge.id.clone()))
                }),
            )
            .collect()
    }

    pub fn selected_record_geometries(
        &self,
        selection: &CanvasSelection,
    ) -> Vec<CanvasRecordGeometry> {
        selection
            .selected_nodes()
            .filter_map(|id| self.record_geometry(&CanvasRecordId::Node(id.clone())))
            .chain(
                selection
                    .selected_shapes()
                    .filter_map(|id| self.record_geometry(&CanvasRecordId::Shape(id.clone()))),
            )
            .collect()
    }

    pub fn selected_bounds(&self, selection: &CanvasSelection) -> Option<Bounds<Pixels>> {
        union_record_geometry_bounds(
            self.selected_record_geometries(selection)
                .into_iter()
                .filter(CanvasRecordGeometry::is_visible_unlocked),
        )
    }

    pub fn node_shape_bounds_for_records<'b>(
        &self,
        record_ids: impl IntoIterator<Item = &'b CanvasRecordId>,
    ) -> Option<Bounds<Pixels>> {
        union_record_geometry_bounds(record_ids.into_iter().filter_map(|record_id| {
            if !matches!(
                record_id,
                CanvasRecordId::Node(_) | CanvasRecordId::Shape(_)
            ) {
                return None;
            }

            self.record_geometry(record_id)
                .filter(CanvasRecordGeometry::is_visible_unlocked)
        }))
    }

    pub(crate) fn hit_records(&self) -> Vec<HitRecord> {
        let mut records = Vec::new();

        for node in self.document.nodes() {
            records.extend(self.hit_records_for_record(&CanvasRecordId::Node(node.id.clone())));
        }

        for shape in self.document.shapes() {
            records.extend(self.hit_records_for_record(&CanvasRecordId::Shape(shape.id.clone())));
        }

        for edge in self.document.edges() {
            records.extend(self.hit_records_for_record(&CanvasRecordId::Edge(edge.id.clone())));
        }

        records
    }

    pub(crate) fn hit_records_for_record(&self, record_id: &CanvasRecordId) -> Vec<HitRecord> {
        let mut records = Vec::new();

        match record_id {
            CanvasRecordId::Node(id) => {
                let Some(node) = self.document.node(id) else {
                    return records;
                };

                records.push(HitRecord {
                    target: HitTarget::Node(node.id.clone()),
                    bounds: self.node_bounds(node),
                    z_index: node.z_index,
                    hidden: node.hidden,
                    locked: node.locked,
                });

                for handle in &node.handles {
                    records.push(HitRecord {
                        target: HitTarget::Handle {
                            node_id: node.id.clone(),
                            handle_id: handle.id.clone(),
                        },
                        bounds: self.handle_bounds(node, handle),
                        z_index: node.z_index,
                        hidden: node.hidden || handle.hidden || !handle.connectable,
                        locked: node.locked,
                    });
                }
            }
            CanvasRecordId::Edge(id) => {
                let Some(edge) = self.document.edge(id) else {
                    return records;
                };

                if let Ok(bounds) = self.edge_bounds(edge) {
                    records.push(HitRecord {
                        target: HitTarget::Edge(edge.id.clone()),
                        bounds,
                        z_index: edge.z_index,
                        hidden: edge.hidden,
                        locked: edge.locked,
                    });
                }
            }
            CanvasRecordId::Shape(id) => {
                let Some(shape) = self.document.shape(id) else {
                    return records;
                };

                records.push(HitRecord {
                    target: HitTarget::Shape(shape.id.clone()),
                    bounds: self.shape_bounds(shape),
                    z_index: shape.z_index,
                    hidden: shape.hidden,
                    locked: shape.locked,
                });
            }
        }

        records
    }

    pub fn record_contains_point(
        &self,
        record: &HitRecord,
        point: Point<Pixels>,
        options: HitOptions,
    ) -> bool {
        self.record_contains_point_with_edge_geometry(record, point, options, None)
    }

    pub fn record_intersects_bounds(
        &self,
        record: &HitRecord,
        bounds: Bounds<Pixels>,
        options: HitOptions,
    ) -> bool {
        if !record_options_match(record, options) {
            return false;
        }

        let record_bounds = if options.margin == Pixels::ZERO {
            record.bounds
        } else {
            record.bounds.dilate(options.margin)
        };
        let Some(intersection) = intersect_bounds(record_bounds, bounds) else {
            return false;
        };

        match &record.target {
            HitTarget::Node(id) => {
                if let Some(intersects) = self.document.node(id).and_then(|node| {
                    self.kind_registry.and_then(|registry| {
                        registry.node_intersects_bounds(node, record.bounds, bounds, options.margin)
                    })
                }) {
                    return intersects;
                }

                selection_sample_points(intersection)
                    .into_iter()
                    .any(|point| self.record_contains_point(record, point, options))
            }
            HitTarget::Shape(id) => {
                if let Some(intersects) = self.document.shape(id).and_then(|shape| {
                    self.kind_registry.and_then(|registry| {
                        registry.shape_intersects_bounds(
                            shape,
                            record.bounds,
                            bounds,
                            options.margin,
                        )
                    })
                }) {
                    return intersects;
                }

                selection_sample_points(intersection)
                    .into_iter()
                    .any(|point| self.record_contains_point(record, point, options))
            }
            HitTarget::Edge(_) | HitTarget::Handle { .. } => true,
        }
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
                let Some(node) = self.document.node(id) else {
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
                let Some(shape) = self.document.shape(id) else {
                    return false;
                };
                self.kind_registry
                    .and_then(|registry| {
                        registry.shape_contains_point(shape, point, record.bounds, options.margin)
                    })
                    .unwrap_or(true)
            }
            HitTarget::Edge(id) => {
                let Some(edge) = self.document.edge(id) else {
                    return false;
                };
                if let Some(edge_geometry) = edge_geometry {
                    return edge_geometry.contains_point(point, options.margin);
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
                    let node = self.document.node(node_id)?;
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
        Ok(geometry.contains_point(point, margin))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasRecordGeometry {
    pub id: CanvasRecordId,
    pub bounds: Bounds<Pixels>,
    pub z_index: i32,
    pub hidden: bool,
    pub locked: bool,
}

impl CanvasRecordGeometry {
    pub fn is_node_or_shape(&self) -> bool {
        matches!(self.id, CanvasRecordId::Node(_) | CanvasRecordId::Shape(_))
    }

    pub fn is_visible_unlocked(&self) -> bool {
        !self.hidden && !self.locked
    }
}

pub(crate) fn union_record_geometry_bounds(
    geometries: impl IntoIterator<Item = CanvasRecordGeometry>,
) -> Option<Bounds<Pixels>> {
    geometries.into_iter().fold(None, |current, geometry| {
        union_bounds(current, geometry.bounds)
    })
}

fn union_bounds(current: Option<Bounds<Pixels>>, next: Bounds<Pixels>) -> Option<Bounds<Pixels>> {
    Some(match current {
        None => next,
        Some(current) => Bounds::from_corners(
            Point::new(
                current.origin.x.min(next.origin.x),
                current.origin.y.min(next.origin.y),
            ),
            Point::new(
                (current.origin.x + current.size.width).max(next.origin.x + next.size.width),
                (current.origin.y + current.size.height).max(next.origin.y + next.size.height),
            ),
        ),
    })
}

fn intersect_bounds(left: Bounds<Pixels>, right: Bounds<Pixels>) -> Option<Bounds<Pixels>> {
    let left_min_x = left.origin.x;
    let left_min_y = left.origin.y;
    let left_max_x = left.origin.x + left.size.width;
    let left_max_y = left.origin.y + left.size.height;
    let right_min_x = right.origin.x;
    let right_min_y = right.origin.y;
    let right_max_x = right.origin.x + right.size.width;
    let right_max_y = right.origin.y + right.size.height;

    let min_x = left_min_x.max(right_min_x);
    let min_y = left_min_y.max(right_min_y);
    let max_x = left_max_x.min(right_max_x);
    let max_y = left_max_y.min(right_max_y);
    if max_x < min_x || max_y < min_y {
        return None;
    }

    Some(Bounds::from_corners(
        Point::new(min_x, min_y),
        Point::new(max_x, max_y),
    ))
}

fn selection_sample_points(bounds: Bounds<Pixels>) -> [Point<Pixels>; 9] {
    let left = bounds.origin.x;
    let top = bounds.origin.y;
    let right = bounds.origin.x + bounds.size.width;
    let bottom = bounds.origin.y + bounds.size.height;
    let center = bounds.center();

    [
        Point::new(left, top),
        Point::new(center.x, top),
        Point::new(right, top),
        Point::new(left, center.y),
        center,
        Point::new(right, center.y),
        Point::new(left, bottom),
        Point::new(center.x, bottom),
        Point::new(right, bottom),
    ]
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

#[derive(Clone, Copy, Debug, PartialEq)]
struct NearestPoint {
    point: Point<Pixels>,
    distance_squared: f32,
}

fn segment_nearest_point(
    segment: &CanvasRouteSegment,
    point: Point<Pixels>,
) -> Option<NearestPoint> {
    match segment {
        CanvasRouteSegment::Line { from, to } => {
            Some(point_to_line_segment_nearest_point(point, *from, *to))
        }
        CanvasRouteSegment::CubicBezier {
            from,
            control_1,
            control_2,
            to,
        } => cubic_bezier_nearest_point(point, *from, *control_1, *control_2, *to),
    }
}

fn cubic_bezier_nearest_point(
    point: Point<Pixels>,
    from: Point<Pixels>,
    control_1: Point<Pixels>,
    control_2: Point<Pixels>,
    to: Point<Pixels>,
) -> Option<NearestPoint> {
    const STEPS: usize = 24;

    let mut closest = None;
    let mut previous = from;
    for step in 1..=STEPS {
        let t = step as f32 / STEPS as f32;
        let current = cubic_bezier_point(from, control_1, control_2, to, t);
        let nearest = point_to_line_segment_nearest_point(point, previous, current);
        if closest
            .is_none_or(|closest: NearestPoint| nearest.distance_squared < closest.distance_squared)
        {
            closest = Some(nearest);
        }
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

fn point_to_line_segment_nearest_point(
    point: Point<Pixels>,
    from: Point<Pixels>,
    to: Point<Pixels>,
) -> NearestPoint {
    let point_x = point.x.as_f32();
    let point_y = point.y.as_f32();
    let ax = from.x.as_f32();
    let ay = from.y.as_f32();
    let bx = to.x.as_f32();
    let by = to.y.as_f32();
    let dx = bx - ax;
    let dy = by - ay;
    let length_squared = dx * dx + dy * dy;

    if length_squared <= f32::EPSILON {
        let dx = point_x - ax;
        let dy = point_y - ay;
        return NearestPoint {
            point: from,
            distance_squared: dx * dx + dy * dy,
        };
    }

    let t = (((point_x - ax) * dx + (point_y - ay) * dy) / length_squared).clamp(0.0, 1.0);
    let nearest_x = ax + t * dx;
    let nearest_y = ay + t * dy;
    let dx = point_x - nearest_x;
    let dy = point_y - nearest_y;
    NearestPoint {
        point: Point::new(px(nearest_x), px(nearest_y)),
        distance_squared: dx * dx + dy * dy,
    }
}

fn record_options_match(record: &HitRecord, options: HitOptions) -> bool {
    (options.include_hidden || !record.hidden)
        && (options.include_locked || !record.locked)
        && (options.include_handles || !matches!(record.target, HitTarget::Handle { .. }))
}

pub fn connection_hit_options() -> HitOptions {
    HitOptions {
        include_handles: true,
        margin: px(4.0),
        ..HitOptions::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CanvasEdge, CanvasHandle, CanvasNode, CanvasNodeBoundsHitTest, CanvasNodeInteractionPolicy,
        CanvasNodeKind, CanvasShape, CanvasStyle, HandleRole, test_support::document_fixture,
    };
    use open_gpui::{point, px, size};

    #[test]
    fn facts_use_same_endpoint_position_for_handles_and_node_centers() {
        let mut node = CanvasNode::new("a", point(px(10.0), px(20.0)), size(px(40.0), px(60.0)));
        node.handles
            .push(CanvasHandle::new("out", point(px(40.0), px(30.0))));
        let document = document_fixture().node(node).build();
        let facts = CanvasGeometryFacts::new(&document);

        assert_eq!(
            facts
                .endpoint_position(&CanvasEndpoint::new("a", None::<&str>))
                .unwrap(),
            point(px(30.0), px(50.0))
        );
        assert_eq!(
            facts
                .endpoint_position(&CanvasEndpoint::new("a", Some("out")))
                .unwrap(),
            point(px(50.0), px(50.0))
        );
    }

    #[test]
    fn facts_pick_connection_handles_by_role() {
        let mut node = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        let mut target_only = CanvasHandle::new("in", point(px(100.0), px(50.0)));
        target_only.role = HandleRole::Target;
        node.handles.push(target_only);
        let document = document_fixture().node(node).build();
        let facts = CanvasGeometryFacts::new(&document);
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
            facts.connection_endpoint_at(&records, CanvasConnectionEndpointRole::Target),
            Some(CanvasEndpoint::new("a", Some("in")))
        );
        assert_eq!(
            facts.connection_endpoint_at(&records, CanvasConnectionEndpointRole::Source),
            None
        );
    }

    #[test]
    fn facts_materialize_hit_records_for_nodes_handles_shapes_and_edges() {
        let mut node = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        node.handles
            .push(CanvasHandle::new("out", point(px(10.0), px(5.0))));
        let document = document_fixture()
            .node(node)
            .node(CanvasNode::new(
                "b",
                point(px(100.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .shape(CanvasShape::new(
                "shape",
                Bounds::new(point(px(0.0), px(40.0)), size(px(20.0), px(20.0))),
            ))
            .edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", Some("out")),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .build();

        let records = CanvasGeometryFacts::new(&document).hit_records();
        let targets = records
            .into_iter()
            .map(|record| record.target)
            .collect::<Vec<_>>();

        assert_eq!(
            targets,
            vec![
                HitTarget::Node("a".into()),
                HitTarget::Handle {
                    node_id: "a".into(),
                    handle_id: "out".into(),
                },
                HitTarget::Node("b".into()),
                HitTarget::Shape("shape".into()),
                HitTarget::Edge("a-b".into()),
            ]
        );
    }

    #[test]
    fn resolved_edge_geometry_answers_nearest_point_and_hit() {
        let mut edge = CanvasEdge::new(
            "a-b",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        );
        edge.style = CanvasStyle {
            stroke: Some("#0969da".to_string()),
            stroke_width: px(4.0),
            fill: None,
        };
        let document = document_fixture()
            .node(CanvasNode::new(
                "a",
                point(px(0.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .node(CanvasNode::new(
                "b",
                point(px(100.0), px(0.0)),
                size(px(10.0), px(10.0)),
            ))
            .edge(edge.clone())
            .build();

        let geometry = CanvasGeometryFacts::new(&document)
            .edge_geometry(&edge)
            .unwrap();

        assert_eq!(geometry.hit_radius, px(6.0));
        assert_eq!(
            geometry.nearest_point(point(px(40.0), px(8.0))).unwrap(),
            point(px(40.0), px(5.0))
        );
        assert!(geometry.contains_point(point(px(40.0), px(10.5)), Pixels::ZERO));
        assert!(!geometry.contains_point(point(px(40.0), px(11.5)), Pixels::ZERO));
        assert!(geometry.contains_point(point(px(40.0), px(11.5)), px(1.0)));
    }

    #[test]
    fn record_intersects_bounds_uses_shape_interaction_policy() {
        let mut group = CanvasShape::new(
            "group",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        );
        group.kind = CanvasKindRegistry::GROUP_SHAPE_KIND.to_string();
        let document = document_fixture().shape(group).build();
        let registry = CanvasKindRegistry::open();
        let facts = CanvasGeometryFacts::with_kind_registry(&document, &registry);
        let group_record = facts
            .hit_records()
            .into_iter()
            .find(|record| record.target == HitTarget::Shape("group".into()))
            .unwrap();

        assert!(!facts.record_intersects_bounds(
            &group_record,
            Bounds::new(point(px(40.0), px(40.0)), size(px(20.0), px(20.0))),
            HitOptions::default()
        ));
        assert!(facts.record_intersects_bounds(
            &group_record,
            Bounds::new(point(px(0.0), px(40.0)), size(px(8.0), px(20.0))),
            HitOptions::default()
        ));
    }

    #[test]
    fn record_intersects_bounds_uses_node_bounds_interaction_policy() {
        struct RightHalfBoundsKind;

        impl CanvasNodeInteractionPolicy for RightHalfBoundsKind {
            fn node_intersects_bounds(&self, hit: CanvasNodeBoundsHitTest<'_>) -> Option<bool> {
                let active = Bounds::from_corners(
                    Point::new(hit.bounds.center().x, hit.bounds.origin.y),
                    Point::new(
                        hit.bounds.origin.x + hit.bounds.size.width,
                        hit.bounds.origin.y + hit.bounds.size.height,
                    ),
                );
                Some(active.intersects(&hit.query_bounds))
            }
        }

        let mut node = CanvasNode::new("node", point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        node.kind = "right-half-bounds".to_string();
        let document = document_fixture().node(node).build();
        let mut registry = CanvasKindRegistry::open();
        registry.register_node_kind(
            "right-half-bounds",
            CanvasNodeKind::new().with_interaction_policy(RightHalfBoundsKind),
        );
        let facts = CanvasGeometryFacts::with_kind_registry(&document, &registry);
        let node_record = facts
            .hit_records()
            .into_iter()
            .find(|record| record.target == HitTarget::Node("node".into()))
            .unwrap();

        assert!(!facts.record_intersects_bounds(
            &node_record,
            Bounds::new(point(px(10.0), px(10.0)), size(px(20.0), px(20.0))),
            HitOptions::default()
        ));
        assert!(facts.record_intersects_bounds(
            &node_record,
            Bounds::new(point(px(60.0), px(10.0)), size(px(20.0), px(20.0))),
            HitOptions::default()
        ));
    }

    #[test]
    fn record_intersects_bounds_defaults_to_bounds_for_regular_shapes() {
        let document = document_fixture()
            .shape(CanvasShape::new(
                "shape",
                Bounds::new(point(px(10.0), px(10.0)), size(px(20.0), px(20.0))),
            ))
            .build();
        let facts = CanvasGeometryFacts::new(&document);
        let shape_record = facts
            .hit_records()
            .into_iter()
            .find(|record| record.target == HitTarget::Shape("shape".into()))
            .unwrap();

        assert!(facts.record_intersects_bounds(
            &shape_record,
            Bounds::new(point(px(25.0), px(25.0)), size(px(20.0), px(20.0))),
            HitOptions::default()
        ));
        assert!(!facts.record_intersects_bounds(
            &shape_record,
            Bounds::new(point(px(40.0), px(40.0)), size(px(20.0), px(20.0))),
            HitOptions::default()
        ));
    }
}
