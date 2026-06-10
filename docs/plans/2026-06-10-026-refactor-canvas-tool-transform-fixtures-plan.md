---
title: Refactor Canvas Tool Transform Fixtures
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Tool Transform Fixtures

## Summary

Migrate transform, resize, box selection, translation, snap, and locked-record tool tests to
`document_fixture()` for initial setup. Runtime behavior must remain exercised through
`CanvasEditor` and built-in tool state machines so undo, cancellation, snapping, and policy checks
continue to cover the semantic mutation path.

---

## Requirements

- R1. Replace direct initial document writes in transform/resize/snap tests with
  `document_fixture()`.
- R2. Keep resize, translation, selection, cancellation, snapping, and undo assertions on the
  editor/tool path.
- R3. Preserve registered geometry and resize policy coverage.
- R4. Keep connect, custom tool, gesture effect, persistence-adjacent, and registry/schema tests
  out of this batch.

---

## Implementation Units

### U1. Transform And Resize Fixtures

- **Files:** `crates/canvas/src/tool.rs`.
- **Verification:** Transform handle, resize, resize policy, cancel, and selection box tests pass.

### U2. Translation And Snap Fixtures

- **Files:** `crates/canvas/src/tool.rs`.
- **Verification:** Mixed node/shape translation, shift-axis lock, snap, and locked-record tests pass.

### U3. Verify Canvas Suite

- **Verification:** `cargo fmt`, `cargo check`, `cargo nextest`, and rustdoc tests pass.

---

## Deferred Work

- Continue `tool.rs` with connect and custom tool fixtures next.
