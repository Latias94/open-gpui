---
title: Refactor Canvas Tool Connect Fixtures
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Tool Connect Fixtures

## Summary

Migrate connect tool tests to `document_fixture()` for initial graph setup. The connect behavior
must remain exercised through pointer events on `CanvasEditor` so edge creation, handle role
validation, locked endpoint filtering, undo, and redo continue to cover the tool and mutation seams.

---

## Requirements

- R1. Replace direct initial document writes in connect tool tests with `document_fixture()`.
- R2. Keep edge creation and rejection behavior on the built-in connect tool event path.
- R3. Preserve handle role assertions for source-only and target-only handles.
- R4. Keep custom tool, registry, gesture effect, and persistence-adjacent tests out of this batch.

---

## Implementation Units

### U1. Connect Tool Fixtures

- **Files:** `crates/canvas/src/tool.rs`.
- **Verification:** Connect tool create, locked endpoint, source handle, and target handle tests pass.

### U2. Verify Canvas Suite

- **Verification:** `cargo fmt`, `cargo check`, `cargo nextest`, and rustdoc tests pass.

---

## Deferred Work

- Continue `tool.rs` with custom tool and tool effect fixtures next.
