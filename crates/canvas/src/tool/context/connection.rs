use super::*;

impl CanvasToolReducerContext<'_> {
    pub(crate) fn connection_hit_at(
        &self,
        point: Point<Pixels>,
        role: CanvasConnectionEndpointRole,
    ) -> CanvasConnectionHit {
        let facts = CanvasGeometryFacts::with_router_and_kind_registry(
            self.document,
            self.edge_router,
            Some(self.kind_registry),
        );
        let records =
            self.runtime
                .precise_hit_test_with_facts(facts, point, connection_hit_options());

        for record in records {
            match &record.target {
                HitTarget::Handle { node_id, handle_id } => {
                    let Some(node) = self.document.node(node_id) else {
                        continue;
                    };
                    let Some(handle) = node.handle(Some(handle_id)) else {
                        continue;
                    };
                    if handle.is_pickable_connection_endpoint(role) {
                        return CanvasConnectionHit::Valid(CanvasEndpoint {
                            node_id: node_id.clone(),
                            handle_id: Some(handle_id.clone()),
                        });
                    }
                    return CanvasConnectionHit::Invalid;
                }
                HitTarget::Node(node_id) => {
                    let Some(node) = self.document.node(node_id) else {
                        continue;
                    };
                    if self
                        .kind_registry
                        .node_accepts_connection_endpoint(node, role)
                    {
                        return CanvasConnectionHit::Valid(CanvasEndpoint {
                            node_id: node_id.clone(),
                            handle_id: None,
                        });
                    }
                }
                HitTarget::Edge(_) | HitTarget::Shape(_) => {}
            }
        }

        CanvasConnectionHit::Empty
    }
    pub(crate) fn selected_reconnect_target_at(
        &self,
        point: Point<Pixels>,
    ) -> Option<CanvasReconnectTarget> {
        let facts = CanvasGeometryFacts::with_router_and_kind_registry(
            self.document,
            self.edge_router,
            Some(self.kind_registry),
        );
        let handle_size = size(
            RECONNECT_HANDLE_VIEW_SIZE * (1.0 / self.viewport.zoom),
            RECONNECT_HANDLE_VIEW_SIZE * (1.0 / self.viewport.zoom),
        );

        self.selection
            .selected_edges()
            .filter_map(|edge_id| {
                let edge = self.document.edge(edge_id)?;
                if edge.locked || edge.hidden {
                    return None;
                }
                let source = facts.endpoint_position(&edge.source).ok()?;
                let target_position = facts.endpoint_position(&edge.target).ok()?;
                let mut candidates = Vec::new();
                if Bounds::centered_at(source, handle_size).contains(&point) {
                    candidates.push((
                        distance_squared(point, source),
                        CanvasReconnectTarget {
                            edge_id: edge_id.clone(),
                            endpoint: CanvasConnectionEndpointRole::Source,
                            fixed: edge.target.clone(),
                        },
                    ));
                }
                if Bounds::centered_at(target_position, handle_size).contains(&point) {
                    candidates.push((
                        distance_squared(point, target_position),
                        CanvasReconnectTarget {
                            edge_id: edge_id.clone(),
                            endpoint: CanvasConnectionEndpointRole::Target,
                            fixed: edge.source.clone(),
                        },
                    ));
                }
                candidates
                    .into_iter()
                    .min_by(|(left, _), (right, _)| left.total_cmp(right))
                    .map(|(_, target)| target)
            })
            .next()
    }
    pub(crate) fn reconnect_edge_transaction(
        &self,
        edge_id: &EdgeId,
        endpoint: CanvasConnectionEndpointRole,
        candidate: CanvasEndpoint,
    ) -> Result<CanvasTransaction, DocumentError> {
        let Some(edge) = self.document.edge(edge_id) else {
            return Err(DocumentError::MissingEdge(edge_id.clone()));
        };
        if edge.locked || edge.hidden {
            return Ok(CanvasTransaction::default());
        }

        let mut edge = edge.clone();
        match endpoint {
            CanvasConnectionEndpointRole::Source => {
                if candidate == edge.source || candidate == edge.target {
                    return Ok(CanvasTransaction::default());
                }
                edge.source = candidate;
            }
            CanvasConnectionEndpointRole::Target => {
                if candidate == edge.target || candidate == edge.source {
                    return Ok(CanvasTransaction::default());
                }
                edge.target = candidate;
            }
        }

        Ok(CanvasTransaction::single(DocumentCommand::UpdateEdge(edge)))
    }
}

fn distance_squared(left: Point<Pixels>, right: Point<Pixels>) -> f32 {
    let dx = (left.x - right.x).as_f32();
    let dy = (left.y - right.y).as_f32();
    dx * dx + dy * dy
}
