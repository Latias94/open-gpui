---
title: Refactor Canvas Tool Custom Fixtures
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Tool Custom Fixtures

## Summary

Migrate custom tool and tool registry entry tests to `document_fixture()` for initial setup. The
custom tool reducers and registry dispatch paths must still run through `CanvasEditor`, preserving
hit testing, viewport mapping, and tool effect behavior.

---

## Requirements

- R1. Replace direct initial document writes in custom tool and registry entry tests with
  `document_fixture()`.
- R2. Keep custom reducer execution and registry dispatch on the editor event path.
- R3. Preserve viewport-aware hit testing for the stamped anchor records.
- R4. Keep gesture, persistence, relation-history, and direct transaction tests out of this batch.

---

## Implementation Units

### U1. Custom Tool Fixtures

- **Files:** `crates/canvas/src/tool.rs`.
- **Verification:** Custom tool reducer and builtin-tool bypass tests pass.

### U2. Registry Entry Fixtures

- **Files:** `crates/canvas/src/tool.rs`.
- **Verification:** Tool registry dispatch and builtin-tool entry tests pass.

### U3. Verify Canvas Suite

- **Verification:** `cargo fmt`, `cargo check`, `cargo nextest`, and rustdoc tests pass.

---

## Deferred Work

- Continue `tool.rs` with history, relation, and persistence-adjacent fixtures next.
