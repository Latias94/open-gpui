---
title: Refactor Canvas Store Graph Schema Fixtures
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Store Graph Schema Fixtures

## Summary

Migrate the remaining small test fixtures in graph, schema, and store modules to
`document_fixture()`. These tests should reserve direct document mutation for the behavior under
test, while initial record setup stays behind the construction boundary.

---

## Requirements

- R1. Build graph sample documents through `document_fixture()`.
- R2. Build schema snapshot inputs through `document_fixture()`.
- R3. Build store initial documents through `document_fixture()`.
- R4. Keep relation and store mutation behavior on transaction paths.
- R5. Avoid changing production behavior.

---

## Implementation Units

### U1. Graph And Schema Fixtures

- **Files:** `crates/canvas/src/graph.rs`, `crates/canvas/src/schema.rs`.
- **Verification:** Graph query and schema normalization tests pass.

### U2. Store Fixtures

- **Files:** `crates/canvas/src/store.rs`.
- **Verification:** Runtime rebuild, listener, relation-only change, and relation cleanup tests pass.

### U3. Verify Canvas Suite

- **Verification:** `cargo fmt`, `cargo check`, `cargo nextest`, and rustdoc tests pass.

---

## Deferred Work

- Migrate `gpui.rs` and `tool.rs` in separate batches; both have larger behavior-heavy setup.
