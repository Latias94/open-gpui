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

## Deferred Work

- Loro CRDT adapter.
- `rkyv` zero-copy snapshots.
- `redb` local cache.
- Rich text editing inside nodes.
- GPU-specialized rendering paths.
- Obsidian Canvas import/export.
- Figma-like constraints/components.
