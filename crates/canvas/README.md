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
- `CanvasRecordRelations` stores typed parent and group membership facts between any canvas
  records, giving future frame, group, and layout features a shared structural layer instead of
  hiding those relationships in arbitrary payload data.
- Handles can be hidden, non-connectable, source-only, target-only, or bidirectional.
  `CanvasConnectionEndpointRole` and `CanvasHandle` helpers share endpoint-role semantics between
  built-in tools, custom tools, and rendering adapters.
- Hidden and non-connectable handles stay out of default hit testing. The connect tool respects
  source and target roles while picking endpoints, and connection previews snap to valid target
  endpoints.
- `CanvasGraph` provides zero-copy graph queries over the canonical document records.
- `CanvasRuntime` owns runtime caches for spatial hit testing, edge geometry, and indexed graph
  queries. Its runtime query module keeps final filtering, ordering, stale-record suppression, and
  precise hit testing inside canvas-owned code; future R-tree, tile, or GPU-assisted indexes should
  act as coarse candidate providers.
- `CanvasStore` owns the canonical document, runtime cache, history, edge router, kind registry, and
  committed change feed. No-op and failed mutations do not advance history or notify listeners.
- `CanvasDocumentBuilder` is the explicit construction path for snapshots, imports, examples, and
  fixtures. It validates records and relation facts without publishing edit history or store
  changes.
- `CanvasGeometryFacts` centralizes record bounds, handle positions, route paths, edge bounds,
  hit areas, endpoint picking, previews, and paint fallback geometry.
- `CanvasKindRegistry` lets applications register node, edge, and shape kind policy bundles for
  schema, geometry, interaction, render, and transform hooks while unknown kinds remain open
  records.
- Locked records remain visible for culling and painting, but default hit testing and selection
  skip them unless `HitOptions::include_locked` is enabled.
- `CanvasEditor` is the ergonomic editing facade over `CanvasStore` plus a crate-private
  `CanvasEditorSession`. Durable document changes pass through the store mutation path; viewport,
  selection, active tool state, and gesture baselines stay in the ephemeral session boundary.
- `CanvasEditor` exposes command methods for delete, copy, cut, paste, duplicate, undo, redo, and
  z-order changes so applications do not need to mutate document collections directly.
- `CanvasTransformHandle` and `CanvasResizeHandle` describe selected-record resize affordances in
  interaction snapshots. They are hit targets and paint feedback, not persisted document records.
- `CanvasSnapGuide` records transient alignment feedback for move and resize gestures. Snapping
  adjusts the proposed transaction while the document stores only final positions and bounds.
- `CanvasEvent` normalizes pointer, wheel, key, and cancel events; pointer and key events carry
  modifiers, and the select tool can delete editable selections with Delete or Backspace through
  the same transaction path as other edits.
- `CanvasInputMapper` maps GPUI Escape key presses to `CanvasEvent::Cancel`, giving tools a
  renderer-neutral cancellation path instead of treating Escape as ordinary text input.
- `CanvasInputMapper::key_down_event` lets focus-owning widgets dispatch keyboard input without a
  canvas-local bounds mapper; the native smoke example forwards Delete, Backspace, and Escape this
  way.
- Built-in tool state machines read through a crate-private reducer context and emit effects that
  the editor routes through store or session paths. `CanvasToolIntent` remains the public
  custom-tool vocabulary for document, selection, viewport, and tool-mode changes.
- The built-in select tool supports shift-click selection toggling through the same selection
  semantics exposed to custom tools as intents.
- The built-in select tool also supports shift-drag additive marquee selection, seeded from the
  drag start selection so box selection can grow a baseline set without accumulating during move
  events.
- Pointing and box-selection gestures snapshot the starting selection and restore it on cancel,
  keeping transient selection changes out of the committed editor state.
- Idle Escape cancels also clear the current selection, so the same key exits both active gestures
  and passive multi-selection states.
