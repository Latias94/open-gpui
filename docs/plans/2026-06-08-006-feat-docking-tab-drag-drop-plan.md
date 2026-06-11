---
title: "feat: Add docking tab drag and drop"
type: feat
status: completed
date: 2026-06-08
---

# feat: Add docking tab drag and drop

## Summary

Add the first tab drag/drop path for `open-gpui-docking`: users can drag one tab from a rendered tab stack and drop it onto another stack's center or edge zones. The implementation should reuse the pure graph operations that already move items and keep preview/commit targeting driven by one pure resolver.

## Problem Frame

The docking model already supports `DockOp::MoveItem` for center and edge docking, and the owner-backed `DockHost` now handles tab clicks and splitter drags. The next missing interaction is moving a tab by drag/drop. This phase should not attempt the full docking interaction surface at once. It should build the reusable resolver and payload shape, then wire a minimal single-tab drag path through GPUI.

## Requirements

- R1. Represent a dragged tab as a typed payload containing source dock space, source tabs node, and item id.
- R2. Resolve drop targets through a pure geometry helper that maps pointer position and target tabs bounds to `DropZone`.
- R3. Commit center drops by moving the dragged item into the target tabs stack.
- R4. Commit edge drops by creating or inserting split nodes through the existing graph operation path.
- R5. Treat drops back onto the same tab stack center as no-ops unless a later reorder implementation is explicitly added.
- R6. Keep drop preview geometry and commit geometry on the same resolver seam.
- R7. Keep drag/drop state outside `DockGraph`; graph persistence must remain view/window free.
- R8. Cover resolver behavior with unit tests and rendered drag/drop behavior with GPUI visual tests.

## Scope Boundaries

In scope:

- Single-tab drag payloads.
- Center and edge drops onto rendered tabs nodes.
- Pure target resolution for tabs bounds.
- Commit through `DockWorkspace` / `DockOp`.
- Minimal debug/test observability for drop targets.

Deferred:

- Reordering tabs within the same tabs node.
- Dragging a whole tabs stack as a group.
- Floating by dragging outside a dock target.
- Rich preview overlay rendering beyond the shared resolver seam.
- Cross-window or platform-window drag routing.

## Key Technical Decisions

- KTD1. Start with single-tab drag only: this validates the graph move and GPUI drag/drop path before adding group moves.
- KTD2. Use the graph's `MoveItem` operation directly: no new graph mutation primitive is needed for this phase.
- KTD3. Make the resolver pure: target bounds, pointer position, and thresholds produce a `DockDropIntent` without needing GPUI state.
- KTD4. Keep same-stack center drop as no-op: reordering has different insertion-index semantics and should be implemented deliberately later.
- KTD5. Test commit results through graph shape and rendered output rather than screenshots.

## Implementation Units

### U1. Drag Payload And Drop Resolver

**Goal:** Add typed drag payloads and pure drop target resolution.

**Files:**

- `crates/gpui_docking/src/drag.rs`
- `crates/gpui_docking/src/drop_target.rs`
- `crates/gpui_docking/src/lib.rs`

**Test scenarios:**

- A point in the center of a target tabs bounds resolves to `DropZone::Center`.
- Points near left, right, top, and bottom edges resolve to the matching edge zone.
- Points outside target bounds return no intent.
- Edge threshold is clamped so small targets still leave a center zone.

### U2. Commit Tab Drop Through The Owner

**Goal:** Convert resolved intents into checked graph mutations through `DockWorkspace`.

**Files:**

- `crates/gpui_docking/src/action.rs`
- `crates/gpui_docking/src/drag.rs`
- `crates/gpui_docking/src/drop_target.rs`
- `crates/gpui_docking/src/host_tests.rs`

**Test scenarios:**

- Dropping item `B` on target tabs center moves `B` into that tabs node and selects it.
- Dropping item `B` on the right edge of target tabs creates a horizontal split.
- Dropping onto the same tabs center returns unchanged and preserves graph state.
- Invalid source item or target node returns a typed failure without mutating graph state.

### U3. Render Integration

**Goal:** Wire tab label drag payloads and target drop handling into `DockHost` rendering.

**Files:**

- `crates/gpui_docking/src/render.rs`
- `crates/gpui_docking/src/debug.rs`
- `crates/gpui_docking/src/host_tests.rs`

**Test scenarios:**

- Visual drag from one tab to another stack center changes the active panel in the target stack.
- Visual drag to a right-edge target creates a split with the moved panel in the expected side.
- Dragging less than GPUI's drag threshold still behaves as a tab click, not a move.

### U4. Example And Verification

**Goal:** Make the new capability visible to example users and future implementers.

**Files:**

- `examples/docking-native/src/main.rs`
- `docs/plans/2026-06-08-006-feat-docking-tab-drag-drop-plan.md`

**Test scenarios:**

- The native example still compiles.
- Example copy reflects tab drag/drop as available while floating remains deferred.

## Verification

- `cargo fmt --check`
- `cargo nextest run -p open-gpui-docking`
- `cargo clippy -p open-gpui-docking --all-targets --no-deps -- -D warnings`
- `RUSTDOCFLAGS="-D warnings" cargo doc -p open-gpui-docking --no-deps`
- `cargo check -p open-gpui-docking-native`
- `cargo clippy -p open-gpui-docking-native --no-deps -- -D warnings`
