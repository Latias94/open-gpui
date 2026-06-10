---
title: Refactor Canvas Document Fixtures
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Document Fixtures

## Summary

Migrate the initial snapshot and relation fixture setup in `document.rs` to `document_fixture()`.
The snapshot round-trip and duplicate-relation assertions must still exercise the real document and
snapshot conversion paths.

---

## Requirements

- R1. Replace the selected default-document setup in `document.rs` with `document_fixture()`.
- R2. Keep snapshot round-trip and duplicate-relation assertions on the document/snapshot paths.
- R3. Preserve the existing node, edge, shape, and relation expectations.
- R4. Keep the remaining transaction-heavy document tests out of this batch.

---

## Implementation Units

### U1. Document Fixtures

- **Files:** `crates/canvas/src/document.rs`.
- **Verification:** Snapshot round-trip and duplicate-relation tests pass.

### U2. Verify Canvas Suite

- **Verification:** `cargo fmt`, `cargo check`, `cargo nextest`, and rustdoc tests pass.

---

## Deferred Work

- Continue the transaction-heavy document tests only if more pure fixture setup remains.