- The built-in select tool uses pointer-move modifiers for shift-constrained node and shape
  translation, locking to the first shifted move's dominant axis while the modifier remains held.
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

    let mut builder = CanvasDocument::builder();
    builder.add_node(source)?;
    builder.add_node(target)?;
    builder.add_edge(CanvasEdge::new(
        "source-target",
        CanvasEndpoint::new("source", Some("out")),
        CanvasEndpoint::new("target", Some("in")),
    ))?;
    builder.build()
}
```

Use `CanvasDocumentBuilder` when creating a document from snapshots, import formats, examples, or
fixtures. Use `CanvasDocument::apply_transaction`, `CanvasStore`, or `CanvasEditor` when modeling an
edit to an existing document, because those paths produce committed mutation facts, inverse
transactions, runtime-cache updates, history entries, persistence log records, and listener
notifications as appropriate.

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

`CanvasGraph` is scan-based and zero-cache. For hot graph traversal that is independent of
`CanvasEditor`, build a `CanvasGraphIndex` explicitly and keep it in sync with
`CanvasDocumentDiff`. Application editing paths should prefer `CanvasRuntime`, which keeps graph,
spatial, and edge-geometry caches behind one owner.

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

## Model Record Relationships

Use `CanvasRecordRelations` when a relationship is part of the canvas structure rather than a
kind-specific payload. Parent and group membership relations are expressed with `CanvasRecordId`,
so nodes, edges, and shapes can participate in the same future frame, group, mind-map, or layout
ownership model.

```rust
use open_gpui::{point, px, size};
use open_gpui_canvas::{
    CanvasDocument, CanvasNode, CanvasRecordId, CanvasShape, CanvasTransaction, DocumentCommand,
    NodeId, ShapeId,
};

let note = CanvasRecordId::Node(NodeId::from("note"));
let frame = CanvasRecordId::Shape(ShapeId::from("frame"));

let mut document = CanvasDocument::default();
document
    .apply_transaction(CanvasTransaction::new([
        DocumentCommand::InsertShape(CanvasShape::new(
            "frame",
            open_gpui::bounds(point(px(0.0), px(0.0)), size(px(320.0), px(240.0))),
        )),
        DocumentCommand::InsertNode(CanvasNode::new(
            "note",
            point(px(40.0), px(40.0)),
            size(px(120.0), px(64.0)),
        )),
        DocumentCommand::SetRecordParent {
            child: note.clone(),
            parent: frame.clone(),
        },
        DocumentCommand::AddRecordToGroup {
            group: frame.clone(),
            member: note.clone(),
        },
    ]))
    .unwrap();

assert_eq!(document.relations().parent_of(&note), Some(&frame));
```

Relation commands validate that both endpoints exist and reject self-parenting. The mutation
journal prunes relations that point at deleted records, including edges removed implicitly by node
deletion, so persistence, undo/redo, runtime sync, and future CRDT adapters see the same committed
structural facts. This slice does not implement group editing tools, frame layout, clipping, or
parent-relative transforms yet.

`CanvasRecordRelation` is the unified relation record vocabulary for parent and group membership
facts. `CanvasRecordRelationsBuilder` is the construction path for clipboard payloads, imports, and
fixtures that need to assemble a relation set without treating each relation as an editor mutation.
Future edge bindings, frame containment, or layout ownership should extend this relationship layer
as first-class records rather than hiding cross-record structure in `CanvasValue`.

## Edit Through Commands

Applications should route product editing actions through `CanvasEditor` methods. The editor
delegates durable document mutation, undo/redo, runtime cache updates, kind validation, and committed
change notification to `CanvasStore`, while viewport, selection, active tool state, and gesture
baselines live in its crate-private `CanvasEditorSession`.
Copy, cut, paste, and duplicate preserve internal edges plus internal parent/group relations when
both relationship endpoints are included in the clipboard payload; relationships to records outside
the payload are intentionally omitted.

```rust
use open_gpui::{point, px};
use open_gpui_canvas::{
    CanvasEditor, CanvasNode, CanvasSelection, CanvasZOrderCommand, DocumentError, NodeId,
};

