use super::*;

impl CanvasToolReducerContext<'_> {
    pub(crate) fn resize_selection_scope(&self) -> CanvasResizeSelectionScope {
        let scope = resolve_selection_scope_with_predicates(
            self.document,
            self.selection,
            CanvasRecordScopeOptions::structural(),
            |record_id| self.document.contains_record(record_id),
            |record_id| self.is_resizable_record(record_id),
        );
        let structural = !scope.structural_records().is_empty();
        let mut records = scope
            .action_records()
            .records()
            .cloned()
            .collect::<IndexSet<_>>();

        if !structural {
            let node_ids = records
                .iter()
                .filter_map(|id| match id {
                    CanvasRecordId::Node(id) => Some(id.clone()),
                    CanvasRecordId::Edge(_) | CanvasRecordId::Shape(_) => None,
                })
                .collect();
            let shape_ids = records
                .iter()
                .filter_map(|id| match id {
                    CanvasRecordId::Shape(id) => Some(id.clone()),
                    CanvasRecordId::Node(_) | CanvasRecordId::Edge(_) => None,
                })
                .collect();
            return CanvasResizeSelectionScope {
                node_ids,
                edge_ids: Vec::new(),
                shape_ids,
                structural,
            };
        }

        include_internal_edges(self.document, &mut records, &mut |record_id| {
            self.is_resizable_record(record_id)
        });
        let node_ids = records
            .iter()
            .filter_map(|id| match id {
                CanvasRecordId::Node(id) => Some(id.clone()),
                CanvasRecordId::Edge(_) | CanvasRecordId::Shape(_) => None,
            })
            .collect();
        let edge_ids = records
            .iter()
            .filter_map(|id| match id {
                CanvasRecordId::Edge(id) => Some(id.clone()),
                CanvasRecordId::Node(_) | CanvasRecordId::Shape(_) => None,
            })
            .collect();
        let shape_ids = records
            .iter()
            .filter_map(|id| match id {
                CanvasRecordId::Shape(id) => Some(id.clone()),
                CanvasRecordId::Node(_) | CanvasRecordId::Edge(_) => None,
            })
            .collect();

        CanvasResizeSelectionScope {
            node_ids,
            edge_ids,
            shape_ids,
            structural,
        }
    }
    pub(crate) fn structural_resize_bounds(
        &self,
        node_ids: &[NodeId],
        shape_ids: &[ShapeId],
    ) -> Option<Bounds<Pixels>> {
        let mut records = node_ids
            .iter()
            .cloned()
            .map(CanvasRecordId::Node)
            .chain(shape_ids.iter().cloned().map(CanvasRecordId::Shape))
            .collect::<Vec<_>>();
        records.retain(|record_id| self.is_resizable_record(record_id));
        self.geometry_bounds_for_records(&records)
    }
    pub(crate) fn resize_selection_transaction(
        &self,
        handle: CanvasResizeHandle,
        delta: Point<Pixels>,
        node_ids: &[NodeId],
        edge_ids: &[EdgeId],
        shape_ids: &[ShapeId],
        structural: bool,
    ) -> Result<CanvasTransaction, DocumentError> {
        if delta.x == Pixels::ZERO && delta.y == Pixels::ZERO {
            return Ok(CanvasTransaction::default());
        }

        if structural {
            return self.resize_structural_selection_transaction(
                handle, delta, node_ids, edge_ids, shape_ids,
            );
        }

        let mut commands = Vec::new();
        for id in node_ids {
            let Some(node) = self.document.node(id) else {
                continue;
            };
            if node.locked {
                continue;
            }
            let mut node = node.clone();
            let proposed = resize_bounds_by_handle(node.bounds(), handle, delta);
            let bounds = self.kind_registry.resize_node_bounds(&node, proposed)?;
            node.position = bounds.origin;
            node.size = bounds.size;
            commands.push(DocumentCommand::UpdateNode(node));
        }

        for id in shape_ids {
            let Some(shape) = self.document.shape(id) else {
                continue;
            };
            if shape.locked {
                continue;
            }
            let mut shape = shape.clone();
            let proposed = resize_bounds_by_handle(shape.bounds, handle, delta);
            shape.bounds = self.kind_registry.resize_shape_bounds(&shape, proposed)?;
            commands.push(DocumentCommand::UpdateShape(shape));
        }

        for id in edge_ids {
            let Some(edge) = self.document.edge(id) else {
                continue;
            };
            if edge.locked {
                continue;
            }
            let mut edge = edge.clone();
            edge.route
                .waypoints
                .iter_mut()
                .for_each(|point| *point += delta);
            edge.route
                .control_points
                .iter_mut()
                .for_each(|point| *point += delta);
            commands.push(DocumentCommand::UpdateEdge(edge));
        }

        Ok(CanvasTransaction::new(commands))
    }
    fn resize_structural_selection_transaction(
        &self,
        handle: CanvasResizeHandle,
        delta: Point<Pixels>,
        node_ids: &[NodeId],
        edge_ids: &[EdgeId],
        shape_ids: &[ShapeId],
    ) -> Result<CanvasTransaction, DocumentError> {
        let Some(source_bounds) = self.structural_resize_bounds(node_ids, shape_ids) else {
            return Ok(CanvasTransaction::default());
        };
        let target_bounds = resize_bounds_by_handle(source_bounds, handle, delta);
        if source_bounds == target_bounds {
            return Ok(CanvasTransaction::default());
        }

        let mut commands = Vec::new();
        for id in node_ids {
            let Some(node) = self.document.node(id) else {
                continue;
            };
            if node.locked || node.hidden {
                continue;
            }
            let mut node = node.clone();
            let proposed = resize_bounds_within(source_bounds, target_bounds, node.bounds());
            let bounds = self.kind_registry.resize_node_bounds(&node, proposed)?;
            node.position = bounds.origin;
            node.size = bounds.size;
            commands.push(DocumentCommand::UpdateNode(node));
        }

        for id in shape_ids {
            let Some(shape) = self.document.shape(id) else {
                continue;
            };
            if shape.locked || shape.hidden {
                continue;
            }
            let mut shape = shape.clone();
            let proposed = resize_bounds_within(source_bounds, target_bounds, shape.bounds);
            shape.bounds = self.kind_registry.resize_shape_bounds(&shape, proposed)?;
            commands.push(DocumentCommand::UpdateShape(shape));
        }

        for id in edge_ids {
            let Some(edge) = self.document.edge(id) else {
                continue;
            };
            if edge.locked || edge.hidden {
                continue;
            }
            let mut edge = edge.clone();
            edge.route.waypoints = edge
                .route
                .waypoints
                .iter()
                .map(|point| resize_point_within(source_bounds, target_bounds, *point))
                .collect();
            edge.route.control_points = edge
                .route
                .control_points
                .iter()
                .map(|point| resize_point_within(source_bounds, target_bounds, *point))
                .collect();
            commands.push(DocumentCommand::UpdateEdge(edge));
        }

        Ok(CanvasTransaction::new(commands))
    }
    fn is_resizable_record(&self, record_id: &CanvasRecordId) -> bool {
        match record_id {
            CanvasRecordId::Node(id) => self
                .document
                .node(id)
                .is_some_and(|node| !node.locked && !node.hidden),
            CanvasRecordId::Shape(id) => self
                .document
                .shape(id)
                .is_some_and(|shape| !shape.locked && !shape.hidden),
            CanvasRecordId::Edge(id) => self
                .document
                .edge(id)
                .is_some_and(|edge| !edge.locked && !edge.hidden),
        }
    }
    fn geometry_bounds_for_records(&self, record_ids: &[CanvasRecordId]) -> Option<Bounds<Pixels>> {
        let facts = CanvasGeometryFacts::with_router_and_kind_registry(
            self.document,
            self.edge_router,
            Some(self.kind_registry),
        );
        let geometries = record_ids
            .iter()
            .filter_map(|record_id| facts.record_geometry(record_id))
            .filter(CanvasRecordGeometry::is_visible_unlocked);
        crate::geometry_facts::union_record_geometry_bounds(geometries)
    }
}

