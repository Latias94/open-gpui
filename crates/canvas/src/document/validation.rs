use super::*;

impl CanvasDocument {
    pub fn validate_endpoint(&self, endpoint: &CanvasEndpoint) -> Result<(), DocumentError> {
        self.endpoint_parts(endpoint)?;
        Ok(())
    }

    pub fn validate_edge(&self, edge: &CanvasEdge) -> Result<(), DocumentError> {
        Self::validate_edge_route(edge)?;
        self.validate_source_endpoint(&edge.source)?;
        self.validate_target_endpoint(&edge.target)?;
        Ok(())
    }

    pub fn validate_integrity(&self) -> Result<(), DocumentError> {
        for node in self.nodes.values() {
            Self::validate_node(node)?;
        }

        for edge in self.edges.values() {
            self.validate_edge(edge)?;
        }

        self.validate_relations()?;

        Ok(())
    }

    pub(super) fn validate_record_id(&self, id: &CanvasRecordId) -> Result<(), DocumentError> {
        if self.contains_record(id) {
            Ok(())
        } else {
            Err(DocumentError::MissingRelationRecord(id.clone()))
        }
    }

    pub(super) fn record_id_set(&self) -> IndexSet<CanvasRecordId> {
        self.nodes
            .keys()
            .cloned()
            .map(CanvasRecordId::Node)
            .chain(self.edges.keys().cloned().map(CanvasRecordId::Edge))
            .chain(self.shapes.keys().cloned().map(CanvasRecordId::Shape))
            .collect()
    }

    pub(super) fn validate_node(node: &CanvasNode) -> Result<(), DocumentError> {
        let mut handle_ids = IndexSet::new();
        for handle in &node.handles {
            if !handle_ids.insert(handle.id.clone()) {
                return Err(DocumentError::DuplicateHandle {
                    node_id: node.id.clone(),
                    handle_id: handle.id.clone(),
                });
            }
        }

        Ok(())
    }

    fn validate_edge_route(edge: &CanvasEdge) -> Result<(), DocumentError> {
        if edge.route.kind.as_str().trim().is_empty() {
            return Err(DocumentError::EmptyEdgeRouteKind(edge.id.clone()));
        }

        if !edge.route.interaction_width.as_f32().is_finite()
            || edge.route.interaction_width < Pixels::ZERO
        {
            return Err(DocumentError::InvalidEdgeInteractionWidth(edge.id.clone()));
        }

        for point in edge
            .route
            .waypoints
            .iter()
            .chain(edge.route.control_points.iter())
        {
            if !point.x.as_f32().is_finite() || !point.y.as_f32().is_finite() {
                return Err(DocumentError::InvalidEdgeRoutePoint(edge.id.clone()));
            }
        }

        Ok(())
    }

    fn validate_source_endpoint(&self, endpoint: &CanvasEndpoint) -> Result<(), DocumentError> {
        let Some(handle) = self.endpoint_parts(endpoint)?.1 else {
            return Ok(());
        };
        self.validate_connectable_handle(endpoint, handle)?;

        if handle.role == HandleRole::Target {
            return Err(DocumentError::InvalidSourceHandle {
                node_id: endpoint.node_id.clone(),
                handle_id: handle.id.clone(),
            });
        }

        Ok(())
    }

    fn validate_target_endpoint(&self, endpoint: &CanvasEndpoint) -> Result<(), DocumentError> {
        let Some(handle) = self.endpoint_parts(endpoint)?.1 else {
            return Ok(());
        };
        self.validate_connectable_handle(endpoint, handle)?;

        if handle.role == HandleRole::Source {
            return Err(DocumentError::InvalidTargetHandle {
                node_id: endpoint.node_id.clone(),
                handle_id: handle.id.clone(),
            });
        }

        Ok(())
    }

    fn validate_connectable_handle(
        &self,
        endpoint: &CanvasEndpoint,
        handle: &CanvasHandle,
    ) -> Result<(), DocumentError> {
        if !handle.connectable {
            return Err(DocumentError::NonConnectableHandle {
                node_id: endpoint.node_id.clone(),
                handle_id: handle.id.clone(),
            });
        }

        Ok(())
    }

    fn endpoint_parts(
        &self,
        endpoint: &CanvasEndpoint,
    ) -> Result<(&CanvasNode, Option<&CanvasHandle>), DocumentError> {
        let node = self
            .nodes
            .get(&endpoint.node_id)
            .ok_or_else(|| DocumentError::MissingNode(endpoint.node_id.clone()))?;

        let Some(handle_id) = &endpoint.handle_id else {
            return Ok((node, None));
        };

        let handle = node
            .handle(Some(handle_id))
            .ok_or_else(|| DocumentError::MissingHandle {
                node_id: endpoint.node_id.clone(),
                handle_id: handle_id.clone(),
            })?;

        Ok((node, Some(handle)))
    }
}
