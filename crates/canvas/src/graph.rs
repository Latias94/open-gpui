use crate::{
    CanvasDocument, CanvasDocumentDiff, CanvasEdge, CanvasEndpoint, CanvasHandle, CanvasNode,
    CanvasRecordId, EdgeId, NodeId,
};
use indexmap::{IndexMap, IndexSet};
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
        self.document.node(id)
    }

    pub fn edge(&self, id: &EdgeId) -> Option<&'a CanvasEdge> {
        self.document.edge(id)
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
            .edges()
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
            .edges()
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
            .edges()
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

    pub fn graph_index(&self) -> CanvasGraphIndex {
        CanvasGraphIndex::rebuild(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CanvasGraphEndpointIds {
    pub source: NodeId,
    pub target: NodeId,
}

impl CanvasGraphEndpointIds {
    pub fn new(source: impl Into<NodeId>, target: impl Into<NodeId>) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
        }
    }

    pub fn from_edge(edge: &CanvasEdge) -> Self {
        Self {
            source: edge.source.node_id.clone(),
            target: edge.target.node_id.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CanvasGraphIndex {
    outgoing: IndexMap<NodeId, IndexSet<EdgeId>>,
    incoming: IndexMap<NodeId, IndexSet<EdgeId>>,
    incident: IndexMap<NodeId, IndexSet<EdgeId>>,
    edge_endpoints: IndexMap<EdgeId, CanvasGraphEndpointIds>,
    #[serde(default, skip)]
    edge_ordinals: IndexMap<EdgeId, usize>,
}

impl CanvasGraphIndex {
    pub fn rebuild(document: &CanvasDocument) -> Self {
        let mut index = Self::default();
        for edge in document.edges() {
            index.insert_edge(edge);
        }
        index.refresh_edge_ordinals(document);
        index
    }

    pub fn apply_diff(&mut self, document: &CanvasDocument, diff: &CanvasDocumentDiff) {
        if diff.is_empty() {
            return;
        }

        let mut graph_changed = false;

        for record_id in &diff.removed {
            match record_id {
                CanvasRecordId::Edge(edge_id) => {
                    graph_changed |= self.remove_edge_id(edge_id);
                }
                CanvasRecordId::Node(node_id) => {
                    let edge_ids = self.incident_edge_ids(node_id).cloned().collect::<Vec<_>>();
                    for edge_id in edge_ids {
                        graph_changed |= self.remove_edge_id(&edge_id);
                    }
                }
                CanvasRecordId::Shape(_) => {}
            }
        }

        for record_id in &diff.updated {
            if let CanvasRecordId::Edge(edge_id) = record_id
                && let Some(endpoints) = self.edge_endpoints.get(edge_id).cloned()
                && document.edge(edge_id).is_some_and(|edge| {
                    edge.source.node_id != endpoints.source
                        || edge.target.node_id != endpoints.target
                })
            {
                graph_changed |= self.remove_edge_id(edge_id);
            }
        }

        for record_id in diff.updated.iter().chain(&diff.inserted) {
            if let CanvasRecordId::Edge(edge_id) = record_id
                && let Some(edge) = document.edge(edge_id)
                && !self.edge_endpoints.contains_key(edge_id)
            {
                self.insert_edge(edge);
                graph_changed = true;
            }
        }

        if graph_changed {
            self.refresh_edge_ordinals(document);
            self.sort_adjacency_by_edge_ordinals();
        }
    }

    pub fn graph<'a>(&'a self, document: &'a CanvasDocument) -> CanvasIndexedGraph<'a> {
        CanvasIndexedGraph::new(document, self)
    }

    pub fn edge_count(&self) -> usize {
        self.edge_endpoints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.edge_endpoints.is_empty()
    }

    pub fn contains_edge(&self, edge_id: &EdgeId) -> bool {
        self.edge_endpoints.contains_key(edge_id)
    }

    pub fn edge_endpoints(&self, edge_id: &EdgeId) -> Option<&CanvasGraphEndpointIds> {
        self.edge_endpoints.get(edge_id)
    }

    pub fn outgoing_edge_ids<'a>(
        &'a self,
        node_id: &NodeId,
    ) -> impl Iterator<Item = &'a EdgeId> + 'a {
        self.outgoing
            .get(node_id)
            .into_iter()
            .flat_map(|edge_ids| edge_ids.iter())
    }

    pub fn incoming_edge_ids<'a>(
        &'a self,
        node_id: &NodeId,
    ) -> impl Iterator<Item = &'a EdgeId> + 'a {
        self.incoming
            .get(node_id)
            .into_iter()
            .flat_map(|edge_ids| edge_ids.iter())
    }

    pub fn incident_edge_ids<'a>(
        &'a self,
        node_id: &NodeId,
    ) -> impl Iterator<Item = &'a EdgeId> + 'a {
        self.incident
            .get(node_id)
            .into_iter()
            .flat_map(|edge_ids| edge_ids.iter())
    }

    pub fn edge_ids_for_node<'a>(
        &'a self,
        node_id: &'a NodeId,
        direction: CanvasEdgeDirection,
    ) -> Box<dyn Iterator<Item = &'a EdgeId> + 'a> {
        match direction {
            CanvasEdgeDirection::Incoming => Box::new(self.incoming_edge_ids(node_id)),
            CanvasEdgeDirection::Outgoing => Box::new(self.outgoing_edge_ids(node_id)),
            CanvasEdgeDirection::Any => Box::new(self.incident_edge_ids(node_id)),
        }
    }

    pub fn edge_ids_between<'a>(
        &'a self,
        source: &'a NodeId,
        target: &'a NodeId,
    ) -> impl Iterator<Item = &'a EdgeId> + 'a {
        self.outgoing_edge_ids(source).filter(move |edge_id| {
            self.edge_endpoints
                .get(*edge_id)
                .is_some_and(|endpoints| endpoints.source == *source && endpoints.target == *target)
        })
    }

    pub fn has_edge_between(&self, source: &NodeId, target: &NodeId) -> bool {
        self.edge_ids_between(source, target).next().is_some()
    }

    pub fn neighbor_node_ids<'a>(
        &'a self,
        node_id: &'a NodeId,
        direction: CanvasEdgeDirection,
    ) -> Box<dyn Iterator<Item = &'a NodeId> + 'a> {
        match direction {
            CanvasEdgeDirection::Incoming => {
                Box::new(self.incoming_edge_ids(node_id).filter_map(move |edge_id| {
                    self.edge_endpoints
                        .get(edge_id)
                        .map(|endpoints| &endpoints.source)
                }))
            }
            CanvasEdgeDirection::Outgoing => {
                Box::new(self.outgoing_edge_ids(node_id).filter_map(move |edge_id| {
                    self.edge_endpoints
                        .get(edge_id)
                        .map(|endpoints| &endpoints.target)
                }))
            }
            CanvasEdgeDirection::Any => {
                Box::new(self.incident_edge_ids(node_id).filter_map(move |edge_id| {
                    let endpoints = self.edge_endpoints.get(edge_id)?;
                    if endpoints.source == *node_id {
                        Some(&endpoints.target)
                    } else {
                        Some(&endpoints.source)
                    }
                }))
            }
        }
    }

    pub fn incident_edge_count(&self, node_id: &NodeId) -> usize {
        self.incident_edge_ids(node_id).count()
    }

    fn insert_edge(&mut self, edge: &CanvasEdge) {
        if self.edge_endpoints.contains_key(&edge.id) {
            self.remove_edge_id(&edge.id);
        }
        let endpoints = CanvasGraphEndpointIds::from_edge(edge);
        self.outgoing
            .entry(endpoints.source.clone())
            .or_default()
            .insert(edge.id.clone());
        self.incoming
            .entry(endpoints.target.clone())
            .or_default()
            .insert(edge.id.clone());
        self.incident
            .entry(endpoints.source.clone())
            .or_default()
            .insert(edge.id.clone());
        if endpoints.source != endpoints.target {
            self.incident
                .entry(endpoints.target.clone())
                .or_default()
                .insert(edge.id.clone());
        }
        self.edge_endpoints.insert(edge.id.clone(), endpoints);
    }

    fn remove_edge_id(&mut self, edge_id: &EdgeId) -> bool {
        let Some(endpoints) = self.edge_endpoints.shift_remove(edge_id) else {
            return false;
        };
        self.edge_ordinals.shift_remove(edge_id);
        remove_edge_from_node(&mut self.outgoing, &endpoints.source, edge_id);
        remove_edge_from_node(&mut self.incoming, &endpoints.target, edge_id);
        remove_edge_from_node(&mut self.incident, &endpoints.source, edge_id);
        if endpoints.source != endpoints.target {
            remove_edge_from_node(&mut self.incident, &endpoints.target, edge_id);
        }
        true
    }

    fn refresh_edge_ordinals(&mut self, document: &CanvasDocument) {
        self.edge_ordinals.clear();
        for (ordinal, edge_id) in document.edge_ids().enumerate() {
            self.edge_ordinals.insert(edge_id.clone(), ordinal);
        }
    }

    fn sort_adjacency_by_edge_ordinals(&mut self) {
        let ordinals = &self.edge_ordinals;
        for edge_ids in self
            .outgoing
            .values_mut()
            .chain(self.incoming.values_mut())
            .chain(self.incident.values_mut())
        {
            edge_ids
                .sort_by_cached_key(|edge_id| ordinals.get(edge_id).copied().unwrap_or(usize::MAX));
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CanvasIndexedGraph<'a> {
    document: &'a CanvasDocument,
    index: &'a CanvasGraphIndex,
}

impl<'a> CanvasIndexedGraph<'a> {
    pub fn new(document: &'a CanvasDocument, index: &'a CanvasGraphIndex) -> Self {
        Self { document, index }
    }

    pub fn document(&self) -> &'a CanvasDocument {
        self.document
    }

    pub fn index(&self) -> &'a CanvasGraphIndex {
        self.index
    }

    pub fn node(&self, id: &NodeId) -> Option<&'a CanvasNode> {
        self.document.node(id)
    }

    pub fn edge(&self, id: &EdgeId) -> Option<&'a CanvasEdge> {
        self.document.edge(id)
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
        self.index
            .outgoing_edge_ids(node_id)
            .filter_map(|edge_id| self.document.edge(edge_id))
    }

    pub fn incoming_edges<'q>(
        &'q self,
        node_id: &'q NodeId,
    ) -> impl Iterator<Item = &'a CanvasEdge> + 'q
    where
        'a: 'q,
    {
        self.index
            .incoming_edge_ids(node_id)
            .filter_map(|edge_id| self.document.edge(edge_id))
    }

    pub fn incident_edges<'q>(
        &'q self,
        node_id: &'q NodeId,
    ) -> impl Iterator<Item = &'a CanvasEdge> + 'q
    where
        'a: 'q,
    {
        self.index
            .incident_edge_ids(node_id)
            .filter_map(|edge_id| self.document.edge(edge_id))
    }

    pub fn edges_for_node<'q>(
        &'q self,
        node_id: &'q NodeId,
        direction: CanvasEdgeDirection,
    ) -> Box<dyn Iterator<Item = &'a CanvasEdge> + 'q>
    where
        'a: 'q,
    {
        match direction {
            CanvasEdgeDirection::Incoming => Box::new(self.incoming_edges(node_id)),
            CanvasEdgeDirection::Outgoing => Box::new(self.outgoing_edges(node_id)),
            CanvasEdgeDirection::Any => Box::new(self.incident_edges(node_id)),
        }
    }

    pub fn edges_between<'q>(
        &'q self,
        source: &'q NodeId,
        target: &'q NodeId,
    ) -> impl Iterator<Item = &'a CanvasEdge> + 'q
    where
        'a: 'q,
    {
        self.index
            .edge_ids_between(source, target)
            .filter_map(|edge_id| self.document.edge(edge_id))
    }

    pub fn has_edge_between(&self, source: &NodeId, target: &NodeId) -> bool {
        self.index.has_edge_between(source, target)
    }

    pub fn neighbor_node_ids<'q>(
        &'q self,
        node_id: &'q NodeId,
        direction: CanvasEdgeDirection,
    ) -> Box<dyn Iterator<Item = &'a NodeId> + 'q>
    where
        'a: 'q,
    {
        match direction {
            CanvasEdgeDirection::Incoming => Box::new(
                self.index
                    .incoming_edge_ids(node_id)
                    .filter_map(move |edge_id| {
                        let endpoints = self.index.edge_endpoints(edge_id)?;
                        self.document
                            .contains_node(&endpoints.source)
                            .then_some(&endpoints.source)
                    }),
            ),
            CanvasEdgeDirection::Outgoing => Box::new(
                self.index
                    .outgoing_edge_ids(node_id)
                    .filter_map(move |edge_id| {
                        let endpoints = self.index.edge_endpoints(edge_id)?;
                        self.document
                            .contains_node(&endpoints.target)
                            .then_some(&endpoints.target)
                    }),
            ),
            CanvasEdgeDirection::Any => Box::new(self.index.incident_edge_ids(node_id).filter_map(
                move |edge_id| {
                    let endpoints = self.index.edge_endpoints(edge_id)?;
                    let neighbor_id = if endpoints.source == *node_id {
                        &endpoints.target
                    } else {
                        &endpoints.source
                    };
                    self.document
                        .contains_node(neighbor_id)
                        .then_some(neighbor_id)
                },
            )),
        }
    }

    pub fn incident_edge_count(&self, node_id: &NodeId) -> usize {
        self.index.incident_edge_count(node_id)
    }
}