fn resize_bounds_within(
    source: Bounds<Pixels>,
    target: Bounds<Pixels>,
    bounds: Bounds<Pixels>,
) -> Bounds<Pixels> {
    Bounds::from_corners(
        resize_point_within(source, target, bounds.origin),
        resize_point_within(
            source,
            target,
            Point::new(
                bounds.origin.x + bounds.size.width,
                bounds.origin.y + bounds.size.height,
            ),
        ),
    )
}
fn resize_point_within(
    source: Bounds<Pixels>,
    target: Bounds<Pixels>,
    point: Point<Pixels>,
) -> Point<Pixels> {
    Point::new(
        resize_axis_within(
            source.origin.x,
            source.size.width,
            target.origin.x,
            target.size.width,
            point.x,
        ),
        resize_axis_within(
            source.origin.y,
            source.size.height,
            target.origin.y,
            target.size.height,
            point.y,
        ),
    )
}
fn resize_axis_within(
    source_origin: Pixels,
    source_size: Pixels,
    target_origin: Pixels,
    target_size: Pixels,
    value: Pixels,
) -> Pixels {
    if source_size == Pixels::ZERO {
        return target_origin;
    }

    let ratio = (value - source_origin).as_f32() / source_size.as_f32();
    target_origin + px(target_size.as_f32() * ratio)
}
