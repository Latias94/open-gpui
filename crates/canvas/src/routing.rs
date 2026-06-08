use crate::document::{CanvasEdge, CanvasEdgeRouteKind};
use open_gpui::{Bounds, Pixels, Point};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasRouteRequest<'a> {
    pub edge: &'a CanvasEdge,
    pub source: Point<Pixels>,
    pub target: Point<Pixels>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CanvasRoutePath {
    #[serde(default)]
    pub segments: Vec<CanvasRouteSegment>,
}

impl CanvasRoutePath {
    pub fn new(segments: impl IntoIterator<Item = CanvasRouteSegment>) -> Self {
        Self {
            segments: segments.into_iter().collect(),
        }
    }

    pub fn polyline(points: impl IntoIterator<Item = Point<Pixels>>) -> Self {
        let points = points.into_iter().collect::<Vec<_>>();
        Self::new(points.windows(2).map(|points| CanvasRouteSegment::Line {
            from: points[0],
            to: points[1],
        }))
    }

    pub fn orthogonal(points: impl IntoIterator<Item = Point<Pixels>>) -> Self {
        let points = points.into_iter().collect::<Vec<_>>();
        let mut orthogonal_points = Vec::new();

        for points in points.windows(2) {
            append_orthogonal_leg(&mut orthogonal_points, points[0], points[1]);
        }

        Self::polyline(orthogonal_points)
    }

