---
title: Refactor Canvas Gesture Relation Journal Handling
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Gesture Relation Journal Handling

## Summary

Gesture commit should emit the same committed mutation facts as direct transactions. After record
relations became first-class document state, gesture coalescing must include relation changes and
must avoid history/log side effects for empty committed diffs.

---

## Problem Frame

`CanvasGestureSession::transaction_between` reconstructs a transaction by comparing nodes, edges,
and shapes only. That was enough before structural relations existed, but it now means a gesture
that changes parent or group relations can mutate the transient document and then commit with no
undo entry, no persistence log entry, and no relation operation batch.

`CanvasEditor::apply_prepared_gesture_commit` also pushes undo unconditionally for any prepared
gesture commit. Persistence already checks for empty committed diffs before logging, but it still
delegates to the editor path that can push a stale undo entry.

---

## Requirements

- R1. Gesture transaction coalescing must include parent relation add/change/remove semantics.
- R2. Gesture transaction coalescing must include group membership add/remove semantics.
- R3. Relation-only gesture commits must push one undo entry and persistent gesture commits must
  append one committed mutation log entry with relation operation facts.
- R4. Empty prepared gesture commits must clear the active gesture without pushing undo, syncing
  runtime caches, appending persistence logs, or advancing the cursor.
- R5. Keep the public custom tool API unchanged; `CanvasToolEffect` remains crate-private.

---

## Key Technical Decisions

- KTD1. Keep gesture coalescing in `crates/canvas/src/gesture.rs`; it is the Module that owns
  baseline-to-current gesture intent reconstruction.
- KTD2. Emit relation commands after record insert/update commands so new relation endpoints exist
  before validation.
- KTD3. Skip explicit relation removals for records that are deleted by the same gesture; document
  mutation pruning already expresses those as committed relation changes.
- KTD4. Treat empty committed diff as the editor/persistence side-effect gate for gesture commits,
  matching direct transaction handling.

---

## Implementation Units

### U1. Make gesture transaction_between relation-aware

- **Files:** `crates/canvas/src/gesture.rs`.
- **Test scenarios:** A baseline-to-current relation parent change produces `SetRecordParent`;
  parent removal produces `ClearRecordParent`; group membership add/remove produces the matching
  group commands; replaying the transaction recreates the target document.

### U2. Skip empty prepared gesture side effects

- **Files:** `crates/canvas/src/tool.rs`, `crates/canvas/src/persistence/store.rs`.
- **Test scenarios:** Empty gesture commit does not push undo or persistence log entries; direct and
  persistent paths share the same editor behavior.

### U3. Cover relation-only gesture commit through editor and persistence

- **Files:** `crates/canvas/src/tool.rs`, `crates/canvas/src/persistence/tests.rs`.
- **Test scenarios:** Relation-only gesture commit creates one undo entry; undo removes the
  relation; persistent relation-only gesture commit writes one log entry exposing committed
  relation operations and replay restores the relation.

---

## Scope Boundaries

- This plan does not expose gesture effects as public custom tool intents.
- This plan does not add CRDT, redb, or rkyv adapters.
- This plan does not change relation schema beyond parent and group facts already supported.
