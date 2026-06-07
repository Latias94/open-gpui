//! Reusable canvas model and interaction primitives for Open GPUI.
//!
//! The crate keeps the document model separate from rendering. Applications can use
//! nodes, edges, shapes, handles, viewport transforms, hit testing, and tool state
//! transitions without committing to one GPUI element per canvas object.

mod document;
mod geometry;
mod gpui;
mod index;
mod json_canvas;
mod persistence;
#[cfg(test)]
mod test_support;
mod tool;

pub use document::{
    CANVAS_DOCUMENT_FORMAT_VERSION, CanvasDocument, CanvasDocumentDiff, CanvasEdge,
    CanvasEdgeRoute, CanvasEdgeRouteKind, CanvasEndpoint, CanvasHandle, CanvasNode, CanvasRecordId,
    CanvasShape, CanvasSnapshot, CanvasStyle, CanvasTransaction, CanvasValue, DocumentCommand,
    DocumentError, EdgeId, HandleId, HandleRole, NodeId, ShapeId,
};
pub use geometry::{CanvasViewport, TransformError};
pub use gpui::{
    CanvasInputMapper, CanvasPaintConnectionPreview, CanvasPaintFrame, CanvasPaintInteraction,
    CanvasPaintInteractionFrame, CanvasPaintModel, CanvasPaintOptions, CanvasPaintRecord,
    CanvasPaintTheme, canvas_view, collect_visible_records, paint_canvas_frame,
};
pub use index::{HitOptions, HitRecord, HitTarget, SpatialIndex};
pub use json_canvas::{
    JsonCanvas, JsonCanvasEdge, JsonCanvasEndpointShape, JsonCanvasError, JsonCanvasNode,
    JsonCanvasSide, document_from_json_canvas_str, document_to_json_canvas_string,
};
pub use persistence::{
    CanvasCheckpoint, CanvasLogEntry, CanvasPersistenceError, CanvasPersistenceStore,
    CanvasReplayError, MemoryCanvasPersistenceStore, load_canvas_document, replay_canvas_log,
};
pub use tool::{
    CanvasEditor, CanvasEvent, CanvasHistory, CanvasSelection, CanvasTool, PointerButton, ToolState,
};
