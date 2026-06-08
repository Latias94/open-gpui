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
- Handles can be hidden, non-connectable, source-only, target-only, or bidirectional.
  `CanvasConnectionEndpointRole` and `CanvasHandle` helpers share endpoint-role semantics between
  built-in tools, custom tools, and rendering adapters.
- Hidden and non-connectable handles stay out of default hit testing. The connect tool respects
  source and target roles while picking endpoints, and connection previews snap to valid target
  endpoints.
- `CanvasGraph` provides zero-copy graph queries over the canonical document records.
- `CanvasRuntime` owns runtime caches for spatial hit testing and indexed graph queries. The
  underlying `SpatialIndex` still exposes the replaceable query boundary for future R-tree, tile,
  or GPU-assisted indexes.
- `CanvasGeometryResolver` centralizes record bounds, handle positions, route paths, edge bounds,
  hit areas, endpoint picking, previews, and paint fallback geometry.
- `CanvasKindRegistry` lets applications register node, edge, and shape kind handlers for
  defaults, migrations, validation, and geometry hooks while unknown kinds remain open records.
- Locked records remain visible for culling and painting, but default hit testing and selection
  skip them unless `HitOptions::include_locked` is enabled.
- `CanvasEditor` owns document mutation, undo/redo, selection, gestures, runtime cache sync, edge
  router policy, and kind registry policy behind explicit methods.
- `CanvasEvent` normalizes pointer, wheel, key, and cancel events; pointer and key events carry
  modifiers, and the select tool can delete editable selections with Delete or Backspace through
  the same transaction path as other edits.
- `CanvasInputMapper` maps GPUI Escape key presses to `CanvasEvent::Cancel`, giving tools a
  renderer-neutral cancellation path instead of treating Escape as ordinary text input.
- `CanvasInputMapper::key_down_event` lets focus-owning widgets dispatch keyboard input without a
  canvas-local bounds mapper; the native smoke example forwards Delete, Backspace, and Escape this
  way.
- `CanvasToolEffect` is the mutation vocabulary shared by built-in tools and application-defined
  custom tools, including replace/add/remove/toggle selection effects for multi-select workflows.
- The built-in select tool supports shift-click selection toggling through the same incremental
  selection effect path that custom tools can use.
- The built-in select tool also supports shift-drag additive marquee selection, seeded from the
  drag start selection so box selection can grow a baseline set without accumulating during move
  events.
- Pointing and box-selection gestures snapshot the starting selection and restore it on cancel,
  keeping transient selection changes out of the committed editor state.
- Idle Escape cancels also clear the current selection, so the same key exits both active gestures
  and passive multi-selection states.
- The built-in select tool uses pointer-move modifiers for shift-constrained node translation,
  locking to the first shifted move's dominant axis while the modifier remains held.
- `CanvasPersistenceStore` defines checkpoint plus ordered transaction-log replay without pulling
  redb, Loro, or rkyv into the default build.
- `CanvasPersistenceCodec` and `CanvasPersistenceByteStore` separate typed canvas records from
  encoded bytes so local databases and zero-copy snapshot formats can plug in later.

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

`CanvasGraph` is scan-based and zero-cache. For hot graph traversal, build a
`CanvasGraphIndex` explicitly and keep it in sync with `CanvasDocumentDiff`.

```rust
use open_gpui_canvas::{CanvasEdgeDirection, CanvasGraphIndex, NodeId};

fn inspect_with_index(document: &open_gpui_canvas::CanvasDocument) {
    let index = CanvasGraphIndex::rebuild(document);
    let graph = index.graph(document);
    let source = NodeId::from("source");

    let outgoing_count = graph.outgoing_edges(&source).count();
    let neighbor_ids = graph
        .neighbor_node_ids(&source, CanvasEdgeDirection::Outgoing)
        .map(|id| id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(outgoing_count, neighbor_ids.len());
}
```

The index is an application-owned cache. It preserves document edge order, deduplicates self-loop
incident edges, and can apply diffs without changing document serialization.

## Inspect Record Changes

`DocumentCommand` remains the canonical replay vocabulary. Commands and transactions still expose
an ordered intent view, but sync, audit, and CRDT adapters should prefer committed mutations when
they need the actual semantic changes produced by document rules such as incident edge removal.

