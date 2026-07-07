use std::fmt;

use crate::record_scope::{
    CanvasRecordScopeOptions, include_internal_edges, resolve_selection_scope_with_predicates,
};
use crate::session::ToolState;
use crate::transform::{
    CanvasResizeHandle, CanvasTransformHandle, canvas_transform_handles, resize_bounds_by_handle,
};
use crate::{
    CanvasConnectionEndpointRole, CanvasDocument, CanvasEdgeRouter, CanvasEndpoint,
    CanvasGeometryFacts, CanvasHistory, CanvasKindRegistry, CanvasRecordGeometry, CanvasRecordId,
    CanvasRecordScope, CanvasRuntime, CanvasSelection, CanvasSelectionMode, CanvasTool,
    CanvasToolId, CanvasTransaction, CanvasViewport, DEFAULT_SNAP_THRESHOLD, DocumentCommand,
    DocumentError, EdgeId, HitOptions, HitRecord, HitTarget, NodeId, ShapeId,
    connection_hit_options, selection_record_scope, snap_delta_for_resize_selection,
    snap_delta_for_selection,
};
use indexmap::IndexSet;
use open_gpui::{Bounds, Pixels, Point, px, size};

mod connection;
mod pointer;
mod resize;
mod selection;
mod snap;

pub(crate) const RECONNECT_HANDLE_VIEW_SIZE: Pixels = px(20.0);

#[derive(Debug)]
pub(crate) struct CanvasResizeSelectionScope {
    pub(crate) node_ids: Vec<NodeId>,
    pub(crate) edge_ids: Vec<EdgeId>,
    pub(crate) shape_ids: Vec<ShapeId>,
    pub(crate) structural: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CanvasReconnectTarget {
    pub(crate) edge_id: EdgeId,
    pub(crate) endpoint: CanvasConnectionEndpointRole,
    pub(crate) fixed: CanvasEndpoint,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CanvasConnectionHit {
    Valid(CanvasEndpoint),
    Invalid,
    Empty,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CanvasPointerOwner {
    Reconnect(CanvasReconnectTarget),
    ConnectionSource(CanvasEndpoint),
    Transform(CanvasTransformHandle),
    NodeDrag(HitTarget),
    Record(HitTarget),
    Pane,
}

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
}
