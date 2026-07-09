//! Reusable canvas model and interaction primitives for Open GPUI.
//!
//! The crate keeps the document model separate from rendering. Applications can use
//! nodes, edges, shapes, handles, viewport transforms, hit testing, and tool state
//! transitions without committing to one GPUI element per canvas object.

mod changes;
mod clipboard;
mod document;
mod format;
mod geometry;
mod geometry_facts;
mod gesture;
mod gpui;
mod graph;
mod index;
mod json_canvas;
mod layer;
mod mutation;
pub mod persistence;
#[cfg(test)]
mod public_surface_tests;
mod record_scope;
mod relations;
mod routing;
mod runtime;
mod runtime_query;
mod schema;
mod session;
mod snap;
mod spatial_cache;
mod store;
#[cfg(test)]
mod test_support;
mod tool;
mod transform;

pub mod adapter {
    //! GPUI adapter types and rendering helpers for canvas views.

    pub use crate::gpui::{
        CanvasConnectionPreviewRoute, CanvasEditorInputHandler, CanvasEditorInputMapper,
        CanvasInputMapper, CanvasPaintConnectionPreview, CanvasPaintConnectionTargetFeedback,
        CanvasPaintConnectionTargetState, CanvasPaintEdgeGeometry, CanvasPaintFrame,
        CanvasPaintInteraction, CanvasPaintInteractionFrame, CanvasPaintLabel, CanvasPaintModel,
        CanvasPaintOptions, CanvasPaintReconnectEndpoint, CanvasPaintReconnectHandle,
        CanvasPaintReconnectHandleShape, CanvasPaintRecord, CanvasPaintSnapGuide, CanvasPaintTheme,
        CanvasPaintTransformHandle, CanvasPaintWireVisualState, CanvasPreparedPaintFrame,
        CanvasSceneFrame, CanvasSceneLayerItem, CanvasSceneLayerPhase, CanvasSceneRecordGroup,
        CanvasWidgetOverlayFrame, CanvasWidgetOverlayHitPriority, CanvasWidgetOverlayOptions,
        CanvasWidgetOverlayPlacement, canvas_editor_scene_view_with_frame, canvas_editor_view,
        canvas_editor_view_with_frame, canvas_scene_view, canvas_view, collect_visible_records,
        collect_widget_overlay_frame, paint_canvas_frame, paint_canvas_scene_phase,
        prepaint_canvas_frame, prepare_canvas_frame, register_canvas_editor_input,
    };
}

pub mod advanced {
    //! Lower-level graph, geometry, routing, relation, mutation, and indexing APIs.

    pub use crate::changes::{
        CanvasChangeOrigin, CanvasRecord, CanvasRecordChange, CanvasRecordOperation,
        CanvasRecordOperationBatch, CanvasRelationChange, CanvasRelationOperation,
        CanvasRelationOperationBatch,
    };
    pub use crate::geometry_facts::{
        CanvasGeometryFacts, CanvasRecordGeometry, CanvasResolvedEdgeGeometry,
        connection_hit_options,
    };
    pub use crate::graph::{
        CanvasEdgeDirection, CanvasGraph, CanvasGraphEndpointIds, CanvasGraphIndex,
        CanvasIndexedGraph,
    };
    pub use crate::index::SpatialIndex;
    pub use crate::mutation::{CanvasCommittedMutation, CanvasPreparedMutation};
    pub use crate::record_scope::{
        CanvasRecordScope, CanvasRecordScopeOptions, CanvasResolvedSelectionScope,
        normalize_selection, resolve_selection_scope, selection_record_scope,
    };
    pub use crate::relations::{
        CanvasRecordBindingRelation, CanvasRecordGroupRelation, CanvasRecordParentRelation,
        CanvasRecordRelation, CanvasRecordRelationKey, CanvasRecordRelationKind,
        CanvasRecordRelations, CanvasRecordRelationsBuilder,
    };
    pub use crate::routing::{
        CanvasDefaultEdgeRouter, CanvasEdgeRouter, CanvasRoutePath, CanvasRouteRequest,
        CanvasRouteSegment,
    };
}

pub use clipboard::{CanvasClipboardPayload, CanvasPasteTransaction};
pub use document::{
    BindingId, CanvasConnectionEndpointRole, CanvasDocument, CanvasDocumentBuilder,
    CanvasDocumentDiff, CanvasEdge, CanvasEdgeRoute, CanvasEdgeRouteKind, CanvasEndpoint,
    CanvasHandle, CanvasNode, CanvasRecordId, CanvasShape, CanvasSnapshot, CanvasStyle,
    CanvasTransaction, CanvasValue, DocumentCommand, DocumentError, EdgeId, HandleId, HandleRole,
    NodeId, ShapeId,
};
pub use geometry::{CanvasViewport, TransformError};
pub use index::{HitOptions, HitRecord, HitTarget};
pub use json_canvas::{
    JsonCanvas, JsonCanvasEdge, JsonCanvasEndpointShape, JsonCanvasError, JsonCanvasNode,
    JsonCanvasSide, document_from_json_canvas_str, document_to_json_canvas_string,
};
pub use layer::CanvasZOrderCommand;
pub use runtime::CanvasRuntime;
pub use schema::{
    CanvasEdgeKind, CanvasEdgeRenderPolicy, CanvasEdgeSchemaPolicy, CanvasKindLabel,
    CanvasKindPaint, CanvasKindRegistry, CanvasNodeBoundsHitTest, CanvasNodeGeometryPolicy,
    CanvasNodeHitTest, CanvasNodeInteractionPolicy, CanvasNodeKind, CanvasNodeRenderPolicy,
    CanvasNodeResizeProposal, CanvasNodeSchemaPolicy, CanvasNodeTransformPolicy, CanvasRecordKind,
    CanvasSchemaError, CanvasShapeBoundsHitTest, CanvasShapeGeometryPolicy, CanvasShapeHitTest,
    CanvasShapeInteractionPolicy, CanvasShapeKind, CanvasShapeRenderPolicy,
    CanvasShapeResizeProposal, CanvasShapeSchemaPolicy, CanvasShapeTransformPolicy,
};
pub use snap::{
    CanvasSnapAxis, CanvasSnapGuide, CanvasSnapResult, DEFAULT_SNAP_THRESHOLD,
    snap_delta_for_resize_selection, snap_delta_for_selection,
};
pub use store::{
    CanvasStore, CanvasStoreChange, CanvasStoreHistoryEffect, CanvasStoreListenerId,
    CanvasStoreMutationSource,
};
pub use tool::{
    CanvasConnectedRelease, CanvasConnectionDragState, CanvasConnectionRejectReason,
    CanvasConnectionRelease, CanvasDroppedConnectionRelease, CanvasDroppedReconnectRelease,
    CanvasEditor, CanvasEvent, CanvasHistory, CanvasKey, CanvasKeyModifiers,
    CanvasReconnectedRelease, CanvasRejectedConnectionRelease, CanvasSelection,
    CanvasSelectionMode, CanvasTool, CanvasToolContext, CanvasToolId, CanvasToolIntent,
    CanvasToolReducer, CanvasToolRegistry, CanvasToolRegistryError, PointerButton,
};
pub use transform::{
    CanvasResizeHandle, CanvasTransformHandle, CanvasTransformTarget, canvas_transform_handles,
};
