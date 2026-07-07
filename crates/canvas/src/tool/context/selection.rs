use super::*;

impl CanvasToolReducerContext<'_> {
    pub(crate) fn delete_selection_transaction(&self) -> CanvasTransaction {
        let (node_ids, edge_ids, shape_ids) = self.deletable_selection_ids(self.selection);

        let mut commands = Vec::new();

        for id in edge_ids {
            let Some(edge) = self.document.edge(&id) else {
                continue;
            };
            if edge.locked
                || node_ids.contains(&edge.source.node_id)
                || node_ids.contains(&edge.target.node_id)
            {
                continue;
            }

            commands.push(DocumentCommand::RemoveEdge(id));
        }

        commands.extend(node_ids.iter().cloned().map(DocumentCommand::RemoveNode));

        for id in shape_ids {
            let Some(shape) = self.document.shape(&id) else {
                continue;
            };
            if shape.locked {
                continue;
            }

            commands.push(DocumentCommand::RemoveShape(id));
        }

        CanvasTransaction::new(commands)
    }
    fn deletable_selection_ids(
        &self,
        selection: &CanvasSelection,
    ) -> (IndexSet<NodeId>, IndexSet<EdgeId>, IndexSet<ShapeId>) {
        let records = self.selection_action_records(
            selection,
            CanvasRecordScopeOptions::structural(),
            |record_id| self.document.contains_record(record_id),
            |record_id| self.is_deletable_record(record_id),
        );

        let mut node_ids = IndexSet::new();
        let mut edge_ids = IndexSet::new();
        let mut shape_ids = IndexSet::new();
        for record in records {
            match record {
                CanvasRecordId::Node(id) => {
                    node_ids.insert(id);
                }
                CanvasRecordId::Edge(id) => {
                    edge_ids.insert(id);
                }
                CanvasRecordId::Shape(id) => {
                    shape_ids.insert(id);
                }
            }
        }

        (node_ids, edge_ids, shape_ids)
    }
    fn is_deletable_record(&self, record_id: &CanvasRecordId) -> bool {
        match record_id {
            CanvasRecordId::Node(id) => self.document.node(id).is_some_and(|node| !node.locked),
            CanvasRecordId::Edge(id) => self.document.edge(id).is_some_and(|edge| !edge.locked),
            CanvasRecordId::Shape(id) => self.document.shape(id).is_some_and(|shape| !shape.locked),
        }
    }
    pub(crate) fn translatable_selection_ids(
        &self,
        selection: &CanvasSelection,
    ) -> (Vec<NodeId>, Vec<ShapeId>) {
        let records = self.selection_action_records(
            selection,
            CanvasRecordScopeOptions::structural(),
            |record_id| self.document.contains_record(record_id),
            |record_id| self.is_translatable_record(record_id),
        );

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

        (node_ids, shape_ids)
    }
    pub(crate) fn selection_structurally_contains_target(&self, target: &HitTarget) -> bool {
        let Some(record_id) = record_id_for_selection_target(target) else {
            return false;
        };

        self.selection_structurally_contains_record(&record_id)
    }
    fn selection_structurally_contains_record(&self, record_id: &CanvasRecordId) -> bool {
        let scope = resolve_selection_scope_with_predicates(
            self.document,
            self.selection,
            CanvasRecordScopeOptions::structural(),
            |record_id| self.document.contains_record(record_id),
            |record_id| self.is_translatable_record(record_id),
        );
        scope.contains_action_record(record_id)
    }
    fn is_translatable_record(&self, record_id: &CanvasRecordId) -> bool {
        match record_id {
            CanvasRecordId::Node(id) => self.document.node(id).is_some_and(|node| !node.locked),
            CanvasRecordId::Shape(id) => self.document.shape(id).is_some_and(|shape| !shape.locked),
            CanvasRecordId::Edge(_) => false,
        }
    }
    pub(crate) fn selection_for_intersections_with_mode(
        &self,
        bounds: Bounds<Pixels>,
        mode: CanvasSelectionMode,
        base_selection: &CanvasSelection,
    ) -> CanvasSelection {
        let selection = self.selection_for_intersections(bounds);
        match mode {
            CanvasSelectionMode::Replace => selection,
            CanvasSelectionMode::Add => {
                let mut combined = base_selection.clone();
                combined.extend_selection(selection);
                combined
            }
        }
    }
    fn selection_for_intersections(&self, bounds: Bounds<Pixels>) -> CanvasSelection {
        let mut selection = CanvasSelection::default();
        let facts = CanvasGeometryFacts::with_router_and_kind_registry(
            self.document,
            self.edge_router,
            Some(self.kind_registry),
        );
        for record in self
            .runtime
            .query_with_options(bounds, HitOptions::default())
        {
            match &record.target {
                HitTarget::Node(_) | HitTarget::Edge(_) | HitTarget::Shape(_) => {
                    if facts.record_intersects_bounds(record, bounds, HitOptions::default()) {
                        selection.insert_target(record.target.clone());
                    }
                }
                HitTarget::Handle { .. } => {}
            }
        }
        selection
    }
    fn selection_action_records(
        &self,
        selection: &CanvasSelection,
        options: CanvasRecordScopeOptions,
        can_traverse: impl FnMut(&CanvasRecordId) -> bool,
        can_include: impl FnMut(&CanvasRecordId) -> bool,
    ) -> IndexSet<CanvasRecordId> {
        resolve_selection_scope_with_predicates(
            self.document,
            selection,
            options,
            can_traverse,
            can_include,
        )
        .into_action_records()
        .into_index_set()
    }
}

fn record_id_for_selection_target(target: &HitTarget) -> Option<CanvasRecordId> {
    match target {
        HitTarget::Node(id) => Some(CanvasRecordId::Node(id.clone())),
        HitTarget::Shape(id) => Some(CanvasRecordId::Shape(id.clone())),
        HitTarget::Edge(_) | HitTarget::Handle { .. } => None,
    }
}
