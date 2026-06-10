---
title: "refactor: Deepen Canvas Editor Gesture And Mutation API"
type: refactor
status: active
date: 2026-06-08
---

# refactor: Deepen Canvas Editor Gesture And Mutation API

## Summary

This plan tightens the editor-facing mutation API so application code, built-in tools, custom tools,
undo/redo, persistence, and future CRDT adapters all pass through one prepared mutation and gesture
boundary.

---

## Problem Frame

`CanvasEditor` now owns document, runtime, selection, history, gestures, router, and kind registry
state, but the public surface can still grow shallow if custom tools need to know commit ordering or
if persistence has to infer whether a transaction is temporary or durable. A product-grade canvas
needs deep modules around mutation preparation and gesture lifecycle, not public state shortcuts or
duplicated transaction choreography.

---

## Requirements

**Mutation Boundary**

- R1. Public editor APIs must not let callers mutate document, runtime, selection, or history out of
  sync.
- R2. Undo, redo, direct transactions, tool effects, and persistence helpers must reuse prepared
  mutations where a log append or validation step already happened.
- R3. Actual committed diffs and record operation batches must remain the semantic source for
  persistence and future CRDT adapters.

**Gesture Boundary**

- R4. Built-in and custom tools should express begin, update, commit, and cancel intent without
  manually maintaining inverse transactions.
- R5. Temporary gesture updates must never appear in persistent logs until committed.
- R6. Cancel and failed commit paths must restore the baseline without leaving stale runtime or
  selection state.

**API Cleanup**

- R7. Pre-release APIs that expose inconsistent editor/runtime construction paths should be deleted
  instead of deprecated.
- R8. Tests must cover atomicity for prepare, log, apply, undo, redo, commit, cancel, and rejection.

---

## Key Technical Decisions

- KTD1. **Prepared mutation is the handoff unit:** code that prepares a mutation should be able to
  append it to logs and then apply that same prepared value.
- KTD2. **Gesture sessions are a deep module:** custom tools should not need to understand inverse
  transaction stack mechanics to implement drag, resize, or drawing.
- KTD3. **Editor remains the consistency owner:** runtime cache, selection pruning, and history
  updates stay inside `CanvasEditor`.
- KTD4. **Delete unstable shallow APIs:** 0.1 has no compatibility burden, so misleading entrypoints
  should be removed rather than kept with warnings.

---

## Implementation Units

### U1. Prepared Mutation Reuse

**Goal:** Make persistence undo/redo and direct transaction helpers reuse a single prepared mutation.

**Requirements:** R2, R3, R8.

**Files:**

- `crates/canvas/src/tool.rs`
- `crates/canvas/src/persistence/store.rs`
- `crates/canvas/src/journal.rs`

**Test scenarios:**

- `crates/canvas/src/persistence/store.rs`: undo appends the prepared log entry and applies that
  same prepared undo mutation.
- `crates/canvas/src/persistence/store.rs`: redo behaves the same way and leaves state unchanged if
  append fails.
- `crates/canvas/src/tool.rs`: schema rejection during prepare leaves document, runtime, and history
  unchanged.

### U2. Gesture Lifecycle Module

**Goal:** Collect gesture begin/update/commit/cancel semantics behind an editor-owned module.

**Requirements:** R4, R5, R6, R8.

**Files:**

- `crates/canvas/src/gesture.rs`
- `crates/canvas/src/tool.rs`
- `crates/canvas/src/persistence/store.rs`

**Test scenarios:**

- `crates/canvas/src/gesture.rs`: update applies transient document state without history.
- `crates/canvas/src/gesture.rs`: commit produces one undo entry and one durable operation batch.
- `crates/canvas/src/gesture.rs`: cancel restores the baseline without touching persistence.

### U3. Editor API Guardrails

**Goal:** Remove or narrow public APIs that let callers provide partial runtime, index, or history
state.

**Requirements:** R1, R7.

**Files:**

- `crates/canvas/src/tool.rs`
- `crates/canvas/src/runtime.rs`
- `crates/canvas/src/gpui.rs`
- `crates/canvas/src/lib.rs`

**Test scenarios:**

- `crates/canvas/src/gpui.rs`: paint models are built from a full editor or a document plus rebuilt
  runtime.
- `crates/canvas/src/runtime.rs`: runtime construction always receives the document, router, and
  kind registry that define geometry.
- Public export review confirms no partial state constructor remains.

### U4. Documentation And Migration Notes

**Goal:** Document the preferred editor command and gesture path for crate users.

**Requirements:** R1, R4, R7.

**Files:**

- `crates/canvas/README.md`
- `docs/adr/0002-open-gpui-canvas-architecture.md`
- `CHANGELOG.md`

**Test scenarios:**

- README snippets use editor commands or tool effects rather than direct state mutation.
- ADR names prepared mutations and gesture sessions as the durable consistency boundary.

---

## Scope Boundaries

- The plan does not introduce a full scripting runtime for custom tools.
- The plan does not make `CanvasEditor` generic over application state.
- The plan does not add networking or collaborative presence.
- The plan does not preserve unstable pre-release APIs that conflict with the consistency model.

---

## System-Wide Impact

This refactor affects the API surface that product examples and persistence adapters will build on.
It should reduce future CRDT and storage bugs by making the editor mutation path the only place that
can advance document state, runtime caches, selection, and history together.

---

## Sources / Research

- `crates/canvas/src/tool.rs`
- `crates/canvas/src/gesture.rs`
- `crates/canvas/src/persistence/store.rs`
- `crates/canvas/src/journal.rs`
- `docs/adr/0002-open-gpui-canvas-architecture.md`