```rust
use open_gpui_canvas::{
    CanvasDocument, CanvasNode, CanvasRecordChange, CanvasRecordOperationBatch, CanvasTransaction,
    DocumentCommand,
};
use open_gpui::{point, px, size};

fn inspect(transaction: &CanvasTransaction) {
    for change in transaction.record_changes() {
        let id = change.id();
        let is_delete = matches!(change, CanvasRecordChange::Delete(_));
        let _ = (id, is_delete);
    }

    let batch = CanvasRecordOperationBatch::new(7, transaction).with_origin("local-client");
    for operation in batch.operations {
        let ordered_key = (operation.transaction_sequence, operation.operation_index);
        let _ = (ordered_key, operation.id());
    }
}

let mut document = CanvasDocument::default();
let committed = document
    .commit_transaction(CanvasTransaction::single(DocumentCommand::InsertNode(
        CanvasNode::new("note", point(px(0.0), px(0.0)), size(px(120.0), px(64.0))),
    )))
    .unwrap();
let actual_batch = committed.record_operation_batch(8);
assert_eq!(actual_batch.operations.len(), 1);
```

## Register Canvas Kinds

The persisted model stays open: `kind` is still a string and payload data is still JSON-shaped
`CanvasValue`. Applications that need stronger contracts can install a `CanvasKindRegistry`.
Registered handlers can migrate older data, add defaults, validate records, and override geometry
used by mutation, runtime indexes, hit testing, previews, and GPUI paint.

Unknown kinds are intentionally left unchanged so imported JSON Canvas files, application-specific
records, and future ecosystem extensions can still be loaded before a handler exists.

```rust
use open_gpui::{Bounds, Pixels, Point, point, px, size};
use open_gpui_canvas::{
    CanvasDocument, CanvasKindRegistry, CanvasNode, CanvasNodeKind, CanvasRecordKind,
    CanvasSchemaError, CanvasTransaction, CanvasValue, DocumentCommand, NodeId,
};
use serde_json::{Value, json};

struct NoteKind;

impl CanvasNodeKind for NoteKind {
    fn default_data(&self) -> CanvasValue {
        CanvasValue::from_iter([("title".to_string(), json!("Untitled"))])
    }

    fn migrate_node(&self, node: &mut CanvasNode) -> Result<(), CanvasSchemaError> {
        if let Some(label) = node.data.remove("label") {
            node.data.insert("title".to_string(), label);
        }
        Ok(())
    }

    fn validate_node(&self, node: &CanvasNode) -> Result<(), CanvasSchemaError> {
        match node.data.get("title") {
            Some(Value::String(title)) if !title.trim().is_empty() => Ok(()),
            None => Err(CanvasSchemaError::missing_required_data(
                CanvasRecordKind::Node,
                node.id.clone(),
                &node.kind,
                "title",
            )),
            Some(_) => Err(CanvasSchemaError::invalid_data(
                CanvasRecordKind::Node,
                node.id.clone(),
                &node.kind,
                "title must be a non-empty string",
            )),
        }
    }

    fn node_bounds(&self, node: &CanvasNode) -> Option<Bounds<Pixels>> {
        Some(node.bounds().dilate(px(8.0)))
    }

    fn handle_position(
        &self,
        node: &CanvasNode,
        handle_id: &open_gpui_canvas::HandleId,
    ) -> Option<Point<Pixels>> {
        (handle_id.as_str() == "out")
            .then(|| point(node.position.x + node.size.width + px(16.0), node.position.y))
    }
}

let mut registry = CanvasKindRegistry::open();
registry.register_node_kind("note", NoteKind);

let mut node = CanvasNode::new("note-1", point(px(0.0), px(0.0)), size(px(160.0), px(72.0)));
node.kind = "note".to_string();
node.data.insert("label".to_string(), json!("Migrated title"));

let mut document = CanvasDocument::default();
document
    .commit_transaction_with_kind_registry(
        CanvasTransaction::single(DocumentCommand::InsertNode(node)),
        &registry,
    )
    .unwrap();

assert_eq!(
    document.nodes[&NodeId::from("note-1")].data.get("title"),
    Some(&json!("Migrated title"))
);
```

Use `CanvasDocument::from_snapshot_with_kind_registry` to normalize and validate a snapshot at load
time. Use `CanvasEditor::try_new_with_kind_registry` or `CanvasEditor::set_kind_registry` when the
interactive editor should apply the same registry to transactions, gestures, undo/redo validation,
runtime caches, and paint snapshots.

