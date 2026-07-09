use super::*;
use crate::routing::{CanvasEdgeRouter, CanvasRoutePath, CanvasRouteRequest};

#[test]
fn removes_edges_when_node_is_removed() {
    let mut document = connected_pair_fixture().build();

    document.remove_node(&NodeId::from("a")).unwrap();

    assert!(document.edges.is_empty());
}

#[test]
fn validates_edge_handles() {
    let mut node = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
    node.handles
        .push(CanvasHandle::new("out", point(px(10.0), px(5.0))));
    let mut document = document_fixture()
        .node(node)
        .node(CanvasNode::new(
            "b",
            point(px(20.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .build();

    let err = document
        .insert_edge(CanvasEdge::new(
            "bad",
            CanvasEndpoint::new("a", Some("missing")),
            CanvasEndpoint::new("b", None::<&str>),
        ))
        .unwrap_err();

    assert_eq!(
        err,
        DocumentError::MissingHandle {
            node_id: NodeId::from("a"),
            handle_id: HandleId::from("missing")
        }
    );
}

#[test]
fn handle_connection_role_helpers_respect_roles_and_pickability() {
    let mut source_only = CanvasHandle::new("out", point(px(10.0), px(5.0)));
    source_only.role = HandleRole::Source;
    assert!(source_only.accepts_connection_role(CanvasConnectionEndpointRole::Source));
    assert!(!source_only.accepts_connection_role(CanvasConnectionEndpointRole::Target));
    assert!(source_only.is_pickable_connection_endpoint(CanvasConnectionEndpointRole::Source));

    source_only.hidden = true;
    assert!(source_only.accepts_connection_role(CanvasConnectionEndpointRole::Source));
    assert!(!source_only.is_pickable_connection_endpoint(CanvasConnectionEndpointRole::Source));

    let mut target_only = CanvasHandle::new("in", point(px(0.0), px(5.0)));
    target_only.role = HandleRole::Target;
    target_only.connectable = false;
    assert!(!target_only.accepts_connection_role(CanvasConnectionEndpointRole::Target));
    assert!(!target_only.is_pickable_connection_endpoint(CanvasConnectionEndpointRole::Target));
}

#[test]
fn rejects_duplicate_handles_on_node_insert() {
    let mut node = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
    node.handles
        .push(CanvasHandle::new("out", point(px(10.0), px(5.0))));
    node.handles
        .push(CanvasHandle::new("out", point(px(0.0), px(5.0))));

    let mut document = document_fixture().build();
    let err = document.insert_node(node).unwrap_err();

    assert_eq!(
        err,
        DocumentError::DuplicateHandle {
            node_id: NodeId::from("a"),
            handle_id: HandleId::from("out")
        }
    );
}

#[test]
fn validates_handle_roles_for_edges() {
    let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
    let mut target_only = CanvasHandle::new("in", point(px(10.0), px(5.0)));
    target_only.role = HandleRole::Target;
    source.handles.push(target_only);

    let mut target = CanvasNode::new("b", point(px(20.0), px(0.0)), size(px(10.0), px(10.0)));
    let mut source_only = CanvasHandle::new("out", point(px(0.0), px(5.0)));
    source_only.role = HandleRole::Source;
    target.handles.push(source_only);

    let mut document = document_fixture().node(source).node(target).build();

    let err = document
        .insert_edge(CanvasEdge::new(
            "bad",
            CanvasEndpoint::new("a", Some("in")),
            CanvasEndpoint::new("b", Some("out")),
        ))
        .unwrap_err();

    assert_eq!(
        err,
        DocumentError::InvalidSourceHandle {
            node_id: NodeId::from("a"),
            handle_id: HandleId::from("in")
        }
    );
}

#[test]
fn rejects_non_connectable_edge_handles() {
    let mut source = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
    let mut handle = CanvasHandle::new("out", point(px(10.0), px(5.0)));
    handle.connectable = false;
    source.handles.push(handle);

    let mut document = document_fixture()
        .node(source)
        .node(CanvasNode::new(
            "b",
            point(px(20.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .build();

    let err = document
        .insert_edge(CanvasEdge::new(
            "bad",
            CanvasEndpoint::new("a", Some("out")),
            CanvasEndpoint::new("b", None::<&str>),
        ))
        .unwrap_err();

    assert_eq!(
        err,
        DocumentError::NonConnectableHandle {
            node_id: NodeId::from("a"),
            handle_id: HandleId::from("out")
        }
    );
}

#[test]
fn edge_route_defaults_keep_legacy_edges_readable() {
    let edge: CanvasEdge = serde_json::from_str(
        r#"{
            "id": "a-b",
            "source": { "node_id": "a" },
            "target": { "node_id": "b" }
        }"#,
    )
    .unwrap();

    assert_eq!(
        edge.route.kind,
        CanvasEdgeRouteKind::from(CanvasEdgeRouteKind::STRAIGHT)
    );
    assert!(edge.route.waypoints.is_empty());
    assert!(edge.route.control_points.is_empty());
    assert_eq!(edge.route.interaction_width, px(12.0));
}

#[test]
fn edge_route_points_include_waypoints_between_endpoints() {
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
        .build();
    let mut edge = CanvasEdge::new(
        "a-b",
        CanvasEndpoint::new("a", None::<&str>),
        CanvasEndpoint::new("b", None::<&str>),
    );
    edge.route = CanvasEdgeRoute::polyline([point(px(40.0), px(50.0)), point(px(80.0), px(50.0))]);

    let route_points = document.edge_route_points(&edge).unwrap();

    assert_eq!(
        route_points,
        vec![
            point(px(5.0), px(5.0)),
            point(px(40.0), px(50.0)),
            point(px(80.0), px(50.0)),
            point(px(105.0), px(5.0)),
        ]
    );
}

#[test]
fn edge_bounds_include_route_points_and_interaction_width() {
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
        .build();
    let mut edge = CanvasEdge::new(
        "a-b",
        CanvasEndpoint::new("a", None::<&str>),
        CanvasEndpoint::new("b", None::<&str>),
    );
    edge.route = CanvasEdgeRoute::polyline([point(px(40.0), px(50.0))]);
    edge.route.interaction_width = px(20.0);

    let bounds = document.edge_bounds(&edge).unwrap();

    assert_eq!(bounds.origin, point(px(-5.0), px(-5.0)));
    assert_eq!(bounds.size, size(px(120.0), px(65.0)));
}

#[test]
fn edge_bounds_can_use_custom_router_path() {
    struct OffsetRouter;

    impl CanvasEdgeRouter for OffsetRouter {
        fn route_edge(&self, request: CanvasRouteRequest<'_>) -> CanvasRoutePath {
            CanvasRoutePath::polyline([request.source, point(px(50.0), px(120.0)), request.target])
        }
    }

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
        .build();
    let mut edge = CanvasEdge::new(
        "a-b",
        CanvasEndpoint::new("a", None::<&str>),
        CanvasEndpoint::new("b", None::<&str>),
    );
    edge.route.interaction_width = px(10.0);

    let path = document
        .edge_route_path_with_router(&edge, &OffsetRouter)
        .unwrap();
    let bounds = document
        .edge_bounds_with_router(&edge, &OffsetRouter)
        .unwrap();

    assert_eq!(
        path.document_points(),
        vec![
            point(px(5.0), px(5.0)),
            point(px(50.0), px(120.0)),
            point(px(105.0), px(5.0)),
        ]
    );
    assert_eq!(bounds.origin, point(px(0.0), px(0.0)));
    assert_eq!(bounds.size, size(px(110.0), px(125.0)));
}

#[test]
fn rejects_invalid_edge_route_metadata() {
    let mut document = document_fixture()
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
        .build();

    let mut empty_kind = CanvasEdge::new(
        "empty-kind",
        CanvasEndpoint::new("a", None::<&str>),
        CanvasEndpoint::new("b", None::<&str>),
    );
    empty_kind.route.kind = CanvasEdgeRouteKind::new("");
    assert_eq!(
        document.insert_edge(empty_kind).unwrap_err(),
        DocumentError::EmptyEdgeRouteKind(EdgeId::from("empty-kind"))
    );

    let mut negative_width = CanvasEdge::new(
        "negative-width",
        CanvasEndpoint::new("a", None::<&str>),
        CanvasEndpoint::new("b", None::<&str>),
    );
    negative_width.route.interaction_width = px(-1.0);
    assert_eq!(
        document.insert_edge(negative_width).unwrap_err(),
        DocumentError::InvalidEdgeInteractionWidth(EdgeId::from("negative-width"))
    );

    let mut invalid_point = CanvasEdge::new(
        "invalid-point",
        CanvasEndpoint::new("a", None::<&str>),
        CanvasEndpoint::new("b", None::<&str>),
    );
    invalid_point
        .route
        .waypoints
        .push(point(px(f32::NAN), px(0.0)));
    assert_eq!(
        document.insert_edge(invalid_point).unwrap_err(),
        DocumentError::InvalidEdgeRoutePoint(EdgeId::from("invalid-point"))
    );
}

#[test]
fn node_update_cannot_break_existing_edge_endpoints() {
    let mut node = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
    node.handles
        .push(CanvasHandle::new("out", point(px(10.0), px(5.0))));

    let mut document = document_fixture()
        .node(node.clone())
        .node(CanvasNode::new(
            "b",
            point(px(20.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .edge(CanvasEdge::new(
            "a-b",
            CanvasEndpoint::new("a", Some("out")),
            CanvasEndpoint::new("b", None::<&str>),
        ))
        .build();

    node.handles.clear();
    let err = document
        .apply_transaction_with_diff(CanvasTransaction::single(DocumentCommand::UpdateNode(node)))
        .unwrap_err();

    assert_eq!(
        err,
        DocumentError::MissingHandle {
            node_id: NodeId::from("a"),
            handle_id: HandleId::from("out")
        }
    );
    assert!(
        document.nodes[&NodeId::from("a")]
            .handle(Some(&HandleId::from("out")))
            .is_some()
    );
}
