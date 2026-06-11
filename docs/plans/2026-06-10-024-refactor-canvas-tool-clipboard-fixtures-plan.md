---
title: Refactor Canvas Tool Clipboard Fixtures
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Tool Clipboard Fixtures

## Summary

Continue the `tool.rs` fixture migration with delete, duplicate, cut, and paste tests. Initial
records should be constructed through `document_fixture()`, while relation setup, delete key
handling, duplicate, cut, paste, undo, and runtime sync remain exercised through editor command
paths.

---

## Requirements

- R1. Replace direct initial document writes in delete and clipboard tool tests with
  `document_fixture()`.
- R2. Keep relation setup and clipboard behavior on transaction/editor paths.
- R3. Preserve delete undo, locked-record skipping, duplicate ID remapping, relation remapping,
  cut/paste selection, and runtime hit-test behavior.
- R4. Keep layer, resize, connect, custom tool, and registry tests out of this batch.

---

## Implementation Units

### U1. Delete Fixtures

- **Files:** `crates/canvas/src/tool.rs`.
- **Verification:** Delete and locked-delete tests pass.

### U2. Clipboard Fixtures

- **Files:** `crates/canvas/src/tool.rs`.
- **Verification:** Duplicate and cut/paste tests pass, including relation remapping.

### U3. Verify Canvas Suite

- **Verification:** `cargo fmt`, `cargo check`, `cargo nextest`, and rustdoc tests pass.

---

## Deferred Work

- Continue `tool.rs` with layer ordering, resize/snap, connect tool, custom tool effects, and
  registry/schema batches.
