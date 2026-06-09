//! Reusable canvas model and interaction primitives for Open GPUI.
//!
//! The crate keeps the document model separate from rendering. Applications can use
//! nodes, edges, shapes, handles, viewport transforms, hit testing, and tool state
//! transitions without committing to one GPUI element per canvas object.

mod changes;
mod clipboard;
mod document;
mod geometry;
mod gesture;
mod gpui;
mod graph;
#[doc(hidden)]
pub mod index;
mod json_canvas;
mod layer;
mod mutation;
mod persistence;
mod resolve;
mod routing;
mod runtime;
mod runtime_query;
mod schema;
mod snap;
mod spatial_cache;
#[cfg(test)]
mod test_support;
mod tool;
mod transform;

pub use changes::{
    CanvasChangeOrigin, CanvasRecord, CanvasRecordChange, CanvasRecordOperation,
    CanvasRecordOperationBatch,
};
pub use clipboard::{CanvasClipboardPayload, CanvasPasteTransaction};
pub use document::{
    CANVAS_DOCUMENT_FORMAT_VERSION, CANVAS_DOCUMENT_MIN_SUPPORTED_FORMAT_VERSION,
    CANVAS_SNAPSHOT_MIGRATIONS, CanvasConnectionEndpointRole, CanvasDocument, CanvasDocumentDiff,
    CanvasEdge, CanvasEdgeRoute, CanvasEdgeRouteKind, CanvasEndpoint, CanvasHandle, CanvasNode,
    CanvasRecordId, CanvasShape, CanvasSnapshot, CanvasSnapshotMigration, CanvasStyle,
    CanvasTransaction, CanvasValue, DocumentCommand, DocumentError, EdgeId, HandleId, HandleRole,
    NodeId, ShapeId, migrate_canvas_snapshot,
};
pub use geometry::{CanvasViewport, TransformError};
pub use gpui::{
    CanvasEditorInputHandler, CanvasEditorInputMapper, CanvasInputMapper,
    CanvasPaintConnectionPreview, CanvasPaintEdgeGeometry, CanvasPaintFrame,
    CanvasPaintInteraction, CanvasPaintInteractionFrame, CanvasPaintLabel, CanvasPaintModel,
    CanvasPaintOptions, CanvasPaintRecord, CanvasPaintSnapGuide, CanvasPaintTheme,
    CanvasPaintTransformHandle, CanvasPreparedPaintFrame, CanvasWidgetOverlayFrame,
    CanvasWidgetOverlayHitPriority, CanvasWidgetOverlayOptions, CanvasWidgetOverlayPlacement,
    canvas_editor_key_down_event, canvas_editor_view, canvas_editor_view_with_frame, canvas_view,
    collect_visible_records, collect_widget_overlay_frame, paint_canvas_frame,
    prepaint_canvas_frame, prepare_canvas_frame, register_canvas_editor_input,
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
pub use mutation::CanvasCommittedMutation;
pub use persistence::{
    CANVAS_LORO_CRDT_FEATURE, CANVAS_PERSISTENCE_ADAPTERS, CANVAS_PERSISTENCE_CODEC_VERSION,
    CANVAS_REDB_STORE_FEATURE, CANVAS_RKYV_SNAPSHOT_FEATURE, CanvasCheckpoint,
    CanvasEncodedLogEntry, CanvasJsonPersistenceCodec, CanvasLogEntry, CanvasLogEntryKind,
    CanvasPersistenceAdapter, CanvasPersistenceAdapterStatus, CanvasPersistenceByteStore,
    CanvasPersistenceByteStoreAdapter, CanvasPersistenceByteStoreError, CanvasPersistenceCodec,
    CanvasPersistenceCodecError, CanvasPersistenceCursor, CanvasPersistenceEnvelope,
    CanvasPersistenceError, CanvasPersistenceRecord, CanvasPersistenceRecordKind,
    CanvasPersistenceStore, CanvasPersistentToolRegistryError, CanvasReplayError,
    MemoryCanvasPersistenceByteStore, MemoryCanvasPersistenceStore, apply_persistent_tool_intent,
    apply_persistent_tool_intents, apply_persistent_transaction,
    canvas_persistence_adapter_statuses, handle_persistent_event,
    handle_persistent_event_with_custom_tool, handle_persistent_event_with_tool_registry,
    load_canvas_document, load_canvas_persistence_cursor, redo_persistent_transaction,
    replay_canvas_log, save_canvas_checkpoint, undo_persistent_transaction,
};
pub use resolve::{CanvasGeometryResolver, CanvasResolvedEdgeGeometry, connection_hit_options};
pub use routing::{
    CanvasDefaultEdgeRouter, CanvasEdgeRouter, CanvasRoutePath, CanvasRouteRequest,
    CanvasRouteSegment,
};
pub use runtime::CanvasRuntime;
pub use schema::{
    CanvasEdgeKind, CanvasKindLabel, CanvasKindPaint, CanvasKindRegistry, CanvasNodeHitTest,
    CanvasNodeKind, CanvasNodeResizeProposal, CanvasRecordKind, CanvasSchemaError,
    CanvasShapeHitTest, CanvasShapeKind, CanvasShapeResizeProposal,
};
pub use snap::{
    CanvasSnapAxis, CanvasSnapGuide, CanvasSnapResult, DEFAULT_SNAP_THRESHOLD,
    snap_delta_for_resize_selection, snap_delta_for_selection,
};
pub use tool::{
    CanvasEditor, CanvasEvent, CanvasHistory, CanvasKey, CanvasKeyModifiers, CanvasSelection,
    CanvasSelectionMode, CanvasTool, CanvasToolContext, CanvasToolId, CanvasToolIntent,
    CanvasToolReducer, CanvasToolRegistry, CanvasToolRegistryError, PointerButton,
};
pub use transform::{
    CanvasResizeHandle, CanvasTransformHandle, CanvasTransformTarget, canvas_transform_handles,
};
