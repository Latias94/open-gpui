# Open GPUI Canvas

`open-gpui-canvas` is a reusable infinite-canvas foundation for Open GPUI applications. It is
designed for node graphs, mind maps, whiteboards, JSON Canvas import/export, and xyflow-style
handles without adopting DOM-style per-node rendering.

The crate is still pre-1.0. The core API is intentionally small and favors stable document,
command, query, tool, and persistence boundaries over early feature breadth.

## Design

- `CanvasDocument` stores nodes, edges, and shapes as separate record collections.
- `CanvasNode` owns position, size, z-index, payload data, style, flags, and invisible handles.
- `CanvasEdge` references source and target endpoints by node ID plus optional handle ID.
- `CanvasGraph` provides zero-copy graph queries over the canonical document records.
- `SpatialIndex` supports hit testing and visible-record culling without one GPUI element per
  canvas object.
- `CanvasEditor` applies transactions, tracks undo/redo, maintains selection, and dispatches tool
  events.
- `CanvasToolEffect` is the mutation vocabulary shared by built-in tools and application-defined
  custom tools.
- `CanvasPersistenceStore` defines checkpoint plus ordered transaction-log replay without pulling
  redb, Loro, or rkyv into the default build.

## Build A Document

```rust
use open_gpui::{point, px, size};
use open_gpui_canvas::{
    CanvasDocument, CanvasEdge, CanvasEndpoint, CanvasHandle, CanvasNode, DocumentError,
};

fn build_document() -> Result<CanvasDocument, DocumentError> {
    let mut source = CanvasNode::new(
        "source",
        point(px(0.0), px(0.0)),
        size(px(160.0), px(80.0)),
    );
    source
        .handles
        .push(CanvasHandle::new("out", point(px(160.0), px(40.0))));

    let mut target = CanvasNode::new(
        "target",
        point(px(260.0), px(0.0)),
        size(px(160.0), px(80.0)),
    );
    target
        .handles
        .push(CanvasHandle::new("in", point(px(0.0), px(40.0))));

    let mut document = CanvasDocument::default();
    document.insert_node(source)?;
    document.insert_node(target)?;
    document.insert_edge(CanvasEdge::new(
        "source-target",
        CanvasEndpoint::new("source", Some("out")),
        CanvasEndpoint::new("target", Some("in")),
    ))?;
    Ok(document)
}
```

## Query Graph Structure

```rust
use open_gpui_canvas::{CanvasEdgeDirection, NodeId};

fn inspect(document: &open_gpui_canvas::CanvasDocument) {
    let graph = document.graph();
    let source = NodeId::from("source");

    let outgoing_count = graph.outgoing_edges(&source).count();
    let neighbor_ids = graph
        .neighbor_node_ids(&source, CanvasEdgeDirection::Outgoing)
        .map(|id| id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(outgoing_count, neighbor_ids.len());
}
```

`CanvasGraph` is scan-based today. If graph traversal becomes hot in real applications, an
incremental adjacency index can be built from `CanvasDocumentDiff` without changing document
serialization.

## Render Through GPUI

The default adapter snapshots document state into `CanvasPaintModel`, culls visible records through
`SpatialIndex`, and paints the resulting frame through GPUI's low-level canvas callback.

```rust
use open_gpui_canvas::{CanvasPaintModel, CanvasPaintOptions, CanvasPaintTheme, canvas_view};

fn view(document: open_gpui_canvas::CanvasDocument) {
    let model = CanvasPaintModel::new(document, Default::default());
    let element = canvas_view(model, CanvasPaintOptions::default(), CanvasPaintTheme::default());
    let _ = element;
}
```

Applications may still layer selected node widgets or text editors on top of this batched base
renderer. The core path does not require one GPUI element per canvas record.

## Add A Custom Tool

Custom tools read editor state through `CanvasToolContext` and return `CanvasToolEffect` values.
They do not receive `&mut CanvasEditor`, so undo, selection pruning, spatial-index updates,
persistence, and future CRDT translation keep passing through one mutation path.

