---
title: Refactor Canvas No-Op Mutation Journal Handling
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas No-Op Mutation Journal Handling

## Summary

The mutation journal can now prove when a non-empty transaction has no semantic document effect.
Editor history and persistence should use that committed fact instead of treating any non-empty
command list as undoable or durable.

---

## Problem Frame

`CanvasTransaction::is_empty()` only describes command intent. A relation command can be non-empty
while producing an empty `CanvasDocumentDiff`, for example setting the same parent twice or adding
an existing group membership again. The journal correctly reports an empty committed diff and no
record or relation changes, but editor and persistence paths can still push undo entries or append
log entries because they only checked command emptiness before preparing.

That weakens the committed mutation fact source and can create noisy durable logs.

---

## Requirements

- R1. Direct editor transactions with empty committed diffs must not push undo history or clear redo.
- R2. Persistent transaction helpers with empty committed diffs must not append log entries or
  advance the cursor.
- R3. Persistent undo/redo should treat an empty prepared undo/redo as no state change.
- R4. Gesture commit with an empty prepared diff should not append a log entry or advance the
  cursor.
- R5. `CanvasDocument::apply_transaction_with_diff` may continue returning an empty diff after
  applying a no-op transaction; this plan is about editor/persistence side effects.

---

## Key Technical Decisions

- KTD1. Keep command-intent emptiness and committed semantic emptiness separate.
- KTD2. Put the check at prepared committed mutation application/logging boundaries, not in every
  individual command.
- KTD3. Preserve pre-1.0 simplicity: no deprecated compatibility behavior for no-op undo/log
  entries.

---

## Implementation Units

### U1. Skip editor history for empty committed diffs

- **Files:** `crates/canvas/src/tool.rs`.
- **Test scenarios:** Applying the same parent relation twice returns an empty diff the second time,
  does not increase undo depth, and does not clear an existing redo stack.

### U2. Skip persistence logs for empty committed diffs

- **Files:** `crates/canvas/src/persistence/store.rs`, `crates/canvas/src/persistence/tests.rs`.
- **Test scenarios:** Persistent no-op relation transaction does not append a log entry or advance
  the cursor; undo/redo no-op prepared mutations do not write logs; empty gesture commit stays out
  of the log.

---

## Scope Boundaries

- This plan does not change transaction normalization or relation validation.
- This plan does not change replay semantics for legacy logs that already contain no-op commands.
- This plan does not add CRDT or storage adapters.
