use crate::{CanvasDocument, CanvasEdge, CanvasEndpoint, CanvasHandle, CanvasNode, EdgeId, NodeId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CanvasEdgeDirection {
    Incoming,
    Outgoing,
    Any,
}

#[derive(Clone, Copy, Debug)]
pub struct CanvasGraph<'a> {
    document: &'a CanvasDocument,
}

impl<'a> CanvasGraph<'a> {
    pub fn new(document: &'a CanvasDocument) -> Self {
        Self { document }
    }

    pub fn document(&self) -> &'a CanvasDocument {
        self.document
    }

    pub fn node(&self, id: &NodeId) -> Option<&'a CanvasNode> {
        self.document.nodes.get(id)
    }

    pub fn edge(&self, id: &EdgeId) -> Option<&'a CanvasEdge> {
        self.document.edges.get(id)
    }

    pub fn endpoint_node(&self, endpoint: &CanvasEndpoint) -> Option<&'a CanvasNode> {
        self.node(&endpoint.node_id)
    }

    pub fn endpoint_handle(&self, endpoint: &CanvasEndpoint) -> Option<&'a CanvasHandle> {
        let handle_id = endpoint.handle_id.as_ref()?;
        self.endpoint_node(endpoint)?.handle(Some(handle_id))
    }

    pub fn outgoing_edges<'q>(
        &'q self,
        node_id: &'q NodeId,
    ) -> impl Iterator<Item = &'a CanvasEdge> + 'q
    where
        'a: 'q,
    {
        self.edges_for_node(node_id, CanvasEdgeDirection::Outgoing)
    }

    pub fn incoming_edges<'q>(
        &'q self,
        node_id: &'q NodeId,
    ) -> impl Iterator<Item = &'a CanvasEdge> + 'q
    where
        'a: 'q,
    {
        self.edges_for_node(node_id, CanvasEdgeDirection::Incoming)
    }

    pub fn incident_edges<'q>(
        &'q self,
        node_id: &'q NodeId,
    ) -> impl Iterator<Item = &'a CanvasEdge> + 'q
    where
        'a: 'q,
    {
        self.edges_for_node(node_id, CanvasEdgeDirection::Any)
    }

    pub fn edges_for_node<'q>(
        &'q self,
        node_id: &'q NodeId,
        direction: CanvasEdgeDirection,
    ) -> impl Iterator<Item = &'a CanvasEdge> + 'q
    where
        'a: 'q,
    {
        let document = self.document;
        document
            .edges
            .values()
            .filter(move |edge| edge_matches_node(edge, node_id, direction))
    }

    pub fn edges_between<'q>(
        &'q self,
        source: &'q NodeId,
        target: &'q NodeId,
    ) -> impl Iterator<Item = &'a CanvasEdge> + 'q
    where
        'a: 'q,
    {
        let document = self.document;
        document
            .edges
            .values()
            .filter(move |edge| edge.source.node_id == *source && edge.target.node_id == *target)
    }

    pub fn has_edge_between(&self, source: &NodeId, target: &NodeId) -> bool {
        self.edges_between(source, target).next().is_some()
    }

    pub fn neighbor_node_ids<'q>(
        &'q self,
        node_id: &'q NodeId,
        direction: CanvasEdgeDirection,
    ) -> impl Iterator<Item = &'a NodeId> + 'q
    where
        'a: 'q,
    {
        let document = self.document;
        document
            .edges
            .values()
            .filter_map(move |edge| neighbor_node_id(edge, node_id, direction))
    }

    pub fn incident_edge_count(&self, node_id: &NodeId) -> usize {
        self.incident_edges(node_id).count()
    }
}

impl CanvasDocument {
    pub fn graph(&self) -> CanvasGraph<'_> {
        CanvasGraph::new(self)
    }
}

fn edge_matches_node(edge: &CanvasEdge, node_id: &NodeId, direction: CanvasEdgeDirection) -> bool {
    match direction {
        CanvasEdgeDirection::Incoming => edge.target.node_id == *node_id,
        CanvasEdgeDirection::Outgoing => edge.source.node_id == *node_id,
        CanvasEdgeDirection::Any => {
            edge.source.node_id == *node_id || edge.target.node_id == *node_id
        }
    }
}