fn edit_selection(editor: &mut CanvasEditor) -> Result<(), DocumentError> {
    editor.duplicate_selection(point(px(24.0), px(24.0)))?;
    editor.reorder_selection(CanvasZOrderCommand::BringToFront)?;

    if let Some(payload) = editor.copy_selection() {
        editor.paste_clipboard(&payload, point(px(48.0), px(48.0)))?;
    }

    Ok(())
}
```

## Inspect Record Changes

`DocumentCommand` remains the canonical replay vocabulary. Commands and transactions still expose
an ordered intent view, but sync, audit, and CRDT adapters should prefer committed mutations when
they need the actual semantic changes produced by document rules such as incident edge removal.
The mutation journal is the fact source for those committed mutations: it prepares a normalized
transaction against a draft, validates the result, derives the inverse transaction, and records the
actual semantic diff. It also reports committed relation changes, so parent/group updates and
relations pruned by record deletion are observable without re-diffing full snapshots.
`CanvasStore` wraps each non-empty committed mutation in a `CanvasStoreChange` with source metadata,
history effect, and post-commit document/runtime snapshots. Store listeners are synchronous and only
observe fully applied changes. Failed transactions, empty transactions, transient gesture updates,
and relation-order-only no-ops do not notify listeners or push history. `CanvasEditor::listen`
delegates to the same store feed for editor-backed applications.

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
let relation_batch = committed.relation_operation_batch(8);
assert!(relation_batch.operations.is_empty());
```

## Register Canvas Kinds

The persisted model stays open: `kind` is still a string and payload data is still JSON-shaped
`CanvasValue`. Applications that need stronger contracts can install a `CanvasKindRegistry`.
Registered kind bundles can attach focused schema, geometry, interaction, render, and transform
policies. The registry facade is what mutation, runtime indexes, hit testing, previews, resize
gestures, and GPUI paint consume.

Unknown kinds are intentionally left unchanged so imported JSON Canvas files, application-specific
records, and future ecosystem extensions can still be loaded before a handler exists.

```rust
use open_gpui::{Bounds, Pixels, Point, point, px, size};
use open_gpui_canvas::{
    CanvasDocument, CanvasKindLabel, CanvasKindPaint, CanvasKindRegistry, CanvasNode,
    CanvasNodeGeometryPolicy, CanvasNodeKind, CanvasNodeRenderPolicy, CanvasNodeResizeProposal,
    CanvasNodeSchemaPolicy, CanvasNodeTransformPolicy, CanvasRecordKind, CanvasSchemaError,
    CanvasTransaction, CanvasValue, DocumentCommand, NodeId,
};
use serde_json::{Value, json};

struct NoteKind;

impl CanvasNodeSchemaPolicy for NoteKind {
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
}

impl CanvasNodeGeometryPolicy for NoteKind {
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

impl CanvasNodeTransformPolicy for NoteKind {
    fn resize_node_bounds(
        &self,
        proposal: CanvasNodeResizeProposal<'_>,
    ) -> Result<Bounds<Pixels>, CanvasSchemaError> {
        Ok(Bounds::new(
            proposal.bounds.origin,
            size(
                proposal.bounds.size.width.max(px(120.0)),
                proposal.bounds.size.height.max(px(64.0)),
            ),
        ))
    }
}

impl CanvasNodeRenderPolicy for NoteKind {
    fn node_paint(&self, _node: &CanvasNode) -> Option<CanvasKindPaint> {
        Some(CanvasKindPaint {
            fill: Some("#fff8c5".to_string()),
            stroke: Some("#bf8700".to_string()),
            stroke_width: Some(px(2.0)),
            corner_radius: Some(px(8.0)),
        })
    }

    fn node_label(&self, node: &CanvasNode) -> Option<CanvasKindLabel> {
        let title = node.data.get("title")?.as_str()?;
        Some(
            CanvasKindLabel::new(title)
                .with_inset(px(8.0))
                .with_color("#24292f"),
        )
    }
}

let mut registry = CanvasKindRegistry::open();
registry.register_node_kind(
    "note",
    CanvasNodeKind::new()
        .with_schema_policy(NoteKind)
        .with_geometry_policy(NoteKind)
        .with_transform_policy(NoteKind)
        .with_render_policy(NoteKind),
);

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
    document
        .node(&NodeId::from("note-1"))
        .unwrap()
        .data
        .get("title"),
    Some(&json!("Migrated title"))
);
```

