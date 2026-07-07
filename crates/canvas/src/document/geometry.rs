use super::*;
use crate::geometry_facts::CanvasGeometryFacts;
use crate::routing::{CanvasDefaultEdgeRouter, CanvasEdgeRouter, CanvasRoutePath};

impl CanvasDocument {
    pub fn endpoint_position(
        &self,
        endpoint: &CanvasEndpoint,
    ) -> Result<Point<Pixels>, DocumentError> {
        CanvasGeometryFacts::new(self).endpoint_position(endpoint)
    }

    pub fn edge_route_path(&self, edge: &CanvasEdge) -> Result<CanvasRoutePath, DocumentError> {
        self.edge_route_path_with_router(edge, &CanvasDefaultEdgeRouter)
    }

    pub fn edge_route_path_with_router<R>(
        &self,
        edge: &CanvasEdge,
        router: &R,
    ) -> Result<CanvasRoutePath, DocumentError>
    where
        R: CanvasEdgeRouter + ?Sized,
    {
        CanvasGeometryFacts::with_router(self, router).edge_route_path(edge)
    }

    pub fn edge_bounds(&self, edge: &CanvasEdge) -> Result<Bounds<Pixels>, DocumentError> {
        self.edge_bounds_with_router(edge, &CanvasDefaultEdgeRouter)
    }

    pub fn edge_bounds_with_router<R>(
        &self,
        edge: &CanvasEdge,
        router: &R,
    ) -> Result<Bounds<Pixels>, DocumentError>
    where
        R: CanvasEdgeRouter + ?Sized,
    {
        CanvasGeometryFacts::with_router(self, router).edge_bounds(edge)
    }

    pub fn edge_route_points(
        &self,
        edge: &CanvasEdge,
    ) -> Result<Vec<Point<Pixels>>, DocumentError> {
        Ok(self.edge_route_path(edge)?.document_points())
    }
}