fn remove_edge_from_node(
    edges_by_node: &mut IndexMap<NodeId, IndexSet<EdgeId>>,
    node_id: &NodeId,
    edge_id: &EdgeId,
) {
    let Some(edge_ids) = edges_by_node.get_mut(node_id) else {
        return;
    };
    edge_ids.shift_remove(edge_id);
    if edge_ids.is_empty() {
        edges_by_node.shift_remove(node_id);
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
    use crate::{
        CanvasHandle, CanvasTransaction, DocumentCommand, HandleRole,
        test_support::document_fixture,
    };
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

    #[test]
    fn graph_index_queries_match_scan_graph() {
        let document = sample_document();
        let graph = document.graph();
        let index = document.graph_index();
        let indexed = index.graph(&document);
        let a = NodeId::from("a");
        let b = NodeId::from("b");

        assert_eq!(index.edge_count(), 3);
        assert!(!index.is_empty());
        assert_eq!(
            index.edge_endpoints(&EdgeId::from("a-b")).cloned(),
            Some(CanvasGraphEndpointIds::new("a", "b"))
        );
        assert!(index.contains_edge(&EdgeId::from("a-b")));

        assert_eq!(
            edge_id_strings(index.outgoing_edge_ids(&a)),
            edge_ids(graph.outgoing_edges(&a))
        );
        assert_eq!(
            edge_id_strings(index.incoming_edge_ids(&a)),
            edge_ids(graph.incoming_edges(&a))
        );
        assert_eq!(
            edge_id_strings(index.incident_edge_ids(&a)),
            edge_ids(graph.incident_edges(&a))
        );
        assert_eq!(
            edge_id_strings(index.edge_ids_for_node(&a, CanvasEdgeDirection::Any)),
            edge_ids(graph.edges_for_node(&a, CanvasEdgeDirection::Any))
        );
        assert_eq!(
            edge_id_strings(index.edge_ids_between(&a, &b)),
            edge_ids(graph.edges_between(&a, &b))
        );
        assert_eq!(
            node_ids(index.neighbor_node_ids(&a, CanvasEdgeDirection::Any)),
            node_ids(graph.neighbor_node_ids(&a, CanvasEdgeDirection::Any))
        );
        assert_eq!(index.incident_edge_count(&a), graph.incident_edge_count(&a));

        assert_eq!(
            edge_ids(indexed.outgoing_edges(&a)),
            edge_ids(graph.outgoing_edges(&a))
        );
        assert_eq!(
            edge_ids(indexed.incoming_edges(&a)),
            edge_ids(graph.incoming_edges(&a))
        );
        assert_eq!(
            edge_ids(indexed.incident_edges(&a)),
            edge_ids(graph.incident_edges(&a))
        );
        assert_eq!(
            edge_ids(indexed.edges_for_node(&a, CanvasEdgeDirection::Any)),
            edge_ids(graph.edges_for_node(&a, CanvasEdgeDirection::Any))
        );
        assert_eq!(
            edge_ids(indexed.edges_between(&a, &b)),
            edge_ids(graph.edges_between(&a, &b))
        );
        assert_eq!(
            node_ids(indexed.neighbor_node_ids(&a, CanvasEdgeDirection::Any)),
            node_ids(graph.neighbor_node_ids(&a, CanvasEdgeDirection::Any))
        );
        assert!(indexed.has_edge_between(&a, &b));
        assert_eq!(
            indexed.incident_edge_count(&a),
            graph.incident_edge_count(&a)
        );
    }

    #[test]
    fn graph_index_applies_document_diffs_incrementally() {
        let mut document = sample_document();
        let mut index = document.graph_index();

        let diff = document
            .apply_transaction_with_diff(CanvasTransaction::single(DocumentCommand::InsertEdge(
                CanvasEdge::new(
                    "b-c",
                    CanvasEndpoint::new("b", None::<&str>),
                    CanvasEndpoint::new("c", None::<&str>),
                ),
            )))
            .unwrap();
        index.apply_diff(&document, &diff);
        assert_eq!(
            edge_id_strings(index.outgoing_edge_ids(&NodeId::from("b"))),
            vec!["b-c".to_string()]
        );
        assert_eq!(index, document.graph_index());

        let diff = document
            .apply_transaction_with_diff(CanvasTransaction::single(DocumentCommand::UpdateEdge(
                CanvasEdge::new(
                    "a-b",
                    CanvasEndpoint::new("b", None::<&str>),
                    CanvasEndpoint::new("c", None::<&str>),
                ),
            )))
            .unwrap();
        index.apply_diff(&document, &diff);
        assert!(!index.has_edge_between(&NodeId::from("a"), &NodeId::from("b")));
        assert_eq!(
            edge_id_strings(index.edge_ids_between(&NodeId::from("b"), &NodeId::from("c"))),
            vec!["a-b".to_string(), "b-c".to_string()]
        );
        assert_eq!(index, document.graph_index());

        let diff = document
            .apply_transaction_with_diff(CanvasTransaction::single(DocumentCommand::RemoveNode(
                NodeId::from("b"),
            )))
            .unwrap();
        index.apply_diff(&document, &diff);
        assert!(!index.contains_edge(&EdgeId::from("a-b")));
        assert!(!index.contains_edge(&EdgeId::from("b-c")));
        assert_eq!(index, document.graph_index());
    }

    #[test]
    fn graph_index_preserves_adjacency_when_edge_metadata_changes() {
        let mut document = sample_document();
        let mut index = document.graph_index();
        let mut edge = document.edge(&EdgeId::from("a-b")).unwrap().clone();
        edge.z_index = 42;

        let diff = document
            .apply_transaction_with_diff(CanvasTransaction::single(DocumentCommand::UpdateEdge(
                edge,
            )))
            .unwrap();
        index.apply_diff(&document, &diff);

        assert_eq!(
            edge_id_strings(index.outgoing_edge_ids(&NodeId::from("a"))),
            vec!["a-b".to_string(), "a-a".to_string()]
        );
        assert_eq!(index, document.graph_index());
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

        document_fixture()
            .node(a)
            .node(b)
            .node(c)
            .edge(CanvasEdge::new(
                "a-b",
                CanvasEndpoint::new("a", Some("out")),
                CanvasEndpoint::new("b", None::<&str>),
            ))
            .edge(CanvasEdge::new(
                "c-a",
                CanvasEndpoint::new("c", None::<&str>),
                CanvasEndpoint::new("a", Some("in")),
            ))
            .edge(CanvasEdge::new(
                "a-a",
                CanvasEndpoint::new("a", Some("out")),
                CanvasEndpoint::new("a", Some("in")),
            ))
            .build()
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

    fn edge_id_strings<'a>(ids: impl IntoIterator<Item = &'a EdgeId>) -> Vec<String> {
        ids.into_iter().map(|id| id.as_str().to_string()).collect()
    }
}
