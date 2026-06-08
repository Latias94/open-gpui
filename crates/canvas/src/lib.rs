//! Reusable canvas model and interaction primitives for Open GPUI.
//!
//! The crate keeps the document model separate from rendering. Applications can use
//! nodes, edges, shapes, handles, viewport transforms, hit testing, and tool state
//! transitions without committing to one GPUI element per canvas object.

mod changes;
mod document;
mod geometry;
mod gesture;
mod gpui;
mod graph;
mod index;
mod journal;
mod json_canvas;
mod persistence;
mod resolve;
mod routing;
mod runtime;
#[cfg(test)]
mod test_support;
mod tool;

pub use changes::{
    CanvasChangeOrigin, CanvasRecord, CanvasRecordChange, CanvasRecordOperation,
    CanvasRecordOperationBatch,
};
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
    CanvasInputMapper, CanvasPaintConnectionPreview, CanvasPaintFrame, CanvasPaintInteraction,
    CanvasPaintInteractionFrame, CanvasPaintModel, CanvasPaintOptions, CanvasPaintRecord,
    CanvasPaintTheme, canvas_view, collect_visible_records, paint_canvas_frame,
};
pub use graph::{
    CanvasEdgeDirection, CanvasGraph, CanvasGraphEndpointIds, CanvasGraphIndex, CanvasIndexedGraph,
};
pub use index::{CanvasSpatialIndex, HitOptions, HitRecord, HitTarget, SpatialIndex};
pub use journal::CanvasCommittedMutation;
pub use json_canvas::{
    JsonCanvas, JsonCanvasEdge, JsonCanvasEndpointShape, JsonCanvasError, JsonCanvasNode,
    JsonCanvasSide, document_from_json_canvas_str, document_to_json_canvas_string,
};
pub use persistence::{
    CANVAS_LORO_CRDT_FEATURE, CANVAS_PERSISTENCE_ADAPTERS, CANVAS_PERSISTENCE_CODEC_VERSION,
    CANVAS_REDB_STORE_FEATURE, CANVAS_RKYV_SNAPSHOT_FEATURE, CanvasCheckpoint,
    CanvasEncodedLogEntry, CanvasJsonPersistenceCodec, CanvasLogEntry, CanvasPersistenceAdapter,
    CanvasPersistenceAdapterStatus, CanvasPersistenceByteStore, CanvasPersistenceByteStoreAdapter,
    CanvasPersistenceByteStoreError, CanvasPersistenceCodec, CanvasPersistenceCodecError,
    CanvasPersistenceCursor, CanvasPersistenceEnvelope, CanvasPersistenceError,
    CanvasPersistenceRecord, CanvasPersistenceRecordKind, CanvasPersistenceStore,
    CanvasPersistentToolRegistryError, CanvasReplayError, MemoryCanvasPersistenceByteStore,
    MemoryCanvasPersistenceStore, apply_persistent_tool_effect, apply_persistent_tool_effects,
    apply_persistent_transaction, canvas_persistence_adapter_statuses, handle_persistent_event,
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
pub use tool::{
    CanvasEditor, CanvasEvent, CanvasHistory, CanvasKey, CanvasKeyModifiers, CanvasSelection,
    CanvasSelectionMode, CanvasTool, CanvasToolContext, CanvasToolEffect, CanvasToolId,
    CanvasToolReducer, CanvasToolRegistry, CanvasToolRegistryError, PointerButton, ToolState,
};