Use `CanvasDocument::from_snapshot_with_kind_registry` to normalize and validate a snapshot at load
time. Snapshot loading and JSON Canvas import use the construction path rather than store changes;
the resulting document is already valid, but no edit history or listeners are produced. Use
`CanvasEditor::try_new_with_kind_registry` or `CanvasEditor::set_kind_registry` when the interactive
editor should apply the same registry to transactions, gestures, undo/redo validation, runtime
caches, and paint snapshots.

Node, edge, and shape render policies can all return renderer-neutral `CanvasKindPaint` defaults.
Node and shape render policies can also return `CanvasKindLabel` metadata for paint-frame snapshots.
Record style fields still win first, then kind defaults, then the active paint theme.

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
canvas callback. `CanvasPaintModel` owns a consistent document, runtime, kind-registry, viewport,
and interaction snapshot; applications construct it from a document or `CanvasEditor` instead of
assembling those parts by hand.

The public `gpui` facade is intentionally thin. Internally, the adapter keeps model snapshots,
input mapping, paint-frame construction, style fallback resolution, low-level GPUI painting, and
view helper wiring in separate modules. That split preserves batched paint while leaving room for
custom painters, text overlays, and sparse selected-record widgets without turning the document
model into a GPUI element tree.

Editor-backed applications should start with `canvas_editor_view` or
`canvas_editor_view_with_frame`. These helpers keep canvas-bounds-dependent GPUI input wiring inside
the canvas adapter: mouse, wheel, focus, and drag-capture events become renderer-neutral
`CanvasEvent` values, and the application decides how to apply those events to its editor.
Keyboard events are owned by the focused element and should be forwarded explicitly through
`canvas_editor_key_down_event` or `CanvasEditorInputHandler::dispatch_key_down`.

```rust
use open_gpui_canvas::{
    CanvasEditorInputHandler, CanvasPaintModel, CanvasPaintOptions, CanvasPaintTheme,
    canvas_editor_key_down_event, canvas_editor_view,
};

fn render_canvas(this: &MyView, cx: &mut open_gpui::Context<MyView>) -> impl open_gpui::IntoElement {
    let model = CanvasPaintModel::from(&this.editor);
    canvas_editor_view(
        model,
        cx.entity(),
        this.focus_handle.clone(),
        CanvasEditorInputHandler::new(
            |view: &MyView| !view.editor.is_tool_state_idle(),
            |view, event, cx| {
                view.editor.handle_event(event).ok();
                cx.notify();
            },
        ),
        CanvasPaintOptions::default(),
        CanvasPaintTheme::default(),
    )
}

fn handle_key_down(view: &mut MyView, event: &open_gpui::KeyDownEvent) {
    let _ = view.editor.handle_event(canvas_editor_key_down_event(event));
}
```

Custom renderers and read-only previews can still use the lower-level `canvas_view` path directly.

