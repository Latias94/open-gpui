# Open GPUI Canvas Goal

## Goal

Build `open-gpui-canvas`, a reusable infinite-canvas crate for Open GPUI that supports node graphs,
mind maps, whiteboard shapes, and xyflow-like handles without inheriting DOM-style rendering
constraints.

## Run Envelope

- Start date: 2026-06-08.
- Active run target: continue through the 0.1 release waiting windows and develop until
  2026-06-08 10:00 Asia/Shanghai unless the goal is complete earlier.
- Commit policy: scoped Conventional Commits are allowed after verification.
- Main release line: keep `F:\SourceCodes\Rust\open-gpui` focused on publishing and tag work.
- Canvas worktree: `F:\SourceCodes\Rust\open-gpui-worktrees\canvas`.
- Canvas branch: `codex/open-gpui-canvas`.

## Discovery Evidence

| Source | Finding |
| --- | --- |
| `docs/adr/0001-open-gpui-fork-strategy.md` | New crates should preserve license hygiene, package identity, and workspace publishability. |
| `repo-ref/xyflow/packages/system/src/types/nodes.ts` | Node schema separates id, position, data, flags, parent, z-index, and handles. |
| `repo-ref/xyflow/packages/system/src/types/edges.ts` | Edges reference source/target node IDs and optional handle IDs. |
| `repo-ref/xyflow/packages/system/src/types/handles.ts` | Handles are invisible connection points with id, node id, role, position, and permissions. |
| `repo-ref/tldraw/packages/editor/src/lib/editor/tools/StateNode.ts` | Tool states should be explicit and event-driven, with enter/exit transitions. |
| `repo-ref/tldraw/packages/store/src/index.ts` | Document records, diffs, migrations, and local-first storage are separate from rendering. |
| `crates/gpui/src/elements/canvas.rs` | GPUI already has a low-level paint callback suitable for a first adapter. |
| `crates/gpui/src/geometry.rs` | Reuse GPUI geometry units instead of inventing parallel pixel types. |

## Success Metrics

| Metric | Target | Verification |
| --- | --- | --- |
| Workspace integration | `open-gpui-canvas` is a workspace package | `cargo metadata --format-version 1 --locked --no-deps` |
| Core model | Nodes, edges, handles, shapes, viewport, commands, and hit tests exist | `cargo test -p open-gpui-canvas` |
| GPUI compatibility | The crate can be checked with current Open GPUI packages | `cargo check -p open-gpui-canvas` |
| Release hygiene | Manifest metadata matches Open GPUI naming and attribution | manifest review |
| Architecture traceability | ADR explains alternatives, risks, and future CRDT/storage boundaries | ADR review |

## Lane Map

mode: serial-first with later parallel slices
repo: `F:\SourceCodes\Rust\open-gpui`
base_ref: `9dca4cd`
goal: `open-gpui-canvas`
commit_policy: autonomous scoped commits allowed
verification_owner: primary agent
stop_conditions:
- Release work requires moving `v0.1.0` tag or changing main branch history.
- Canvas implementation requires changing existing GPUI rendering internals before the crate MVP.
- A verification failure appears unrelated to the canvas crate and needs user-owned changes.

### Serial First

| Reason | Unlocks |
| --- | --- |
| Public data model and command boundary are shared contracts | Implementation of storage, renderer, tools, and examples |
| Workspace manifest and lockfile are global files | Any worker touching crates must wait for this step |
| Hit-test and culling model shape rendering choices | GPUI adapter and examples |

### Lanes

| ID | Role | Classification | Target | Writable Files | Verification |
| --- | --- | --- | --- | --- | --- |
| canvas-architecture | architecture | serial-first | ADR and goal plan | `docs/adr/0002-open-gpui-canvas-architecture.md`, `.loom/goals/open-gpui-canvas.md` | doc review |
| canvas-core-model | worker | serial-first | `open-gpui-canvas` model crate | `Cargo.toml`, `Cargo.lock`, `crates/canvas/**` | `cargo test -p open-gpui-canvas` |
| canvas-hit-test | worker | parallel after core model | spatial index, culling, hit testing | `crates/canvas/src/index.rs`, tests | `cargo test -p open-gpui-canvas hit` |
| canvas-tools | worker | parallel after command model | event and tool state machine | `crates/canvas/src/tool.rs`, tests | `cargo test -p open-gpui-canvas tool` |
| canvas-gpui-adapter | worker | parallel after core model | basic GPUI canvas element adapter | `crates/canvas/src/gpui.rs`, example files | `cargo check -p open-gpui-canvas --features gpui` |
| canvas-review | reviewer | review | all canvas diffs | read-only | findings-first review |