```rust
use open_gpui::{px, size};
use open_gpui_canvas::{
    CanvasEvent, CanvasNode, CanvasTool, CanvasToolContext, CanvasToolEffect, CanvasToolReducer,
    CanvasTransaction, DocumentCommand, DocumentError, NodeId, PointerButton,
};

struct StampTool;

impl CanvasToolReducer for StampTool {
    fn handle_event(
        &mut self,
        context: CanvasToolContext<'_>,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolEffect>, DocumentError> {
        let CanvasEvent::PointerDown {
            position,
            button: PointerButton::Primary,
        } = event
        else {
            return Ok(Vec::new());
        };

        let node = CanvasNode::new(
            NodeId::from("stamp"),
            context.document_position(position),
            size(px(120.0), px(64.0)),
        );

        Ok(vec![
            CanvasToolEffect::ApplyTransaction(CanvasTransaction::single(
                DocumentCommand::InsertNode(node),
            )),
            CanvasToolEffect::SetTool(CanvasTool::Select),
        ])
    }
}
```

Register application tools with `CanvasToolRegistry`, then call
`CanvasEditor::handle_event_with_tool_registry`.

## JSON Canvas

JSON Canvas is treated as an interchange format, not the canonical storage model.

```rust
use open_gpui_canvas::{JsonCanvas, document_from_json_canvas_str};

let json = r#"{"nodes":[],"edges":[]}"#;
let document = document_from_json_canvas_str(json).unwrap();
let exported = JsonCanvas::from_document(&document).unwrap().to_string_pretty().unwrap();
assert!(exported.contains("nodes"));
```

Text, file, link, and group nodes are mapped into `CanvasNode` records. Edge sides become
deterministic node handles so Obsidian-style connections remain round-trippable.

## Persistence

The default crate ships only the persistence contract and an in-memory store.

```rust
use open_gpui_canvas::{
    CanvasCheckpoint, CanvasDocument, CanvasNode, CanvasPersistenceCursor,
    CanvasPersistenceStore, CanvasTransaction, DocumentCommand, MemoryCanvasPersistenceStore,
    apply_persistent_tool_effect, apply_persistent_transaction, handle_persistent_event,
    load_canvas_document, save_canvas_checkpoint,
};

let document = CanvasDocument::default();
let mut store = MemoryCanvasPersistenceStore::default();
store.save_checkpoint(CanvasCheckpoint::new(1, &document)).unwrap();

let restored = load_canvas_document(&store).unwrap();
assert_eq!(restored.nodes.len(), 0);

let mut editor = open_gpui_canvas::CanvasEditor::new(restored);
let mut cursor = CanvasPersistenceCursor::new(1);
apply_persistent_transaction(
    &mut editor,
    &mut store,
    &mut cursor,
    CanvasTransaction::single(DocumentCommand::InsertNode(CanvasNode::new(
        "note",
        open_gpui::point(open_gpui::px(0.0), open_gpui::px(0.0)),
        open_gpui::size(open_gpui::px(120.0), open_gpui::px(64.0)),
    ))),
)
.unwrap();

save_canvas_checkpoint(&editor, &mut store, &cursor).unwrap();
```

For tool reducers, use `apply_persistent_tool_effect` or `apply_persistent_tool_effects` so
recorded transactions enter the log and unrecorded gesture updates stay transient until committed.
Applications that want one entrypoint can dispatch normalized canvas events through
`handle_persistent_event`, `handle_persistent_event_with_custom_tool`, or
`handle_persistent_event_with_tool_registry`; those helpers reduce the active tool to effects, log
recorded transactions, apply transient updates in memory, and leave concrete storage ownership in
the application.

Feature names are reserved for future adapters:

- `redb-store`
- `loro-crdt`
- `rkyv-snapshot`

Those features currently describe capability boundaries. Concrete adapters should remain
feature-gated and should not become default dependencies.

## Relationship To Reference Projects

The model borrows proven ideas from xyflow's separated nodes/edges/handles data design and
tldraw's explicit tool-state mindset. It intentionally does not copy xyflow's DOM rendering layer;
large Open GPUI canvases should use retained records, culling, hit testing, and batched paint.
