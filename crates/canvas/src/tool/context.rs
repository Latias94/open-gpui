use std::fmt;

use crate::record_scope::{CanvasRecordScopeOptions, collect_selection_record_scope};
use crate::session::ToolState;
use crate::transform::{
    CanvasResizeHandle, CanvasTransformHandle, canvas_transform_handles, resize_bounds_by_handle,
};
use crate::{
    CanvasConnectionEndpointRole, CanvasDocument, CanvasEdgeRouter, CanvasEndpoint,
    CanvasGeometryFacts, CanvasHistory, CanvasKindRegistry, CanvasRecordId, CanvasRecordScope,
    CanvasRuntime, CanvasSelection, CanvasSelectionMode, CanvasTool, CanvasToolId,
    CanvasTransaction, CanvasViewport, DEFAULT_SNAP_THRESHOLD, DocumentCommand, DocumentError,
    EdgeId, HitOptions, HitRecord, HitTarget, NodeId, ShapeId, connection_hit_options,
    selection_record_scope, snap_delta_for_resize_selection, snap_delta_for_selection,
};
use indexmap::IndexSet;
use open_gpui::{Bounds, Pixels, Point};

#[derive(Clone, Copy)]
pub struct CanvasToolContext<'a> {
    pub(crate) document: &'a CanvasDocument,
    pub(crate) viewport: CanvasViewport,
    pub(crate) tool: &'a CanvasTool,
    pub(crate) runtime: &'a CanvasRuntime,
    pub(crate) edge_router: &'a (dyn CanvasEdgeRouter + Send + Sync),
    pub(crate) kind_registry: &'a CanvasKindRegistry,
    pub(crate) selection: &'a CanvasSelection,
    pub(crate) history: &'a CanvasHistory,
}

impl fmt::Debug for CanvasToolContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CanvasToolContext")
            .field("document", self.document)
            .field("viewport", &self.viewport)
            .field("tool", self.tool)
            .field("runtime", self.runtime)
            .field("edge_router", &"<dyn CanvasEdgeRouter>")
            .field("kind_registry", self.kind_registry)
            .field("selection", self.selection)
            .field("history", self.history)
            .finish()
    }
}

impl CanvasToolContext<'_> {
    pub fn document(&self) -> &CanvasDocument {
        self.document
    }

    pub fn viewport(&self) -> &CanvasViewport {
        &self.viewport
    }

    pub fn tool(&self) -> &CanvasTool {
        self.tool
    }

    pub fn runtime(&self) -> &CanvasRuntime {
        self.runtime
    }

    pub fn edge_router(&self) -> &(dyn CanvasEdgeRouter + Send + Sync) {
        self.edge_router
    }

    pub fn kind_registry(&self) -> &CanvasKindRegistry {
        self.kind_registry
    }

    pub fn selection(&self) -> &CanvasSelection {
        self.selection
    }

    pub fn history(&self) -> &CanvasHistory {
        self.history
    }

    pub fn active_custom_tool_id(&self) -> Option<&CanvasToolId> {
        self.tool.custom_id()
    }

    pub fn document_position(&self, view_position: Point<Pixels>) -> Point<Pixels> {
        self.viewport.view_to_document(view_position)
    }

    pub fn hit_test_view(
        &self,
        view_position: Point<Pixels>,
        options: HitOptions,
    ) -> impl Iterator<Item = &HitRecord> {
        self.runtime.precise_hit_test_with_kind_registry(
            self.document,
            self.kind_registry,
            self.document_position(view_position),
            options,
        )
    }

    pub fn selection_record_scope(&self, options: CanvasRecordScopeOptions) -> CanvasRecordScope {
        selection_record_scope(self.document, self.selection, options)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct CanvasToolReducerContext<'a> {
    pub(crate) document: &'a CanvasDocument,
    pub(crate) viewport: CanvasViewport,
    pub(crate) state: &'a ToolState,
    pub(crate) runtime: &'a CanvasRuntime,
    pub(crate) edge_router: &'a (dyn CanvasEdgeRouter + Send + Sync),
    pub(crate) kind_registry: &'a CanvasKindRegistry,
    pub(crate) selection: &'a CanvasSelection,
}

