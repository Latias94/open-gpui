use crate::{
    CanvasConnectionEndpointRole, CanvasDefaultEdgeRouter, CanvasDocument, CanvasEdge,
    CanvasEdgeRouter, CanvasEndpoint, CanvasRoutePath, CanvasRouteRequest, DocumentError,
    HitOptions, HitRecord, HitTarget,
};
use open_gpui::{Bounds, Pixels, Point};

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasResolvedEdgeGeometry {
    pub path: CanvasRoutePath,
    pub bounds: Bounds<Pixels>,
}

#[derive(Clone, Copy, Debug)]
pub struct CanvasGeometryResolver<'a, R = CanvasDefaultEdgeRouter> {
    document: &'a CanvasDocument,
    router: R,
}

impl<'a> CanvasGeometryResolver<'a> {
    pub fn new(document: &'a CanvasDocument) -> Self {
        Self::with_router(document, CanvasDefaultEdgeRouter)
    }
}

impl<'a, R> CanvasGeometryResolver<'a, R>
where
    R: CanvasEdgeRouter,
{
    pub fn with_router(document: &'a CanvasDocument, router: R) -> Self {
        Self { document, router }
    }

    pub fn document(&self) -> &'a CanvasDocument {
        self.document
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
            return Ok(node.position + handle.position);
        }

        Ok(node.bounds().center())
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