## Route Edges

`CanvasEdgeRoute` stores route intent. `CanvasDefaultEdgeRouter` turns straight, polyline,
orthogonal, and cubic-bezier intent into renderer-neutral `CanvasRouteSegment` values that hit
testing and GPUI painting can share.

```rust
use open_gpui_canvas::{CanvasDefaultEdgeRouter, CanvasEdgeRouter};

fn route(document: &open_gpui_canvas::CanvasDocument, edge: &open_gpui_canvas::CanvasEdge) {
    let path = document.edge_route_path_with_router(edge, &CanvasDefaultEdgeRouter).unwrap();
    assert!(!path.segments.is_empty());
}
```

The default orthogonal route uses simple midpoint doglegs and optional waypoints. Applications can
provide their own `CanvasEdgeRouter` for obstacle-aware, port-aware, or preview routes without
changing `CanvasEdgeRoute` serialization.

## Render Through GPUI

The default adapter snapshots document state and runtime caches into `CanvasPaintModel`, culls
visible records through `CanvasRuntime`, and paints the resulting frame through GPUI's low-level
canvas callback.

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

## Large Canvas Baseline

The crate includes a focused stress regression for the default GPUI culling path and a Criterion
benchmark for larger documents. The regression builds a 12,288-node document and verifies that a
paint frame only carries visible records. The benchmark builds a 20,000-node graph with horizontal
edges and measures spatial-index rebuild, visible query, and paint-frame culling.

```sh
cargo nextest run -p open-gpui-canvas gpui::tests::collect_visible_records_keeps_large_canvas_frame_bounded
cargo bench -p open-gpui-canvas --bench large_canvas
```

Use this before and after replacing `SpatialIndex` with an R-tree, tile index, or GPU-assisted
culling adapter. The important signal is not the absolute number on one machine; it is whether
large documents continue to route rendering work through visible-record culling instead of
per-record GPUI elements.

## Add A Custom Tool

Custom tools read editor state through `CanvasToolContext` and return `CanvasToolEffect` values.
They do not receive `&mut CanvasEditor`, so undo, selection pruning, runtime-cache updates,
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
            ..
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
Unknown JSON Canvas payload fields are preserved in record data when possible, and unknown canvas
record kinds remain loadable through the open kind registry.

## Persistence

The default crate ships only the persistence contract and an in-memory store.

```rust
use open_gpui_canvas::{
    CanvasCheckpoint, CanvasDocument, CanvasNode, CanvasPersistenceCursor,
    CanvasPersistenceStore, CanvasTransaction, DocumentCommand, MemoryCanvasPersistenceStore,
    apply_persistent_tool_effect, apply_persistent_transaction, handle_persistent_event,
    load_canvas_document, redo_persistent_transaction, save_canvas_checkpoint,
    undo_persistent_transaction,
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

When an editor is attached to a store, call `undo_persistent_transaction` and
`redo_persistent_transaction` instead of `CanvasEditor::undo` / `CanvasEditor::redo`. Those helpers
append the document-changing transaction before mutating the editor, so store failures do not leave
the in-memory document ahead of the replay log.

For byte-oriented stores, wrap a `CanvasPersistenceByteStore` with
`CanvasPersistenceByteStoreAdapter`. The default `CanvasJsonPersistenceCodec` writes an explicit
envelope containing the codec version, document format version, record kind, sequence, and typed
payload.

```rust
use open_gpui_canvas::{
    CanvasCheckpoint, CanvasDocument, CanvasPersistenceByteStoreAdapter,
    CanvasPersistenceStore, MemoryCanvasPersistenceByteStore,
};

let mut store =
    CanvasPersistenceByteStoreAdapter::new(MemoryCanvasPersistenceByteStore::default());
store
    .save_checkpoint(CanvasCheckpoint::new(0, &CanvasDocument::default()))
    .unwrap();
```

For tool reducers, use `apply_persistent_tool_effect` or `apply_persistent_tool_effects` so
recorded transactions enter the log and gesture updates stay transient until `CommitGesture`.
Gesture sessions begin with `BeginGesture`, update the in-memory document with `UpdateGesture`,
and commit or cancel without asking tool authors to construct inverse transactions by hand.
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
