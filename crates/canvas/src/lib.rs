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
#[doc(hidden)]
pub mod index;
mod json_canvas;
mod layer;
mod mutation;
mod persistence;
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

pub use changes::{
    CanvasChangeOrigin, CanvasRecord, CanvasRecordChange, CanvasRecordOperation,
    CanvasRecordOperationBatch, CanvasRelationChange, CanvasRelationOperation,
    CanvasRelationOperationBatch,
};
pub use clipboard::{CanvasClipboardPayload, CanvasPasteTransaction};
pub use document::{
    BindingId, CanvasConnectionEndpointRole, CanvasDocument, CanvasDocumentBuilder,
    CanvasDocumentDiff, CanvasEdge, CanvasEdgeRoute, CanvasEdgeRouteKind, CanvasEndpoint,
    CanvasHandle, CanvasNode, CanvasRecordId, CanvasShape, CanvasSnapshot, CanvasStyle,
    CanvasTransaction, CanvasValue, DocumentCommand, DocumentError, EdgeId, HandleId, HandleRole,
    NodeId, ShapeId,
};
pub use format::{
    CANVAS_DOCUMENT_FORMAT_VERSION, CANVAS_DOCUMENT_MIN_SUPPORTED_FORMAT_VERSION,
    CANVAS_SNAPSHOT_MIGRATIONS, CanvasSnapshotMigration, default_document_format_version,
    migrate_canvas_snapshot, validate_canvas_document_format_version,
};
pub use geometry::{CanvasViewport, TransformError};
pub use geometry_facts::{
    CanvasGeometryFacts, CanvasRecordGeometry, CanvasResolvedEdgeGeometry, connection_hit_options,
};
pub use gpui::{
    CanvasEditorInputHandler, CanvasEditorInputMapper, CanvasInputMapper,
    CanvasPaintConnectionPreview, CanvasPaintEdgeGeometry, CanvasPaintFrame,
    CanvasPaintInteraction, CanvasPaintInteractionFrame, CanvasPaintLabel, CanvasPaintModel,
    CanvasPaintOptions, CanvasPaintRecord, CanvasPaintSnapGuide, CanvasPaintTheme,
    CanvasPaintTransformHandle, CanvasPreparedPaintFrame, CanvasWidgetOverlayFrame,
    CanvasWidgetOverlayHitPriority, CanvasWidgetOverlayOptions, CanvasWidgetOverlayPlacement,
    canvas_editor_view, canvas_editor_view_with_frame, canvas_view, collect_visible_records,
    collect_widget_overlay_frame, paint_canvas_frame, prepaint_canvas_frame, prepare_canvas_frame,
    register_canvas_editor_input,
};
pub use graph::{
    CanvasEdgeDirection, CanvasGraph, CanvasGraphEndpointIds, CanvasGraphIndex, CanvasIndexedGraph,
};
pub use index::{HitOptions, HitRecord, HitTarget};
pub use json_canvas::{
    JsonCanvas, JsonCanvasEdge, JsonCanvasEndpointShape, JsonCanvasError, JsonCanvasNode,
    JsonCanvasSide, document_from_json_canvas_str, document_to_json_canvas_string,
};
pub use layer::CanvasZOrderCommand;
pub use mutation::{CanvasCommittedMutation, CanvasPreparedMutation};
pub use persistence::{
    CANVAS_LORO_CRDT_FEATURE, CANVAS_PERSISTENCE_ADAPTERS, CANVAS_PERSISTENCE_CODEC_VERSION,
    CANVAS_REDB_STORE_FEATURE, CANVAS_RKYV_SNAPSHOT_FEATURE, CanvasCheckpoint,
    CanvasEncodedLogEntry, CanvasJsonPersistenceCodec, CanvasLogEntry, CanvasLogEntryKind,
    CanvasPersistenceAdapter, CanvasPersistenceAdapterStatus, CanvasPersistenceByteStore,
    CanvasPersistenceByteStoreAdapter, CanvasPersistenceByteStoreError, CanvasPersistenceCodec,
    CanvasPersistenceCodecError, CanvasPersistenceCursor, CanvasPersistenceEnvelope,
    CanvasPersistenceError, CanvasPersistenceRecord, CanvasPersistenceRecordKind,
    CanvasPersistenceStore, CanvasPersistentToolRegistryError, CanvasReplayError,
    MemoryCanvasPersistenceByteStore, MemoryCanvasPersistenceStore,
    apply_persistent_store_transaction, apply_persistent_tool_intent,
    apply_persistent_tool_intents, apply_persistent_transaction,
    canvas_persistence_adapter_statuses, handle_persistent_event,
    handle_persistent_event_with_custom_tool, handle_persistent_event_with_tool_registry,
    load_canvas_document, load_canvas_persistence_cursor, redo_persistent_store_transaction,
    redo_persistent_transaction, replay_canvas_log, save_canvas_checkpoint,
    save_canvas_store_checkpoint, undo_persistent_store_transaction, undo_persistent_transaction,
};
pub use record_scope::{CanvasRecordScope, CanvasRecordScopeOptions, selection_record_scope};
pub use relations::{
    CanvasRecordBindingRelation, CanvasRecordGroupRelation, CanvasRecordParentRelation,
    CanvasRecordRelation, CanvasRecordRelationKey, CanvasRecordRelationKind, CanvasRecordRelations,
    CanvasRecordRelationsBuilder,
};
pub use routing::{
    CanvasDefaultEdgeRouter, CanvasEdgeRouter, CanvasRoutePath, CanvasRouteRequest,
    CanvasRouteSegment,
};
pub use runtime::CanvasRuntime;
pub use schema::{
    CanvasEdgeKind, CanvasEdgeRenderPolicy, CanvasEdgeSchemaPolicy, CanvasKindLabel,
    CanvasKindPaint, CanvasKindRegistry, CanvasNodeGeometryPolicy, CanvasNodeHitTest,
    CanvasNodeInteractionPolicy, CanvasNodeKind, CanvasNodeRenderPolicy, CanvasNodeResizeProposal,
    CanvasNodeSchemaPolicy, CanvasNodeTransformPolicy, CanvasRecordKind, CanvasSchemaError,
    CanvasShapeGeometryPolicy, CanvasShapeHitTest, CanvasShapeInteractionPolicy, CanvasShapeKind,
    CanvasShapeRenderPolicy, CanvasShapeResizeProposal, CanvasShapeSchemaPolicy,
    CanvasShapeTransformPolicy,
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
    CanvasEditor, CanvasEvent, CanvasHistory, CanvasKey, CanvasKeyModifiers, CanvasSelection,
    CanvasSelectionMode, CanvasTool, CanvasToolContext, CanvasToolId, CanvasToolIntent,
    CanvasToolReducer, CanvasToolRegistry, CanvasToolRegistryError, PointerButton,
};
pub use transform::{
    CanvasResizeHandle, CanvasTransformHandle, CanvasTransformTarget, canvas_transform_handles,
};
