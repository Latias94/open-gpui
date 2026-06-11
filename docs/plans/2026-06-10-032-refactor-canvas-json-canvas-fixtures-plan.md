---
title: Refactor Canvas JSON Canvas Fixtures
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas JSON Canvas Fixtures

## Summary

Migrate JSON Canvas export helper fixtures in `json_canvas.rs` to `document_fixture()`. The export
and import assertions must continue to exercise the JSON Canvas codec against real document
structures, not hand-built default documents.

---

## Requirements

- R1. Replace direct initial document writes in JSON Canvas export helper fixtures with
  `document_fixture()`.
- R2. Keep export/import assertions on the existing codec paths.
- R3. Preserve z-index ordering, handle-side export, and incomplete-node error coverage.
- R4. Keep unrelated document mutation, persistence, and runtime tests out of this batch.

---

## Implementation Units

### U1. JSON Canvas Fixtures

- **Files:** `crates/canvas/src/json_canvas.rs`.
- **Verification:** Export ordering, handle side mapping, and incomplete-node rejection tests pass.

### U2. Verify Canvas Suite

- **Verification:** `cargo fmt`, `cargo check`, `cargo nextest`, and rustdoc tests pass.

---

## Deferred Work

- Continue other JSON-oriented fixtures after this batch if they remain.
