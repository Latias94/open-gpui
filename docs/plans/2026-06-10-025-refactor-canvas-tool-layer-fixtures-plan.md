---
title: Refactor Canvas Tool Layer Fixtures
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Tool Layer Fixtures

## Summary

Migrate z-order and layer tool tests to `document_fixture()` for initial setup. The reordering
behavior must remain exercised through `CanvasEditor::reorder_selection`, preserving undo and runtime
hit-test assertions.

---

## Requirements

- R1. Replace direct initial document writes in layer/z-order tests with `document_fixture()`.
- R2. Keep layer changes on the editor reorder path.
- R3. Preserve sparse z-index, duplicate z-index, mixed record kind, multi-select, undo, and runtime
  hit-test behavior.
- R4. Keep transform, resize, connect, custom tool, and registry tests out of this batch.

---

## Implementation Units

### U1. Layer Fixtures

- **Files:** `crates/canvas/src/tool.rs`.
- **Verification:** Reorder, bring-forward, send-backward, and mixed-kind z-order tests pass.

### U2. Verify Canvas Suite

- **Verification:** `cargo fmt`, `cargo check`, `cargo nextest`, and rustdoc tests pass.

---

## Deferred Work

- Continue `tool.rs` with transform/resize/snap fixtures next.
