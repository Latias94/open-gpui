---
title: Refactor Canvas Tool Selection Fixtures
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Tool Selection Fixtures

## Summary

Start migrating `tool.rs` in small batches. This first batch covers basic select-tool tests:
translation, locked hit behavior, canvas press selection clearing, cancel behavior, shift toggling,
and registered precise-hit policy. Initial records should come from `document_fixture()`, while user
interaction remains driven through `CanvasEditor::handle_event`.

---

## Requirements

- R1. Replace direct initial document writes in basic select-tool tests with `document_fixture()`.
- R2. Keep behavior under test on event dispatch, tool effects, undo, and redo paths.
- R3. Preserve node and shape translation, selection cancellation, locked-record skipping, shift
  toggling, and kind-registry hit policy behavior.
- R4. Avoid touching later delete, duplicate, resize, layer, connect, custom-tool, and registry
  tests in this batch.

---

## Implementation Units

### U1. Basic Select Tool Fixtures

- **Files:** `crates/canvas/src/tool.rs`.
- **Verification:** Select tool translation, cancellation, shift-click, and precise-hit tests pass.

### U2. Verify Canvas Suite

- **Verification:** `cargo fmt`, `cargo check`, `cargo nextest`, and rustdoc tests pass.

---

## Deferred Work

- Continue `tool.rs` with separate batches for delete/clipboard, resize/snap, layer ordering,
  connect tool, custom tool effects, and registry/schema behavior.