```rust
use open_gpui_canvas::{CanvasPaintModel, CanvasPaintOptions, CanvasPaintTheme, canvas_view};

fn preview(document: open_gpui_canvas::CanvasDocument) {
    let model = CanvasPaintModel::new(document, Default::default());
    let element = canvas_view(model, CanvasPaintOptions::default(), CanvasPaintTheme::default());
    let _ = element;
}
```

Applications may still layer selected node widgets or text editors on top of this batched base
renderer. The core path does not require one GPUI element per canvas record.

For rich node content, derive a sparse widget overlay frame from the same paint frame instead of
querying the document a second time. Overlay placements carry only target identity, document/view
bounds, z-order, and hit priority; application widget state stays outside the canvas document.

```rust
use open_gpui_canvas::{
    CanvasPaintFrame, CanvasWidgetOverlayOptions, CanvasWidgetOverlayPlacement,
};

fn selected_note_widgets(frame: &CanvasPaintFrame) -> Vec<CanvasWidgetOverlayPlacement> {
    frame
        .widget_overlay_frame(CanvasWidgetOverlayOptions::selected_nodes())
        .placements
}
```

Widget event handlers should route edits back through `CanvasEditor` APIs, `DocumentCommand`
transactions, or custom `CanvasToolIntent` values. Treat overlay placement as layout data, not as a
second mutation path.

Run the native note-map example to see JSON Canvas import, kind registry labels, resize policy,
selection, batched paint, and sparse selected-node overlay placement together:

```sh
cargo run -p open-gpui-canvas-notes
```

## Large Canvas Baseline

The crate includes a focused stress regression for the default GPUI culling path and a Criterion
benchmark for larger documents. The regression builds a 12,288-node document and verifies that a
paint frame only carries visible records. The benchmark builds a 20,000-node graph with horizontal
edges and measures spatial-index rebuild, visible query, and paint-frame culling.

```sh
cargo nextest run -p open-gpui-canvas gpui::tests::collect_visible_records_keeps_large_canvas_frame_bounded
cargo bench -p open-gpui-canvas --bench large_canvas
```

Use this before and after changing the runtime candidate cache, such as replacing the internal base
with an R-tree, tile index, packed AABB index, or GPU-assisted culling adapter. The important signal
is not the absolute number on one machine; it is whether large documents continue to route
rendering work through visible-record culling instead of per-record GPUI elements.

## Add A Custom Tool

Custom tools read editor state through `CanvasToolContext` and return `CanvasToolIntent` values.
They do not receive `&mut CanvasEditor`, so undo, selection pruning, runtime-cache updates,
persistence, and future CRDT translation keep passing through one mutation path. Built-in reducer
state stays behind the crate-private session/reducer context rather than becoming part of the
custom-tool API. For continuous interactions such as dragging or resizing, return
`BeginTransientTransaction`, one or more `UpdateTransientTransaction` intents, and then
`CommitTransientTransaction` or `CancelTransientTransaction`; the editor coalesces those updates
into one undo entry and persistence log entry.

```rust
use open_gpui::{px, size};
use open_gpui_canvas::{
    CanvasEvent, CanvasNode, CanvasTool, CanvasToolContext, CanvasToolIntent, CanvasToolReducer,
    CanvasTransaction, DocumentCommand, DocumentError, NodeId, PointerButton,
};

struct StampTool;

impl CanvasToolReducer for StampTool {
    fn handle_event(
        &mut self,
        context: CanvasToolContext<'_>,
        event: CanvasEvent,
    ) -> Result<Vec<CanvasToolIntent>, DocumentError> {
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
            CanvasToolIntent::ApplyTransaction(CanvasTransaction::single(
                DocumentCommand::InsertNode(node),
            )),
            CanvasToolIntent::SetTool(CanvasTool::Select),
        ])
    }
}
```

