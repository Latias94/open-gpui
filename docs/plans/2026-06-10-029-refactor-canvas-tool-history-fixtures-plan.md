---
title: Refactor Canvas Tool History Fixtures
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Tool History Fixtures

## Summary

Migrate history, relation, and runtime-geometry helper fixtures in `tool.rs` to
`document_fixture()` for initial setup. Transaction, gesture, undo/redo, and runtime assertions must
remain on the editor path so the tests continue to cover the mutation seam.

---

## Requirements

- R1. Replace direct initial document writes in the selected history and relation tests with
  `document_fixture()`.
- R2. Keep gesture commit, undo/redo, and runtime assertions on `CanvasEditor`.
- R3. Preserve relation and geometry helper coverage.
- R4. Keep direct transaction, selection-discard, and runtime rebuild tests out of this batch.

---

## Implementation Units

### U1. History Fixtures

- **Files:** `crates/canvas/src/tool.rs`.
- **Verification:** Gesture commit, empty commit, and transient selection tests pass.

### U2. Runtime Helper Fixtures

- **Files:** `crates/canvas/src/tool.rs`.
- **Verification:** Kind registry rejection and connected-edge runtime helper tests pass.

### U3. Verify Canvas Suite

- **Verification:** `cargo fmt`, `cargo check`, `cargo nextest`, and rustdoc tests pass.

---

## Deferred Work

- Continue `tool.rs` with the remaining mutation-path history and direct transaction tests next.
