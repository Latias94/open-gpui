---
title: Refactor Canvas Mutation Fixtures
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Mutation Fixtures

## Summary

Migrate relation-oriented helper fixtures in `mutation.rs` to `document_fixture()`. The committed
mutation assertions must still exercise transaction commit, relation diffing, and inverse replay on
real `CanvasDocument` state.

---

## Requirements

- R1. Replace direct initial document writes in the selected mutation tests with `document_fixture()`.
- R2. Keep committed mutation, diff, and inverse assertions on the transaction path.
- R3. Preserve relation-only change, replacement, and no-op relation coverage.
- R4. Keep failed-transaction and metadata-only tests out of this batch.

---

## Implementation Units

### U1. Mutation Fixtures

- **Files:** `crates/canvas/src/mutation.rs`.
- **Verification:** Relation-only change, parent replacement, no-op relation, and connected-document tests pass.

### U2. Verify Canvas Suite

- **Verification:** `cargo fmt`, `cargo check`, `cargo nextest`, and rustdoc tests pass.

---

## Deferred Work

- Continue other mutation tests only if more pure fixture setup remains.
