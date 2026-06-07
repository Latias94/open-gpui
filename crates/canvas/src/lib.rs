//! Reusable canvas model and interaction primitives for Open GPUI.
//!
//! The crate keeps the document model separate from rendering. Applications can use
//! nodes, edges, shapes, handles, viewport transforms, hit testing, and tool state
//! transitions without committing to one GPUI element per canvas object.

mod document;
mod geometry;
mod index;
mod tool;

pub use document::{
    CANVAS_DOCUMENT_FORMAT_VERSION, CanvasDocument, CanvasDocumentDiff, CanvasEdge, CanvasEndpoint,
    CanvasHandle, CanvasNode, CanvasRecordId, CanvasShape, CanvasSnapshot, CanvasStyle,
    CanvasTransaction, CanvasValue, DocumentCommand, DocumentError, EdgeId, HandleId, HandleRole,
    NodeId, ShapeId,
};
pub use geometry::{CanvasViewport, TransformError};
pub use index::{HitOptions, HitRecord, HitTarget, SpatialIndex};
pub use tool::{
    CanvasEditor, CanvasEvent, CanvasHistory, CanvasSelection, CanvasTool, PointerButton, ToolState,
};