## Initial Implementation Scope

1. Create `crates/canvas` package `open-gpui-canvas`.
2. Add core records and ID newtypes.
3. Add document mutation commands for insert/update/remove of nodes, edges, and shapes.
4. Add viewport transforms.
5. Add a simple spatial index that can later be replaced by an R-tree or tile index without
   changing public document records.
6. Add built-in tool states for select, pan, and connect as a testable reducer.
7. Add unit tests for endpoints, hit order, viewport transforms, and basic tool transitions.

## Implemented Foundation

| Area | Status | Notes |
| --- | --- | --- |
| Workspace package | Done | `crates/canvas` builds as `open-gpui-canvas` and imports as `open_gpui_canvas`. |
| Record model | Done | Nodes, edges, shapes, handles, strong IDs, JSON payloads, and styles are implemented. |
| Snapshot model | Done | `CanvasSnapshot` serializes records as arrays, validates format version on restore, and exposes an explicit migration boundary. |
| Command boundary | Done | `DocumentCommand` and `CanvasTransaction` provide atomic document mutation and inverse generation. |
| Integrity checks | Done | Duplicate handles, missing handles, non-connectable handles, handle roles, and edge endpoint breakage are rejected. |
| Tool state machine | Done | Select, pan, and connect tools are event-driven and testable. |
| Undo/redo | Done | `CanvasHistory` tracks inverse transactions and clears redo on new edits. |
| Selection hygiene | Done | Selection is pruned after transactions, undo, redo, and unrecorded drag updates. |
| Document diff | Done | `CanvasDocumentDiff` reports inserted, updated, removed, and metadata-changed records. |
| Spatial index | Done | Hit testing and culling support nodes, handles, shapes, and edges. Indexes can apply document diffs incrementally. |
| Viewport | Done | View/document transforms, anchored zoom, zoom factor validation, and visible document bounds are implemented. |
| Selection rectangle and multi-select | Done | Select tool supports box selection and multi-node drag for selected nodes. |
| Edge routing metadata | Done | `CanvasEdgeRoute` records route kind, waypoints, control points, options, and interaction width without binding core to a renderer. |
| Router strategy boundary | Done | `CanvasEdgeRouter` resolves route metadata into renderer-neutral `CanvasRoutePath` / `CanvasRouteSegment` values so hit testing and GPUI painting share the same route interpretation while applications can supply custom routers. |
| JSON Canvas import/export | Done | `JsonCanvas` converts text/file/link/group nodes and side-based edges to and from `CanvasDocument`. |
| Graph query view | Done | `CanvasGraph` provides zero-copy node, edge, endpoint, incoming/outgoing edge, neighbor, and directed edge-between queries for xyflow-style graph applications. |
| Cached graph index | Done | `CanvasGraphIndex` provides an explicit adjacency cache with diff application, document-order query results, and an indexed graph view for hot traversal without hiding cache state in `CanvasDocument`. |
| Persistent storage traits | Done | `CanvasPersistenceStore` defines checkpoint plus monotonic transaction-log replay without introducing redb/Loro/rkyv core dependencies. |
| Randomized invariant tests | Done | Deterministic stress tests cover transaction inverse/diff replay and incremental spatial-index equivalence against full rebuilds. |
| GPUI adapter prototype | Done | `canvas_view` renders a model snapshot through GPUI's low-level canvas callback using spatial culling and batched paint commands. |
| Smoke example | Done | `open-gpui-smoke-native` renders nodes, handles, shapes, routed edges, and the default GPUI canvas paint adapter. |
| Adapter interaction bridge | Done | `CanvasInputMapper` converts GPUI pointer and wheel events into canvas-local `CanvasEvent` values without coupling paint to editor mutation. |
| Interactive smoke example | Done | The smoke example owns a `CanvasEditor`, snapshots it for paint, registers GPUI pointer/wheel listeners during canvas paint, and exercises select-tool dragging plus viewport wheel panning. |
| Interaction paint feedback | Done | `CanvasPaintModel` carries selection and tool-state snapshots; paint frames mark selected records and expose selection rectangle plus connection preview overlays without per-record GPUI elements. |
| Tool effect boundary | Done | `CanvasToolEffect` centralizes recorded transactions, unrecorded transactions, selection, viewport, and tool-state updates so built-in and future custom tools share one editor mutation path. |
| Schema evolution boundary | Done | `migrate_canvas_snapshot`, minimum supported format version, and migration table exports make future snapshot changes explicit before storage and CRDT adapters depend on them. |
| Custom tool reducer boundary | Done | `CanvasTool::custom`, `CanvasToolContext`, and `CanvasToolReducer` let applications build custom tools that read editor state and emit effects without mutating `CanvasEditor` directly. |
| Persistence adapter feature boundary | Done | `redb-store`, `loro-crdt`, and `rkyv-snapshot` features plus adapter capability statuses reserve future adapter names without pulling optional dependencies into the default core build. |
| Tool registry ergonomics | Done | `CanvasToolRegistry` maps custom tool IDs to reducers and lets applications dispatch registered tools while keeping builtin tools on the same editor event entrypoint. |
| Example custom tools | Done | `open-gpui-smoke-native` registers a custom stamp tool through `CanvasToolRegistry` and dispatches right-click pointer events through the same editor effect path as builtin tools. |
| Crate README/API examples | Done | `crates/canvas/README.md` documents the model, graph queries, GPUI rendering path, custom tools, JSON Canvas, and persistence boundaries with copyable API snippets. |
| Package README verification | Done | `cargo package -p open-gpui-canvas --locked --allow-dirty` verifies the crate README is packaged with the future publish artifact. |
| Editor persistence hook | Done | `CanvasPersistenceCursor` and `apply_persistent_transaction` connect successful recorded editor transactions to monotonic store log entries without binding `CanvasEditor` to a concrete backend. |
| Persistence checkpoint helper | Done | `save_canvas_checkpoint` persists an editor snapshot at the current cursor sequence and compacts older log entries through the same store abstraction. |
| Persistent tool effect runner | Done | `apply_persistent_tool_effects` logs recorded tool transactions and commits finished unrecorded gestures through `PushUndo` while leaving transient updates out of the log. |
| Persistent event dispatch helper | Done | `handle_persistent_event*` helpers reduce built-in, custom, or registry-dispatched tool events into effects and apply them through the persistence runner without making `CanvasEditor` own the store. |
| Persistence byte codec boundary | Done | `CanvasPersistenceCodec`, `CanvasJsonPersistenceCodec`, `CanvasPersistenceByteStore`, and `CanvasPersistenceByteStoreAdapter` separate typed checkpoint/log records from encoded bytes before redb/rkyv adapters land. |
| Persistent undo/redo helpers | Done | `undo_persistent_transaction` and `redo_persistent_transaction` append replayable history transactions before mutating the editor so persistence failures do not desynchronize memory from the log. |
| Persistence module split | Done | Persistence codec, byte-store adapter, typed store helpers, memory stores, and tests are split into submodules while preserving the crate root public exports. |
| Record change view | Done | `CanvasRecord` / `CanvasRecordChange` and transaction helpers expose ordered upsert/delete record changes for future Loro, sync, audit, and indexing adapters without replacing command replay. |
| Locked interaction semantics | Done | `HitOptions::include_locked`, locked hit records, and select-tool filtering keep locked records visible for paint while default interaction skips selection, endpoint picking, and translation. |
| Record operation batches | Done | `CanvasRecordOperation` / `CanvasRecordOperationBatch` wrap record changes with transaction sequence, operation index, origin, and metadata for future Loro, sync, audit, and persistence adapters. |
| Spatial query trait | Done | `CanvasSpatialIndex` provides an object-safe visitor boundary over query and hit-test traversal so future R-tree, tile, or GPU-assisted indexes can plug in without changing document records. |
| Keyboard delete interaction | Done | `CanvasKey`, `CanvasKeyModifiers`, GPUI keydown mapping, and select-tool Delete/Backspace transactions give keyboard edits the same undo/persistence/CRDT path as pointer edits. |
| Incremental selection effects | Done | `CanvasSelection` and `CanvasToolEffect` support add/remove/toggle/contains operations so custom tools and future modifier-key workflows can share one selection mutation path. |
| Pointer modifier events | Done | Pointer down/up events now carry `CanvasKeyModifiers`, and GPUI pointer mapping forwards platform modifiers for future shift-click, constrained dragging, and modifier-aware tools. |
| Shift-click selection toggle | Done | The built-in select tool uses pointer modifiers and `ToggleSelection` to add or remove clicked records without entering drag state or creating undo history. |

## Next Implementation Slices

| Priority | Slice | Rationale | Candidate Verification |
| --- | --- | --- | --- |
| Medium | Concrete persistence adapters | Implement feature-gated redb/Loro/rkyv adapters only after each adapter has a focused contract and no default dependency leakage. | adapter-specific integration tests |

## Deferred Work

- Loro CRDT adapter.
- `rkyv` zero-copy snapshots.
- `redb` local cache.
- Rich text editing inside nodes.
- GPU-specialized rendering paths.
- Figma-like constraints/components.