impl CanvasToolReducerContext<'_> {
    pub(crate) fn document(&self) -> &CanvasDocument {
        self.document
    }

    pub(crate) fn viewport(&self) -> CanvasViewport {
        self.viewport
    }

    pub(crate) fn state(&self) -> &ToolState {
        self.state
    }

    pub(crate) fn selection(&self) -> &CanvasSelection {
        self.selection
    }

    pub(crate) fn runtime(&self) -> &CanvasRuntime {
        self.runtime
    }

    pub(crate) fn kind_registry(&self) -> &CanvasKindRegistry {
        self.kind_registry
    }

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
        let records = self.document.relations().collect_related_records(
            selection
                .selected_nodes()
                .cloned()
                .map(CanvasRecordId::Node)
                .chain(
                    selection
                        .selected_edges()
                        .cloned()
                        .map(CanvasRecordId::Edge),
                )
                .chain(
                    selection
                        .selected_shapes()
                        .cloned()
                        .map(CanvasRecordId::Shape),
                ),
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
        let records = self.document.relations().collect_related_records(
            selection
                .selected_nodes()
                .cloned()
                .map(CanvasRecordId::Node)
                .chain(
                    selection
                        .selected_shapes()
                        .cloned()
                        .map(CanvasRecordId::Shape),
                ),
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
        collect_selection_record_scope(
            self.document,
            self.selection,
            CanvasRecordScopeOptions::structural(),
            |record_id| self.is_translatable_record(record_id),
        )
        .contains(record_id)
    }

    fn is_translatable_record(&self, record_id: &CanvasRecordId) -> bool {
        match record_id {
            CanvasRecordId::Node(id) => self.document.node(id).is_some_and(|node| !node.locked),
            CanvasRecordId::Shape(id) => self.document.shape(id).is_some_and(|shape| !shape.locked),
            CanvasRecordId::Edge(_) => false,
        }
    }

    pub(crate) fn transform_handle_at(
        &self,
        point: Point<Pixels>,
    ) -> Option<CanvasTransformHandle> {
        canvas_transform_handles(
            self.document,
            self.selection,
            self.viewport,
            Some(self.kind_registry),
        )
        .into_iter()
        .rev()
        .find(|handle| handle.document_bounds.contains(&point))
    }

    pub(crate) fn resizable_selection_ids(&self) -> (Vec<NodeId>, Vec<ShapeId>) {
        let node_ids = self
            .selection
            .selected_nodes()
            .filter_map(|id| self.document.node(id))
            .filter(|node| !node.locked && !node.hidden)
            .map(|node| node.id.clone())
            .collect();
        let shape_ids = self
            .selection
            .selected_shapes()
            .filter_map(|id| self.document.shape(id))
            .filter(|shape| !shape.locked && !shape.hidden)
            .map(|shape| shape.id.clone())
            .collect();

        (node_ids, shape_ids)
    }

    pub(crate) fn snap_delta_for_translation(
        &self,
        delta: Point<Pixels>,
        node_ids: &[NodeId],
        shape_ids: &[ShapeId],
    ) -> crate::snap::CanvasSnapResult {
        let mut selection = CanvasSelection::default();
        for id in node_ids {
            selection.insert_node(id.clone());
        }
        for id in shape_ids {
            selection.insert_shape(id.clone());
        }
        snap_delta_for_selection(
            self.document,
            &selection,
            delta,
            DEFAULT_SNAP_THRESHOLD,
            Some(self.kind_registry),
        )
    }

    pub(crate) fn snap_delta_for_resize(
        &self,
        handle: CanvasResizeHandle,
        delta: Point<Pixels>,
        node_ids: &[NodeId],
        shape_ids: &[ShapeId],
    ) -> crate::snap::CanvasSnapResult {
        let mut selection = CanvasSelection::default();
        for id in node_ids {
            selection.insert_node(id.clone());
        }
        for id in shape_ids {
            selection.insert_shape(id.clone());
        }
        snap_delta_for_resize_selection(
            self.document,
            &selection,
            handle,
            delta,
            DEFAULT_SNAP_THRESHOLD,
            Some(self.kind_registry),
        )
    }

    pub(crate) fn node_endpoint_at(
        &self,
        point: Point<Pixels>,
        role: CanvasConnectionEndpointRole,
    ) -> Option<CanvasEndpoint> {
        let facts = CanvasGeometryFacts::with_router_and_kind_registry(
            self.document,
            self.edge_router,
            Some(self.kind_registry),
        );
        let records =
            self.runtime
                .precise_hit_test_with_facts(facts, point, connection_hit_options());
        facts.connection_endpoint_at(records, role)
    }

    pub(crate) fn resize_selection_transaction(
        &self,
        handle: CanvasResizeHandle,
        delta: Point<Pixels>,
        node_ids: &[NodeId],
        shape_ids: &[ShapeId],
    ) -> Result<CanvasTransaction, DocumentError> {
        if delta.x == Pixels::ZERO && delta.y == Pixels::ZERO {
            return Ok(CanvasTransaction::default());
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

        Ok(CanvasTransaction::new(commands))
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
        for record in self
            .runtime
            .query_with_options(bounds, HitOptions::default())
        {
            match &record.target {
                HitTarget::Node(_) | HitTarget::Edge(_) | HitTarget::Shape(_) => {
                    selection.insert_target(record.target.clone());
                }
                HitTarget::Handle { .. } => {}
            }
        }
        selection
    }
}

fn record_id_for_selection_target(target: &HitTarget) -> Option<CanvasRecordId> {
    match target {
        HitTarget::Node(id) => Some(CanvasRecordId::Node(id.clone())),
        HitTarget::Shape(id) => Some(CanvasRecordId::Shape(id.clone())),
        HitTarget::Edge(_) | HitTarget::Handle { .. } => None,
    }
}