    pub fn cubic_bezier(
        from: Point<Pixels>,
        control_1: Point<Pixels>,
        control_2: Point<Pixels>,
        to: Point<Pixels>,
    ) -> Self {
        Self::new([CanvasRouteSegment::CubicBezier {
            from,
            control_1,
            control_2,
            to,
        }])
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn document_points(&self) -> Vec<Point<Pixels>> {
        let mut points = Vec::new();
        for segment in &self.segments {
            let start = segment.start();
            if points.last().copied() != Some(start) {
                points.push(start);
            }
            points.push(segment.end());
        }
        points
    }

    pub fn bounds(&self) -> Option<Bounds<Pixels>> {
        let mut points = self
            .segments
            .iter()
            .flat_map(CanvasRouteSegment::bounds_points);
        let first = points.next()?;
        let (min_x, min_y, max_x, max_y) = points.fold(
            (first.x, first.y, first.x, first.y),
            |(min_x, min_y, max_x, max_y), point| {
                (
                    min_x.min(point.x),
                    min_y.min(point.y),
                    max_x.max(point.x),
                    max_y.max(point.y),
                )
            },
        );
        Some(Bounds::from_corners(
            Point::new(min_x, min_y),
            Point::new(max_x, max_y),
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum CanvasRouteSegment {
    Line {
        from: Point<Pixels>,
        to: Point<Pixels>,
    },
    CubicBezier {
        from: Point<Pixels>,
        control_1: Point<Pixels>,
        control_2: Point<Pixels>,
        to: Point<Pixels>,
    },
}

impl CanvasRouteSegment {
    pub fn start(&self) -> Point<Pixels> {
        match self {
            Self::Line { from, .. } | Self::CubicBezier { from, .. } => *from,
        }
    }

    pub fn end(&self) -> Point<Pixels> {
        match self {
            Self::Line { to, .. } | Self::CubicBezier { to, .. } => *to,
        }
    }

    pub fn bounds_points(&self) -> [Point<Pixels>; 4] {
        match self {
            Self::Line { from, to } => [*from, *to, *from, *to],
            Self::CubicBezier {
                from,
                control_1,
                control_2,
                to,
            } => [*from, *control_1, *control_2, *to],
        }
    }
}

pub trait CanvasEdgeRouter {
    fn route_edge(&self, request: CanvasRouteRequest<'_>) -> CanvasRoutePath;
}

impl<T> CanvasEdgeRouter for &T
where
    T: CanvasEdgeRouter + ?Sized,
{
    fn route_edge(&self, request: CanvasRouteRequest<'_>) -> CanvasRoutePath {
        (*self).route_edge(request)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CanvasDefaultEdgeRouter;

impl CanvasEdgeRouter for CanvasDefaultEdgeRouter {
    fn route_edge(&self, request: CanvasRouteRequest<'_>) -> CanvasRoutePath {
        let route = &request.edge.route;
        if route.kind.as_str() == CanvasEdgeRouteKind::CUBIC_BEZIER
            && route.control_points.len() >= 2
        {
            return CanvasRoutePath::cubic_bezier(
                request.source,
                route.control_points[0],
                route.control_points[1],
                request.target,
            );
        }

        if route.kind.as_str() == CanvasEdgeRouteKind::ORTHOGONAL {
            return CanvasRoutePath::orthogonal(
                [request.source]
                    .into_iter()
                    .chain(route.waypoints.iter().copied())
                    .chain([request.target]),
            );
        }

        CanvasRoutePath::polyline(
            [request.source]
                .into_iter()
                .chain(route.waypoints.iter().copied())
                .chain([request.target]),
        )
    }
}

fn append_orthogonal_leg(points: &mut Vec<Point<Pixels>>, from: Point<Pixels>, to: Point<Pixels>) {
    push_unique_point(points, from);

    if from.x != to.x && from.y != to.y {
        let mid_x = (from.x + to.x) * 0.5;
        push_unique_point(points, Point::new(mid_x, from.y));
        push_unique_point(points, Point::new(mid_x, to.y));
    }

    push_unique_point(points, to);
}

fn push_unique_point(points: &mut Vec<Point<Pixels>>, point: Point<Pixels>) {
    if points.last().copied() != Some(point) {
        points.push(point);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanvasEdge, CanvasEdgeRoute, CanvasEndpoint};
    use open_gpui::{point, px};

    #[test]
    fn default_router_emits_polyline_segments_from_waypoints() {
        let mut edge = CanvasEdge::new(
            "edge",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        );
        edge.route = CanvasEdgeRoute::polyline([point(px(40.0), px(50.0))]);

        let path = CanvasDefaultEdgeRouter.route_edge(CanvasRouteRequest {
            edge: &edge,
            source: point(px(0.0), px(0.0)),
            target: point(px(100.0), px(0.0)),
        });

        assert_eq!(
            path.segments,
            vec![
                CanvasRouteSegment::Line {
                    from: point(px(0.0), px(0.0)),
                    to: point(px(40.0), px(50.0)),
                },
                CanvasRouteSegment::Line {
                    from: point(px(40.0), px(50.0)),
                    to: point(px(100.0), px(0.0)),
                },
            ]
        );
        assert_eq!(
            path.document_points(),
            vec![
                point(px(0.0), px(0.0)),
                point(px(40.0), px(50.0)),
                point(px(100.0), px(0.0)),
            ]
        );
    }

    #[test]
    fn default_router_emits_cubic_segment_from_control_points() {
        let mut edge = CanvasEdge::new(
            "edge",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        );
        edge.route = CanvasEdgeRoute::new(CanvasEdgeRouteKind::CUBIC_BEZIER);
        edge.route.control_points = vec![point(px(25.0), px(40.0)), point(px(75.0), px(-40.0))];

        let path = CanvasDefaultEdgeRouter.route_edge(CanvasRouteRequest {
            edge: &edge,
            source: point(px(0.0), px(0.0)),
            target: point(px(100.0), px(0.0)),
        });

        assert_eq!(
            path.segments,
            vec![CanvasRouteSegment::CubicBezier {
                from: point(px(0.0), px(0.0)),
                control_1: point(px(25.0), px(40.0)),
                control_2: point(px(75.0), px(-40.0)),
                to: point(px(100.0), px(0.0)),
            }]
        );
        assert_eq!(
            path.bounds().unwrap(),
            Bounds::from_corners(point(px(0.0), px(-40.0)), point(px(100.0), px(40.0)))
        );
    }

    #[test]
    fn default_router_emits_orthogonal_segments() {
        let mut edge = CanvasEdge::new(
            "edge",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        );
        edge.route = CanvasEdgeRoute::new(CanvasEdgeRouteKind::ORTHOGONAL);

        let path = CanvasDefaultEdgeRouter.route_edge(CanvasRouteRequest {
            edge: &edge,
            source: point(px(0.0), px(0.0)),
            target: point(px(100.0), px(50.0)),
        });

        assert_eq!(
            path.document_points(),
            vec![
                point(px(0.0), px(0.0)),
                point(px(50.0), px(0.0)),
                point(px(50.0), px(50.0)),
                point(px(100.0), px(50.0)),
            ]
        );
        assert_eq!(
            path.segments,
            vec![
                CanvasRouteSegment::Line {
                    from: point(px(0.0), px(0.0)),
                    to: point(px(50.0), px(0.0)),
                },
                CanvasRouteSegment::Line {
                    from: point(px(50.0), px(0.0)),
                    to: point(px(50.0), px(50.0)),
                },
                CanvasRouteSegment::Line {
                    from: point(px(50.0), px(50.0)),
                    to: point(px(100.0), px(50.0)),
                },
            ]
        );
    }

    #[test]
    fn default_router_routes_orthogonal_waypoints_by_leg() {
        let mut edge = CanvasEdge::new(
            "edge",
            CanvasEndpoint::new("a", None::<&str>),
            CanvasEndpoint::new("b", None::<&str>),
        );
        edge.route = CanvasEdgeRoute::new(CanvasEdgeRouteKind::ORTHOGONAL);
        edge.route.waypoints = vec![point(px(40.0), px(30.0))];

        let path = CanvasDefaultEdgeRouter.route_edge(CanvasRouteRequest {
            edge: &edge,
            source: point(px(0.0), px(0.0)),
            target: point(px(100.0), px(30.0)),
        });

        assert_eq!(
            path.document_points(),
            vec![
                point(px(0.0), px(0.0)),
                point(px(20.0), px(0.0)),
                point(px(20.0), px(30.0)),
                point(px(40.0), px(30.0)),
                point(px(100.0), px(30.0)),
            ]
        );
    }
}
