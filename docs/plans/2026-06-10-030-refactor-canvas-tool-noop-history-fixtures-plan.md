---
title: Refactor Canvas Tool No-op History Fixtures
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Tool No-op History Fixtures

## Summary

Migrate the remaining no-op history and relation fixture setup in `tool.rs` to `document_fixture()`.
The tests must still exercise their transaction and undo/redo assertions through `CanvasEditor` and
`CanvasTransaction`.

---

## Requirements

- R1. Replace direct initial document writes in the selected no-op history tests with
  `document_fixture()`.
- R2. Keep no-op transaction, relation-order, undo, and redo assertions on the editor path.
- R3. Preserve the existing history depth and relation-order expectations.
- R4. Keep direct mutation-path tests and runtime rebuild tests out of this batch.

---

## Implementation Units

### U1. No-op History Fixtures

- **Files:** `crates/canvas/src/tool.rs`.
- **Verification:** No-op commit, relation-order, and undo/redo discard tests pass.

### U2. Verify Canvas Suite

- **Verification:** `cargo fmt`, `cargo check`, `cargo nextest`, and rustdoc tests pass.

---

## Deferred Work

- Continue `tool.rs` with direct mutation-path tests only if they still need fixture cleanup.