```rust
# use open_gpui_canvas::{CanvasToolIntent, CanvasTransaction};
let drag_update = CanvasTransaction::default();
let intents = vec![
    CanvasToolIntent::BeginTransientTransaction,
    CanvasToolIntent::UpdateTransientTransaction(drag_update),
    CanvasToolIntent::CommitTransientTransaction,
];
# let _ = intents;
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

The `examples/canvas-notes/assets/sample.canvas` fixture is used by the native notes example and by
integration tests, so import/export expectations stay tied to a runnable example.

## Persistence

The default crate ships only the persistence contract and an in-memory store.

```rust
use open_gpui_canvas::{
    CanvasCheckpoint, CanvasDocument, CanvasNode, CanvasPersistenceCursor, CanvasStore,
    CanvasPersistenceStore, CanvasTransaction, DocumentCommand, MemoryCanvasPersistenceStore,
    apply_persistent_store_transaction, load_canvas_document, save_canvas_store_checkpoint,
};

let document = CanvasDocument::default();
let mut store = MemoryCanvasPersistenceStore::default();
store.save_checkpoint(CanvasCheckpoint::new(1, &document)).unwrap();

let restored = load_canvas_document(&store).unwrap();
assert_eq!(restored.node_count(), 0);

let mut canvas_store = CanvasStore::new(restored);
let mut cursor = CanvasPersistenceCursor::new(1);
apply_persistent_store_transaction(
    &mut canvas_store,
    &mut store,
    &mut cursor,
    CanvasTransaction::single(DocumentCommand::InsertNode(CanvasNode::new(
        "note",
        open_gpui::point(open_gpui::px(0.0), open_gpui::px(0.0)),
        open_gpui::size(open_gpui::px(120.0), open_gpui::px(64.0)),
    ))),
)
.unwrap();

save_canvas_store_checkpoint(&canvas_store, &mut store, &cursor).unwrap();
```

When an editor is attached to a persistence store, call `undo_persistent_transaction` and
`redo_persistent_transaction` instead of `CanvasEditor::undo` / `CanvasEditor::redo`. Those helpers
append the document-changing transaction before mutating the editor, so store failures do not leave
the in-memory document ahead of the replay log.
Non-editor adapters can call `apply_persistent_store_transaction`,
`undo_persistent_store_transaction`, `redo_persistent_store_transaction`, and
`save_canvas_store_checkpoint` directly on `CanvasStore`; the editor helpers are thin wrappers that
also prune editor selection after a committed document change.

For byte-oriented stores, wrap a `CanvasPersistenceByteStore` with
`CanvasPersistenceByteStoreAdapter`. The default `CanvasJsonPersistenceCodec` writes an explicit
envelope containing the codec version, document format version, record kind, sequence, and typed
payload.
Document format support and snapshot migration facts live in the `CanvasFormat` boundary, so byte
stores and future codecs validate the same supported document-version range without duplicating
snapshot rules.

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

For tool reducers, use `apply_persistent_tool_intents` so recorded transactions enter the log and
custom tool output stays on the intent surface. The editor owns gesture lifecycle and turns
selected built-in tool events into internal effects itself. New log entries written by the
persistence helpers are created from the same committed mutations that become `CanvasStoreChange`
facts, so their record operation batches describe actual document effects and their relation
operation batches describe actual parent/group structural changes.
`CanvasLogEntry::from_committed_mutation` is the durable committed-fact constructor;
`CanvasLogEntry::from_replay_transaction` is reserved for replaying or testing older
transaction-only logs where only command intent is available. Those legacy entries are marked
`LegacyReplayTransaction` and do not expose committed record or relation operations.
Older committed logs that contain only some committed operation facts are marked
`PartialCommittedMutation`; adapters should treat missing batches as unavailable facts, not as
empty semantic changes.
Applications that want one entrypoint can dispatch normalized canvas events through
`handle_persistent_event`, `handle_persistent_event_with_custom_tool`, or
`handle_persistent_event_with_tool_registry`; those helpers route built-in tools through the
editor's internal gesture path, route custom tools through reducer intents, and leave concrete
storage ownership in the application.

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