fn neighbor_node_id<'a>(
    edge: &'a CanvasEdge,
    node_id: &NodeId,
    direction: CanvasEdgeDirection,
) -> Option<&'a NodeId> {
    match direction {
        CanvasEdgeDirection::Incoming if edge.target.node_id == *node_id => {
            Some(&edge.source.node_id)
        }
        CanvasEdgeDirection::Outgoing if edge.source.node_id == *node_id => {
            Some(&edge.target.node_id)
        }
        CanvasEdgeDirection::Any if edge.source.node_id == *node_id => Some(&edge.target.node_id),
        CanvasEdgeDirection::Any if edge.target.node_id == *node_id => Some(&edge.source.node_id),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanvasHandle, HandleRole};
    use open_gpui::{point, px, size};

    #[test]
    fn graph_queries_directed_edges() {
        let document = sample_document();
        let graph = document.graph();
        let a = NodeId::from("a");
        let b = NodeId::from("b");

        assert_eq!(
            edge_ids(graph.outgoing_edges(&a)),
            vec!["a-b".to_string(), "a-a".to_string()]
        );
        assert_eq!(
            edge_ids(graph.incoming_edges(&a)),
            vec!["c-a".to_string(), "a-a".to_string()]
        );
        assert_eq!(
            edge_ids(graph.incident_edges(&a)),
            vec!["a-b".to_string(), "c-a".to_string(), "a-a".to_string()]
        );
        assert_eq!(
            edge_ids(graph.edges_between(&a, &b)),
            vec!["a-b".to_string()]
        );
        assert!(graph.has_edge_between(&a, &b));
        assert!(!graph.has_edge_between(&b, &a));
        assert_eq!(graph.incident_edge_count(&a), 3);
    }

    #[test]
    fn graph_queries_neighbors_by_direction() {
        let document = sample_document();
        let graph = document.graph();
        let a = NodeId::from("a");

        assert_eq!(
            node_ids(graph.neighbor_node_ids(&a, CanvasEdgeDirection::Outgoing)),
            vec!["b".to_string(), "a".to_string()]
        );
        assert_eq!(
            node_ids(graph.neighbor_node_ids(&a, CanvasEdgeDirection::Incoming)),
            vec!["c".to_string(), "a".to_string()]
        );
        assert_eq!(
            node_ids(graph.neighbor_node_ids(&a, CanvasEdgeDirection::Any)),
            vec!["b".to_string(), "c".to_string(), "a".to_string()]
        );
    }

    #[test]
    fn graph_queries_endpoint_parts() {
        let document = sample_document();
        let graph = document.graph();
        let endpoint = CanvasEndpoint::new("a", Some("out"));

        assert_eq!(
            graph.endpoint_node(&endpoint).unwrap().id,
            NodeId::from("a")
        );
        assert_eq!(
            graph.endpoint_handle(&endpoint).unwrap().id,
            crate::HandleId::from("out")
        );
        assert!(
            graph
                .endpoint_handle(&CanvasEndpoint::new("a", None::<&str>))
                .is_none()
        );
        assert!(
            graph
                .endpoint_node(&CanvasEndpoint::new("missing", None::<&str>))
                .is_none()
        );
    }

    fn sample_document() -> CanvasDocument {
        let mut a = CanvasNode::new("a", point(px(0.0), px(0.0)), size(px(100.0), px(60.0)));
        let mut out = CanvasHandle::new("out", point(px(100.0), px(30.0)));
        out.role = HandleRole::Source;
        a.handles.push(out);
        let mut input = CanvasHandle::new("in", point(px(0.0), px(30.0)));
        input.role = HandleRole::Target;
        a.handles.push(input);

        let b = CanvasNode::new("b", point(px(160.0), px(0.0)), size(px(100.0), px(60.0)));
        let c = CanvasNode::new("c", point(px(-160.0), px(0.0)), size(px(100.0), px(60.0)));

        let mut document = CanvasDocument::default();
        document.insert_node(a).unwrap();
        document.insert_node(b).unwrap();
        document.insert_node(c).unwrap();
        document
            .insert_edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", Some("out")),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .unwrap();
        document
            .insert_edge(CanvasEdge::new(
                "c-a",
                CanvasEndpoint::new("c", None::<&str>),
                CanvasEndpoint::new("a", Some("in")),
            ))
            .unwrap();
        document
            .insert_edge(CanvasEdge::new(
                "a-a",
                CanvasEndpoint::new("a", Some("out")),
                CanvasEndpoint::new("a", Some("in")),
            ))
            .unwrap();
        document
    }

    fn edge_ids<'a>(edges: impl IntoIterator<Item = &'a CanvasEdge>) -> Vec<String> {
        edges
            .into_iter()
            .map(|edge| edge.id.as_str().to_string())
            .collect()
    }

    fn node_ids<'a>(ids: impl IntoIterator<Item = &'a NodeId>) -> Vec<String> {
        ids.into_iter().map(|id| id.as_str().to_string()).collect()
    }
}
